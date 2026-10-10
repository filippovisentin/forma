//! Mesh tools: plane sections, weld / unweld by angle, and vertex deformations
//! (twist, taper) that also apply to curve points.

use crate::curvetools::chain_segments;
use crate::{Mesh, Plane, Point3, Vec3, Xform};
use std::collections::HashMap;

/// Polylines where `plane` cuts the mesh. Vertices exactly on the plane count as
/// being above it, so a face lying in the plane gives no section.
pub fn mesh_plane_section(mesh: &Mesh, plane: &Plane, tol: f64) -> Vec<Vec<Point3>> {
    let d: Vec<f64> = mesh.positions.iter().map(|p| plane.coords(*p).2).collect();
    let mut segs = Vec::new();
    for t in &mesh.triangles {
        let mut hits = Vec::with_capacity(2);
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (da, db) = (d[a as usize], d[b as usize]);
            if (da >= 0.0) != (db >= 0.0) {
                let s = da / (da - db);
                let (pa, pb) = (mesh.positions[a as usize], mesh.positions[b as usize]);
                hits.push(pa + (pb - pa) * s);
            }
        }
        if hits.len() == 2 && hits[0].distance_to(hits[1]) > 1e-12 {
            segs.push([hits[0], hits[1]]);
        }
    }
    chain_segments(&segs, tol)
}

/// Points where a polyline crosses `plane` (same on-plane rule as the mesh section).
pub fn polyline_plane_points(points: &[Point3], plane: &Plane) -> Vec<Point3> {
    let mut out = Vec::new();
    for w in points.windows(2) {
        let (da, db) = (plane.coords(w[0]).2, plane.coords(w[1]).2);
        if (da >= 0.0) != (db >= 0.0) {
            let s = da / (da - db);
            out.push(w[0] + (w[1] - w[0]) * s);
        }
    }
    out
}

/// Edges of a mesh drawn as visible edges (edges used by one triangle: naked
/// borders and the creases of meshes built face by face).
pub fn mesh_edge_segments(mesh: &Mesh) -> Vec<[Point3; 2]> {
    mesh.boundary_edges()
        .iter()
        .map(|[a, b]| [mesh.positions[*a as usize], mesh.positions[*b as usize]])
        .collect()
}

/// Naked border loops of a mesh (after welding coincident vertices) as closed
/// polylines.
pub fn mesh_border_loops(mesh: &Mesh, tol: f64) -> Vec<Vec<Point3>> {
    let w = mesh.welded(tol);
    w.boundary_loops()
        .into_iter()
        .map(|lp| {
            let mut pts: Vec<Point3> = lp.iter().map(|i| w.positions[*i as usize]).collect();
            pts.push(pts[0]);
            pts
        })
        .collect()
}

impl Mesh {
    /// Rebuild the vertices so that faces meeting at an angle of at most `angle`
    /// (radians) share vertices and smooth normals, and sharper edges are split
    /// (shown as visible edges). `angle = π` welds everything (Rhino's Weld),
    /// `angle = 0` splits every edge between non-coplanar faces (Unweld).
    pub fn split_by_angle(&self, angle: f64, tol: f64) -> Mesh {
        let w = self.welded(tol);
        let nt = w.triangles.len();
        let normals: Vec<Vec3> = (0..nt)
            .map(|t| {
                let [a, b, c] = w.triangles[t].map(|i| w.positions[i as usize]);
                (b - a).cross(c - a)
            })
            .collect();
        let unit: Vec<Vec3> = normals
            .iter()
            .map(|n| n.normalized().unwrap_or(Vec3::Z))
            .collect();
        // Corners: triangle t, slot k → index 3t + k.
        let mut parent: Vec<usize> = (0..3 * nt).collect();
        fn find(p: &mut [usize], mut i: usize) -> usize {
            while p[i] != i {
                p[i] = p[p[i]];
                i = p[i];
            }
            i
        }
        // Edge (sorted vertex pair) → (triangle, corner slot of each end).
        type EdgeUses = HashMap<(u32, u32), Vec<(usize, usize, usize)>>;
        let mut edges: EdgeUses = HashMap::new();
        for (t, tri) in w.triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                let key = (a.min(b), a.max(b));
                // Remember the corner slots of both edge ends.
                let (ka, kb) = if a < b {
                    (k, (k + 1) % 3)
                } else {
                    ((k + 1) % 3, k)
                };
                edges.entry(key).or_default().push((t, ka, kb));
            }
        }
        let cos_limit = angle.cos();
        for list in edges.values() {
            for i in 0..list.len() {
                for j in i + 1..list.len() {
                    let (t1, a1, b1) = list[i];
                    let (t2, a2, b2) = list[j];
                    let smooth = angle >= std::f64::consts::PI - 1e-12
                        || unit[t1].dot(unit[t2]) >= cos_limit - 1e-12;
                    if smooth {
                        for (x, y) in [(3 * t1 + a1, 3 * t2 + a2), (3 * t1 + b1, 3 * t2 + b2)] {
                            let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
                            parent[rx] = ry;
                        }
                    }
                }
            }
        }
        let mut out = Mesh::default();
        let mut index: HashMap<usize, u32> = HashMap::new();
        let mut acc: Vec<Vec3> = Vec::new();
        for (t, tri) in w.triangles.iter().enumerate() {
            let mut nt_ = [0u32; 3];
            for k in 0..3 {
                let r = find(&mut parent, 3 * t + k);
                let v = *index.entry(r).or_insert_with(|| {
                    out.positions.push(w.positions[tri[k] as usize]);
                    acc.push(Vec3::new(0.0, 0.0, 0.0));
                    (out.positions.len() - 1) as u32
                });
                acc[v as usize] = acc[v as usize] + normals[t];
                nt_[k] = v;
            }
            out.triangles.push(nt_);
        }
        out.normals = acc
            .into_iter()
            .map(|n| n.normalized().unwrap_or(Vec3::Z))
            .collect();
        out
    }
}

impl Mesh {
    /// Centroid of the surface (area-weighted triangle centroids).
    pub fn area_centroid(&self) -> Option<Point3> {
        let mut acc = Vec3::new(0.0, 0.0, 0.0);
        let mut total = 0.0;
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            let w = (b - a).cross(c - a).length() * 0.5;
            acc = acc + (a.to_vec() + b.to_vec() + c.to_vec()) * (w / 3.0);
            total += w;
        }
        (total > 1e-300).then(|| Point3::ORIGIN + acc * (1.0 / total))
    }

    /// Centroid of the enclosed volume (closed, consistently oriented mesh).
    pub fn volume_centroid(&self) -> Option<Point3> {
        let mut acc = Vec3::new(0.0, 0.0, 0.0);
        let mut total = 0.0;
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i as usize].to_vec());
            let v = a.dot(b.cross(c)) / 6.0;
            acc = acc + (a + b + c) * (v / 4.0);
            total += v;
        }
        (total.abs() > 1e-300).then(|| Point3::ORIGIN + acc * (1.0 / total))
    }

    /// Copy where every triangle is split into `k`² smaller ones (edges cut into
    /// `k` equal parts, so neighbouring triangles still match). Vertices are not
    /// shared between the original triangles; weld afterwards if needed.
    pub fn subdivided(&self, k: usize) -> Mesh {
        let k = k.max(1);
        let mut out = Mesh::default();
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            let base = out.positions.len() as u32;
            // Row i has k - i + 1 points: a + (b-a)·j/k + (c-a)·i/k.
            let mut index = Vec::new();
            for i in 0..=k {
                let mut row = Vec::new();
                for j in 0..=k - i {
                    row.push(out.positions.len() as u32 - base);
                    out.positions.push(
                        a + (b - a) * (j as f64 / k as f64) + (c - a) * (i as f64 / k as f64),
                    );
                }
                index.push(row);
            }
            for i in 0..k {
                for j in 0..k - i {
                    let (p, q, r) = (index[i][j], index[i][j + 1], index[i + 1][j]);
                    out.triangles.push([base + p, base + q, base + r]);
                    if j + 1 < index[i + 1].len() {
                        let s = index[i + 1][j + 1];
                        out.triangles.push([base + q, base + s, base + r]);
                    }
                }
            }
        }
        out
    }
}

/// Centroid of a closed planar polygon (no repeated point needed).
pub fn polygon_centroid(points: &[Point3]) -> Option<Point3> {
    let mut pts = points.to_vec();
    if pts.len() > 1 && pts[0].distance_to(*pts.last().expect("len")) < 1e-12 {
        pts.pop();
    }
    let n = crate::newell_area(&pts).normalized()?;
    let o = pts[0];
    let mut acc = Vec3::new(0.0, 0.0, 0.0);
    let mut total = 0.0;
    for w in pts[1..].windows(2) {
        let a = (w[0] - o).cross(w[1] - o).dot(n) * 0.5;
        acc = acc + (o.to_vec() + w[0].to_vec() + w[1].to_vec()) * (a / 3.0);
        total += a;
    }
    (total.abs() > 1e-300).then(|| Point3::ORIGIN + acc * (1.0 / total))
}

/// A deformation of space along an axis (Rhino's Twist and Taper).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Deform {
    /// Rotation about the axis growing from 0 at `start` to `angle` (radians) at
    /// `end`; points beyond the ends get the end values.
    Twist {
        start: Point3,
        end: Point3,
        angle: f64,
    },
    /// Distance from the axis scaled from `s0` at `start` to `s1` at `end`.
    Taper {
        start: Point3,
        end: Point3,
        s0: f64,
        s1: f64,
    },
    /// The spine `start → end` bent into a circular arc that passes through
    /// `through` (Rhino's Bend). Points before the start stay; points past the end
    /// follow the tangent at the end of the arc.
    Bend {
        start: Point3,
        end: Point3,
        through: Point3,
    },
}

impl Deform {
    fn axis(start: Point3, end: Point3, p: Point3) -> Option<(Vec3, f64, Point3)> {
        let d = end - start;
        let l = d.length();
        let dir = d.normalized()?;
        let s = (p - start).dot(dir);
        Some((dir, (s / l).clamp(0.0, 1.0), start + dir * s))
    }

    pub fn point(&self, p: Point3) -> Point3 {
        match *self {
            Deform::Twist { start, end, angle } => match Self::axis(start, end, p) {
                Some((dir, f, _)) => Xform::rotation(start, dir, angle * f).point(p),
                None => p,
            },
            Deform::Taper { start, end, s0, s1 } => match Self::axis(start, end, p) {
                Some((_, f, foot)) => foot + (p - foot) * (s0 + (s1 - s0) * f),
                None => p,
            },
            Deform::Bend {
                start,
                end,
                through,
            } => Self::bend(start, end, through, p),
        }
    }

    fn bend(start: Point3, end: Point3, through: Point3, p: Point3) -> Point3 {
        let Some(u) = (end - start).normalized() else {
            return p;
        };
        let len = start.distance_to(end);
        let t = through - start;
        let Some(v) = (t - u * t.dot(u)).normalized() else {
            return p; // through point on the spine: nothing to bend
        };
        let (tu, tv) = (t.dot(u), t.dot(v));
        // Circle tangent to the spine at its start, through the through point.
        let r = (tu * tu + tv * tv) / (2.0 * tv);
        let d = p - start;
        let (s, w) = (d.dot(u), d.dot(v));
        let rest = d - u * s - v * w;
        if s <= 0.0 {
            return p;
        }
        let a = s.min(len) / r;
        // Point at arc length min(s, len), offset by w towards the centre.
        let (sin, cos) = a.sin_cos();
        let mut q = start + u * ((r - w) * sin) + v * (r - (r - w) * cos);
        if s > len {
            q = q + (u * cos + v * sin) * (s - len);
        }
        q + rest
    }

    /// Deformed copy of a mesh (normals recomputed).
    pub fn mesh(&self, m: &Mesh) -> Mesh {
        let mut out = Mesh {
            positions: m.positions.iter().map(|p| self.point(*p)).collect(),
            normals: Vec::new(),
            triangles: m.triangles.clone(),
        };
        if m.normals.len() == m.positions.len() {
            // Keep the original smoothing groups: recompute per-vertex normals.
            out.compute_smooth_normals();
        }
        out
    }
}

impl Mesh {
    /// Parameters `t` (sorted) where the line `origin + t·dir` crosses the mesh.
    pub fn ray_hits(&self, origin: Point3, dir: Vec3) -> Vec<f64> {
        let mut out = Vec::new();
        for tri in &self.triangles {
            let [a, b, c] = tri.map(|i| self.positions[i as usize]);
            // Möller–Trumbore.
            let (e1, e2) = (b - a, c - a);
            let h = dir.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-18 {
                continue;
            }
            let f = 1.0 / det;
            let s = origin - a;
            let u = f * s.dot(h);
            if !(-1e-12..=1.0 + 1e-12).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = f * dir.dot(q);
            if v < -1e-12 || u + v > 1.0 + 1e-12 {
                continue;
            }
            out.push(f * e2.dot(q));
        }
        out.sort_by(f64::total_cmp);
        out
    }
}

/// Points of a polyline with extra points inserted so that no segment is longer
/// than `max_len` (so deformations bend straight segments).
pub fn densify(points: &[Point3], max_len: f64) -> Vec<Point3> {
    if points.len() < 2 || max_len <= 0.0 {
        return points.to_vec();
    }
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        let n = ((w[0].distance_to(w[1]) / max_len).ceil() as usize).clamp(1, 10_000);
        for i in 1..=n {
            out.push(w[0] + (w[1] - w[0]) * (i as f64 / n as f64));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bend_keeps_length_and_passes_through() {
        let d = Deform::Bend {
            start: Point3::ORIGIN,
            end: Point3::new(100.0, 0.0, 0.0),
            through: Point3::new(100.0, 100.0, 0.0),
        };
        // Through point at (100, 100): radius 100, a quarter turn for the spine
        // length 100·π/2; a spine of 100 turns by 1 radian.
        let e = d.point(Point3::new(100.0, 0.0, 0.0));
        assert!(
            e.distance_to(Point3::new(
                100.0 * 1f64.sin(),
                100.0 * (1.0 - 1f64.cos()),
                0.0
            )) < 1e-9
        );
        let q = d.point(Point3::new(50.0, 0.0, 0.0));
        assert!((q.distance_to(Point3::new(0.0, 100.0, 0.0)) - 100.0).abs() < 1e-9);
        // Points before the start do not move; z offsets are kept.
        let b = Point3::new(-5.0, 3.0, 2.0);
        assert!(d.point(b).distance_to(b) < 1e-12);
        assert!((d.point(Point3::new(50.0, 0.0, 7.0)).z - 7.0).abs() < 1e-12);
        // Beyond the end: along the end tangent.
        let f = d.point(Point3::new(110.0, 0.0, 0.0));
        assert!((f.distance_to(e) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn ray_hits_a_box() {
        let m = crate::box_mesh(&Plane::TOP, 10.0, 10.0, 5.0);
        let h = m.ray_hits(Point3::new(3.0, 4.0, 100.0), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(h.len(), 2, "{h:?}");
        assert!((h[0] - 95.0).abs() < 1e-9 && (h[1] - 100.0).abs() < 1e-9);
        assert!(m
            .ray_hits(Point3::new(30.0, 4.0, 100.0), Vec3::new(0.0, 0.0, -1.0))
            .is_empty());
    }
    use crate::box_mesh;

    const TOL: f64 = 1e-9;

    #[test]
    fn section_of_a_box_is_a_closed_rectangle() {
        let b = box_mesh(&Plane::TOP, 10.0, 20.0, 30.0);
        let s = mesh_plane_section(&b, &Plane::TOP.moved_to(Point3::new(0.0, 0.0, 12.0)), 1e-6);
        assert_eq!(s.len(), 1);
        let lp = &s[0];
        assert!(lp[0].distance_to(*lp.last().unwrap()) < 1e-6);
        let len: f64 = lp.windows(2).map(|w| w[0].distance_to(w[1])).sum();
        assert!((len - 60.0).abs() < 1e-6, "{len}");
        assert!(lp.iter().all(|p| (p.z - 12.0).abs() < TOL));
        // A plane missing the box.
        assert!(
            mesh_plane_section(&b, &Plane::TOP.moved_to(Point3::new(0.0, 0.0, 40.0)), 1e-6)
                .is_empty()
        );
    }

    #[test]
    fn polyline_crossings() {
        let pts = vec![
            Point3::new(0.0, 0.0, -1.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(2.0, 0.0, -1.0),
        ];
        let c = polyline_plane_points(&pts, &Plane::TOP);
        assert_eq!(c.len(), 2);
        assert!(c[1].distance_to(Point3::new(1.0, 0.0, 0.0)) < TOL);
    }

    #[test]
    fn weld_and_unweld_a_box() {
        let b = box_mesh(&Plane::TOP, 1.0, 1.0, 1.0);
        assert_eq!(b.boundary_edges().len(), 24); // 12 edges, each seen from 2 faces
        let welded = b.split_by_angle(std::f64::consts::PI, 1e-9);
        assert_eq!(welded.positions.len(), 8);
        assert!(welded.boundary_edges().is_empty());
        let split = welded.split_by_angle(10f64.to_radians(), 1e-9);
        assert_eq!(split.positions.len(), 24);
        assert_eq!(mesh_edge_segments(&split).len(), 24);
        assert!(split.is_closed(1e-9));
        assert!((split.volume() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn borders_of_an_open_mesh() {
        let b = box_mesh(&Plane::TOP, 1.0, 1.0, 1.0);
        let mut open = b.clone();
        open.triangles.truncate(10); // drop the right face
        let loops = mesh_border_loops(&open, 1e-9);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 5);
        assert!(mesh_border_loops(&b, 1e-9).is_empty());
    }

    #[test]
    fn twist_and_taper() {
        let tw = Deform::Twist {
            start: Point3::ORIGIN,
            end: Point3::new(0.0, 0.0, 10.0),
            angle: std::f64::consts::FRAC_PI_2,
        };
        let q = tw.point(Point3::new(1.0, 0.0, 10.0));
        assert!(q.distance_to(Point3::new(0.0, 1.0, 10.0)) < TOL);
        let q = tw.point(Point3::new(1.0, 0.0, 0.0));
        assert!(q.distance_to(Point3::new(1.0, 0.0, 0.0)) < TOL);
        let ta = Deform::Taper {
            start: Point3::ORIGIN,
            end: Point3::new(0.0, 0.0, 10.0),
            s0: 1.0,
            s1: 0.5,
        };
        let q = ta.point(Point3::new(2.0, 0.0, 5.0));
        assert!(q.distance_to(Point3::new(1.5, 0.0, 5.0)) < TOL);
        assert_eq!(
            densify(&[Point3::ORIGIN, Point3::new(10.0, 0.0, 0.0)], 3.0).len(),
            5
        );
    }

    #[test]
    fn centroids_and_subdivision() {
        let b = box_mesh(&Plane::TOP, 2.0, 4.0, 6.0);
        let c = b.volume_centroid().unwrap();
        assert!(c.distance_to(Point3::new(1.0, 2.0, 3.0)) < TOL);
        let c = b.area_centroid().unwrap();
        assert!(c.distance_to(Point3::new(1.0, 2.0, 3.0)) < TOL);
        let l = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(4.0, 0.0, 0.0),
            Point3::new(4.0, 2.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
            Point3::new(0.0, 0.0, 0.0),
        ];
        assert!(
            polygon_centroid(&l)
                .unwrap()
                .distance_to(Point3::new(2.0, 1.0, 0.0))
                < TOL
        );
        let s = b.subdivided(12);
        assert!((s.area() - b.area()).abs() < 1e-9);
        assert!((s.volume() - b.volume()).abs() < 1e-9);
        assert!(s.triangles.len() > 12 * 16);
    }
}
