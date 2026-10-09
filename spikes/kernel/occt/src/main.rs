//! Kernel spike, C++ candidate: OpenCascade through the `opencascade` crate.

#[path = "../../common.rs"]
mod common;

use common::*;
use glam::DVec3;
use opencascade::primitives::{Edge, Face, JoinType, Shape, Solid, Wire};
use std::f64::consts::PI;
use std::time::Instant;

const MESH_TOL: f64 = 0.5;

fn bx(min: [f64; 3], size: [f64; 3]) -> Shape {
    let a = DVec3::from_array(min);
    Shape::box_from_corners(a, a + DVec3::from_array(size))
}

fn mesh(s: &Shape) -> Result<TriMesh, String> {
    let m = s.mesh_with_tolerance(MESH_TOL).map_err(|e| e.to_string())?;
    Ok(TriMesh {
        positions: m.vertices.iter().map(|v| v.to_array()).collect(),
        triangles: m.indices.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
    })
}

fn solid(s: &Shape, expected: Option<f64>, note: &str) -> Outcome {
    match mesh(s) {
        Ok(mesh) => Outcome::Solid {
            mesh,
            expected_volume: expected,
            note: format!("{} face(s){note}", s.faces().count()),
        },
        Err(e) => Outcome::Failed(e),
    }
}

fn run(case: &str, f: impl FnOnce() -> Outcome + std::panic::UnwindSafe) {
    let t = Instant::now();
    let out = std::panic::catch_unwind(f).unwrap_or_else(|_| Outcome::Failed("panic".into()));
    report("occt", case, out, t.elapsed());
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    header();

    run("1-niche", || {
        let r = bx([0.0; 3], WALL).subtract(&bx(NICHE_MIN, NICHE));
        solid(&r, Some(expected_niche()), "")
    });

    run("2-cable-hole", || {
        let top = bx([0.0; 3], TOP);
        let hole = Shape::cylinder(
            DVec3::new(TOP[0] / 2.0, TOP[1] / 2.0, -10.0),
            HOLE_R,
            DVec3::Z,
            TOP[2] + 20.0,
        );
        solid(&top.subtract(&hole), Some(expected_hole()), "")
    });

    run("3-cabinet-union", || {
        let mut acc = bx([0.0; 3], MODULE);
        for i in 1..5 {
            acc = acc.union(&bx([i as f64 * MODULE[0], 0.0, 0.0], MODULE)).shape;
        }
        let merged = acc.clean();
        solid(
            &merged,
            Some(expected_cabinet()),
            &format!(" after clean, {} before (ideal: 6)", acc.faces().count()),
        )
    });

    run("4-fillet-panel", || {
        let panel = bx([0.0; 3], PANEL).fillet(FILLET_R);
        solid(&panel, Some(expected_fillet()), "")
    });

    run("5-loft-cut", || {
        let h = LOFT_SQUARE / 2.0;
        let sq = [(h, h), (-h, h), (-h, -h), (h, -h)].map(|(x, y)| DVec3::new(x, y, 0.0));
        let square: Vec<Edge> = (0..4).map(|i| Edge::segment(sq[i], sq[(i + 1) % 4])).collect();
        let at = |a: f64| DVec3::new(LOFT_R * a.cos(), LOFT_R * a.sin(), LOFT_H);
        let circle: Vec<Edge> = (0..4)
            .map(|i| {
                let a0 = PI / 4.0 + PI / 2.0 * i as f64;
                Edge::arc(at(a0), at(a0 + PI / 4.0), at(a0 + PI / 2.0))
            })
            .collect();
        let w0 = Wire::from_edges(&square);
        let w1 = Wire::from_edges(&circle);
        let loft: Shape = Solid::loft([&w0, &w1]).into();
        let cut = bx([LOFT_CUT_X, -500.0, -100.0], [1000.0, 1000.0, LOFT_H + 200.0]);
        solid(&loft.subtract(&cut), None, " (smooth loft)")
    });

    run("6-offset-profile", || {
        let profile = Wire::rect(PROFILE[0], PROFILE[1]).fillet(PROFILE_R);
        let inner = Face::from_wire(&profile).surface_area();
        let off = profile.offset(OFFSET, JoinType::Arc);
        let area = Face::from_wire(&off).surface_area();
        let mut note = format!("profile area {:.1} (exact {:.1})", inner, rounded_rect_area(PROFILE[0], PROFILE[1], PROFILE_R));
        let area = if area < inner {
            note.push_str(", sign flipped");
            Face::from_wire(&profile.offset(-OFFSET, JoinType::Arc)).surface_area()
        } else {
            area
        };
        Outcome::Area { area, expected: expected_offset_area(), note }
    });
}
