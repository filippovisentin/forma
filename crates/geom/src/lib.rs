//! Geometry primitives for Forma.
//!
//! This crate knows nothing about documents, commands or UI. The NURBS kernel
//! (curvo / truck / OpenCascade, see `docs/spikes/S2-kernel.md`) will live behind
//! the types defined here.

mod arc;
mod curve;
mod faces;
mod nurbs;
mod plane;
mod solids;
mod surface;
mod xform;

pub use arc::CircleArc;
pub use curve::{
    carrier_intersections, chamfer_lines, crossings, extend, fillet_corners, fillet_lines, join,
    offset, side_of, split, trim, Chain, CurveError, ExtendTo, Seg,
};
pub use faces::MeshFace;
pub use nurbs::{Bezier2, NurbsCurve};
pub use plane::Plane;
pub use solids::{box_mesh, cylinder_mesh, extrude_mesh, sphere_mesh, triangulate_polygon};
pub use surface::{
    cap_planar_holes, extrude_open_mesh, loft_mesh, newell_area, planar_mesh, point_in_polygon,
    resample, revolve_mesh, sweep1_mesh, triangulate_with_holes,
};
pub use xform::Xform;

use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

/// Modelling tolerances. Never compare floats with `==`; use these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Absolute distance tolerance in model units (mm by default).
    pub absolute: f64,
    /// Angle tolerance in radians.
    pub angle: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            absolute: 0.001,
            angle: 1.0_f64.to_radians(),
        }
    }
}

/// A point in model space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A displacement in model space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub const ORIGIN: Point3 = Point3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(self, other: Point3) -> f64 {
        (other - self).length()
    }

    /// True when the two points coincide within `tol.absolute`.
    pub fn almost_eq(self, other: Point3, tol: Tolerance) -> bool {
        self.distance_to(other) <= tol.absolute
    }
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Unit vector in the same direction, or `None` for a zero-length vector.
    pub fn normalized(self) -> Option<Vec3> {
        let l = self.length();
        (l > 1e-300).then(|| self * (1.0 / l))
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Sub<Vec3> for Point3 {
    type Output = Point3;
    fn sub(self, v: Vec3) -> Point3 {
        Point3::new(self.x - v.x, self.y - v.y, self.z - v.z)
    }
}

impl Point3 {
    /// The point as a vector from the origin.
    pub fn to_vec(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    /// Midpoint of two points.
    pub fn mid(self, o: Point3) -> Point3 {
        Point3::new(
            (self.x + o.x) / 2.0,
            (self.y + o.y) / 2.0,
            (self.z + o.z) / 2.0,
        )
    }
}

impl Vec3 {
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);
}

impl Sub for Point3 {
    type Output = Vec3;
    fn sub(self, o: Point3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Add<Vec3> for Point3 {
    type Output = Point3;
    fn add(self, v: Vec3) -> Point3 {
        Point3::new(self.x + v.x, self.y + v.y, self.z + v.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        self * -1.0
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

impl fmt::Display for Point3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: Point3,
    pub max: Point3,
}

impl BoundingBox {
    pub fn from_points(points: &[Point3]) -> Option<Self> {
        let first = *points.first()?;
        let mut bb = BoundingBox {
            min: first,
            max: first,
        };
        for p in &points[1..] {
            bb.min = Point3::new(bb.min.x.min(p.x), bb.min.y.min(p.y), bb.min.z.min(p.z));
            bb.max = Point3::new(bb.max.x.max(p.x), bb.max.y.max(p.y), bb.max.z.max(p.z));
        }
        Some(bb)
    }

    pub fn union(self, o: BoundingBox) -> BoundingBox {
        BoundingBox::from_points(&[self.min, self.max, o.min, o.max]).expect("points")
    }

    pub fn center(&self) -> Point3 {
        self.min.mid(self.max)
    }

    /// The 8 corners.
    pub fn corners(&self) -> [Point3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Point3::new(a.x, a.y, a.z),
            Point3::new(b.x, a.y, a.z),
            Point3::new(b.x, b.y, a.z),
            Point3::new(a.x, b.y, a.z),
            Point3::new(a.x, a.y, b.z),
            Point3::new(b.x, a.y, b.z),
            Point3::new(b.x, b.y, b.z),
            Point3::new(a.x, b.y, b.z),
        ]
    }
}

/// A straight line segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineCurve {
    pub from: Point3,
    pub to: Point3,
}

impl LineCurve {
    pub fn new(from: Point3, to: Point3) -> Self {
        Self { from, to }
    }

    pub fn length(&self) -> f64 {
        self.from.distance_to(self.to)
    }

    /// Point at normalised parameter `t` in `[0, 1]`.
    pub fn point_at(&self, t: f64) -> Point3 {
        self.from + (self.to - self.from) * t
    }

    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::from_points(&[self.from, self.to]).expect("two points")
    }

    /// A line is degenerate when its length is within tolerance of zero.
    pub fn is_degenerate(&self, tol: Tolerance) -> bool {
        self.length() <= tol.absolute
    }
}

/// Triangle mesh used for display (and later for export).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub positions: Vec<Point3>,
    /// Per-vertex unit normals; empty means "compute flat normals".
    pub normals: Vec<Vec3>,
    /// Counter-clockwise triangles seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        BoundingBox::from_points(&self.positions)
    }

    /// Edges used by exactly one triangle (by vertex index). For meshes built face by
    /// face (breps, solids) these are the visible edges.
    pub fn boundary_edges(&self) -> Vec<[u32; 2]> {
        use std::collections::HashMap;
        let mut count: HashMap<(u32, u32), (u32, [u32; 2])> = HashMap::new();
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let key = (a.min(b), a.max(b));
                count.entry(key).or_insert((0, [a, b])).0 += 1;
            }
        }
        let mut out: Vec<[u32; 2]> = count
            .into_values()
            .filter(|(n, _)| *n == 1)
            .map(|(_, e)| e)
            .collect();
        out.sort_unstable();
        out
    }

    /// Append another mesh, re-indexing its triangles.
    pub fn append(&mut self, other: &Mesh) {
        let base = self.positions.len() as u32;
        let keep_normals = self.normals.len() == self.positions.len()
            && other.normals.len() == other.positions.len();
        self.positions.extend_from_slice(&other.positions);
        if keep_normals {
            self.normals.extend_from_slice(&other.normals);
        } else {
            self.normals.clear();
        }
        self.triangles
            .extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
    }

    /// Split into pieces whose triangles share vertices (the faces of a box built
    /// face by face, the parts of a joined mesh).
    pub fn components(&self) -> Vec<Mesh> {
        let n = self.positions.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], mut i: usize) -> usize {
            while p[i] != i {
                p[i] = p[p[i]];
                i = p[i];
            }
            i
        }
        for t in &self.triangles {
            let a = find(&mut parent, t[0] as usize);
            for &v in &t[1..] {
                let b = find(&mut parent, v as usize);
                parent[b] = a;
            }
        }
        let mut groups: std::collections::BTreeMap<usize, Vec<[u32; 3]>> = Default::default();
        for t in &self.triangles {
            let r = find(&mut parent, t[0] as usize);
            groups.entry(r).or_default().push(*t);
        }
        let has_normals = self.normals.len() == n;
        groups
            .into_values()
            .map(|tris| {
                let mut map = std::collections::HashMap::new();
                let mut m = Mesh::default();
                for t in tris {
                    let nt = t.map(|v| {
                        *map.entry(v).or_insert_with(|| {
                            m.positions.push(self.positions[v as usize]);
                            if has_normals {
                                m.normals.push(self.normals[v as usize]);
                            }
                            (m.positions.len() - 1) as u32
                        })
                    });
                    m.triangles.push(nt);
                }
                m
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_length_and_midpoint() {
        let l = LineCurve::new(Point3::ORIGIN, Point3::new(3.0, 4.0, 0.0));
        assert!((l.length() - 5.0).abs() < 1e-12);
        assert!(l
            .point_at(0.5)
            .almost_eq(Point3::new(1.5, 2.0, 0.0), Tolerance::default()));
    }

    #[test]
    fn degenerate_line() {
        let l = LineCurve::new(Point3::ORIGIN, Point3::new(0.0005, 0.0, 0.0));
        assert!(l.is_degenerate(Tolerance::default()));
    }

    #[test]
    fn mesh_append_reindexes() {
        let tri = Mesh {
            positions: vec![
                Point3::ORIGIN,
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            normals: vec![],
            triangles: vec![[0, 1, 2]],
        };
        let mut m = tri.clone();
        m.append(&tri);
        assert_eq!(m.triangles, vec![[0, 1, 2], [3, 4, 5]]);
    }

    #[test]
    fn cross_product() {
        let z = Vec3::new(1.0, 0.0, 0.0).cross(Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(z, Vec3::new(0.0, 0.0, 1.0));
    }
}
