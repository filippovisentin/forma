//! More mesh primitives and surface builders (no solid kernel needed): cones,
//! truncated cones, tori, ellipsoids, pyramids, tubes, Coons patches, grids with
//! sharp creases, tapered extrusions.
//!
//! Like `solids.rs`, faces that meet at a sharp edge get their own vertices so the
//! edge is drawn.

use crate::solids::triangulate_polygon;
use crate::surface::{extrude_open_mesh, loft_mesh, planar_mesh, resample};
use crate::{sphere_mesh, Mesh, Plane, Point3, Vec3, Xform};
use std::f64::consts::TAU;

/// Circle points on `plane` (no repeated point).
pub fn ring(plane: &Plane, r: f64, seg: usize) -> Vec<Point3> {
    (0..seg)
        .map(|i| {
            let a = TAU * i as f64 / seg as f64;
            plane.point_at(r * a.cos(), r * a.sin(), 0.0)
        })
        .collect()
}

/// Flat cap through a closed loop (no repeated point) facing `normal`.
fn cap(points: &[Point3], normal: Vec3) -> Mesh {
    let n = normal.normalized().unwrap_or(Vec3::Z);
    let mut tris = triangulate_polygon(points, n);
    // Make every triangle face `n`.
    for t in &mut tris {
        let [a, b, c] = t.map(|i| points[i as usize]);
        if (b - a).cross(c - a).dot(n) < 0.0 {
            t.swap(1, 2);
        }
    }
    Mesh {
        positions: points.to_vec(),
        normals: vec![n; points.len()],
        triangles: tris,
    }
}

/// Cone or truncated cone on `base`: radius `r0` at the base, `r1` at `height`
/// along the normal (`r1 = 0` gives a pointed cone). Capped, closed, outward.
pub fn cone_mesh(base: &Plane, r0: f64, r1: f64, height: f64, seg: usize) -> Mesh {
    if height < 0.0 {
        let flipped = Plane {
            origin: base.origin,
            x: base.x,
            y: -base.y,
            z: -base.z,
        };
        return cone_mesh(&flipped, r0, r1, -height, seg);
    }
    let seg = seg.max(8);
    let top = base.moved_to(base.point_at(0.0, 0.0, height));
    let bottom_ring = ring(base, r0, seg);
    let mut m = Mesh::default();
    // Side normal: perpendicular to the slanted generator.
    let slope = (r0 - r1) / height.max(1e-12);
    let side_normal = |p: Point3| {
        let (u, v, _) = base.coords(p);
        let radial = (base.x * u + base.y * v).normalized().unwrap_or(base.x);
        (radial + base.z * slope).normalized().unwrap_or(radial)
    };
    let side: Vec<Vec3> = bottom_ring.iter().map(|p| side_normal(*p)).collect();
    m.positions.extend_from_slice(&bottom_ring);
    m.normals.extend_from_slice(&side);
    let s = seg as u32;
    if r1.abs() < 1e-12 {
        m.positions.push(top.origin);
        m.normals.push(base.z);
        for i in 0..s {
            m.triangles.push([i, (i + 1) % s, s]);
        }
    } else {
        let top_ring = ring(&top, r1, seg);
        m.positions.extend_from_slice(&top_ring);
        m.normals.extend_from_slice(&side);
        for i in 0..s {
            let j = (i + 1) % s;
            m.triangles.push([i, j, s + j]);
            m.triangles.push([i, s + j, s + i]);
        }
        m.append(&cap(&top_ring, base.z));
    }
    m.append(&cap(&bottom_ring, -base.z));
    m
}

/// Torus around the normal of `plane`: `major` radius to the tube centre,
/// `minor` tube radius.
pub fn torus_mesh(plane: &Plane, major: f64, minor: f64, seg: usize) -> Mesh {
    let seg_u = seg.max(8);
    let seg_v = (seg / 2).max(8);
    let mut m = Mesh::default();
    for i in 0..seg_u {
        let a = TAU * i as f64 / seg_u as f64;
        let radial = plane.x * a.cos() + plane.y * a.sin();
        for j in 0..seg_v {
            let b = TAU * j as f64 / seg_v as f64;
            let n = radial * b.cos() + plane.z * b.sin();
            m.positions.push(plane.origin + radial * major + n * minor);
            m.normals.push(n);
        }
    }
    let id = |i: usize, j: usize| ((i % seg_u) * seg_v + j % seg_v) as u32;
    for i in 0..seg_u {
        for j in 0..seg_v {
            let (a, b, c, d) = (id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1));
            m.triangles.push([a, b, c]);
            m.triangles.push([a, c, d]);
        }
    }
    m
}

/// Ellipsoid with semi-axes `rx`, `ry`, `rz` along the axes of `plane`.
pub fn ellipsoid_mesh(plane: &Plane, rx: f64, ry: f64, rz: f64, seg: usize) -> Mesh {
    let s = sphere_mesh(Point3::ORIGIN, 1.0, seg);
    // Unit sphere → scaled → placed on the plane.
    let place = Xform {
        m: [
            [plane.x.x * rx, plane.y.x * ry, plane.z.x * rz],
            [plane.x.y * rx, plane.y.y * ry, plane.z.y * rz],
            [plane.x.z * rx, plane.y.z * ry, plane.z.z * rz],
        ],
        t: plane.origin.to_vec(),
    };
    let flip = place.flips();
    Mesh {
        positions: s.positions.iter().map(|p| place.point(*p)).collect(),
        normals: s.normals.iter().map(|n| place.normal(*n)).collect(),
        triangles: if flip {
            s.triangles.iter().map(|t| [t[0], t[2], t[1]]).collect()
        } else {
            s.triangles
        },
    }
}

/// Pyramid over a closed base polygon (no repeated point) with its apex at `apex`.
/// Every face is flat with its own vertices; the result faces outwards.
pub fn pyramid_mesh(base: &[Point3], apex: Point3) -> Mesh {
    let n = base.len();
    let mut m = Mesh::default();
    if n < 3 {
        return m;
    }
    let centroid = base
        .iter()
        .fold(Vec3::new(0.0, 0.0, 0.0), |a, p| a + p.to_vec())
        * (1.0 / n as f64);
    let inside = Point3::ORIGIN + (centroid + (apex.to_vec() - centroid) * 0.25);
    for i in 0..n {
        let (a, b) = (base[i], base[(i + 1) % n]);
        let mut nrm = (b - a).cross(apex - a).normalized().unwrap_or(Vec3::Z);
        let mut tri = [a, b, apex];
        if nrm.dot(a - inside) < 0.0 {
            nrm = -nrm;
            tri = [b, a, apex];
        }
        let k = m.positions.len() as u32;
        m.positions.extend_from_slice(&tri);
        m.normals.extend_from_slice(&[nrm; 3]);
        m.triangles.push([k, k + 1, k + 2]);
    }
    let bn = crate::newell_area(base).normalized().unwrap_or(Vec3::Z);
    let outward = if bn.dot(base[0] - inside) < 0.0 {
        -bn
    } else {
        bn
    };
    m.append(&cap(base, outward));
    m
}

/// Hollow cylinder (pipe section) on `base`.
pub fn tube_mesh(base: &Plane, outer: f64, inner: f64, height: f64, seg: usize) -> Mesh {
    let seg = seg.max(8);
    let o = ring(base, outer.max(inner), seg);
    let i = ring(base, outer.min(inner), seg);
    let region = planar_mesh(&o, &[i], base.z);
    extrude_open_mesh(&region, base.z * height, 1e-9).unwrap_or_default()
}

/// Mesh through a grid of points: `rows[i][j]`, every row with the same number of
/// points. Columns where the first row turns by more than 30° and rows where the
/// row centres turn by more than 30° become visible creases. `closed_u` closes
/// each row into a loop (rows have no repeated point then).
pub fn grid_mesh(rows: &[Vec<Point3>], closed_u: bool) -> Mesh {
    let mut out = Mesh::default();
    let Some(n) = rows.first().map(Vec::len) else {
        return out;
    };
    if rows.len() < 2 || n < 2 || rows.iter().any(|r| r.len() != n) {
        return out;
    }
    let crease = 30f64.to_radians().cos();
    let sharp = |pts: &[Point3], i: usize, closed: bool| -> bool {
        let m = pts.len();
        if !closed && (i == 0 || i == m - 1) {
            return false;
        }
        let (a, b, c) = (pts[(i + m - 1) % m], pts[i], pts[(i + 1) % m]);
        match ((b - a).normalized(), (c - b).normalized()) {
            (Some(d1), Some(d2)) => d1.dot(d2) < crease,
            _ => false,
        }
    };
    let mut col_breaks: Vec<usize> = (0..n).filter(|j| sharp(&rows[0], *j, closed_u)).collect();
    let centres: Vec<Point3> = rows
        .iter()
        .map(|r| {
            Point3::ORIGIN
                + r.iter()
                    .fold(Vec3::new(0.0, 0.0, 0.0), |a, p| a + p.to_vec())
                    * (1.0 / n as f64)
        })
        .collect();
    let mut row_breaks: Vec<usize> = (1..rows.len() - 1)
        .filter(|i| sharp(&centres, *i, false))
        .collect();
    row_breaks.insert(0, 0);
    row_breaks.push(rows.len() - 1);
    // Column spans [a, b] (b may exceed n for the wrap-around span).
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let whole = closed_u && col_breaks.is_empty();
    if closed_u {
        if whole {
            spans.push((0, n)); // whole closed loop
        } else {
            for k in 0..col_breaks.len() {
                let a = col_breaks[k];
                let b = if k + 1 < col_breaks.len() {
                    col_breaks[k + 1]
                } else {
                    col_breaks[0] + n
                };
                spans.push((a, b));
            }
        }
    } else {
        col_breaks.insert(0, 0);
        col_breaks.push(n - 1);
        col_breaks.dedup();
        for w in col_breaks.windows(2) {
            spans.push((w[0], w[1]));
        }
    }
    for rw in row_breaks.windows(2) {
        for &(a, b) in &spans {
            let sections: Vec<Vec<Point3>> = rows[rw[0]..=rw[1]]
                .iter()
                .map(|r| {
                    if whole {
                        r.clone()
                    } else {
                        (a..=b).map(|j| r[j % n]).collect()
                    }
                })
                .collect();
            out.append(&loft_mesh(&sections, whole));
        }
    }
    out
}

/// Round pipe of radius `r` along a polyline rail, with mitred joints at the
/// rail's corners (open ends are left open; cap them with `cap_planar_holes`).
pub fn pipe_mesh(rail: &[Point3], r: f64, seg: usize) -> Mesh {
    let mut pts = rail.to_vec();
    pts.dedup_by(|a, b| a.distance_to(*b) < 1e-9);
    let n = pts.len();
    if n < 2 {
        return Mesh::default();
    }
    let dirs: Vec<Vec3> = pts
        .windows(2)
        .map(|w| (w[1] - w[0]).normalized().unwrap_or(Vec3::Z))
        .collect();
    let first = Plane::from_normal(pts[0], dirs[0]);
    let (mut u, mut v) = (first.x, first.y);
    let seg = seg.max(8);
    let circle = |c: Point3, u: Vec3, v: Vec3| -> Vec<Point3> {
        (0..seg)
            .map(|k| {
                let a = TAU * k as f64 / seg as f64;
                c + u * (r * a.cos()) + v * (r * a.sin())
            })
            .collect()
    };
    let mut rows = vec![circle(pts[0], u, v)];
    for i in 1..n {
        let d_in = dirs[i - 1];
        if i == n - 1 {
            rows.push(circle(pts[i], u, v));
            break;
        }
        let d_out = dirs[i];
        let m = (d_in + d_out).normalized().unwrap_or(d_in);
        let ring_in = circle(pts[i], u, v);
        let denom = d_in.dot(m);
        rows.push(
            ring_in
                .iter()
                .map(|q| {
                    if denom.abs() < 1e-6 {
                        *q
                    } else {
                        *q - d_in * ((*q - pts[i]).dot(m) / denom)
                    }
                })
                .collect(),
        );
        let rot = Xform::rotation_between(d_in, d_out);
        u = rot.vector(u);
        v = rot.vector(v);
    }
    let mut mesh = grid_mesh(&rows, true);
    if mesh.volume() < 0.0 {
        mesh = mesh.flipped();
    }
    mesh
}

/// Coons patch through four boundary polylines: `bottom` P00→P10, `right`
/// P10→P11, `top` P01→P11, `left` P00→P01. Each is resampled by arc length.
pub fn coons_mesh(
    bottom: &[Point3],
    right: &[Point3],
    top: &[Point3],
    left: &[Point3],
    nu: usize,
    nv: usize,
) -> Mesh {
    let (nu, nv) = (nu.max(2), nv.max(2));
    let side = |pts: &[Point3], n: usize| -> Vec<Point3> {
        let len: f64 = pts.windows(2).map(|w| w[0].distance_to(w[1])).sum();
        if len < 1e-12 {
            pts.first().map_or_else(Vec::new, |p| vec![*p; n])
        } else {
            resample(pts, n, false)
        }
    };
    let (b, t, l, r) = (
        side(bottom, nu),
        side(top, nu),
        side(left, nv),
        side(right, nv),
    );
    if b.len() != nu || t.len() != nu || l.len() != nv || r.len() != nv {
        return Mesh::default();
    }
    let (p00, p10, p01, p11) = (b[0], b[nu - 1], t[0], t[nu - 1]);
    let rows: Vec<Vec<Point3>> = (0..nv)
        .map(|j| {
            let v = j as f64 / (nv - 1) as f64;
            (0..nu)
                .map(|i| {
                    let u = i as f64 / (nu - 1) as f64;
                    let ruled_v = b[i].to_vec() * (1.0 - v) + t[i].to_vec() * v;
                    let ruled_u = l[j].to_vec() * (1.0 - u) + r[j].to_vec() * u;
                    let bilinear = p00.to_vec() * ((1.0 - u) * (1.0 - v))
                        + p10.to_vec() * (u * (1.0 - v))
                        + p01.to_vec() * ((1.0 - u) * v)
                        + p11.to_vec() * (u * v);
                    Point3::ORIGIN + (ruled_v + ruled_u - bilinear)
                })
                .collect()
        })
        .collect();
    loft_mesh(&rows, false)
}

/// Order 2–4 edge curves (polylines) into the four sides of a Coons patch.
/// Two curves give a ruled surface (left/right are straight lines), three curves
/// close with a degenerate side. `None` when the curves do not form a loop.
pub fn edge_curves_to_sides(curves: &[Vec<Point3>], tol: f64) -> Option<[Vec<Point3>; 4]> {
    let tol = tol.max(1e-9);
    match curves.len() {
        2 => {
            let a = curves[0].clone();
            let mut b = curves[1].clone();
            // Align directions: the closer pairing of end points.
            let same = a[0].distance_to(b[0]) + a[a.len() - 1].distance_to(b[b.len() - 1]);
            let opp = a[0].distance_to(b[b.len() - 1]) + a[a.len() - 1].distance_to(b[0]);
            if opp < same {
                b.reverse();
            }
            let left = vec![a[0], b[0]];
            let right = vec![a[a.len() - 1], b[b.len() - 1]];
            Some([a, right, b, left])
        }
        3 | 4 => {
            // Walk the loop end to end.
            let mut rest: Vec<Vec<Point3>> = curves[1..].to_vec();
            let mut chain = vec![curves[0].clone()];
            while !rest.is_empty() {
                let end = *chain.last()?.last()?;
                let k = rest.iter().position(|c| {
                    c[0].distance_to(end) <= tol || c[c.len() - 1].distance_to(end) <= tol
                })?;
                let mut c = rest.remove(k);
                if c[0].distance_to(end) > tol {
                    c.reverse();
                }
                chain.push(c);
            }
            let start = chain[0][0];
            let end = *chain.last()?.last()?;
            if start.distance_to(end) > tol {
                return None;
            }
            if chain.len() == 3 {
                // Degenerate fourth side at the start point.
                chain.push(vec![start, start]);
            }
            // Loop: c0 (bottom), c1 (right), c2 (top reversed), c3 (left reversed).
            let rev = |v: &Vec<Point3>| -> Vec<Point3> { v.iter().rev().copied().collect() };
            Some([
                chain[0].clone(),
                chain[1].clone(),
                rev(&chain[2]),
                rev(&chain[3]),
            ])
        }
        _ => None,
    }
}

/// Closed loop (no repeated point) offset inwards by `inset` at every vertex with
/// mitred corners, in the plane with normal `n` (the loop is made CCW around `n`).
pub fn miter_offset(points: &[Point3], n: Vec3, inset: f64) -> Vec<Point3> {
    let mut pts = points.to_vec();
    let m = pts.len();
    if m < 3 {
        return pts;
    }
    if crate::newell_area(&pts).dot(n) < 0.0 {
        pts.reverse();
    }
    let inward = |i: usize| {
        let d = pts[(i + 1) % m] - pts[i];
        n.cross(d).normalized().unwrap_or(Vec3::X)
    };
    (0..m)
        .map(|i| {
            let (n1, n2) = (inward((i + m - 1) % m), inward(i));
            let k = 1.0 + n1.dot(n2);
            if k < 1e-6 {
                pts[i] + n2 * inset
            } else {
                pts[i] + (n1 + n2) * (inset / k)
            }
        })
        .collect()
}

/// Extrude a closed planar loop (no repeated point) by `distance` along its normal
/// `n` while insetting it by `inset` at the top (a draft angle). Capped solid.
pub fn tapered_extrude_mesh(points: &[Point3], n: Vec3, distance: f64, inset: f64) -> Mesh {
    let mut base = points.to_vec();
    let nz = n.normalized().unwrap_or(Vec3::Z) * distance.signum();
    let distance = distance.abs();
    if crate::newell_area(&base).dot(nz) < 0.0 {
        base.reverse();
    }
    let top: Vec<Point3> = miter_offset(&base, nz, inset)
        .into_iter()
        .map(|p| p + nz * distance)
        .collect();
    let mut m = grid_mesh(&[base.clone(), top.clone()], true);
    let (bottom_cap, top_cap) = (cap(&base, -nz), cap(&top, nz));
    m.append(&bottom_cap);
    m.append(&top_cap);
    if m.volume() < 0.0 {
        m = m.flipped();
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const TOL: f64 = 1e-9;

    #[test]
    fn cone_and_truncated_cone_volumes() {
        let c = cone_mesh(&Plane::TOP, 10.0, 0.0, 30.0, 256);
        assert!(c.is_closed(1e-9));
        let exact = PI * 100.0 * 30.0 / 3.0;
        assert!((c.volume() - exact).abs() / exact < 2e-3, "{}", c.volume());
        let t = cone_mesh(&Plane::TOP, 10.0, 5.0, 30.0, 256);
        assert!(t.is_closed(1e-9));
        let exact = PI * 30.0 / 3.0 * (100.0 + 50.0 + 25.0);
        assert!((t.volume() - exact).abs() / exact < 2e-3, "{}", t.volume());
        // Rim edges are drawn (cap vertices are separate).
        assert!(!t.boundary_edges().is_empty());
        let down = cone_mesh(&Plane::TOP, 10.0, 0.0, -30.0, 64);
        assert!(down.volume() > 0.0);
        assert!(down.bounding_box().unwrap().min.z < -29.0);
    }

    #[test]
    fn torus_ellipsoid_pyramid_tube() {
        let t = torus_mesh(&Plane::TOP, 20.0, 5.0, 128);
        let exact = 2.0 * PI * PI * 20.0 * 25.0;
        assert!((t.volume() - exact).abs() / exact < 1e-2, "{}", t.volume());
        let e = ellipsoid_mesh(&Plane::TOP, 10.0, 5.0, 2.0, 96);
        let exact = 4.0 / 3.0 * PI * 100.0;
        assert!((e.volume() - exact).abs() / exact < 1e-2, "{}", e.volume());
        let b = e.bounding_box().unwrap();
        assert!((b.max.x - 10.0).abs() < 1e-6 && (b.max.z - 2.0).abs() < 1e-6);
        let base = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
        ];
        let p = pyramid_mesh(&base, Point3::new(5.0, 5.0, 6.0));
        assert!(p.is_closed(1e-9));
        assert!((p.volume() - 200.0).abs() < 1e-9, "{}", p.volume());
        let tb = tube_mesh(&Plane::TOP, 10.0, 8.0, 5.0, 256);
        assert!(tb.is_closed(1e-9));
        let exact = PI * (100.0 - 64.0) * 5.0;
        assert!(
            (tb.volume() - exact).abs() / exact < 2e-3,
            "{}",
            tb.volume()
        );
    }

    #[test]
    fn grid_with_creases_keeps_box_edges() {
        let sq = |z: f64| {
            vec![
                Point3::new(0.0, 0.0, z),
                Point3::new(1.0, 0.0, z),
                Point3::new(1.0, 1.0, z),
                Point3::new(0.0, 1.0, z),
            ]
        };
        let m = grid_mesh(&[sq(0.0), sq(1.0), sq(2.0)], true);
        assert_eq!(m.triangles.len(), 16);
        // 4 vertical creases (×2 sides × 2 rows) + top and bottom loops.
        assert!(m.boundary_edges().len() >= 16 + 8);
        assert!((m.area() - 8.0).abs() < TOL);
    }

    #[test]
    fn coons_patch_of_a_flat_square_is_flat() {
        let l = |a: Point3, b: Point3| vec![a, a.mid(b), b];
        let (p00, p10, p01, p11) = (
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
        );
        let m = coons_mesh(&l(p00, p10), &l(p10, p11), &l(p01, p11), &l(p00, p01), 5, 5);
        assert!((m.area() - 100.0).abs() < 1e-9);
        assert!(m.positions.iter().all(|p| p.z.abs() < TOL));
        // Sides in any order and direction.
        let curves = vec![l(p01, p11), l(p00, p10), l(p11, p10), l(p00, p01)];
        let s = edge_curves_to_sides(&curves, 1e-6).unwrap();
        let m = coons_mesh(&s[0], &s[1], &s[2], &s[3], 5, 5);
        assert!((m.area() - 100.0).abs() < 1e-9, "{}", m.area());
        // Three sides: a triangle.
        let tri = vec![l(p00, p10), l(p10, p11), l(p11, p00)];
        let s = edge_curves_to_sides(&tri, 1e-6).unwrap();
        let m = coons_mesh(&s[0], &s[1], &s[2], &s[3], 9, 9);
        assert!((m.area() - 50.0).abs() < 1e-6, "{}", m.area());
        assert!(edge_curves_to_sides(&[l(p00, p10), l(p01, p11), l(p10, p11)], 1e-6).is_none());
    }

    #[test]
    fn tapered_box() {
        let base = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
        ];
        let m = tapered_extrude_mesh(&base, Vec3::Z, 3.0, 1.0);
        assert!(m.is_closed(1e-9));
        // Frustum: h/3 (A1 + A2 + sqrt(A1 A2)) = 1 (100 + 64 + 80).
        assert!((m.volume() - 244.0).abs() < 1e-9, "{}", m.volume());
        let top = miter_offset(&base, Vec3::Z, 1.0);
        assert!(top[0].distance_to(Point3::new(1.0, 1.0, 0.0)) < TOL);
    }

    #[test]
    fn mitred_pipe_keeps_its_volume() {
        let rail = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(100.0, 0.0, 0.0),
            Point3::new(100.0, 100.0, 0.0),
        ];
        let m = pipe_mesh(&rail, 2.0, 64);
        let (capped, caps) = crate::cap_planar_holes(&m, 1e-9);
        assert_eq!(caps, 2);
        assert!(capped.is_closed(1e-9));
        let poly = 64.0 / 2.0 * (TAU / 64.0).sin() * 4.0; // area of the 64-gon
        assert!(
            (capped.volume().abs() - poly * 200.0).abs() < 1e-6,
            "{}",
            capped.volume()
        );
    }
}
