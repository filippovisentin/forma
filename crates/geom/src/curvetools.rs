//! More curve tools: circles and arcs through points, division by count or length,
//! polyline simplification, helices and spirals, chaining loose segments, and
//! boolean operations on planar regions bounded by closed polylines.

use crate::{point_in_polygon, Chain, CircleArc, Plane, Point3, Seg, Vec3};
use std::f64::consts::TAU;

/// Circle through three points (`None` when they are collinear).
pub fn circle_3pt(a: Point3, b: Point3, c: Point3) -> Option<CircleArc> {
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    let n2 = n.dot(n);
    if n2 < 1e-18 * (ab.dot(ab) * ac.dot(ac)).max(1e-300) || n2 < 1e-24 {
        return None;
    }
    // Circumcentre: a + ((|ac|² (n × ab)) + (|ab|² (ac × n))) / (2 |n|²)
    let center = a + (n.cross(ab) * ac.dot(ac) + ac.cross(n) * ab.dot(ab)) * (1.0 / (2.0 * n2));
    let plane = Plane::from_normal(center, n);
    let r = center.distance_to(a);
    let x = (a - center).normalized()?;
    Some(CircleArc::circle(
        Plane {
            origin: center,
            x,
            y: plane.z.cross(x),
            z: plane.z,
        },
        r,
    ))
}

/// Arc from `start` through `through` to `end` (`None` when collinear).
pub fn arc_3pt(start: Point3, end: Point3, through: Point3) -> Option<CircleArc> {
    let c = circle_3pt(start, through, end)?;
    // Orientation of start → through → end.
    let n = (through - start).cross(end - through);
    CircleArc::from_center_start_end(c.center(), start, end, n)
}

/// Lengths of the segments of a chain.
fn seg_lengths(chain: &Chain) -> Vec<f64> {
    chain.segs.iter().map(Seg::length).collect()
}

/// Points of a line/arc chain at the given distances from its start (exact).
pub fn chain_points_at_lengths(chain: &Chain, lengths: &[f64]) -> Vec<Point3> {
    let lens = seg_lengths(chain);
    let mut out = Vec::with_capacity(lengths.len());
    for &s in lengths {
        let mut acc = 0.0;
        let mut placed = false;
        for (i, l) in lens.iter().enumerate() {
            if s <= acc + l + 1e-12 || i + 1 == lens.len() {
                let t = if *l > 0.0 {
                    ((s - acc) / l).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                out.push(chain.segs[i].point_at(t));
                placed = true;
                break;
            }
            acc += l;
        }
        if !placed {
            if let Some(last) = chain.segs.last() {
                out.push(last.end());
            }
        }
    }
    out
}

/// Distances that split a curve of length `total` into `count` equal parts:
/// `count + 1` values for an open curve, `count` for a closed one.
pub fn division_lengths(total: f64, count: usize, closed: bool) -> Vec<f64> {
    let count = count.max(1);
    let n = if closed { count } else { count + 1 };
    (0..n).map(|i| total * i as f64 / count as f64).collect()
}

/// Distances every `step` along a curve of length `total`, starting at 0.
pub fn step_lengths(total: f64, step: f64) -> Vec<f64> {
    if step <= 0.0 {
        return vec![0.0];
    }
    let n = (total / step + 1e-9).floor() as usize;
    (0..=n).map(|i| step * i as f64).collect()
}

/// Douglas–Peucker simplification: the fewest points within `tol` of the original
/// polyline (end points kept; a closed polyline keeps its closing point).
pub fn simplify_polyline(points: &[Point3], tol: f64) -> Vec<Point3> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    *keep.last_mut().expect("len") = true;
    let closed = points[0].distance_to(*points.last().expect("len")) <= tol;
    let mut stack = vec![(0usize, points.len() - 1)];
    if closed {
        // Split a closed loop at its farthest point from the start.
        let far = (1..points.len() - 1)
            .max_by(|a, b| {
                points[*a]
                    .distance_to(points[0])
                    .total_cmp(&points[*b].distance_to(points[0]))
            })
            .unwrap_or(1);
        keep[far] = true;
        stack = vec![(0, far), (far, points.len() - 1)];
    }
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (a, b) = (points[i], points[j]);
        let (mut best, mut at) = (0.0, i);
        for (k, p) in points.iter().enumerate().take(j).skip(i + 1) {
            let d = dist_to_segment(*p, a, b);
            if d > best {
                best = d;
                at = k;
            }
        }
        if best > tol {
            keep[at] = true;
            stack.push((i, at));
            stack.push((at, j));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

/// Distance from `p` to the segment `a b`.
pub fn dist_to_segment(p: Point3, a: Point3, b: Point3) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    if l2 < 1e-24 {
        return p.distance_to(a);
    }
    let t = ((p - a).dot(d) / l2).clamp(0.0, 1.0);
    p.distance_to(a + d * t)
}

/// Points of a spiral in `plane` around its origin: the radius goes from `r0` to
/// `r1` and the height (along the normal) from 0 to `height` over `turns` turns,
/// starting on the plane's x axis. With `r0 == r1` it is a helix.
pub fn spiral_points(
    plane: &Plane,
    r0: f64,
    r1: f64,
    turns: f64,
    height: f64,
    per_turn: usize,
) -> Vec<Point3> {
    let n = ((turns.abs() * per_turn as f64).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let f = i as f64 / n as f64;
            let a = TAU * turns * f;
            let r = r0 + (r1 - r0) * f;
            plane.point_at(r * a.cos(), r * a.sin(), height * f)
        })
        .collect()
}

/// Join loose segments that share end points (within `tol`) into polylines.
/// Closed loops repeat their first point at the end.
pub fn chain_segments(segs: &[[Point3; 2]], tol: f64) -> Vec<Vec<Point3>> {
    let tol = tol.max(1e-12);
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    // Spatial hash of end points for speed on big meshes.
    let key = |p: Point3| {
        (
            (p.x / tol).round() as i64,
            (p.y / tol).round() as i64,
            (p.z / tol).round() as i64,
        )
    };
    let mut ends: std::collections::HashMap<(i64, i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        for p in s {
            ends.entry(key(*p)).or_default().push(i);
        }
    }
    let find = |p: Point3, used: &[bool]| -> Option<(usize, bool)> {
        let (kx, ky, kz) = key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = ends.get(&(kx + dx, ky + dy, kz + dz)) {
                        for &i in v {
                            if used[i] {
                                continue;
                            }
                            if segs[i][0].distance_to(p) <= tol {
                                return Some((i, false));
                            }
                            if segs[i][1].distance_to(p) <= tol {
                                return Some((i, true));
                            }
                        }
                    }
                }
            }
        }
        None
    };
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut line = vec![segs[start][0], segs[start][1]];
        // Grow forwards, then backwards.
        for forward in [true, false] {
            loop {
                let end = if forward {
                    *line.last().expect("len")
                } else {
                    line[0]
                };
                if line.len() > 2 && line[0].distance_to(*line.last().expect("len")) <= tol {
                    break;
                }
                let Some((i, flipped)) = find(end, &used) else {
                    break;
                };
                used[i] = true;
                let next = if flipped { segs[i][0] } else { segs[i][1] };
                if forward {
                    line.push(next);
                } else {
                    line.insert(0, next);
                }
            }
        }
        line.dedup_by(|a, b| a.distance_to(*b) <= tol * 0.5);
        if line.len() >= 2 {
            out.push(line);
        }
    }
    out
}

/// Boolean operation on planar regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionOp {
    Union,
    Intersection,
    /// The first region minus all the others.
    Difference,
}

/// Boolean operation on the regions bounded by closed polylines lying in `plane`
/// (each loop with or without a repeated closing point). Edges are split at every
/// crossing and kept when the result region lies on exactly one side, so shared
/// and overlapping edges are handled. Returns closed loops (first point repeated),
/// outer boundaries counter-clockwise around the plane normal, holes clockwise.
pub fn region_boolean(
    loops: &[Vec<Point3>],
    op: RegionOp,
    plane: &Plane,
    tol: f64,
) -> Vec<Vec<Point3>> {
    let tol = tol.max(1e-9);
    // 2D loops without closing point.
    let polys: Vec<Vec<(f64, f64)>> = loops
        .iter()
        .map(|l| {
            let mut v: Vec<(f64, f64)> = l
                .iter()
                .map(|p| {
                    let (u, w, _) = plane.coords(*p);
                    (u, w)
                })
                .collect();
            if v.len() > 1 && d2(v[0], *v.last().expect("len")) <= tol {
                v.pop();
            }
            v.dedup_by(|a, b| d2(*a, *b) <= tol);
            v
        })
        .filter(|v| v.len() >= 3)
        .collect();
    if polys.is_empty() {
        return Vec::new();
    }
    // All edges.
    let mut edges: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for p in &polys {
        for i in 0..p.len() {
            edges.push((p[i], p[(i + 1) % p.len()]));
        }
    }
    // Split every edge at its crossings with all other edges (and at the other
    // edges' end points that lie on it).
    let mut pieces: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        let len = d2(a, b);
        if len <= tol {
            continue;
        }
        for (j, &(c, d)) in edges.iter().enumerate() {
            if i == j {
                continue;
            }
            if let Some(t) = seg_seg(a, b, c, d) {
                ts.push(t);
            }
            for q in [c, d] {
                let t = project_t(a, b, q);
                if t > 0.0 && t < 1.0 && d2(lerp(a, b, t), q) <= tol {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|x, y| (*x - *y).abs() * len <= tol);
        for w in ts.windows(2) {
            let (p, q) = (lerp(a, b, w[0]), lerp(a, b, w[1]));
            if d2(p, q) > tol {
                pieces.push((p, q));
            }
        }
    }
    // Remove duplicated pieces (shared or overlapping edges, in either direction).
    let mut unique: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for e in pieces {
        let dup = unique.iter().any(|u| {
            (d2(u.0, e.0) <= tol && d2(u.1, e.1) <= tol)
                || (d2(u.0, e.1) <= tol && d2(u.1, e.0) <= tol)
        });
        if !dup {
            unique.push(e);
        }
    }
    let inside = |pt: (f64, f64)| -> bool {
        let flags: Vec<bool> = polys.iter().map(|p| in_poly(pt, p)).collect();
        match op {
            RegionOp::Union => flags.iter().any(|f| *f),
            RegionOp::Intersection => flags.iter().all(|f| *f),
            RegionOp::Difference => flags[0] && !flags[1..].iter().any(|f| *f),
        }
    };
    // Keep boundary pieces, oriented with the region on the left.
    let mut kept: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for (a, b) in unique {
        let len = d2(a, b);
        let m = lerp(a, b, 0.5);
        let (nx, ny) = (-(b.1 - a.1) / len, (b.0 - a.0) / len);
        let eps = (tol * 4.0).min(len * 0.25);
        let left = inside((m.0 + nx * eps, m.1 + ny * eps));
        let right = inside((m.0 - nx * eps, m.1 - ny * eps));
        if left && !right {
            kept.push((a, b));
        } else if right && !left {
            kept.push((b, a));
        }
    }
    // Walk the loops, taking the leftmost turn at shared vertices.
    let mut used = vec![false; kept.len()];
    let mut out = Vec::new();
    for s in 0..kept.len() {
        if used[s] {
            continue;
        }
        used[s] = true;
        let first = kept[s].0;
        let mut lp = vec![kept[s].0, kept[s].1];
        let mut dir = sub(kept[s].1, kept[s].0);
        let mut guard = 0;
        while d2(*lp.last().expect("len"), first) > tol && guard < kept.len() {
            guard += 1;
            let cur = *lp.last().expect("len");
            let cand = (0..kept.len())
                .filter(|k| !used[*k] && d2(kept[*k].0, cur) <= tol)
                .max_by(|x, y| {
                    let tx = turn(dir, sub(kept[*x].1, kept[*x].0));
                    let ty = turn(dir, sub(kept[*y].1, kept[*y].0));
                    tx.total_cmp(&ty)
                });
            let Some(k) = cand else { break };
            used[k] = true;
            dir = sub(kept[k].1, kept[k].0);
            lp.push(kept[k].1);
        }
        if lp.len() >= 4 && d2(*lp.last().expect("len"), first) <= tol {
            *lp.last_mut().expect("len") = first;
            // Merge collinear points.
            let pts: Vec<Point3> = lp.iter().map(|q| plane.point_at(q.0, q.1, 0.0)).collect();
            let simple = remove_collinear(&pts, tol);
            if simple.len() >= 4 {
                out.push(simple);
            }
        }
    }
    out
}

/// Remove points of a closed polyline (closing point repeated) that lie on the
/// segment between their neighbours.
fn remove_collinear(pts: &[Point3], tol: f64) -> Vec<Point3> {
    let mut v: Vec<Point3> = pts[..pts.len() - 1].to_vec();
    let mut changed = true;
    while changed && v.len() > 3 {
        changed = false;
        for i in 0..v.len() {
            let n = v.len();
            let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            if dist_to_segment(b, a, c) <= tol * 0.5 {
                v.remove(i);
                changed = true;
                break;
            }
        }
    }
    v.push(v[0]);
    v
}

/// Signed turning angle from direction `a` to `b` (left positive).
fn turn(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 * b.1 - a.1 * b.0).atan2(a.0 * b.0 + a.1 * b.1)
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn d2(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn lerp(a: (f64, f64), b: (f64, f64), t: f64) -> (f64, f64) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn project_t(a: (f64, f64), b: (f64, f64), q: (f64, f64)) -> f64 {
    let d = sub(b, a);
    let l2 = d.0 * d.0 + d.1 * d.1;
    if l2 < 1e-300 {
        return 0.0;
    }
    ((q.0 - a.0) * d.0 + (q.1 - a.1) * d.1) / l2
}

/// Parameter on `a b` of a proper crossing with `c d` (strictly inside both).
fn seg_seg(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> Option<f64> {
    let r = sub(b, a);
    let s = sub(d, c);
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-300 {
        return None;
    }
    let ac = sub(c, a);
    let t = (ac.0 * s.1 - ac.1 * s.0) / den;
    let u = (ac.0 * r.1 - ac.1 * r.0) / den;
    (t > 0.0 && t < 1.0 && (0.0..=1.0).contains(&u)).then_some(t)
}

/// Even-odd point in polygon test in 2D.
fn in_poly(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > p.1) != (b.1 > p.1) {
            let x = a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if p.0 < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// True when every loop point lies inside `outer` (used to classify holes).
pub fn loop_inside(inner: &[Point3], outer: &[Point3], plane: &Plane) -> bool {
    inner.iter().all(|p| point_in_polygon(*p, outer, plane))
}

/// Unit vector of a direction projected into a plane with normal `n`.
pub fn in_plane(v: Vec3, n: Vec3) -> Option<Vec3> {
    (v - n * v.dot(n)).normalized()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::newell_area;

    const TOL: f64 = 1e-9;

    fn p(x: f64, y: f64) -> Point3 {
        Point3::new(x, y, 0.0)
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point3> {
        vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)]
    }

    fn area(l: &[Point3]) -> f64 {
        newell_area(&l[..l.len() - 1]).z
    }

    #[test]
    fn circle_and_arc_through_points() {
        let c = circle_3pt(p(10.0, 0.0), p(0.0, 10.0), p(-10.0, 0.0)).unwrap();
        assert!(c.center().distance_to(Point3::ORIGIN) < TOL);
        assert!((c.radius - 10.0).abs() < TOL);
        assert!(circle_3pt(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)).is_none());
        // Clockwise arc over the top: start (10,0), through (0,10), end (-10,0).
        let a = arc_3pt(p(10.0, 0.0), p(-10.0, 0.0), p(0.0, 10.0)).unwrap();
        assert!(a.start().distance_to(p(10.0, 0.0)) < TOL);
        assert!(a.end().distance_to(p(-10.0, 0.0)) < TOL);
        assert!(a.mid().distance_to(p(0.0, 10.0)) < 1e-6);
        // Through the bottom instead.
        let b = arc_3pt(p(10.0, 0.0), p(-10.0, 0.0), p(0.0, -10.0)).unwrap();
        assert!(b.mid().distance_to(p(0.0, -10.0)) < 1e-6);
        assert!((b.sweep - std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn divide_a_chain_exactly() {
        let ch = Chain::from_points(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        let pts = chain_points_at_lengths(&ch, &division_lengths(20.0, 4, false));
        assert_eq!(pts.len(), 5);
        assert!(pts[2].distance_to(p(10.0, 0.0)) < TOL);
        assert!(pts[3].distance_to(p(10.0, 5.0)) < TOL);
        assert_eq!(division_lengths(20.0, 4, true).len(), 4);
        assert_eq!(step_lengths(20.0, 6.0), vec![0.0, 6.0, 12.0, 18.0]);
        // Arcs are exact too.
        let arc = CircleArc::circle(Plane::TOP, 5.0);
        let q = chain_points_at_lengths(
            &Chain::new(vec![Seg::Arc(arc)]),
            &division_lengths(TAU * 5.0, 4, true),
        );
        assert!(q[1].distance_to(p(0.0, 5.0)) < TOL);
    }

    #[test]
    fn simplify_removes_flat_points() {
        let pts = vec![p(0.0, 0.0), p(1.0, 0.001), p(2.0, 0.0), p(2.0, 5.0)];
        let s = simplify_polyline(&pts, 0.01);
        assert_eq!(s.len(), 3);
        let s = simplify_polyline(&pts, 0.0001);
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn helix_points() {
        let h = spiral_points(&Plane::TOP, 5.0, 5.0, 2.0, 100.0, 32);
        assert_eq!(h.len(), 65);
        assert!(h[64].distance_to(Point3::new(5.0, 0.0, 100.0)) < 1e-9);
        assert!(h
            .iter()
            .all(|q| ((q.x * q.x + q.y * q.y).sqrt() - 5.0).abs() < 1e-9));
    }

    #[test]
    fn segments_chain_into_loops() {
        let segs = vec![
            [p(0.0, 0.0), p(1.0, 0.0)],
            [p(1.0, 1.0), p(0.0, 1.0)],
            [p(1.0, 0.0), p(1.0, 1.0)],
            [p(0.0, 0.0), p(0.0, 1.0)],
            [p(5.0, 5.0), p(6.0, 5.0)],
        ];
        let c = chain_segments(&segs, 1e-9);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].len(), 5);
        assert!(c[0][0].distance_to(c[0][4]) < TOL);
    }

    #[test]
    fn union_of_overlapping_squares() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(5.0, 5.0, 15.0, 15.0);
        let u = region_boolean(&[a.clone(), b.clone()], RegionOp::Union, &Plane::TOP, 1e-6);
        assert_eq!(u.len(), 1);
        assert!((area(&u[0]) - 175.0).abs() < 1e-6, "{}", area(&u[0]));
        assert_eq!(u[0].len(), 9); // 8 corners + closing point
        let i = region_boolean(
            &[a.clone(), b.clone()],
            RegionOp::Intersection,
            &Plane::TOP,
            1e-6,
        );
        assert_eq!(i.len(), 1);
        assert!((area(&i[0]) - 25.0).abs() < 1e-6);
        let d = region_boolean(&[a, b], RegionOp::Difference, &Plane::TOP, 1e-6);
        assert_eq!(d.len(), 1);
        assert!((area(&d[0]) - 75.0).abs() < 1e-6);
    }

    #[test]
    fn union_with_shared_edge_and_hole() {
        // Two rooms sharing a wall merge into one rectangle.
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(10.0, 0.0, 20.0, 10.0);
        let u = region_boolean(&[a, b], RegionOp::Union, &Plane::TOP, 1e-6);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].len(), 5, "{:?}", u[0]);
        assert!((area(&u[0]) - 200.0).abs() < 1e-6);
        // A column inside a room: the difference has an outer loop and a hole.
        let room = rect(0.0, 0.0, 10.0, 10.0);
        let col = rect(4.0, 4.0, 6.0, 6.0);
        let d = region_boolean(&[room, col], RegionOp::Difference, &Plane::TOP, 1e-6);
        assert_eq!(d.len(), 2);
        let total: f64 = d.iter().map(|l| area(l)).sum();
        assert!((total - 96.0).abs() < 1e-6, "{total}");
        assert!(d.iter().any(|l| area(l) < 0.0)); // the hole runs clockwise
    }
}
