//! Circles and circular arcs.

use crate::{BoundingBox, Plane, Point3, Vec3, Xform};
use std::f64::consts::TAU;

/// A circle (`sweep` = 2π) or a counter-clockwise arc in its plane, starting on the
/// plane's x axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircleArc {
    pub plane: Plane,
    pub radius: f64,
    /// Sweep angle in radians, (0, 2π].
    pub sweep: f64,
}

impl CircleArc {
    pub fn circle(plane: Plane, radius: f64) -> CircleArc {
        CircleArc {
            plane,
            radius,
            sweep: TAU,
        }
    }

    /// Arc around `center` in the plane with normal `normal`, from the direction of
    /// `start` counter-clockwise to the direction of `end`.
    pub fn from_center_start_end(
        center: Point3,
        start: Point3,
        end: Point3,
        normal: Vec3,
    ) -> Option<CircleArc> {
        let base = Plane::from_normal(center, normal);
        let (su, sv, _) = base.coords(start);
        let radius = (su * su + sv * sv).sqrt();
        if radius < 1e-12 {
            return None;
        }
        let x = (base.x * su + base.y * sv).normalized()?;
        let y = base.z.cross(x);
        let plane = Plane {
            origin: center,
            x,
            y,
            z: base.z,
        };
        let (eu, ev, _) = plane.coords(end);
        if eu.abs() < 1e-12 && ev.abs() < 1e-12 {
            return None;
        }
        let mut sweep = ev.atan2(eu);
        if sweep <= 1e-12 {
            sweep += TAU;
        }
        Some(CircleArc {
            plane,
            radius,
            sweep,
        })
    }

    pub fn is_closed(&self) -> bool {
        (self.sweep - TAU).abs() < 1e-9
    }

    pub fn center(&self) -> Point3 {
        self.plane.origin
    }

    pub fn point_at_angle(&self, a: f64) -> Point3 {
        self.plane
            .point_at(self.radius * a.cos(), self.radius * a.sin(), 0.0)
    }

    pub fn start(&self) -> Point3 {
        self.point_at_angle(0.0)
    }

    pub fn end(&self) -> Point3 {
        self.point_at_angle(self.sweep)
    }

    pub fn mid(&self) -> Point3 {
        self.point_at_angle(self.sweep / 2.0)
    }

    /// Polyline approximation with `segments_per_turn` segments for a full circle.
    /// Closed circles repeat the first point at the end.
    pub fn points(&self, segments_per_turn: usize) -> Vec<Point3> {
        let n = ((segments_per_turn as f64 * self.sweep / TAU).ceil() as usize).max(2);
        (0..=n)
            .map(|i| self.point_at_angle(self.sweep * i as f64 / n as f64))
            .collect()
    }

    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::from_points(&self.points(64)).expect("points")
    }

    /// Transformed arc. Supports rigid motions, uniform scale and mirrors (a mirror
    /// reverses the direction, so the arc is re-based to keep it counter-clockwise).
    pub fn transformed(&self, x: &Xform) -> CircleArc {
        let origin = x.point(self.plane.origin);
        let scale = x.vector(self.plane.x).length();
        let px = x.vector(self.plane.x).normalized().unwrap_or(Vec3::X);
        let py = x.vector(self.plane.y).normalized().unwrap_or(Vec3::Y);
        let radius = self.radius * scale;
        if x.flips() {
            // Mirrored arc runs clockwise around px×py; express it counter-clockwise
            // around the flipped normal, starting at the old end.
            let z = py.cross(px);
            let end_dir = (px * self.sweep.cos() + py * self.sweep.sin())
                .normalized()
                .unwrap_or(px);
            let y = z.cross(end_dir);
            CircleArc {
                plane: Plane {
                    origin,
                    x: end_dir,
                    y,
                    z,
                },
                radius,
                sweep: self.sweep,
            }
        } else {
            CircleArc {
                plane: Plane {
                    origin,
                    x: px,
                    y: py,
                    z: px.cross(py),
                },
                radius,
                sweep: self.sweep,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_arc_from_points() {
        let a = CircleArc::from_center_start_end(
            Point3::ORIGIN,
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 5.0, 0.0),
            Vec3::Z,
        )
        .unwrap();
        assert!((a.radius - 10.0).abs() < 1e-12);
        assert!((a.sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(a.end().distance_to(Point3::new(0.0, 10.0, 0.0)) < 1e-9);
    }

    #[test]
    fn mirrored_arc_keeps_its_points() {
        let a = CircleArc::from_center_start_end(
            Point3::new(5.0, 0.0, 0.0),
            Point3::new(6.0, 0.0, 0.0),
            Point3::new(5.0, 1.0, 0.0),
            Vec3::Z,
        )
        .unwrap();
        let m = Xform::mirror(Point3::ORIGIN, Vec3::X);
        let b = a.transformed(&m);
        // Same set of points, mirrored: endpoints swap.
        assert!(b.start().distance_to(m.point(a.end())) < 1e-9);
        assert!(b.end().distance_to(m.point(a.start())) < 1e-9);
        assert!(b.mid().distance_to(m.point(a.mid())) < 1e-9);
    }
}
