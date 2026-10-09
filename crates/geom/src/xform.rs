//! Affine transformations.

use crate::{Point3, Vec3};

/// Affine transform: `p' = M·p + t` (3×3 linear part `m`, rows; translation `t`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xform {
    pub m: [[f64; 3]; 3],
    pub t: Vec3,
}

impl Xform {
    pub const IDENTITY: Xform = Xform {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: Vec3::new(0.0, 0.0, 0.0),
    };

    pub fn translation(v: Vec3) -> Xform {
        Xform {
            t: v,
            ..Self::IDENTITY
        }
    }

    /// Linear map about a fixed `center`.
    fn about(center: Point3, m: [[f64; 3]; 3]) -> Xform {
        let lin = Xform {
            m,
            t: Vec3::new(0.0, 0.0, 0.0),
        };
        let c = center.to_vec();
        Xform {
            m,
            t: c - lin.vector(c),
        }
    }

    /// Rotation by `angle` radians about the axis through `center` (right hand rule).
    pub fn rotation(center: Point3, axis: Vec3, angle: f64) -> Xform {
        let a = axis.normalized().unwrap_or(Vec3::Z);
        let (s, c) = angle.sin_cos();
        let k = 1.0 - c;
        let m = [
            [
                c + a.x * a.x * k,
                a.x * a.y * k - a.z * s,
                a.x * a.z * k + a.y * s,
            ],
            [
                a.y * a.x * k + a.z * s,
                c + a.y * a.y * k,
                a.y * a.z * k - a.x * s,
            ],
            [
                a.z * a.x * k - a.y * s,
                a.z * a.y * k + a.x * s,
                c + a.z * a.z * k,
            ],
        ];
        Self::about(center, m)
    }

    /// Uniform scale about `center`.
    pub fn scale(center: Point3, factor: f64) -> Xform {
        let f = factor;
        Self::about(center, [[f, 0.0, 0.0], [0.0, f, 0.0], [0.0, 0.0, f]])
    }

    /// Reflection in the plane through `point` with normal `normal`.
    pub fn mirror(point: Point3, normal: Vec3) -> Xform {
        let n = normal.normalized().unwrap_or(Vec3::X);
        let m = [
            [1.0 - 2.0 * n.x * n.x, -2.0 * n.x * n.y, -2.0 * n.x * n.z],
            [-2.0 * n.y * n.x, 1.0 - 2.0 * n.y * n.y, -2.0 * n.y * n.z],
            [-2.0 * n.z * n.x, -2.0 * n.z * n.y, 1.0 - 2.0 * n.z * n.z],
        ];
        Self::about(point, m)
    }

    pub fn vector(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }

    pub fn point(&self, p: Point3) -> Point3 {
        Point3::ORIGIN + (self.vector(p.to_vec()) + self.t)
    }

    pub fn determinant(&self) -> f64 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// True when the transform flips orientation (mirrors): triangle winding must
    /// be reversed to keep outward normals.
    pub fn flips(&self) -> bool {
        self.determinant() < 0.0
    }

    /// Transform a surface normal. Exact for rigid motions, uniform scale and
    /// mirrors (the transforms Forma uses); renormalised.
    pub fn normal(&self, n: Vec3) -> Vec3 {
        // For orthogonal maps (times a uniform scale) the inverse transpose is
        // proportional to the map itself.
        self.vector(n).normalized().unwrap_or(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point3, b: Point3) -> bool {
        a.distance_to(b) < 1e-9
    }

    #[test]
    fn rotate_quarter_turn_about_point() {
        let x = Xform::rotation(
            Point3::new(1.0, 1.0, 0.0),
            Vec3::Z,
            std::f64::consts::FRAC_PI_2,
        );
        assert!(close(
            x.point(Point3::new(2.0, 1.0, 0.0)),
            Point3::new(1.0, 2.0, 0.0)
        ));
    }

    #[test]
    fn scale_and_translate() {
        let s = Xform::scale(Point3::new(1.0, 0.0, 0.0), 2.0);
        assert!(close(
            s.point(Point3::new(2.0, 1.0, 0.0)),
            Point3::new(3.0, 2.0, 0.0)
        ));
        let t = Xform::translation(Vec3::new(5.0, 0.0, 0.0));
        assert!(close(t.point(Point3::ORIGIN), Point3::new(5.0, 0.0, 0.0)));
    }

    #[test]
    fn mirror_flips_and_keeps_normals_outward() {
        let m = Xform::mirror(Point3::new(10.0, 0.0, 0.0), Vec3::X);
        assert!(close(
            m.point(Point3::new(12.0, 3.0, 1.0)),
            Point3::new(8.0, 3.0, 1.0)
        ));
        assert!(m.flips());
        // A face normal pointing +X at x=12 becomes a face at x=8 pointing −X.
        let n = m.normal(Vec3::X);
        assert!((n - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-9, "{n:?}");
    }
}
