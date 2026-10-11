//! Exact surfaces and solids (boundary representations) carried through Forma
//! untouched: the native openNURBS object (a brep, an extrusion…) is kept as
//! opaque bytes, with a display mesh. Moving, rotating, scaling or mirroring it
//! only records the transform, applied to the exact object when it is written,
//! so a polysurface stays a polysurface when it goes back to Rhino.

use crate::{CircleArc, Mesh, Point3, Xform};
use std::sync::Arc;

#[derive(Clone, PartialEq)]
pub struct Brep {
    /// The native object, serialized by openNURBS (see `forma-io-3dm`).
    pub data: Arc<Vec<u8>>,
    /// Transform to apply to `data` (the display mesh already has it).
    pub xform: Xform,
    /// Triangulated faces, for display, picking, snapping and mesh tools.
    pub mesh: Mesh,
    /// Number of faces of the exact object.
    pub faces: usize,
    /// Closed (a solid).
    pub solid: bool,
    /// Every face is flat (boxes, extruded polylines, walls): mesh operations on
    /// it can be turned back into an exact polysurface.
    pub planar: bool,
    /// For each face, the end of its triangles in `mesh.triangles` (faces are
    /// meshed one after the other). Empty when unknown.
    pub face_ends: Vec<u32>,
    /// Corners of the exact object and the midpoints of its edges (snapping).
    pub vertices: Vec<Point3>,
    pub edge_mids: Vec<Point3>,
    /// Circles and arcs among the edges (centre and quadrant snaps).
    pub arcs: Vec<CircleArc>,
    /// Isocurves across curved faces, for wireframe display (polylines).
    pub wires: Vec<Vec<Point3>>,
}

impl std::fmt::Debug for Brep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Brep")
            .field("bytes", &self.data.len())
            .field("faces", &self.faces)
            .field("solid", &self.solid)
            .field("planar", &self.planar)
            .field("triangles", &self.mesh.triangles.len())
            .field("moved", &self.is_moved())
            .finish()
    }
}

impl Brep {
    /// Transformed copy: the mesh moves now, the exact object when it is written.
    pub fn transformed(&self, x: &Xform) -> Brep {
        let flip = x.flips();
        Brep {
            data: self.data.clone(),
            xform: self.xform.then(x),
            mesh: Mesh {
                positions: self.mesh.positions.iter().map(|p| x.point(*p)).collect(),
                normals: self.mesh.normals.iter().map(|n| x.normal(*n)).collect(),
                triangles: if flip {
                    self.mesh
                        .triangles
                        .iter()
                        .map(|t| [t[0], t[2], t[1]])
                        .collect()
                } else {
                    self.mesh.triangles.clone()
                },
            },
            faces: self.faces,
            solid: self.solid,
            planar: self.planar,
            face_ends: self.face_ends.clone(),
            vertices: self.vertices.iter().map(|p| x.point(*p)).collect(),
            edge_mids: self.edge_mids.iter().map(|p| x.point(*p)).collect(),
            // Arcs stay arcs only under shape-keeping maps.
            arcs: if x.is_similarity() {
                self.arcs.iter().map(|a| a.transformed(x)).collect()
            } else {
                Vec::new()
            },
            wires: self
                .wires
                .iter()
                .map(|w| w.iter().map(|p| x.point(*p)).collect())
                .collect(),
        }
    }
}

impl Brep {
    /// `xform` as a row-major 4×4 matrix (openNURBS `ON_Xform` layout).
    pub fn xform16(&self) -> [f64; 16] {
        let (m, t) = (self.xform.m, self.xform.t);
        [
            m[0][0], m[0][1], m[0][2], t.x, //
            m[1][0], m[1][1], m[1][2], t.y, //
            m[2][0], m[2][1], m[2][2], t.z, //
            0.0, 0.0, 0.0, 1.0,
        ]
    }

    /// Index of the face that display triangle `t` belongs to.
    pub fn face_of_triangle(&self, t: usize) -> Option<usize> {
        let i = self.face_ends.partition_point(|end| (*end as usize) <= t);
        (i < self.face_ends.len()).then_some(i)
    }

    /// True when the exact object still needs its transform applied.
    pub fn is_moved(&self) -> bool {
        self.xform != Xform::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;

    #[test]
    fn transforms_accumulate_and_flip_triangles() {
        let mesh = Mesh {
            positions: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            normals: Vec::new(),
            triangles: vec![[0, 1, 2]],
        };
        let b = Brep {
            data: Arc::new(vec![1, 2, 3]),
            xform: Xform::IDENTITY,
            mesh,
            faces: 1,
            solid: false,
            planar: true,
            face_ends: vec![1],
            vertices: Vec::new(),
            edge_mids: Vec::new(),
            arcs: Vec::new(),
            wires: Vec::new(),
        };
        assert_eq!(b.face_of_triangle(0), Some(0));
        assert_eq!(b.face_of_triangle(1), None);
        assert!(!b.is_moved());
        let moved = b
            .transformed(&Xform::translation(Vec3::new(5.0, 0.0, 0.0)))
            .transformed(&Xform::mirror(Point3::ORIGIN, Vec3::new(0.0, 1.0, 0.0)));
        assert!(moved.is_moved());
        assert_eq!(moved.mesh.triangles[0], [0, 2, 1]);
        let x = moved.xform16();
        assert!((x[3] - 5.0).abs() < 1e-12);
        assert!((x[5] + 1.0).abs() < 1e-12);
        assert!(moved.mesh.positions[1].distance_to(Point3::new(6.0, 0.0, 0.0)) < 1e-12);
    }
}
