//! Planar faces of a triangle mesh as polygons with holes, ready to become an
//! exact polysurface (a brep whose faces are trimmed planes). Used to turn the
//! result of mesh operations on flat-faced solids (push / pull, booleans of
//! boxes and walls) back into exact geometry.

use crate::{Mesh, Point3, Vec3};
use std::collections::HashMap;

/// One planar face: its unit normal and its boundary loops as indices into
/// [`Polyhedron::points`]. The first loop is the outer boundary
/// (counter-clockwise around the normal), the others are holes (clockwise).
#[derive(Debug, Clone, PartialEq)]
pub struct PolyFace {
    pub normal: Vec3,
    pub loops: Vec<Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Polyhedron {
    pub points: Vec<Point3>,
    pub faces: Vec<PolyFace>,
}

/// Newell normal (not normalised; length = twice the area).
fn newell(pts: &[Point3]) -> Vec3 {
    let mut n = Vec3::new(0.0, 0.0, 0.0);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        n = n + Vec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    n
}

impl Mesh {
    /// The mesh as flat polygons: coplanar neighbouring triangles merged into
    /// faces, their outlines chained into loops, and vertices dropped where an
    /// outline runs straight on between the same two faces. `None` when the mesh
    /// is not a clean manifold (an edge shared by three triangles, a vertex where
    /// a face outline touches itself) or has more than `max_faces` faces (a
    /// tessellated curved surface rather than a polyhedron: also when most
    /// edges between faces fold by less than 20°).
    pub fn polyhedron(&self, tol: f64, max_faces: usize) -> Option<Polyhedron> {
        let m = self.welded(tol);
        if m.triangles.is_empty() {
            return None;
        }
        let faces = m.planar_faces(tol);
        if faces.is_empty() || faces.len() > max_faces {
            return None;
        }
        let mut face_of = vec![usize::MAX; m.triangles.len()];
        for (f, face) in faces.iter().enumerate() {
            for &t in &face.triangles {
                face_of[t] = f;
            }
        }
        // Directed edge -> triangle; a second triangle on the same directed edge
        // means a non-manifold or inconsistently wound mesh.
        let mut directed: HashMap<(u32, u32), usize> = HashMap::new();
        for (t, tri) in m.triangles.iter().enumerate() {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                if directed.insert((a, b), t).is_some() {
                    return None;
                }
            }
        }
        // Face on the other side of a directed edge (None = naked border).
        let other = |a: u32, b: u32| directed.get(&(b, a)).map(|t| face_of[*t]);
        // A tessellated curved surface: most edges between faces are nearly
        // flat folds. A polyhedron has sharp ones.
        let (mut soft, mut sharp) = (0usize, 0usize);
        for (&(a, b), &t) in &directed {
            if a < b {
                if let Some(&u) = directed.get(&(b, a)) {
                    let (f, g) = (face_of[t], face_of[u]);
                    if f != g {
                        if faces[f].normal.dot(faces[g].normal) > 20f64.to_radians().cos() {
                            soft += 1;
                        } else {
                            sharp += 1;
                        }
                    }
                }
            }
        }
        if soft > sharp {
            return None;
        }

        let mut keep = vec![false; m.positions.len()];
        let mut raw: Vec<(Vec3, Vec<Vec<u32>>)> = Vec::new();
        for (f, face) in faces.iter().enumerate() {
            // Boundary edges of the face, chained into loops.
            let mut next: HashMap<u32, u32> = HashMap::new();
            for &t in &face.triangles {
                let tri = m.triangles[t];
                for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                    if other(a, b) != Some(f) && next.insert(a, b).is_some() {
                        return None; // the outline touches itself at `a`
                    }
                }
            }
            let mut loops = Vec::new();
            while let Some(&start) = next.keys().min() {
                let mut l = vec![start];
                let mut v = next.remove(&start)?;
                while v != start {
                    l.push(v);
                    v = next.remove(&v)?;
                }
                if l.len() < 3 {
                    return None;
                }
                loops.push(l);
            }
            // Corners: where the outline turns or the neighbouring face changes.
            for l in &loops {
                let n = l.len();
                for i in 0..n {
                    let (p, v, q) = (l[(i + n - 1) % n], l[i], l[(i + 1) % n]);
                    let (pp, pv, pq) = (
                        m.positions[p as usize],
                        m.positions[v as usize],
                        m.positions[q as usize],
                    );
                    let (d0, d1) = ((pv - pp).normalized(), (pq - pv).normalized());
                    let straight = match (d0, d1) {
                        (Some(a), Some(b)) => {
                            a.dot(b) > 0.0
                                && a.cross(b).length() * pv.distance_to(pq).min(pp.distance_to(pv))
                                    <= tol
                        }
                        _ => false,
                    };
                    if !straight || other(p, v) != other(v, q) {
                        keep[v as usize] = true;
                    }
                }
            }
            raw.push((face.normal, loops));
        }
        // Compact the kept vertices.
        let mut index = vec![u32::MAX; m.positions.len()];
        let mut out = Polyhedron::default();
        for (i, k) in keep.iter().enumerate() {
            if *k {
                index[i] = out.points.len() as u32;
                out.points.push(m.positions[i]);
            }
        }
        for (normal, loops) in raw {
            let mut loops: Vec<Vec<u32>> = loops
                .into_iter()
                .map(|l| {
                    l.into_iter()
                        .filter(|v| keep[*v as usize])
                        .map(|v| index[v as usize])
                        .collect::<Vec<u32>>()
                })
                .collect();
            if loops.iter().any(|l| l.len() < 3) {
                return None;
            }
            // Outer loop first: the one with the largest area along the normal.
            let area = |l: &Vec<u32>| {
                let pts: Vec<Point3> = l.iter().map(|i| out.points[*i as usize]).collect();
                newell(&pts).dot(normal)
            };
            loops.sort_by(|a, b| area(b).total_cmp(&area(a)));
            if area(&loops[0]) <= 0.0 || loops[1..].iter().any(|l| area(l) >= 0.0) {
                return None;
            }
            out.faces.push(PolyFace { normal, loops });
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::{box_mesh, Plane, Point3};

    #[test]
    fn a_box_has_six_quads() {
        let m = box_mesh(&Plane::TOP, 10.0, 20.0, 30.0);
        let p = m.polyhedron(1e-6, 100).expect("polyhedron");
        assert_eq!(p.points.len(), 8);
        assert_eq!(p.faces.len(), 6);
        assert!(p
            .faces
            .iter()
            .all(|f| f.loops.len() == 1 && f.loops[0].len() == 4));
    }

    #[test]
    fn subdivided_faces_merge_and_lose_their_midpoints() {
        let m = box_mesh(&Plane::TOP, 10.0, 10.0, 10.0).subdivided(2);
        let p = m.polyhedron(1e-6, 100).expect("polyhedron");
        assert_eq!(p.faces.len(), 6);
        assert_eq!(p.points.len(), 8, "{:?}", p.points);
    }

    #[test]
    fn a_frame_keeps_its_hole() {
        // Square plate with a square hole, extruded.
        let pts = |s: f64, o: f64| {
            vec![
                Point3::new(o, o, 0.0),
                Point3::new(o + s, o, 0.0),
                Point3::new(o + s, o + s, 0.0),
                Point3::new(o, o + s, 0.0),
            ]
        };
        let m = crate::extrude_region(&pts(10.0, 0.0), &[pts(4.0, 3.0)], crate::Vec3::Z * 2.0);
        let p = m.polyhedron(1e-6, 100).expect("polyhedron");
        assert_eq!(p.faces.len(), 10);
        let holed = p.faces.iter().filter(|f| f.loops.len() == 2).count();
        assert_eq!(holed, 2);
    }

    #[test]
    fn a_sphere_has_too_many_faces() {
        let m = crate::sphere_mesh(Point3::ORIGIN, 1.0, 24);
        assert!(m.polyhedron(1e-6, 100).is_none());
    }
}
