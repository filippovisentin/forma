//! Shared measurement code for the kernel spike (included by both binaries with #[path]).
#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Duration;

pub struct TriMesh {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[usize; 3]>,
}

pub struct MeshStats {
    pub triangles: usize,
    pub volume: f64,
    /// Every edge shared by exactly two triangles with opposite directions.
    pub watertight: bool,
    pub boundary_edges: usize,
}

impl TriMesh {
    /// Weld coincident vertices (tessellators emit one vertex per face).
    fn welded(&self, eps: f64) -> Vec<usize> {
        let mut map: HashMap<(i64, i64, i64), usize> = HashMap::new();
        let mut ids = Vec::with_capacity(self.positions.len());
        for p in &self.positions {
            let k = (
                (p[0] / eps).round() as i64,
                (p[1] / eps).round() as i64,
                (p[2] / eps).round() as i64,
            );
            let n = map.len();
            ids.push(*map.entry(k).or_insert(n));
        }
        ids
    }

    pub fn stats(&self) -> MeshStats {
        let ids = self.welded(1e-4);
        let mut directed: HashMap<(usize, usize), i32> = HashMap::new();
        let mut volume = 0.0;
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i]);
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
            let w = t.map(|i| ids[i]);
            if w[0] == w[1] || w[1] == w[2] || w[0] == w[2] {
                continue; // degenerate sliver
            }
            for (u, v) in [(w[0], w[1]), (w[1], w[2]), (w[2], w[0])] {
                *directed.entry((u, v)).or_default() += 1;
            }
        }
        let mut boundary = 0;
        let mut ok = true;
        for (&(u, v), &n) in &directed {
            let back = directed.get(&(v, u)).copied().unwrap_or(0);
            if n != 1 || back != 1 {
                ok = false;
                if back == 0 {
                    boundary += 1;
                }
            }
        }
        MeshStats {
            triangles: self.triangles.len(),
            volume,
            watertight: ok,
            boundary_edges: boundary,
        }
    }

    pub fn write_stl(&self, path: &str) {
        let mut s = String::from("solid forma\n");
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i]);
            let _ = writeln!(s, "facet normal 0 0 0\nouter loop");
            for p in [a, b, c] {
                let _ = writeln!(s, "vertex {} {} {}", p[0], p[1], p[2]);
            }
            let _ = writeln!(s, "endloop\nendfacet");
        }
        s.push_str("endsolid forma\n");
        std::fs::create_dir_all("out").ok();
        std::fs::write(path, s).expect("write stl");
    }
}

/// One line of the results table.
pub enum Outcome {
    Solid { mesh: TriMesh, expected_volume: Option<f64>, note: String },
    Area { area: f64, expected: f64, note: String },
    Failed(String),
    Unsupported(String),
}

pub fn report(kernel: &str, case: &str, outcome: Outcome, time: Duration) {
    let ms = time.as_secs_f64() * 1000.0;
    match outcome {
        Outcome::Solid { mesh, expected_volume, note } => {
            let st = mesh.stats();
            let err = expected_volume
                .map(|e| format!("{:+.4}%", (st.volume - e) / e * 100.0))
                .unwrap_or_else(|| "n/a".into());
            mesh.write_stl(&format!("out/{kernel}_{case}.stl"));
            println!(
                "| {kernel} | {case} | ok | {} | {:.0} | {err} | {} tris | {ms:.0} ms | {note} |",
                if st.watertight { "yes" } else { "NO" },
                st.volume,
                st.triangles
            );
            if !st.watertight {
                println!("|  |  |  | ({} open edges) |  |  |  |  |  |", st.boundary_edges);
            }
        }
        Outcome::Area { area, expected, note } => println!(
            "| {kernel} | {case} | ok | – | area {area:.1} | {:+.4}% | – | {ms:.0} ms | {note} |",
            (area - expected) / expected * 100.0
        ),
        Outcome::Failed(e) => println!("| {kernel} | {case} | **FAILED** | – | – | – | – | {ms:.0} ms | {e} |"),
        Outcome::Unsupported(e) => println!("| {kernel} | {case} | not available | – | – | – | – | – | {e} |"),
    }
}

pub fn header() {
    println!("| kernel | case | result | watertight | volume | vs expected | mesh | time | notes |");
    println!("|---|---|---|---|---|---|---|---|---|");
}

// ---- The six cases (millimetres) and their exact expected values ----

/// 1. Wall 4000×300×2700 with a 1000×150×1200 niche flush with the front face (coplanar).
pub const WALL: [f64; 3] = [4000.0, 300.0, 2700.0];
pub const NICHE_MIN: [f64; 3] = [1500.0, 0.0, 900.0];
pub const NICHE: [f64; 3] = [1000.0, 150.0, 1200.0];
pub fn expected_niche() -> f64 {
    WALL.iter().product::<f64>() - NICHE.iter().product::<f64>()
}

/// 2. Table top 1600×800×30 with a Ø80 cable hole through the centre.
pub const TOP: [f64; 3] = [1600.0, 800.0, 30.0];
pub const HOLE_R: f64 = 40.0;
pub fn expected_hole() -> f64 {
    TOP.iter().product::<f64>() - std::f64::consts::PI * HOLE_R * HOLE_R * TOP[2]
}

/// 3. Five cabinet modules 600×580×720 side by side, sharing faces.
pub const MODULE: [f64; 3] = [600.0, 580.0, 720.0];
pub fn expected_cabinet() -> f64 {
    5.0 * MODULE.iter().product::<f64>()
}

/// 4. Panel 600×400×18, all edges filleted r = 3.
pub const PANEL: [f64; 3] = [600.0, 400.0, 18.0];
pub const FILLET_R: f64 = 3.0;
pub fn expected_fillet() -> f64 {
    let r = FILLET_R;
    let [a, b, c] = PANEL.map(|d| d - 2.0 * r);
    let pi = std::f64::consts::PI;
    a * b * c + 2.0 * r * (a * b + b * c + a * c) + pi * r * r * (a + b + c) + 4.0 / 3.0 * pi * r.powi(3)
}

/// 5. Square-to-round loft (400×400 square at z=0 → Ø300 circle at z=500), then
///    subtract a box removing everything with x > 100 ("cut lamp shade").
pub const LOFT_SQUARE: f64 = 400.0;
pub const LOFT_R: f64 = 150.0;
pub const LOFT_H: f64 = 500.0;
pub const LOFT_CUT_X: f64 = 100.0;

/// 6. Rounded rectangle 600×400 r=50 offset outwards by 20 → 640×440 r=70.
pub const PROFILE: [f64; 2] = [600.0, 400.0];
pub const PROFILE_R: f64 = 50.0;
pub const OFFSET: f64 = 20.0;
pub fn rounded_rect_area(w: f64, h: f64, r: f64) -> f64 {
    w * h - (4.0 - std::f64::consts::PI) * r * r
}
pub fn expected_offset_area() -> f64 {
    rounded_rect_area(PROFILE[0] + 2.0 * OFFSET, PROFILE[1] + 2.0 * OFFSET, PROFILE_R + OFFSET)
}

/// Shoelace area of a closed 2D polyline.
pub fn polygon_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let [x0, y0] = pts[i];
            let [x1, y1] = pts[(i + 1) % n];
            x0 * y1 - x1 * y0
        })
        .sum::<f64>()
        .abs()
        / 2.0
}
