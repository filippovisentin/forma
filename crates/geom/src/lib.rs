//! Geometry primitives for Forma.
//!
//! This crate knows nothing about documents, commands or UI. The NURBS kernel
//! (curvo / truck / OpenCascade, see `docs/spikes/S2-kernel.md`) will live behind
//! the types defined here.

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

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
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
    fn cross_product() {
        let z = Vec3::new(1.0, 0.0, 0.0).cross(Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(z, Vec3::new(0.0, 0.0, 1.0));
    }
}
