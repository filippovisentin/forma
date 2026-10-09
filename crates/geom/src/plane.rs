//! Planes and construction planes (CPlanes).

use crate::{Point3, Vec3};

/// An oriented plane with an orthonormal frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub origin: Point3,
    pub x: Vec3,
    pub y: Vec3,
    /// Normal, `x × y`.
    pub z: Vec3,
}

impl Plane {
    /// World XY (Rhino's Top CPlane).
    pub const TOP: Plane = Plane {
        origin: Point3::ORIGIN,
        x: Vec3::X,
        y: Vec3::Y,
        z: Vec3::Z,
    };
    /// World XZ seen from the front (Rhino's Front CPlane): normal −Y.
    pub const FRONT: Plane = Plane {
        origin: Point3::ORIGIN,
        x: Vec3::X,
        y: Vec3::Z,
        z: Vec3::new(0.0, -1.0, 0.0),
    };
    /// World YZ seen from the right (Rhino's Right CPlane): normal +X.
    pub const RIGHT: Plane = Plane {
        origin: Point3::ORIGIN,
        x: Vec3::Y,
        y: Vec3::Z,
        z: Vec3::X,
    };

    /// Plane through `origin` with normal `normal`. The x axis follows Forma's
    /// convention: world X projected on the plane, or world Y when X is parallel to
    /// the normal. This matches the Top, Front and Right CPlanes.
    pub fn from_normal(origin: Point3, normal: Vec3) -> Plane {
        let z = normal.normalized().unwrap_or(Vec3::Z);
        let px = Vec3::X - z * Vec3::X.dot(z);
        let x = match px.normalized() {
            Some(x) if px.length() > 1e-9 => x,
            _ => (Vec3::Y - z * Vec3::Y.dot(z))
                .normalized()
                .unwrap_or(Vec3::Y),
        };
        let y = z.cross(x);
        Plane { origin, x, y, z }
    }

    /// Same frame, new origin.
    pub fn moved_to(self, origin: Point3) -> Plane {
        Plane { origin, ..self }
    }

    /// World point from plane coordinates.
    pub fn point_at(&self, u: f64, v: f64, w: f64) -> Point3 {
        self.origin + self.x * u + self.y * v + self.z * w
    }

    /// Plane coordinates (u, v, w) of a world point.
    pub fn coords(&self, p: Point3) -> (f64, f64, f64) {
        let d = p - self.origin;
        (d.dot(self.x), d.dot(self.y), d.dot(self.z))
    }

    /// Closest point on the plane.
    pub fn project(&self, p: Point3) -> Point3 {
        let (u, v, _) = self.coords(p);
        self.point_at(u, v, 0.0)
    }

    /// Intersection with the ray `origin + t·dir`, `t` may be negative.
    pub fn intersect_line(&self, origin: Point3, dir: Vec3) -> Option<Point3> {
        let denom = dir.dot(self.z);
        if denom.abs() < 1e-12 {
            return None;
        }
        let t = (self.origin - origin).dot(self.z) / denom;
        Some(origin + dir * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_normal_matches_standard_cplanes() {
        for p in [Plane::TOP, Plane::FRONT, Plane::RIGHT] {
            let q = Plane::from_normal(Point3::ORIGIN, p.z);
            assert!((q.x - p.x).length() < 1e-12, "{p:?}");
            assert!((q.y - p.y).length() < 1e-12, "{p:?}");
        }
    }

    #[test]
    fn coords_round_trip() {
        let p = Plane::from_normal(Point3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 1.0, 1.0));
        let w = p.point_at(4.0, -5.0, 6.0);
        let (u, v, n) = p.coords(w);
        assert!((u - 4.0).abs() < 1e-9 && (v + 5.0).abs() < 1e-9 && (n - 6.0).abs() < 1e-9);
    }

    #[test]
    fn ray_hits_plane() {
        let hit = Plane::FRONT
            .intersect_line(Point3::new(5.0, -10.0, 7.0), Vec3::new(0.0, 1.0, 0.0))
            .unwrap();
        assert_eq!(hit, Point3::new(5.0, 0.0, 7.0));
    }
}
