//! Kernel spike, pure-Rust candidate: truck (B-rep + booleans) and curvo (curve offset).

#[path = "../../common.rs"]
mod common;

use common::*;
use std::f64::consts::PI;
use std::time::Instant;
use truck_meshalgo::prelude::*;
use truck_modeling::*;

const TOL: f64 = 0.01;
const MESH_TOL: f64 = 0.5;

fn bx(min: [f64; 3], size: [f64; 3]) -> Solid {
    let v = builder::vertex(Point3::new(min[0], min[1], min[2]));
    let e = builder::tsweep(&v, Vector3::new(size[0], 0.0, 0.0));
    let f = builder::tsweep(&e, Vector3::new(0.0, size[1], 0.0));
    builder::tsweep(&f, Vector3::new(0.0, 0.0, size[2]))
}

fn cylinder(center: [f64; 3], r: f64, h: f64) -> Solid {
    let c = Point3::new(center[0], center[1], center[2]);
    let v = builder::vertex(c + Vector3::new(r, 0.0, 0.0));
    let wire: Wire = builder::rsweep(&v, c, Vector3::unit_z(), Rad(2.0 * PI));
    let face = builder::try_attach_plane(&[wire]).expect("disk");
    builder::tsweep(&face, Vector3::new(0.0, 0.0, h))
}

fn not(s: &Solid) -> Solid {
    let mut s = s.clone();
    s.not();
    s
}

fn mesh(s: &Solid) -> TriMesh {
    let poly = s.triangulation(MESH_TOL).to_polygon();
    let positions: Vec<[f64; 3]> = poly.positions().iter().map(|p| [p.x, p.y, p.z]).collect();
    let triangles = poly
        .faces()
        .triangle_iter()
        .map(|t| [t[0].pos, t[1].pos, t[2].pos])
        .collect();
    TriMesh { positions, triangles }
}

/// Run `f`, catching panics (truck panics on some degenerate inputs).
fn run(case: &str, f: impl FnOnce() -> Outcome + std::panic::UnwindSafe) {
    let t = Instant::now();
    let out = std::panic::catch_unwind(f).unwrap_or_else(|e| {
        let msg = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "panic".into());
        Outcome::Failed(format!("panic: {}", msg.lines().next().unwrap_or("")))
    });
    report("truck", case, out, t.elapsed());
}

fn solid(s: Option<Solid>, expected: Option<f64>, note: &str) -> Outcome {
    match s {
        Some(s) => Outcome::Solid {
            mesh: mesh(&s),
            expected_volume: expected,
            note: format!("{} face(s)", s.face_iter().count()) + note,
        },
        None => Outcome::Failed("boolean returned None".into()),
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    header();

    run("1-niche", || {
        let wall = bx([0.0; 3], WALL);
        let niche = bx(NICHE_MIN, NICHE);
        solid(truck_shapeops::and(&wall, &not(&niche), TOL), Some(expected_niche()), "")
    });

    // Diagnostic: same niche, but the cutter pokes 10 mm out of the wall (no coplanar faces).
    run("1b-niche-overlap", || {
        let wall = bx([0.0; 3], WALL);
        let niche = bx(
            [NICHE_MIN[0], NICHE_MIN[1] - 10.0, NICHE_MIN[2]],
            [NICHE[0], NICHE[1] + 10.0, NICHE[2]],
        );
        solid(truck_shapeops::and(&wall, &not(&niche), TOL), Some(expected_niche()), "")
    });

    // Diagnostic: two modules overlapping by 1 mm instead of touching.
    run("3b-union-overlap", || {
        let a = bx([0.0; 3], MODULE);
        let b = bx([MODULE[0] - 1.0, 0.0, 0.0], MODULE);
        let exp = 2.0 * MODULE.iter().product::<f64>() - MODULE[1] * MODULE[2];
        solid(truck_shapeops::or(&a, &b, TOL), Some(exp), "")
    });

    run("2-cable-hole", || {
        let top = bx([0.0; 3], TOP);
        let hole = cylinder([TOP[0] / 2.0, TOP[1] / 2.0, -10.0], HOLE_R, TOP[2] + 20.0);
        solid(truck_shapeops::and(&top, &not(&hole), TOL), Some(expected_hole()), "")
    });

    run("3-cabinet-union", || {
        let mut acc = bx([0.0; 3], MODULE);
        for i in 1..5 {
            let m = bx([i as f64 * MODULE[0], 0.0, 0.0], MODULE);
            match truck_shapeops::or(&acc, &m, TOL) {
                Some(s) => acc = s,
                None => return Outcome::Failed(format!("union failed at module {}", i + 1)),
            }
        }
        solid(Some(acc), Some(expected_cabinet()), " (ideal: 6)")
    });

    run("4-fillet-panel", || {
        Outcome::Unsupported("truck 0.6 has no solid edge fillet".into())
    });

    run("5-loft-cut", || {
        let h = LOFT_SQUARE / 2.0;
        let sq: Vec<Vertex> = [(h, h), (-h, h), (-h, -h), (h, -h)]
            .iter()
            .map(|&(x, y)| builder::vertex(Point3::new(x, y, 0.0)))
            .collect();
        let d = LOFT_R / 2f64.sqrt();
        let ci: Vec<Vertex> = [(d, d), (-d, d), (-d, -d), (d, -d)]
            .iter()
            .map(|&(x, y)| builder::vertex(Point3::new(x, y, LOFT_H)))
            .collect();
        let w0: Wire = (0..4).map(|i| builder::line(&sq[i], &sq[(i + 1) % 4])).collect();
        let w1: Wire = (0..4)
            .map(|i| {
                let a = PI / 4.0 + PI / 2.0 * i as f64 + PI / 4.0;
                let transit = Point3::new(LOFT_R * a.cos(), LOFT_R * a.sin(), LOFT_H);
                builder::circle_arc(&ci[i], &ci[(i + 1) % 4], transit)
            })
            .collect();
        let side = match builder::try_wire_homotopy(&w0, &w1) {
            Ok(s) => s,
            Err(e) => return Outcome::Failed(format!("ruled loft: {e}")),
        };
        let bottom = builder::try_attach_plane(&[w0.inverse()]).expect("bottom cap");
        let top = builder::try_attach_plane(&[w1]).expect("top cap");
        let mut shell: Shell = side.into_iter().collect();
        shell.push(bottom);
        shell.push(top);
        let loft = match Solid::try_new(vec![shell.clone()]) {
            Ok(s) => s,
            Err(_) => {
                let mut inv = shell;
                inv.iter_mut().for_each(|f| {
                    f.invert();
                });
                match Solid::try_new(vec![inv]) {
                    Ok(s) => s,
                    Err(e) => return Outcome::Failed(format!("loft not closed: {e}")),
                }
            }
        };
        let cut = bx([LOFT_CUT_X, -500.0, -100.0], [1000.0, 1000.0, LOFT_H + 200.0]);
        solid(truck_shapeops::and(&loft, &not(&cut), TOL), None, " (ruled, not smooth loft)")
    });

    run("6-offset-profile", || {
        use curvo::prelude::*;
        use nalgebra::{Point2, Vector2};
        let [w, hgt] = PROFILE;
        let r = PROFILE_R;
        let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -hgt / 2.0, hgt / 2.0);
        let line = |a: (f64, f64), b: (f64, f64)| {
            NurbsCurve2D::polyline(&[Point2::new(a.0, a.1), Point2::new(b.0, b.1)], true)
        };
        let arc = |cx: f64, cy: f64, start: f64| {
            NurbsCurve2D::try_arc(
                &Point2::new(cx, cy),
                &Vector2::x(),
                &Vector2::y(),
                r,
                start,
                start + PI / 2.0,
            )
            .unwrap()
        };
        let spans = vec![
            line((x0 + r, y0), (x1 - r, y0)),
            arc(x1 - r, y0 + r, -PI / 2.0),
            line((x1, y0 + r), (x1, y1 - r)),
            arc(x1 - r, y1 - r, 0.0),
            line((x1 - r, y1), (x0 + r, y1)),
            arc(x0 + r, y1 - r, PI / 2.0),
            line((x0, y1 - r), (x0, y0 + r)),
            arc(x0 + r, y0 + r, PI),
        ];
        let profile = match CompoundCurve2D::try_new(spans) {
            Ok(c) => c,
            Err(e) => return Outcome::Failed(format!("profile: {e}")),
        };
        let opt = CurveOffsetOption::default()
            .with_distance(OFFSET)
            .with_corner_type(CurveOffsetCornerType::Round);
        let res = match profile.offset(opt) {
            Ok(r) => r,
            Err(e) => return Outcome::Failed(format!("offset: {e}")),
        };
        // curvo's offset sign convention depends on orientation: take the larger result.
        let areas: Vec<f64> = res
            .iter()
            .map(|c| {
                let pts: Vec<[f64; 2]> = c.tessellate(Some(1e-4)).iter().map(|p| [p.x, p.y]).collect();
                polygon_area(&pts)
            })
            .collect();
        let inner = rounded_rect_area(w, hgt, r);
        let mut area = areas.iter().cloned().fold(0.0, f64::max);
        let mut note = format!("{} curve(s)", res.len());
        if area < inner {
            // Offset went inwards; redo with the opposite sign.
            let opt = CurveOffsetOption::default()
                .with_distance(-OFFSET)
                .with_corner_type(CurveOffsetCornerType::Round);
            if let Ok(res) = profile.offset(opt) {
                area = res
                    .iter()
                    .map(|c| {
                        let pts: Vec<[f64; 2]> =
                            c.tessellate(Some(1e-4)).iter().map(|p| [p.x, p.y]).collect();
                        polygon_area(&pts)
                    })
                    .fold(0.0, f64::max);
                note.push_str(", sign flipped");
            }
        }
        Outcome::Area { area, expected: expected_offset_area(), note: format!("curvo, {note}") }
    });
}
