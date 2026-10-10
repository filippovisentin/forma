//! Hidden-line drawings (Rhino's Make2D) of meshes and curves seen along a
//! parallel view direction: the visible and hidden parts of mesh outlines
//! (borders, creases between faces, silhouettes) and of curves, flattened into
//! the view's 2-D coordinates.

use crate::{Mesh, Point3, Vec3};
use std::collections::HashMap;

/// A parallel view: `dir` points from the viewer into the scene; `u`, `v` are
/// the drawing's right and up directions (unit vectors, perpendicular to `dir`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View2D {
    pub dir: Vec3,
    pub u: Vec3,
    pub v: Vec3,
}

impl View2D {
    /// Standard views by name: top, bottom, front, back, right, left.
    pub fn named(name: &str) -> Option<View2D> {
        let v = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        Some(match name.to_ascii_lowercase().as_str() {
            "top" => View2D {
                dir: v(0.0, 0.0, -1.0),
                u: v(1.0, 0.0, 0.0),
                v: v(0.0, 1.0, 0.0),
            },
            "bottom" => View2D {
                dir: v(0.0, 0.0, 1.0),
                u: v(1.0, 0.0, 0.0),
                v: v(0.0, -1.0, 0.0),
            },
            "front" => View2D {
                dir: v(0.0, 1.0, 0.0),
                u: v(1.0, 0.0, 0.0),
                v: v(0.0, 0.0, 1.0),
            },
            "back" => View2D {
                dir: v(0.0, -1.0, 0.0),
                u: v(-1.0, 0.0, 0.0),
                v: v(0.0, 0.0, 1.0),
            },
            "right" => View2D {
                dir: v(-1.0, 0.0, 0.0),
                u: v(0.0, 1.0, 0.0),
                v: v(0.0, 0.0, 1.0),
            },
            "left" => View2D {
                dir: v(1.0, 0.0, 0.0),
                u: v(0.0, -1.0, 0.0),
                v: v(0.0, 0.0, 1.0),
            },
            _ => return None,
        })
    }

    /// Drawing coordinates of a point (z = 0).
    pub fn flatten(&self, p: Point3) -> Point3 {
        let w = p.to_vec();
        Point3::new(w.dot(self.u), w.dot(self.v), 0.0)
    }
}

/// Result of [`make2d`]: segments in drawing coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing {
    pub visible: Vec<[Point3; 2]>,
    pub hidden: Vec<[Point3; 2]>,
}

type Key = (i64, i64, i64);

fn key(p: Point3, q: f64) -> Key {
    (
        (p.x / q).round() as i64,
        (p.y / q).round() as i64,
        (p.z / q).round() as i64,
    )
}

/// Edges of a mesh worth drawing from `dir`: borders, creases sharper than
/// `crease` (radians) and silhouettes (one face towards the viewer, one away).
pub fn outline_edges(m: &Mesh, dir: Vec3, crease: f64, tol: f64) -> Vec<[Point3; 2]> {
    let q = tol.max(1e-9);
    let normals: Vec<Option<Vec3>> = m
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.positions[i as usize]);
            (b - a).cross(c - a).normalized()
        })
        .collect();
    let mut edges: HashMap<(Key, Key), (Point3, Point3, Vec<usize>)> = HashMap::new();
    for (ti, t) in m.triangles.iter().enumerate() {
        if normals[ti].is_none() {
            continue; // degenerate
        }
        for e in 0..3 {
            let a = m.positions[t[e] as usize];
            let b = m.positions[t[(e + 1) % 3] as usize];
            let (ka, kb) = (key(a, q), key(b, q));
            if ka == kb {
                continue;
            }
            let k = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges.entry(k).or_insert((a, b, Vec::new())).2.push(ti);
        }
    }
    let cos_crease = crease.cos();
    let mut out = Vec::new();
    for (a, b, tris) in edges.into_values() {
        let keep = match tris.as_slice() {
            [t1, t2] => {
                let (n1, n2) = (normals[*t1].expect("kept"), normals[*t2].expect("kept"));
                let crease_edge = n1.dot(n2) < cos_crease;
                let (f1, f2) = (n1.dot(dir), n2.dot(dir));
                let silhouette = (f1 < 0.0) != (f2 < 0.0);
                crease_edge || silhouette
            }
            _ => true, // border or non-manifold
        };
        if keep {
            out.push([a, b]);
        }
    }
    out
}

/// Triangles binned on a grid in drawing coordinates, for the visibility rays.
struct Occluders<'a> {
    tris: Vec<[Point3; 3]>,
    min: [f64; 2],
    cell: f64,
    n: usize,
    bins: Vec<Vec<u32>>,
    view: &'a View2D,
}

impl<'a> Occluders<'a> {
    fn new(meshes: &[&Mesh], view: &'a View2D) -> Occluders<'a> {
        let tris: Vec<[Point3; 3]> = meshes
            .iter()
            .flat_map(|m| {
                m.triangles
                    .iter()
                    .map(|t| t.map(|i| m.positions[i as usize]))
            })
            .collect();
        let flat = |p: Point3| {
            let f = view.flatten(p);
            [f.x, f.y]
        };
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for t in &tris {
            for p in t {
                let f = flat(*p);
                for k in 0..2 {
                    lo[k] = lo[k].min(f[k]);
                    hi[k] = hi[k].max(f[k]);
                }
            }
        }
        let n = ((tris.len() as f64).sqrt() as usize).clamp(1, 256);
        let size = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-9);
        let cell = size / n as f64 * 1.0001;
        let mut bins = vec![Vec::new(); n * n];
        if tris.is_empty() {
            return Occluders {
                tris,
                min: lo,
                cell,
                n,
                bins,
                view,
            };
        }
        for (i, t) in tris.iter().enumerate() {
            let fs = t.map(flat);
            let (mut a, mut b) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for f in fs {
                for k in 0..2 {
                    a[k] = a[k].min(f[k]);
                    b[k] = b[k].max(f[k]);
                }
            }
            let c = |x: f64, k: usize| (((x - lo[k]) / cell).floor().max(0.0) as usize).min(n - 1);
            for ix in c(a[0], 0)..=c(b[0], 0) {
                for iy in c(a[1], 1)..=c(b[1], 1) {
                    bins[iy * n + ix].push(i as u32);
                }
            }
        }
        Occluders {
            tris,
            min: lo,
            cell,
            n,
            bins,
            view,
        }
    }

    /// Is `p` hidden: does a triangle lie between it and the viewer (beyond
    /// `eps`)?
    fn hidden(&self, p: Point3, eps: f64) -> bool {
        if self.tris.is_empty() {
            return false;
        }
        let f = self.view.flatten(p);
        let ix = ((f.x - self.min[0]) / self.cell).floor();
        let iy = ((f.y - self.min[1]) / self.cell).floor();
        if ix < 0.0 || iy < 0.0 || ix >= self.n as f64 || iy >= self.n as f64 {
            return false;
        }
        let back = -self.view.dir;
        self.bins[iy as usize * self.n + ix as usize]
            .iter()
            .any(|&i| ray_hits(p, back, &self.tris[i as usize], eps))
    }
}

/// Möller–Trumbore: does the ray `o + t·d` (t > eps) cross the triangle? Rays
/// grazing an edge of the triangle count as misses, so faces do not hide the
/// edges they share with their neighbours.
fn ray_hits(o: Point3, d: Vec3, t: &[Point3; 3], eps: f64) -> bool {
    let e1 = t[1] - t[0];
    let e2 = t[2] - t[0];
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-14 {
        return false;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    const IN: f64 = 1e-7;
    if u <= IN || u >= 1.0 - IN {
        return false;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v <= IN || u + v >= 1.0 - IN {
        return false;
    }
    e2.dot(q) * inv > eps
}

/// Hidden-line drawing of `meshes` and `curves` (polylines) seen along `view`.
/// `size` is the model size, used for the sampling step and the self-hiding
/// tolerance.
pub fn make2d(meshes: &[&Mesh], curves: &[Vec<Point3>], view: &View2D, size: f64) -> Drawing {
    let tol = (size * 1e-7).max(1e-9);
    let mut segs: Vec<[Point3; 2]> = meshes
        .iter()
        .flat_map(|m| outline_edges(m, view.dir, 20f64.to_radians(), tol))
        .collect();
    for c in curves {
        segs.extend(c.windows(2).map(|w| [w[0], w[1]]));
    }
    let occ = Occluders::new(meshes, view);
    let step = (size / 400.0).max(1e-9);
    let eps = (size * 1e-6).max(1e-9);
    let mut d = Drawing::default();
    for [a, b] in segs {
        let len = a.distance_to(b);
        if len <= tol {
            continue;
        }
        let n = ((len / step).ceil() as usize).clamp(1, 200);
        // Visibility at the middle of each piece; runs of equal state are merged.
        let mut run_start = a;
        let mut run_state: Option<bool> = None;
        for k in 0..n {
            let t0 = k as f64 / n as f64;
            let t1 = (k + 1) as f64 / n as f64;
            let mid = a + (b - a) * ((t0 + t1) / 2.0);
            let hidden = occ.hidden(mid, eps);
            let p0 = a + (b - a) * t0;
            match run_state {
                None => {
                    run_state = Some(hidden);
                    run_start = p0;
                }
                Some(s) if s != hidden => {
                    let out = if s { &mut d.hidden } else { &mut d.visible };
                    out.push([view.flatten(run_start), view.flatten(p0)]);
                    run_state = Some(hidden);
                    run_start = p0;
                }
                Some(_) => {}
            }
        }
        if let Some(s) = run_state {
            let out = if s { &mut d.hidden } else { &mut d.visible };
            out.push([view.flatten(run_start), view.flatten(b)]);
        }
    }
    tidy(&mut d, (size * 1e-6).max(1e-9));
    d
}

/// Drop segments seen end-on, repeated segments, and hidden segments drawn
/// over visible ones (edges that coincide in the view).
fn tidy(d: &mut Drawing, q: f64) {
    let k = |s: &[Point3; 2]| {
        let (a, b) = (key(s[0], q), key(s[1], q));
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    };
    let mut seen = std::collections::HashSet::new();
    d.visible
        .retain(|s| s[0].distance_to(s[1]) > q * 10.0 && seen.insert(k(s)));
    d.hidden
        .retain(|s| s[0].distance_to(s[1]) > q * 10.0 && seen.insert(k(s)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{box_mesh, Plane};

    fn total(s: &[[Point3; 2]]) -> f64 {
        s.iter().map(|[a, b]| a.distance_to(*b)).sum()
    }

    #[test]
    fn box_from_the_top_shows_its_outline() {
        let m = box_mesh(&Plane::TOP, 10.0, 20.0, 5.0);
        let view = View2D::named("top").unwrap();
        let d = make2d(&[&m], &[], &view, 25.0);
        // Visible: the top rectangle; the bottom one coincides with it and the
        // vertical edges are seen end-on, so neither is drawn.
        assert!(
            (total(&d.visible) - 60.0).abs() < 1e-6,
            "{}",
            total(&d.visible)
        );
        assert!(total(&d.hidden) < 1e-6, "{:?}", d.hidden);
        assert!(d.visible.iter().flatten().all(|p| p.z.abs() < 1e-12));
    }

    #[test]
    fn box_in_front_hides_part_of_a_line_behind() {
        let m = box_mesh(&Plane::TOP, 10.0, 10.0, 10.0);
        let line = vec![Point3::new(-10.0, 20.0, 5.0), Point3::new(20.0, 20.0, 5.0)];
        let view = View2D::named("front").unwrap();
        let d = make2d(&[&m], &[line], &view, 30.0);
        // Front view of a cube: a 10 x 10 square outline visible.
        // The line behind it (y = 20) is hidden for 10 of its 30 units.
        let hidden_line: f64 = d
            .hidden
            .iter()
            .filter(|[a, b]| (a.y - 5.0).abs() < 1e-9 && (b.y - 5.0).abs() < 1e-9)
            .map(|[a, b]| a.distance_to(*b))
            .sum();
        assert!((hidden_line - 10.0).abs() < 0.2, "{hidden_line}");
        assert!(
            total(&d.visible) >= 40.0 + 20.0 - 0.5,
            "{}",
            total(&d.visible)
        );
    }
}
