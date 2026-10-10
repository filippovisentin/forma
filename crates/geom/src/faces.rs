//! Planar faces of triangle meshes: find the flat region around a triangle and
//! push / pull it along its normal, stretching the neighbouring faces (Rhino's
//! MoveFace and gumball extrusion of a solid's face).

use crate::{Mesh, Point3, Vec3};
use std::collections::{HashMap, HashSet};

type Key = (i64, i64, i64);
type EdgeKey = (Key, Key);

/// Integer key of a position, so duplicated vertices (meshes built face by face)
/// are recognised as the same point.
fn key(p: Point3, tol: f64) -> Key {
    let q = tol.max(1e-9);
    (
        (p.x / q).round() as i64,
        (p.y / q).round() as i64,
        (p.z / q).round() as i64,
    )
}

/// A planar face of a mesh: its triangles, unit normal (from the winding) and a
/// point on it.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshFace {
    pub triangles: Vec<usize>,
    pub normal: Vec3,
    pub center: Point3,
    pub area: f64,
}

impl Mesh {
    /// Unit normal of triangle `t` following its winding.
    pub fn triangle_normal(&self, t: usize) -> Option<Vec3> {
        let [a, b, c] = self.triangles[t].map(|i| self.positions[i as usize]);
        (b - a).cross(c - a).normalized()
    }

    fn triangle_area(&self, t: usize) -> f64 {
        let [a, b, c] = self.triangles[t].map(|i| self.positions[i as usize]);
        (b - a).cross(c - a).length() / 2.0
    }

    /// Closest point of the mesh to `p`: (triangle, point, distance).
    pub fn closest_triangle(&self, p: Point3) -> Option<(usize, Point3, f64)> {
        let mut best: Option<(usize, Point3, f64)> = None;
        for (t, tri) in self.triangles.iter().enumerate() {
            let [a, b, c] = tri.map(|i| self.positions[i as usize]);
            let q = closest_on_triangle(p, a, b, c);
            let d = q.distance_to(p);
            if best.is_none_or(|bb| d < bb.2) {
                best = Some((t, q, d));
            }
        }
        best
    }

    /// The flat face around triangle `t`: neighbouring triangles (sharing an
    /// edge by position) with the same plane, within `tol` model units and about
    /// half a degree.
    pub fn planar_face(&self, t: usize, tol: f64) -> Option<MeshFace> {
        let normal = self.triangle_normal(t)?;
        let origin = self.positions[self.triangles[t][0] as usize];
        let mut edges: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
        for (i, tri) in self.triangles.iter().enumerate() {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let (ka, kb) = (
                    key(self.positions[a as usize], tol),
                    key(self.positions[b as usize], tol),
                );
                let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
                edges.entry(k).or_default().push(i);
            }
        }
        let same_plane = |i: usize| {
            let Some(n) = self.triangle_normal(i) else {
                return false;
            };
            n.dot(normal) > 0.99996
                && self.triangles[i]
                    .iter()
                    .all(|v| (self.positions[*v as usize] - origin).dot(normal).abs() <= tol)
        };
        let mut seen = HashSet::from([t]);
        let mut stack = vec![t];
        while let Some(i) = stack.pop() {
            let tri = self.triangles[i];
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let (ka, kb) = (
                    key(self.positions[a as usize], tol),
                    key(self.positions[b as usize], tol),
                );
                let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
                for &j in edges.get(&k).map_or(&[][..], Vec::as_slice) {
                    if !seen.contains(&j) && same_plane(j) {
                        seen.insert(j);
                        stack.push(j);
                    }
                }
            }
        }
        let mut triangles: Vec<usize> = seen.into_iter().collect();
        triangles.sort_unstable();
        let mut area = 0.0;
        let mut c = Vec3::new(0.0, 0.0, 0.0);
        for &i in &triangles {
            let w = self.triangle_area(i);
            let [a, b, cc] = self.triangles[i].map(|v| self.positions[v as usize]);
            c = c + (a.to_vec() + b.to_vec() + cc.to_vec()) * (w / 3.0);
            area += w;
        }
        let center = if area > 0.0 {
            Point3::new(c.x / area, c.y / area, c.z / area)
        } else {
            origin
        };
        Some(MeshFace {
            triangles,
            normal,
            center,
            area,
        })
    }

    /// All planar faces of the mesh (each triangle in exactly one face).
    pub fn planar_faces(&self, tol: f64) -> Vec<MeshFace> {
        let mut done = vec![false; self.triangles.len()];
        let mut out = Vec::new();
        for t in 0..self.triangles.len() {
            if done[t] {
                continue;
            }
            match self.planar_face(t, tol) {
                Some(f) => {
                    for &i in &f.triangles {
                        done[i] = true;
                    }
                    out.push(f);
                }
                None => done[t] = true,
            }
        }
        out
    }

    /// Outline of a face: its edges used by only one of its triangles.
    pub fn face_outline(&self, face: &MeshFace, tol: f64) -> Vec<[Point3; 2]> {
        let mut count: HashMap<EdgeKey, (u32, [Point3; 2])> = HashMap::new();
        for &i in &face.triangles {
            let tri = self.triangles[i];
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
                let (ka, kb) = (key(pa, tol), key(pb, tol));
                let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
                count.entry(k).or_insert((0, [pa, pb])).0 += 1;
            }
        }
        count
            .into_values()
            .filter(|(n, _)| *n == 1)
            .map(|(_, e)| e)
            .collect()
    }

    /// Move a face along `v`, together with every vertex at the same place in
    /// the neighbouring faces, which stretch to follow (push / pull).
    pub fn move_face(&self, face: &MeshFace, v: Vec3, tol: f64) -> Mesh {
        let keys: HashSet<Key> = face
            .triangles
            .iter()
            .flat_map(|&i| self.triangles[i])
            .map(|vi| key(self.positions[vi as usize], tol))
            .collect();
        let mut m = self.clone();
        for p in &mut m.positions {
            if keys.contains(&key(*p, tol)) {
                *p = *p + v;
            }
        }
        m
    }

    /// The planar face whose normal points along `dir` and lies furthest along
    /// it (the top face for +Z): what a gumball extrude dot pushes or pulls.
    pub fn extreme_face(&self, dir: Vec3, tol: f64) -> Option<MeshFace> {
        let d = dir.normalized()?;
        self.planar_faces(tol)
            .into_iter()
            .filter(|f| f.normal.dot(d) > 0.999)
            .max_by(|a, b| {
                a.center
                    .to_vec()
                    .dot(d)
                    .total_cmp(&b.center.to_vec().dot(d))
            })
    }
}

/// Closest point on triangle `abc` to `p` (Ericson, Real-Time Collision Detection).
fn closest_on_triangle(p: Point3, a: Point3, b: Point3, c: Point3) -> Point3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

impl Mesh {
    /// Copy whose triangles all wind the same way as their neighbours (per
    /// connected piece), closed pieces facing outwards, and the number of
    /// triangles that were flipped. Normals are dropped when anything changed.
    pub fn unified(&self, tol: f64) -> (Mesh, usize) {
        let n = self.triangles.len();
        let keys: Vec<[Key; 3]> = self
            .triangles
            .iter()
            .map(|t| t.map(|i| key(self.positions[i as usize], tol)))
            .collect();
        // Undirected edge -> triangles using it.
        let mut by_edge: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
        for (t, k) in keys.iter().enumerate() {
            for e in 0..3 {
                let (a, b) = (k[e], k[(e + 1) % 3]);
                by_edge
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_default()
                    .push(t);
            }
        }
        // Does triangle `t` (with flip state `f`) run along a -> b?
        let runs = |t: usize, f: bool, a: Key, b: Key| {
            let k = keys[t];
            (0..3).any(|e| {
                let (x, y) = (k[e], k[(e + 1) % 3]);
                if f {
                    (y, x) == (a, b)
                } else {
                    (x, y) == (a, b)
                }
            })
        };
        let mut flip = vec![false; n];
        let mut seen = vec![false; n];
        let mut out = self.clone();
        for start in 0..n {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let mut piece = vec![start];
            let mut stack = vec![start];
            let mut open = false;
            while let Some(t) = stack.pop() {
                let k = keys[t];
                for e in 0..3 {
                    let (mut a, mut b) = (k[e], k[(e + 1) % 3]);
                    if flip[t] {
                        std::mem::swap(&mut a, &mut b);
                    }
                    let ek = if a < b { (a, b) } else { (b, a) };
                    let users = &by_edge[&ek];
                    if users.len() == 1 {
                        open = true;
                    }
                    for &u in users {
                        if u == t || seen[u] {
                            continue;
                        }
                        seen[u] = true;
                        // A consistent neighbour runs the shared edge the other way.
                        flip[u] = runs(u, false, a, b);
                        piece.push(u);
                        stack.push(u);
                    }
                }
            }
            if !open {
                let vol: f64 = piece
                    .iter()
                    .map(|&t| {
                        let [a, mut b, mut c] =
                            self.triangles[t].map(|i| self.positions[i as usize].to_vec());
                        if flip[t] {
                            std::mem::swap(&mut b, &mut c);
                        }
                        a.dot(b.cross(c))
                    })
                    .sum();
                if vol < 0.0 {
                    for &t in &piece {
                        flip[t] = !flip[t];
                    }
                }
            }
        }
        let count = flip.iter().filter(|f| **f).count();
        if count > 0 {
            for (t, f) in flip.iter().enumerate() {
                if *f {
                    out.triangles[t].swap(1, 2);
                }
            }
            out.normals.clear();
        }
        (out, count)
    }
}

#[cfg(test)]
mod tests {
    use crate::{box_mesh, cylinder_mesh, Plane, Point3, Vec3};

    #[test]
    fn box_faces_and_push_pull() {
        let b = box_mesh(&Plane::TOP, 10.0, 20.0, 30.0);
        let faces = b.planar_faces(1e-6);
        assert_eq!(faces.len(), 6);
        let top = b.extreme_face(Vec3::Z, 1e-6).unwrap();
        assert!((top.area - 200.0).abs() < 1e-9);
        assert!((top.center.z - 30.0).abs() < 1e-9);
        assert_eq!(b.face_outline(&top, 1e-6).len(), 4);
        let v0 = b.volume();
        let taller = b.move_face(&top, Vec3::Z * 5.0, 1e-6);
        assert!((taller.volume() - v0 * 35.0 / 30.0).abs() < 1e-6);
        let bb = taller.bounding_box().unwrap();
        assert!((bb.max.z - 35.0).abs() < 1e-9 && bb.min.z.abs() < 1e-9);
        // Pull a side face inwards.
        let side = b.extreme_face(Vec3::X, 1e-6).unwrap();
        let thinner = b.move_face(&side, Vec3::X * -4.0, 1e-6);
        assert!((thinner.volume() - 6.0 * 20.0 * 30.0).abs() < 1e-6);
    }

    #[test]
    fn cylinder_top_face_and_closest_triangle() {
        let c = cylinder_mesh(&Plane::TOP, 5.0, 10.0, 32);
        let top = c.extreme_face(Vec3::Z, 1e-6).unwrap();
        let tall = c.move_face(&top, Vec3::Z * 10.0, 1e-6);
        let bb = tall.bounding_box().unwrap();
        assert!((bb.max.z - 20.0).abs() < 1e-9);
        assert!((tall.volume() - 2.0 * c.volume()).abs() < 1e-6 * c.volume());
        let (t, q, d) = c.closest_triangle(Point3::new(1.0, 1.0, 12.0)).unwrap();
        assert!((d - 2.0).abs() < 1e-9, "{d}");
        assert!((q.z - 10.0).abs() < 1e-9);
        assert!(c.planar_face(t, 1e-6).unwrap().normal.dot(Vec3::Z) > 0.999);
    }
}
