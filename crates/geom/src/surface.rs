//! Surface-like operations producing meshes: planar regions with holes, extruded
//! regions, caps, lofts, revolves and sweeps.
//!
//! Forma has no NURBS surfaces yet (ADR 0001): every "surface" command builds a
//! triangle mesh. The results are good for display, export and area/volume
//! estimates; they are not exact.

use crate::solids::{extrude_walls, polygon_normal};
use crate::{triangulate_polygon, Mesh, Plane, Point3, Vec3, Xform};
use std::collections::HashMap;
use std::f64::consts::TAU;

/// Area vector of a polygon (no repeated closing point needed): its length is the
/// area of a planar polygon, its direction the normal for counter-clockwise order.
pub fn newell_area(points: &[Point3]) -> Vec3 {
    let mut pts = points.to_vec();
    if pts.len() > 1 && pts[0].distance_to(*pts.last().expect("len")) < 1e-12 {
        pts.pop();
    }
    polygon_normal(&pts) * 0.5
}

/// Even-odd test of `p` against a polygon, both projected on `plane`.
pub fn point_in_polygon(p: Point3, poly: &[Point3], plane: &Plane) -> bool {
    let q = to2(plane, p);
    let pts: Vec<[f64; 2]> = poly.iter().map(|x| to2(plane, *x)).collect();
    inside2(q, &pts)
}

fn to2(plane: &Plane, p: Point3) -> [f64; 2] {
    let (u, v, _) = plane.coords(p);
    [u, v]
}

fn inside2(p: [f64; 2], l: &[[f64; 2]]) -> bool {
    let n = l.len();
    let mut inside = false;
    for i in 0..n {
        let [x0, y0] = l[i];
        let [x1, y1] = l[(i + 1) % n];
        if (y0 > p[1]) != (y1 > p[1]) {
            let x = x0 + (p[1] - y0) / (y1 - y0) * (x1 - x0);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn area2(l: &[[f64; 2]]) -> f64 {
    let n = l.len();
    (0..n)
        .map(|i| {
            let (a, b) = (l[i], l[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

fn cross2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Proper crossing of segments `ab` and `cd` (touching at end points does not count).
fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let eps = 1e-12;
    let d1 = cross2(c, d, a);
    let d2 = cross2(c, d, b);
    let d3 = cross2(a, b, c);
    let d4 = cross2(a, b, d);
    ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
        && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
}

/// Triangulate a planar region: an outer loop and holes (no repeated closing
/// points). Triangle indices refer to the concatenation `outer ++ holes[0] ++ …` and
/// run counter-clockwise around `normal`. Holes are bridged into the outer loop and
/// the resulting polygon is ear-clipped.
pub fn triangulate_with_holes(
    outer: &[Point3],
    holes: &[Vec<Point3>],
    normal: Vec3,
) -> Vec<[u32; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let plane = Plane::from_normal(outer[0], normal);
    let mut all: Vec<[f64; 2]> = outer.iter().map(|p| to2(&plane, *p)).collect();
    let mut rings: Vec<Vec<usize>> = Vec::new();
    let mut outer_idx: Vec<usize> = (0..outer.len()).collect();
    if area2(&all) < 0.0 {
        outer_idx.reverse();
    }
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        let base = all.len();
        all.extend(h.iter().map(|p| to2(&plane, *p)));
        let mut idx: Vec<usize> = (base..base + h.len()).collect();
        let ring: Vec<[f64; 2]> = idx.iter().map(|i| all[*i]).collect();
        if area2(&ring) > 0.0 {
            idx.reverse(); // holes clockwise
        }
        rings.push(idx);
    }
    let outer_ring: Vec<[f64; 2]> = outer_idx.iter().map(|i| all[*i]).collect();
    let mut poly = outer_idx;
    // Bridge holes, rightmost first.
    let max_x = |r: &Vec<usize>| r.iter().map(|i| all[*i][0]).fold(f64::MIN, f64::max);
    rings.sort_by(|a, b| max_x(b).total_cmp(&max_x(a)));
    for (h, ring) in rings.iter().enumerate() {
        let (mpos, _) = ring
            .iter()
            .enumerate()
            .max_by(|a, b| all[*a.1][0].total_cmp(&all[*b.1][0]))
            .expect("ring");
        let mut hole: Vec<usize> = ring[mpos..].to_vec();
        hole.extend_from_slice(&ring[..mpos]);
        let m = all[hole[0]];
        let mut cands: Vec<usize> = (0..poly.len()).collect();
        let d2 = |i: usize| {
            let q = all[poly[i]];
            (q[0] - m[0]).powi(2) + (q[1] - m[1]).powi(2)
        };
        cands.sort_by(|a, b| d2(*a).total_cmp(&d2(*b)));
        let visible = |k: usize| {
            let p = all[poly[k]];
            let n = poly.len();
            for i in 0..n {
                if segments_cross(m, p, all[poly[i]], all[poly[(i + 1) % n]]) {
                    return false;
                }
            }
            for r in &rings[h..] {
                for i in 0..r.len() {
                    if segments_cross(m, p, all[r[i]], all[r[(i + 1) % r.len()]]) {
                        return false;
                    }
                }
            }
            let mid = [(m[0] + p[0]) / 2.0, (m[1] + p[1]) / 2.0];
            if !inside2(mid, &outer_ring) {
                return false;
            }
            rings[h..].iter().all(|r| {
                let ring: Vec<[f64; 2]> = r.iter().map(|i| all[*i]).collect();
                !inside2(mid, &ring)
            })
        };
        let Some(k) = cands.into_iter().find(|k| visible(*k)) else {
            continue; // cannot bridge: ignore the hole
        };
        let p = poly[k];
        let mut merged = poly[..=k].to_vec();
        merged.extend_from_slice(&hole);
        merged.push(hole[0]);
        merged.push(p);
        merged.extend_from_slice(&poly[k + 1..]);
        poly = merged;
    }
    ear_clip(&all, poly)
        .into_iter()
        .map(|t| t.map(|i| i as u32))
        .collect()
}

/// Ear clipping of a counter-clockwise polygon given by indices into `pts`; repeated
/// positions (bridges) are allowed.
fn ear_clip(pts: &[[f64; 2]], mut idx: Vec<usize>) -> Vec<[usize; 3]> {
    let same =
        |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12;
    let mut out = Vec::new();
    let mut guard = 0;
    let n0 = idx.len();
    while idx.len() > 3 && guard < n0 * n0 + 10 {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross2(a, b, c) <= 1e-12 {
                continue;
            }
            let blocked = idx.iter().any(|&j| {
                let p = pts[j];
                if same(p, a) || same(p, b) || same(p, c) {
                    return false;
                }
                cross2(a, b, p) >= -1e-12 && cross2(b, c, p) >= -1e-12 && cross2(c, a, p) >= -1e-12
            });
            if !blocked {
                out.push([ia, ib, ic]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // Drop a degenerate (collinear) vertex if there is one, else give up.
            let m = idx.len();
            let flat = (0..m).find(|&i| {
                let (a, b, c) = (
                    pts[idx[(i + m - 1) % m]],
                    pts[idx[i]],
                    pts[idx[(i + 1) % m]],
                );
                cross2(a, b, c).abs() <= 1e-12
            });
            match flat {
                Some(i) => {
                    idx.remove(i);
                }
                None => break,
            }
        }
    }
    if idx.len() == 3 {
        let (a, b, c) = (pts[idx[0]], pts[idx[1]], pts[idx[2]]);
        if cross2(a, b, c).abs() > 1e-12 {
            out.push([idx[0], idx[1], idx[2]]);
        }
    } else if idx.len() > 3 {
        for k in 1..idx.len() - 1 {
            out.push([idx[0], idx[k], idx[k + 1]]);
        }
    }
    out
}

/// Planar mesh of a region bounded by `outer` with `holes` (no repeated closing
/// points), facing `normal`.
pub fn planar_mesh(outer: &[Point3], holes: &[Vec<Point3>], normal: Vec3) -> Mesh {
    let n = normal.normalized().unwrap_or(Vec3::Z);
    let triangles = triangulate_with_holes(outer, holes, n);
    let mut positions = outer.to_vec();
    for h in holes.iter().filter(|h| h.len() >= 3) {
        positions.extend_from_slice(h);
    }
    Mesh {
        normals: vec![n; positions.len()],
        positions,
        triangles,
    }
}

impl Mesh {
    /// Total triangle area.
    pub fn area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize]);
                (b - a).cross(c - a).length() / 2.0
            })
            .sum()
    }

    /// Signed volume (divergence theorem); positive for a closed mesh whose
    /// triangles face outwards. Meaningless for open meshes.
    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize].to_vec());
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }

    /// Sum of the triangle area vectors (zero for a closed mesh).
    pub fn area_vector(&self) -> Vec3 {
        self.triangles
            .iter()
            .fold(Vec3::new(0.0, 0.0, 0.0), |acc, t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize]);
                acc + (b - a).cross(c - a) * 0.5
            })
    }

    /// Opposite orientation: reversed winding and normals.
    pub fn flipped(&self) -> Mesh {
        Mesh {
            positions: self.positions.clone(),
            normals: self.normals.iter().map(|n| -*n).collect(),
            triangles: self.triangles.iter().map(|t| [t[0], t[2], t[1]]).collect(),
        }
    }

    /// Copy with vertices closer than `tol` merged and degenerate triangles removed.
    /// Normals are dropped (the display recomputes smooth ones).
    pub fn welded(&self, tol: f64) -> Mesh {
        let tol = tol.max(1e-12);
        let key = |p: Point3| {
            (
                (p.x / tol).floor() as i64,
                (p.y / tol).floor() as i64,
                (p.z / tol).floor() as i64,
            )
        };
        let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
        let mut out = Mesh::default();
        let mut map = Vec::with_capacity(self.positions.len());
        for p in &self.positions {
            let (kx, ky, kz) = key(*p);
            let mut found = None;
            'search: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(v) = grid.get(&(kx + dx, ky + dy, kz + dz)) {
                            for &i in v {
                                if out.positions[i as usize].distance_to(*p) <= tol {
                                    found = Some(i);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
            }
            let i = found.unwrap_or_else(|| {
                out.positions.push(*p);
                let i = (out.positions.len() - 1) as u32;
                grid.entry((kx, ky, kz)).or_default().push(i);
                i
            });
            map.push(i);
        }
        out.triangles = self
            .triangles
            .iter()
            .map(|t| t.map(|i| map[i as usize]))
            .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
            .collect();
        out
    }

    /// True when the welded mesh has no boundary edges.
    pub fn is_closed(&self, tol: f64) -> bool {
        let w = self.welded(tol);
        !w.triangles.is_empty() && w.boundary_edges().is_empty()
    }

    /// Boundary loops as vertex index lists, each in the direction of its edges
    /// (the mesh should be welded first).
    pub fn boundary_loops(&self) -> Vec<Vec<u32>> {
        let edges = self.boundary_edges();
        let mut next: HashMap<u32, Vec<usize>> = HashMap::new();
        for (k, e) in edges.iter().enumerate() {
            next.entry(e[0]).or_default().push(k);
        }
        let mut used = vec![false; edges.len()];
        let mut loops = Vec::new();
        for start in 0..edges.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let first = edges[start][0];
            let mut lp = vec![first];
            let mut cur = edges[start][1];
            let mut guard = 0;
            while cur != first && guard <= edges.len() {
                guard += 1;
                lp.push(cur);
                let Some(k) = next
                    .get(&cur)
                    .and_then(|ks| ks.iter().copied().find(|k| !used[*k]))
                else {
                    break;
                };
                used[k] = true;
                cur = edges[k][1];
            }
            if cur == first && lp.len() >= 3 {
                loops.push(lp);
            }
        }
        loops
    }

    /// Replace the normals by smooth, area-weighted vertex normals.
    pub fn compute_smooth_normals(&mut self) {
        let mut acc = vec![Vec3::new(0.0, 0.0, 0.0); self.positions.len()];
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            let f = (b - a).cross(c - a);
            for &i in t {
                acc[i as usize] = acc[i as usize] + f;
            }
        }
        self.normals = acc
            .into_iter()
            .map(|v| v.normalized().unwrap_or(Vec3::Z))
            .collect();
    }
}

/// Extrude an open mesh (typically a planar region) along `dir` into a closed
/// solid: the original faces, a translated copy and walls along every boundary
/// loop, oriented outwards. `None` for a closed mesh or a zero direction.
pub fn extrude_open_mesh(mesh: &Mesh, dir: Vec3, tol: f64) -> Option<Mesh> {
    if dir.length() < 1e-12 {
        return None;
    }
    let w = mesh.welded(tol);
    let loops = w.boundary_loops();
    if loops.is_empty() {
        return None;
    }
    let along = mesh.area_vector().dot(dir) >= 0.0;
    let moved = Mesh {
        positions: mesh.positions.iter().map(|p| *p + dir).collect(),
        ..mesh.clone()
    };
    let mut out = if along { mesh.flipped() } else { mesh.clone() };
    out.append(&if along { moved } else { moved.flipped() });
    for lp in loops {
        let pts: Vec<Point3> = lp.iter().map(|i| w.positions[*i as usize]).collect();
        let walls = extrude_walls(&pts, dir, true);
        out.append(&if along { walls } else { walls.flipped() });
    }
    if out.volume() < 0.0 {
        out = out.flipped();
    }
    Some(out)
}

/// Close the planar holes of a mesh: every boundary loop that is planar within
/// `tol` gets a triangulated cap. When the result is closed it is oriented
/// outwards. Returns the capped mesh and the
/// number of caps added.
pub fn cap_planar_holes(mesh: &Mesh, tol: f64) -> (Mesh, usize) {
    let w = mesh.welded(tol);
    let mut out = mesh.clone();
    let mut caps = 0;
    for lp in w.boundary_loops() {
        // The cap runs against the boundary edges.
        let pts: Vec<Point3> = lp.iter().rev().map(|i| w.positions[*i as usize]).collect();
        let Some(n) = polygon_normal(&pts).normalized() else {
            continue;
        };
        let planar = pts
            .iter()
            .all(|p| (*p - pts[0]).dot(n).abs() <= tol.max(1e-9));
        if !planar {
            continue;
        }
        let triangles = triangulate_polygon(&pts, n);
        let cap = Mesh {
            normals: vec![n; pts.len()],
            positions: pts,
            triangles,
        };
        out.append(&cap);
        caps += 1;
    }
    if caps > 0 && out.is_closed(tol) && out.volume() < 0.0 {
        out = out.flipped();
    }
    (out, caps)
}

/// `n` points spaced evenly by arc length along a polyline. For a closed polyline
/// (`closed`; a repeated closing point is optional) the points go once around
/// without repeating the first.
pub fn resample(points: &[Point3], n: usize, closed: bool) -> Vec<Point3> {
    let mut pts = points.to_vec();
    pts.dedup_by(|a, b| a.distance_to(*b) < 1e-12);
    if closed && pts.len() > 1 && pts[0].distance_to(*pts.last().expect("len")) > 1e-12 {
        pts.push(pts[0]);
    }
    if pts.len() < 2 || n < 2 {
        return pts;
    }
    let mut cum = vec![0.0];
    for w in pts.windows(2) {
        cum.push(cum.last().expect("len") + w[0].distance_to(w[1]));
    }
    let total = *cum.last().expect("len");
    let count = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(n);
    let mut seg = 0;
    for k in 0..n {
        let s = total * k as f64 / count as f64;
        while seg + 2 < cum.len() && cum[seg + 1] < s {
            seg += 1;
        }
        let len = cum[seg + 1] - cum[seg];
        let t = if len > 0.0 {
            ((s - cum[seg]) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(pts[seg] + (pts[seg + 1] - pts[seg]) * t);
    }
    out
}

/// Quad-strip mesh through sections that all have the same number of points
/// (`closed`: sections are closed loops without repeated points). Smooth normals.
pub fn loft_mesh(sections: &[Vec<Point3>], closed: bool) -> Mesh {
    let mut m = Mesh::default();
    let Some(n) = sections.first().map(Vec::len) else {
        return m;
    };
    if sections.len() < 2 || n < 2 || sections.iter().any(|s| s.len() != n) {
        return m;
    }
    for s in sections {
        m.positions.extend_from_slice(s);
    }
    let id = |i: usize, j: usize| (i * n + j % n) as u32;
    let cols = if closed { n } else { n - 1 };
    for i in 0..sections.len() - 1 {
        for j in 0..cols {
            let (a, b, c, d) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
            m.triangles.push([a, b, c]);
            m.triangles.push([a, c, d]);
        }
    }
    m.triangles.retain(|t| {
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        (b - a).cross(c - a).length() > 1e-18
    });
    m.compute_smooth_normals();
    m
}

/// Revolve a profile polyline around the axis through `axis_point` along
/// `axis_dir` by `angle` radians (64 segments per turn). A full turn welds the
/// seam; points on the axis are merged. Closed results face outwards.
pub fn revolve_mesh(
    profile: &[Point3],
    axis_point: Point3,
    axis_dir: Vec3,
    angle: f64,
    tol: f64,
) -> Mesh {
    let mut m = Mesh::default();
    let np = profile.len();
    if np < 2 || axis_dir.length() < 1e-12 || angle.abs() < 1e-12 {
        return m;
    }
    let full = angle.abs() >= TAU - 1e-9;
    let angle = if full { TAU } else { angle };
    let seg = ((64.0 * angle.abs() / TAU).ceil() as usize).max(2);
    let cols = if full { seg } else { seg + 1 };
    for k in 0..cols {
        let x = Xform::rotation(axis_point, axis_dir, angle * k as f64 / seg as f64);
        m.positions.extend(profile.iter().map(|p| x.point(*p)));
    }
    let id = |k: usize, j: usize| ((k % cols) * np + j) as u32;
    for k in 0..seg {
        for j in 0..np - 1 {
            let (a, b, c, d) = (id(k, j), id(k + 1, j), id(k + 1, j + 1), id(k, j + 1));
            m.triangles.push([a, b, c]);
            m.triangles.push([a, c, d]);
        }
    }
    let mut w = m.welded(tol);
    if w.boundary_edges().is_empty() && w.volume() < 0.0 {
        w = w.flipped();
    }
    w
}

/// Sweep a profile polyline along a rail polyline with rotation-minimising frames
/// (double reflection, Wang et al. 2008). The profile keeps its placement relative
/// to the rail start. Closed profiles (repeated closing point) give closed tubes.
pub fn sweep1_mesh(rail: &[Point3], profile: &[Point3]) -> Mesh {
    let mut r: Vec<Point3> = rail.to_vec();
    r.dedup_by(|a, b| a.distance_to(*b) < 1e-12);
    let np = profile.len();
    if r.len() < 2 || np < 2 {
        return Mesh::default();
    }
    let nr = r.len();
    let seg = |i: usize| (r[i + 1] - r[i]).normalized().unwrap_or(Vec3::Z);
    let tangents: Vec<Vec3> = (0..nr)
        .map(|i| {
            if i == 0 {
                seg(0)
            } else if i == nr - 1 {
                seg(nr - 2)
            } else {
                (seg(i - 1) + seg(i)).normalized().unwrap_or(seg(i))
            }
        })
        .collect();
    let mut frames = Vec::with_capacity(nr);
    let mut rr = Plane::from_normal(r[0], tangents[0]).x;
    frames.push(rr);
    for i in 0..nr - 1 {
        let v1 = r[i + 1] - r[i];
        let c1 = v1.dot(v1);
        let rl = rr - v1 * (2.0 / c1 * v1.dot(rr));
        let tl = tangents[i] - v1 * (2.0 / c1 * v1.dot(tangents[i]));
        let v2 = tangents[i + 1] - tl;
        let c2 = v2.dot(v2);
        rr = if c2 < 1e-24 {
            rl
        } else {
            rl - v2 * (2.0 / c2 * v2.dot(rl))
        };
        frames.push(rr);
    }
    let (t0, r0) = (tangents[0], frames[0]);
    let s0 = t0.cross(r0);
    let local: Vec<(f64, f64, f64)> = profile
        .iter()
        .map(|p| {
            let d = *p - r[0];
            (d.dot(r0), d.dot(s0), d.dot(t0))
        })
        .collect();
    let mut m = Mesh::default();
    for i in 0..nr {
        let (t, rf) = (tangents[i], frames[i]);
        let s = t.cross(rf);
        m.positions.extend(
            local
                .iter()
                .map(|(a, b, c)| r[i] + rf * *a + s * *b + t * *c),
        );
    }
    let id = |i: usize, j: usize| (i * np + j) as u32;
    for i in 0..nr - 1 {
        for j in 0..np - 1 {
            let (a, b, c, d) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
            m.triangles.push([a, b, c]);
            m.triangles.push([a, c, d]);
        }
    }
    m.welded(1e-9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z)
    }

    fn square(x0: f64, y0: f64, s: f64) -> Vec<Point3> {
        vec![
            p(x0, y0, 0.0),
            p(x0 + s, y0, 0.0),
            p(x0 + s, y0 + s, 0.0),
            p(x0, y0 + s, 0.0),
        ]
    }

    #[test]
    fn planar_region_with_two_holes() {
        let outer = square(0.0, 0.0, 10.0);
        let holes = vec![square(2.0, 2.0, 2.0), square(6.0, 5.0, 3.0)];
        let m = planar_mesh(&outer, &holes, Vec3::Z);
        assert!(
            (m.area() - (100.0 - 4.0 - 9.0)).abs() < 1e-9,
            "{}",
            m.area()
        );
        assert!(m.area_vector().z > 0.0);
        // Every triangle faces +Z.
        for t in &m.triangles {
            let [a, b, c] = t.map(|i| m.positions[i as usize]);
            assert!((b - a).cross(c - a).z > -1e-12);
        }
    }

    #[test]
    fn extruded_region_is_a_closed_solid() {
        let m = planar_mesh(&square(0.0, 0.0, 10.0), &[square(4.0, 4.0, 2.0)], Vec3::Z);
        let s = extrude_open_mesh(&m, Vec3::new(0.0, 0.0, -5.0), 1e-6).unwrap();
        assert!(s.is_closed(1e-6));
        assert!((s.volume() - 96.0 * 5.0).abs() < 1e-6, "{}", s.volume());
        assert!(extrude_open_mesh(&s, Vec3::Z, 1e-6).is_none());
    }

    #[test]
    fn cap_open_box() {
        // Extruded square walls without caps: an open tube with two planar holes.
        let pts = square(0.0, 0.0, 2.0);
        let tube = extrude_walls(&pts, Vec3::new(0.0, 0.0, 3.0), true);
        assert!(!tube.is_closed(1e-9));
        let (capped, n) = cap_planar_holes(&tube, 1e-9);
        assert_eq!(n, 2);
        assert!(capped.is_closed(1e-9));
        assert!((capped.volume() - 12.0).abs() < 1e-9, "{}", capped.volume());
    }

    #[test]
    fn loft_two_circles_is_a_tube() {
        let circle = |z: f64, r: f64| -> Vec<Point3> {
            (0..32)
                .map(|i| {
                    let a = TAU * i as f64 / 32.0;
                    p(r * a.cos(), r * a.sin(), z)
                })
                .collect()
        };
        let m = loft_mesh(&[circle(0.0, 5.0), circle(10.0, 5.0)], true);
        assert_eq!(m.triangles.len(), 64);
        let lateral = 2.0 * PI * 5.0 * 10.0;
        assert!((m.area() - lateral).abs() / lateral < 0.01);
        assert_eq!(m.normals.len(), m.positions.len());
    }

    #[test]
    fn revolved_rectangle_is_a_cylinder() {
        let rect = [
            p(0.0, 0.0, 0.0),
            p(10.0, 0.0, 0.0),
            p(10.0, 0.0, 20.0),
            p(0.0, 0.0, 20.0),
            p(0.0, 0.0, 0.0),
        ];
        let m = revolve_mesh(&rect, Point3::ORIGIN, Vec3::Z, TAU, 1e-9);
        assert!(m.boundary_edges().is_empty());
        let exact = PI * 100.0 * 20.0;
        assert!((m.volume() - exact).abs() / exact < 0.01, "{}", m.volume());
        // A half turn of an open profile is open.
        let h = revolve_mesh(&rect[1..3], Point3::ORIGIN, Vec3::Z, PI, 1e-9);
        assert!(!h.boundary_edges().is_empty());
        assert!((h.area() - PI * 10.0 * 20.0).abs() / (PI * 200.0) < 0.01);
    }

    #[test]
    fn sweep_circle_along_bent_rail() {
        let profile: Vec<Point3> = (0..=32)
            .map(|i| {
                let a = TAU * i as f64 / 32.0;
                p(a.cos(), a.sin(), 0.0)
            })
            .collect();
        let rail = [p(0.0, 0.0, 0.0), p(0.0, 0.0, 10.0), p(10.0, 0.0, 20.0)];
        let m = sweep1_mesh(&rail, &profile);
        // First ring equals the profile.
        for q in &profile[..32] {
            assert!(m.positions.iter().any(|v| v.distance_to(*q) < 1e-9));
        }
        // Every ring stays perpendicular to the rail tangent at the rail end.
        let t = (rail[2] - rail[1]).normalized().unwrap();
        let ring: Vec<&Point3> = m
            .positions
            .iter()
            .filter(|v| (**v - rail[2]).length() < 1.0 + 1e-9)
            .collect();
        assert_eq!(ring.len(), 32);
        for v in ring {
            assert!(((*v - rail[2]).dot(t)).abs() < 1e-9);
            assert!(((*v - rail[2]).length() - 1.0).abs() < 1e-9);
        }
        // Tube along a straight rail has no seam: only two boundary loops.
        let w = sweep1_mesh(&rail[..2], &profile);
        assert_eq!(w.welded(1e-9).boundary_loops().len(), 2);
    }

    #[test]
    fn resample_by_length() {
        let r = resample(&[p(0.0, 0.0, 0.0), p(10.0, 0.0, 0.0)], 5, false);
        assert_eq!(r.len(), 5);
        assert!(r[1].distance_to(p(2.5, 0.0, 0.0)) < 1e-12);
        let c = resample(&square(0.0, 0.0, 1.0), 8, true);
        assert_eq!(c.len(), 8);
        assert!(c[2].distance_to(p(1.0, 0.0, 0.0)) < 1e-12);
    }
}
