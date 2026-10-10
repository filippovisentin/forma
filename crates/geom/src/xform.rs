//! Affine transformations.

use crate::{Plane, Point3, Vec3};

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

    /// Transform a surface normal with the inverse transpose of the linear part
    /// (exact for every affine map, also non-uniform scale); renormalised. For a
    /// projection onto a plane, normals become the plane normal.
    pub fn normal(&self, n: Vec3) -> Vec3 {
        // Cofactor matrix = det · M⁻ᵀ; it also exists for singular maps.
        let m = &self.m;
        let c = |r0: usize, r1: usize, c0: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let cof = [
            [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
            [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
            [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
        ];
        let lin = Xform {
            m: cof,
            t: Vec3::new(0.0, 0.0, 0.0),
        };
        let v = lin.vector(n);
        let v = if self.determinant() < 0.0 { -v } else { v };
        v.normalized().unwrap_or(n)
    }

    /// Scale by `sx`, `sy`, `sz` along the axes of `plane`, about its origin.
    pub fn scale_axes(plane: &Plane, sx: f64, sy: f64, sz: f64) -> Xform {
        let mut m = [[0.0; 3]; 3];
        for (a, s) in [(plane.x, sx), (plane.y, sy), (plane.z, sz)] {
            let a = [a.x, a.y, a.z];
            for (i, row) in m.iter_mut().enumerate() {
                for (j, v) in row.iter_mut().enumerate() {
                    *v += s * a[i] * a[j];
                }
            }
        }
        Self::about(plane.origin, m)
    }

    /// Shear: every point moves along `dir` by `factor` times its distance from
    /// `origin` measured along `across` (`p' = p + dir·factor·((p − origin)·across)`).
    /// `dir` and `across` are normalised; they should be perpendicular.
    pub fn shear(origin: Point3, dir: Vec3, across: Vec3, factor: f64) -> Xform {
        let d = dir.normalized().unwrap_or(Vec3::X);
        let n = across.normalized().unwrap_or(Vec3::Y);
        let (d, n) = ([d.x, d.y, d.z], [n.x, n.y, n.z]);
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = f64::from(u8::from(i == j)) + factor * d[i] * n[j];
            }
        }
        Self::about(origin, m)
    }

    /// Orthogonal projection onto `plane`.
    pub fn projection(plane: &Plane) -> Xform {
        Self::scale_axes(plane, 1.0, 1.0, 0.0)
    }

    /// `self` followed by `next`.
    pub fn then(&self, next: &Xform) -> Xform {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| next.m[i][k] * self.m[k][j]).sum();
            }
        }
        Xform {
            m,
            t: next.vector(self.t) + next.t,
        }
    }

    /// True when the map keeps shapes (rotation, mirror and uniform scale, plus a
    /// translation): circles stay circles. False for non-uniform scale,
    /// projections and shears.
    pub fn is_similarity(&self) -> bool {
        let cols: Vec<Vec3> = [Vec3::X, Vec3::Y, Vec3::Z]
            .iter()
            .map(|e| self.vector(*e))
            .collect();
        let s2 = cols[0].dot(cols[0]);
        if s2 < 1e-24 {
            return false;
        }
        let tol = 1e-9 * s2;
        (cols[1].dot(cols[1]) - s2).abs() < tol
            && (cols[2].dot(cols[2]) - s2).abs() < tol
            && cols[0].dot(cols[1]).abs() < tol
            && cols[0].dot(cols[2]).abs() < tol
            && cols[1].dot(cols[2]).abs() < tol
    }

    /// Smallest rotation about the origin turning direction `a` into direction `b`.
    pub fn rotation_between(a: Vec3, b: Vec3) -> Xform {
        let (Some(a), Some(b)) = (a.normalized(), b.normalized()) else {
            return Self::IDENTITY;
        };
        let axis = a.cross(b);
        let cos = a.dot(b).clamp(-1.0, 1.0);
        if axis.length() < 1e-12 {
            if cos > 0.0 {
                return Self::IDENTITY;
            }
            // Opposite: half turn about any axis perpendicular to `a`.
            let perp = Plane::from_normal(Point3::ORIGIN, a).x;
            return Self::rotation(Point3::ORIGIN, perp, std::f64::consts::PI);
        }
        Self::rotation(Point3::ORIGIN, axis, axis.length().atan2(cos))
    }

    /// Two-point orient: moves `a1` to `b1` and turns the direction `a1→a2` into
    /// `b1→b2` with the smallest rotation; with `scale`, also scales uniformly by
    /// `|b2 − b1| / |a2 − a1|`. `None` when a reference pair is degenerate.
    pub fn orient(a1: Point3, a2: Point3, b1: Point3, b2: Point3, scale: bool) -> Option<Xform> {
        let (da, db) = (a2 - a1, b2 - b1);
        if da.length() < 1e-12 || db.length() < 1e-12 {
            return None;
        }
        let f = if scale {
            db.length() / da.length()
        } else {
            1.0
        };
        let x = Self::translation(-a1.to_vec())
            .then(&Self::rotation_between(da, db))
            .then(&Self::scale(Point3::ORIGIN, f))
            .then(&Self::translation(b1.to_vec()));
        Some(x)
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
    fn non_uniform_scale_and_similarity() {
        let s = Xform::scale_axes(
            &Plane::TOP.moved_to(Point3::new(1.0, 0.0, 0.0)),
            2.0,
            1.0,
            1.0,
        );
        assert!(close(
            s.point(Point3::new(2.0, 3.0, 4.0)),
            Point3::new(3.0, 3.0, 4.0)
        ));
        assert!(!s.is_similarity());
        assert!(Xform::rotation(Point3::ORIGIN, Vec3::Z, 0.3).is_similarity());
        assert!(Xform::mirror(Point3::ORIGIN, Vec3::X).is_similarity());
        // Normal of the plane x + y = 0 under x-scale 2: (1,1,0) → (0.5,1,0).
        let n = s.normal(Vec3::new(1.0, 1.0, 0.0));
        let e = Vec3::new(0.5, 1.0, 0.0).normalized().unwrap();
        assert!((n - e).length() < 1e-9, "{n:?}");
    }

    #[test]
    fn orient_two_points() {
        let a1 = Point3::new(0.0, 0.0, 0.0);
        let a2 = Point3::new(10.0, 0.0, 0.0);
        let b1 = Point3::new(5.0, 5.0, 5.0);
        let b2 = Point3::new(5.0, 5.0, 25.0);
        let x = Xform::orient(a1, a2, b1, b2, false).unwrap();
        assert!(close(x.point(a1), b1));
        assert!(close(x.point(a2), Point3::new(5.0, 5.0, 15.0)));
        assert!(x.is_similarity() && !x.flips());
        let s = Xform::orient(a1, a2, b1, b2, true).unwrap();
        assert!(close(s.point(a2), b2));
        // Opposite directions still work.
        let o = Xform::orient(a1, a2, a1, Point3::new(-3.0, 0.0, 0.0), false).unwrap();
        assert!(close(o.point(a2), Point3::new(-10.0, 0.0, 0.0)));
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

    #[test]
    fn shear_moves_points_by_height() {
        let x = Xform::shear(Point3::new(1.0, 1.0, 0.0), Vec3::X, Vec3::Y, 0.5);
        let p = x.point(Point3::new(1.0, 3.0, 0.0));
        assert!(p.distance_to(Point3::new(2.0, 3.0, 0.0)) < 1e-12);
        let q = x.point(Point3::new(5.0, 1.0, 7.0));
        assert!(q.distance_to(Point3::new(5.0, 1.0, 7.0)) < 1e-12);
        assert!(!x.is_similarity());
    }
}
