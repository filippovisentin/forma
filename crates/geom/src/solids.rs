//! Mesh solids for display and export until the solid kernel lands (ADR 0001).
//!
//! Every builder makes each flat face (or smooth face group) its own set of vertices,
//! so `Mesh::boundary_edges` returns exactly the edges a user expects to see.

use crate::{Mesh, Plane, Point3, Vec3};
use std::f64::consts::TAU;

fn push_quad(m: &mut Mesh, p: [Point3; 4], n: Vec3) {
    let base = m.positions.len() as u32;
    m.positions.extend_from_slice(&p);
    m.normals.extend_from_slice(&[n; 4]);
    m.triangles.push([base, base + 1, base + 2]);
    m.triangles.push([base, base + 2, base + 3]);
}

/// Box on `plane`: from the plane origin, `u` along x, `v` along y, `w` along the
/// normal. Negative sizes are allowed.
pub fn box_mesh(plane: &Plane, u: f64, v: f64, w: f64) -> Mesh {
    // Normalise to positive extents by moving the origin.
    let o = plane.point_at(u.min(0.0), v.min(0.0), w.min(0.0));
    let (u, v, w) = (u.abs(), v.abs(), w.abs());
    let (x, y, z) = (plane.x * u, plane.y * v, plane.z * w);
    let c = |a: f64, b: f64, h: f64| o + x * a + y * b + z * h;
    let mut m = Mesh::default();
    // Bottom (−z), top (+z), front (−y), back (+y), left (−x), right (+x); CCW from outside.
    push_quad(
        &mut m,
        [c(0., 0., 0.), c(0., 1., 0.), c(1., 1., 0.), c(1., 0., 0.)],
        -plane.z,
    );
    push_quad(
        &mut m,
        [c(0., 0., 1.), c(1., 0., 1.), c(1., 1., 1.), c(0., 1., 1.)],
        plane.z,
    );
    push_quad(
        &mut m,
        [c(0., 0., 0.), c(1., 0., 0.), c(1., 0., 1.), c(0., 0., 1.)],
        -plane.y,
    );
    push_quad(
        &mut m,
        [c(1., 1., 0.), c(0., 1., 0.), c(0., 1., 1.), c(1., 1., 1.)],
        plane.y,
    );
    push_quad(
        &mut m,
        [c(0., 1., 0.), c(0., 0., 0.), c(0., 0., 1.), c(0., 1., 1.)],
        -plane.x,
    );
    push_quad(
        &mut m,
        [c(1., 0., 0.), c(1., 1., 0.), c(1., 1., 1.), c(1., 0., 1.)],
        plane.x,
    );
    m
}

/// Ear-clipping triangulation of a simple polygon (no repeated closing point),
/// returned counter-clockwise around `normal`.
pub fn triangulate_polygon(points: &[Point3], normal: Vec3) -> Vec<[u32; 3]> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let plane = Plane::from_normal(points[0], normal);
    let p2: Vec<(f64, f64)> = points
        .iter()
        .map(|p| {
            let (u, v, _) = plane.coords(*p);
            (u, v)
        })
        .collect();
    let area: f64 = (0..n)
        .map(|i| {
            let (a, b) = (p2[i], p2[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0;
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let cross = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    let inside = |p: (f64, f64), a, b, c| {
        cross(a, b, p) >= -1e-12 && cross(b, c, p) >= -1e-12 && cross(c, a, p) >= -1e-12
    };
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (p2[ia], p2[ib], p2[ic]);
            if cross(a, b, c) <= 1e-12 {
                continue; // reflex or degenerate
            }
            let blocked = idx
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && inside(p2[j], a, b, c));
            if !blocked {
                out.push([ia as u32, ib as u32, ic as u32]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // Degenerate input: fall back to a fan so we still return something.
            break;
        }
    }
    if idx.len() >= 3 {
        for k in 1..idx.len() - 1 {
            out.push([idx[0] as u32, idx[k] as u32, idx[k + 1] as u32]);
        }
    }
    out
}

/// Newell normal of a polygon (unnormalised; zero for degenerate input).
fn polygon_normal(points: &[Point3]) -> Vec3 {
    let n = points.len();
    let mut v = Vec3::new(0.0, 0.0, 0.0);
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        v = v + Vec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    v
}

/// Extrude a profile along `dir`. A closed profile (first point repeated at the end,
/// or `closed` set) gives a capped solid; an open one gives a surface. Corners
/// sharper than 30° become visible edges; smoother ones (arcs) are shaded smooth.
pub fn extrude_mesh(profile: &[Point3], dir: Vec3, closed: bool) -> Mesh {
    let mut pts: Vec<Point3> = profile.to_vec();
    let closed = closed || (pts.len() > 2 && pts[0].distance_to(*pts.last().expect("len")) < 1e-9);
    if closed && pts.len() > 1 && pts[0].distance_to(*pts.last().expect("len")) < 1e-9 {
        pts.pop();
    }
    pts.dedup_by(|a, b| a.distance_to(*b) < 1e-9);
    let mut m = Mesh::default();
    if pts.len() < 2 || dir.length() < 1e-12 {
        return m;
    }
    if closed && pts.len() >= 3 && polygon_normal(&pts).dot(dir) < 0.0 {
        pts.reverse();
    }
    let n = pts.len();
    let seg_count = if closed { n } else { n - 1 };
    let seg_dir = |i: usize| (pts[(i + 1) % n] - pts[i]).normalized().unwrap_or(Vec3::X);
    let seg_normal = |i: usize| seg_dir(i).cross(dir).normalized().unwrap_or(Vec3::X);

    // Side: each segment is a quad; vertices are shared with the neighbour when the
    // corner is smooth.
    let crease = 30f64.to_radians().cos();
    let bottom_top = |m: &mut Mesh, p: Point3, nrm: Vec3| -> u32 {
        let i = m.positions.len() as u32;
        m.positions.push(p);
        m.normals.push(nrm);
        m.positions.push(p + dir);
        m.normals.push(nrm);
        i
    };
    // Start column of each segment and end column of each segment.
    let mut start_col = vec![0u32; seg_count];
    let mut end_col = vec![0u32; seg_count];
    for i in 0..seg_count {
        let prev = if i == 0 {
            if closed {
                Some(seg_count - 1)
            } else {
                None
            }
        } else {
            Some(i - 1)
        };
        let smooth_with_prev = prev.is_some_and(|p| seg_dir(p).dot(seg_dir(i)) > crease);
        if smooth_with_prev && i > 0 {
            start_col[i] = end_col[i - 1];
        } else {
            let nrm = match prev {
                Some(p) if smooth_with_prev => (seg_normal(p) + seg_normal(i))
                    .normalized()
                    .unwrap_or(seg_normal(i)),
                _ => seg_normal(i),
            };
            start_col[i] = bottom_top(&mut m, pts[i], nrm);
        }
        let next = if i + 1 < seg_count {
            Some(i + 1)
        } else if closed {
            Some(0)
        } else {
            None
        };
        let smooth_with_next = next.is_some_and(|q| seg_dir(i).dot(seg_dir(q)) > crease);
        let nrm = match next {
            Some(q) if smooth_with_next => (seg_normal(i) + seg_normal(q))
                .normalized()
                .unwrap_or(seg_normal(i)),
            _ => seg_normal(i),
        };
        end_col[i] = bottom_top(&mut m, pts[(i + 1) % n], nrm);
    }
    // Close the smooth seam of a closed profile: last segment's end column = first start.
    if closed && seg_count > 1 && seg_dir(seg_count - 1).dot(seg_dir(0)) > crease {
        // Re-point the last segment's end to the first segment's start column so the
        // seam is not a boundary edge.
        end_col[seg_count - 1] = start_col[0];
    }
    for i in 0..seg_count {
        let (a, b) = (start_col[i], end_col[i]);
        m.triangles.push([a, b, b + 1]);
        m.triangles.push([a, b + 1, a + 1]);
    }

    if closed && n >= 3 {
        let tris = triangulate_polygon(&pts, dir);
        let nd = dir.normalized().unwrap_or(Vec3::Z);
        let base = m.positions.len() as u32;
        m.positions.extend(pts.iter().copied());
        m.normals.extend(std::iter::repeat_n(-nd, n));
        m.triangles
            .extend(tris.iter().map(|t| [base + t[0], base + t[2], base + t[1]]));
        let base = m.positions.len() as u32;
        m.positions.extend(pts.iter().map(|p| *p + dir));
        m.normals.extend(std::iter::repeat_n(nd, n));
        m.triangles
            .extend(tris.iter().map(|t| [base + t[0], base + t[1], base + t[2]]));
    }
    m
}

/// Cylinder standing on `base` (circle centre at the plane origin), height along the
/// plane normal (may be negative).
pub fn cylinder_mesh(base: &Plane, radius: f64, height: f64, segments: usize) -> Mesh {
    let seg = segments.max(8);
    let pts: Vec<Point3> = (0..seg)
        .map(|i| {
            let a = TAU * i as f64 / seg as f64;
            base.point_at(radius * a.cos(), radius * a.sin(), 0.0)
        })
        .collect();
    extrude_mesh(&pts, base.z * height, true)
}

/// UV sphere.
pub fn sphere_mesh(center: Point3, radius: f64, segments: usize) -> Mesh {
    let seg = segments.max(8);
    let rings = seg / 2;
    let mut m = Mesh::default();
    for r in 0..=rings {
        let phi = std::f64::consts::PI * r as f64 / rings as f64;
        for s in 0..seg {
            let th = TAU * s as f64 / seg as f64;
            let n = Vec3::new(phi.sin() * th.cos(), phi.sin() * th.sin(), phi.cos());
            m.positions.push(center + n * radius);
            m.normals.push(n);
        }
    }
    let id = |r: usize, s: usize| (r * seg + s % seg) as u32;
    for r in 0..rings {
        for s in 0..seg {
            let (a, b, c, d) = (id(r, s), id(r + 1, s), id(r + 1, s + 1), id(r, s + 1));
            m.triangles.push([a, b, c]);
            m.triangles.push([a, c, d]);
        }
    }
    // Weld the pole rings: every vertex of ring 0 / last ring is the same point; map
    // them to the first so no boundary edges appear.
    for t in &mut m.triangles {
        for i in t.iter_mut() {
            let r = *i as usize / seg;
            if r == 0 {
                *i = 0;
            } else if r == rings {
                *i = (rings * seg) as u32;
            }
        }
    }
    m.triangles
        .retain(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(m: &Mesh) -> f64 {
        m.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize].to_vec());
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }

    #[test]
    fn box_volume_edges_and_orientation() {
        let m = box_mesh(&Plane::TOP, 2.0, -3.0, 4.0);
        assert!((volume(&m) - 24.0).abs() < 1e-9);
        assert_eq!(m.boundary_edges().len(), 24); // 12 edges × 2 sides (per-face vertices)
    }

    #[test]
    fn extruded_rectangle_is_a_closed_box() {
        let r = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 2.0, 0.0), // clockwise on purpose
            Point3::new(3.0, 2.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, 0.0),
        ];
        let m = extrude_mesh(&r, Vec3::new(0.0, 0.0, 5.0), false);
        assert!((volume(&m) - 30.0).abs() < 1e-9, "{}", volume(&m));
    }

    #[test]
    fn cylinder_volume_and_smooth_side() {
        let m = cylinder_mesh(&Plane::TOP, 1.0, 2.0, 64);
        let exact = std::f64::consts::PI * 2.0;
        let v = volume(&m);
        assert!((v - exact).abs() / exact < 0.01, "{v}");
        // Only the two circles are edges: no vertical seams on a smooth side.
        let verticals = m
            .boundary_edges()
            .iter()
            .filter(|e| {
                let (a, b) = (m.positions[e[0] as usize], m.positions[e[1] as usize]);
                (a.z - b.z).abs() > 1e-9
            })
            .count();
        assert_eq!(verticals, 0);
    }

    #[test]
    fn sphere_is_closed() {
        let m = sphere_mesh(Point3::ORIGIN, 1.0, 32);
        assert!(m.boundary_edges().is_empty());
        let v = volume(&m);
        let exact = 4.0 / 3.0 * std::f64::consts::PI;
        assert!((v - exact).abs() / exact < 0.03, "{v}");
    }

    #[test]
    fn concave_polygon_triangulates() {
        let l = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
        ];
        let t = triangulate_polygon(&l, Vec3::Z);
        assert_eq!(t.len(), 4);
        let area: f64 = t
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| l[i as usize]);
                (b - a).cross(c - a).z / 2.0
            })
            .sum();
        assert!((area - 3.0).abs() < 1e-12);
    }
}
