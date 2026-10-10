//! Non-uniform rational B-spline curves.
//!
//! # Knot convention
//!
//! Knots follow openNURBS: a curve with `n` control points and degree `p` stores
//! `n + p - 1` knots, i.e. the textbook knot vector (`n + p + 1` values) **without**
//! its first and last value, which never influence the curve. A clamped cubic with
//! four points therefore has the knots `[0, 0, 0, 1, 1, 1]`.
//! [`NurbsCurve::full_knots`] gives the textbook vector used by the evaluators.

use crate::{BoundingBox, CircleArc, Plane, Point3, Seg, Vec3, Xform};
use std::f64::consts::{FRAC_PI_2, TAU};

/// A NURBS curve: degree, control points, weights (all 1 for non-rational curves)
/// and openNURBS-style knots (see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct NurbsCurve {
    pub degree: usize,
    pub points: Vec<Point3>,
    pub weights: Vec<f64>,
    /// `points.len() + degree - 1` non-decreasing values.
    pub knots: Vec<f64>,
}

/// A rational quadratic Bézier piece: three control points with weights.
pub type Bezier2 = [(Point3, f64); 3];

impl NurbsCurve {
    /// Checked constructor. Returns `None` when the counts do not match, the knots
    /// decrease, a weight is not positive or the domain is empty.
    pub fn new(
        degree: usize,
        points: Vec<Point3>,
        weights: Vec<f64>,
        knots: Vec<f64>,
    ) -> Option<NurbsCurve> {
        let n = points.len();
        let ok = degree >= 1
            && n > degree
            && weights.len() == n
            && knots.len() == n + degree - 1
            && knots.windows(2).all(|w| w[1] >= w[0])
            && weights.iter().all(|w| *w > 0.0)
            && knots[n - 1] > knots[degree - 1];
        ok.then_some(NurbsCurve {
            degree,
            points,
            weights,
            knots,
        })
    }

    /// Textbook knot vector (`n + p + 1` values): the stored knots with the first
    /// and last one repeated.
    pub fn full_knots(&self) -> Vec<f64> {
        let mut k = Vec::with_capacity(self.knots.len() + 2);
        k.push(self.knots[0]);
        k.extend_from_slice(&self.knots);
        k.push(*self.knots.last().expect("knots"));
        k
    }

    /// Parameter domain `[t0, t1]`.
    pub fn domain(&self) -> (f64, f64) {
        (
            self.knots[self.degree - 1],
            self.knots[self.points.len() - 1],
        )
    }

    pub fn is_rational(&self) -> bool {
        self.weights.iter().any(|w| (w - 1.0).abs() > 1e-12)
    }

    pub fn start(&self) -> Point3 {
        self.point_at(self.domain().0)
    }

    pub fn end(&self) -> Point3 {
        self.point_at(self.domain().1)
    }

    /// True when the curve starts where it ends.
    pub fn is_closed(&self) -> bool {
        self.start().distance_to(self.end()) < 1e-9
    }

    /// Point at parameter `t` (clamped to the domain), by de Boor's algorithm in
    /// homogeneous coordinates.
    pub fn point_at(&self, t: f64) -> Point3 {
        let (t0, t1) = self.domain();
        let t = t.clamp(t0, t1);
        let u = self.full_knots();
        let p = self.degree;
        let span = find_span(self.points.len(), p, t, &u);
        let mut d: Vec<[f64; 4]> = (0..=p)
            .map(|j| {
                let i = span - p + j;
                let (q, w) = (self.points[i], self.weights[i]);
                [q.x * w, q.y * w, q.z * w, w]
            })
            .collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = span - p + j;
                let den = u[i + p + 1 - r] - u[i];
                let a = if den.abs() < 1e-300 {
                    0.0
                } else {
                    (t - u[i]) / den
                };
                let prev = d[j - 1];
                for (v, q) in d[j].iter_mut().zip(prev) {
                    *v = (1.0 - a) * q + a * *v;
                }
            }
        }
        let h = d[p];
        Point3::new(h[0] / h[3], h[1] / h[3], h[2] / h[3])
    }

    /// Unit tangent at `t` (central difference).
    pub fn tangent_at(&self, t: f64) -> Vec3 {
        let (t0, t1) = self.domain();
        let h = (t1 - t0) * 1e-6;
        let a = (t - h).max(t0);
        let b = (t + h).min(t1);
        (self.point_at(b) - self.point_at(a))
            .normalized()
            .unwrap_or(Vec3::X)
    }

    /// Distinct knot spans inside the domain.
    fn spans(&self) -> Vec<(f64, f64)> {
        let (t0, t1) = self.domain();
        let mut out = Vec::new();
        let mut a = t0;
        for &k in &self.knots[self.degree - 1..self.points.len()] {
            if k > a + 1e-14 && k <= t1 {
                out.push((a, k));
                a = k;
            }
        }
        if out.is_empty() {
            out.push((t0, t1));
        }
        out
    }

    /// Polyline through the curve: `per_span` samples per knot span (one for
    /// degree 1, so polylines stay exact). Includes both ends.
    pub fn sample(&self, per_span: usize) -> Vec<Point3> {
        let per = if self.degree == 1 { 1 } else { per_span.max(1) };
        let mut out = vec![self.start()];
        for (a, b) in self.spans() {
            for i in 1..=per {
                out.push(self.point_at(a + (b - a) * i as f64 / per as f64));
            }
        }
        out
    }

    /// Display polyline (24 samples per span).
    pub fn points(&self) -> Vec<Point3> {
        self.sample(24)
    }

    /// Length, from a fine polyline.
    pub fn length(&self) -> f64 {
        self.sample(128)
            .windows(2)
            .map(|w| w[0].distance_to(w[1]))
            .sum()
    }

    pub fn bounding_box(&self) -> BoundingBox {
        BoundingBox::from_points(&self.points()).expect("points")
    }

    /// Points at the given distances along the curve (measured on a fine
    /// polyline; the points themselves lie exactly on the curve).
    pub fn points_at_lengths(&self, lengths: &[f64]) -> Vec<Point3> {
        let mut params = Vec::new();
        for (a, b) in self.spans() {
            let per = if self.degree == 1 { 1 } else { 128 };
            for i in 0..per {
                params.push(a + (b - a) * i as f64 / per as f64);
            }
        }
        params.push(self.domain().1);
        let pts: Vec<Point3> = params.iter().map(|t| self.point_at(*t)).collect();
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().expect("len") + w[0].distance_to(w[1]));
        }
        lengths
            .iter()
            .map(|&s| {
                let k = cum.partition_point(|c| *c < s).clamp(1, cum.len() - 1);
                let seg = cum[k] - cum[k - 1];
                let f = if seg > 0.0 {
                    ((s - cum[k - 1]) / seg).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.point_at(params[k - 1] + (params[k] - params[k - 1]) * f)
            })
            .collect()
    }

    /// Transformed copy. Exact for every affine map (also non-uniform scale and
    /// projections): only the control points move.
    pub fn transformed(&self, x: &Xform) -> NurbsCurve {
        NurbsCurve {
            points: self.points.iter().map(|p| x.point(*p)).collect(),
            ..self.clone()
        }
    }

    /// Same curve, opposite direction (same domain).
    pub fn reversed(&self) -> NurbsCurve {
        let (a, b) = (self.knots[0], *self.knots.last().expect("knots"));
        let mut points = self.points.clone();
        let mut weights = self.weights.clone();
        points.reverse();
        weights.reverse();
        let knots = self.knots.iter().rev().map(|k| a + b - k).collect();
        NurbsCurve {
            degree: self.degree,
            points,
            weights,
            knots,
        }
    }

    /// Clamped uniform curve with the given control points; the degree is lowered
    /// when there are too few points (Rhino's `Curve`).
    pub fn clamped_uniform(points: &[Point3], degree: usize) -> Option<NurbsCurve> {
        let n = points.len();
        if n < 2 {
            return None;
        }
        let p = degree.clamp(1, n - 1);
        let spans = n - p;
        let mut knots = vec![0.0; p];
        for i in 1..spans {
            knots.push(i as f64);
        }
        knots.extend(std::iter::repeat_n(spans as f64, p));
        NurbsCurve::new(p, points.to_vec(), vec![1.0; n], knots)
    }

    /// Curve of the given degree (lowered for few points) passing through all
    /// `points`, with chord-length parameters and averaged knots (The NURBS Book,
    /// §9.2.1). The parameter runs from 0 to the polygon length.
    pub fn interpolate(points: &[Point3], degree: usize) -> Option<NurbsCurve> {
        let n = points.len();
        if n < 2 {
            return None;
        }
        let p = degree.clamp(1, n - 1);
        let total: f64 = points.windows(2).map(|w| w[0].distance_to(w[1])).sum();
        if total < 1e-12 {
            return None;
        }
        let mut params = vec![0.0];
        let mut acc = 0.0;
        for w in points.windows(2) {
            acc += w[0].distance_to(w[1]);
            params.push(acc);
        }
        if params.windows(2).any(|w| w[1] - w[0] < 1e-12) {
            return None; // repeated points
        }
        // Textbook knots: p+1 zeros, averaged interior, p+1 times the end.
        let mut full = vec![0.0; p + 1];
        for j in 1..n - p {
            full.push(params[j..j + p].iter().sum::<f64>() / p as f64);
        }
        full.extend(std::iter::repeat_n(total, p + 1));
        // Collocation matrix.
        let mut a = vec![vec![0.0; n]; n];
        for (k, &t) in params.iter().enumerate() {
            let span = find_span(n, p, t, &full);
            let b = basis(span, t, p, &full);
            for (j, v) in b.iter().enumerate() {
                a[k][span - p + j] = *v;
            }
        }
        let rhs: Vec<[f64; 3]> = points.iter().map(|q| [q.x, q.y, q.z]).collect();
        let sol = solve(a, rhs)?;
        let ctrl = sol.iter().map(|c| Point3::new(c[0], c[1], c[2])).collect();
        NurbsCurve::new(p, ctrl, vec![1.0; n], full[1..full.len() - 1].to_vec())
    }

    /// Degree-2 curve joining rational quadratic Bézier pieces end to end (double
    /// interior knots). The parameter of piece `i` runs over `[i, i + 1]`.
    pub fn from_beziers2(pieces: &[Bezier2]) -> Option<NurbsCurve> {
        let first = pieces.first()?;
        let mut points = vec![first[0].0];
        let mut weights = vec![first[0].1];
        let mut knots = vec![0.0, 0.0];
        for (i, b) in pieces.iter().enumerate() {
            for (q, w) in &b[1..] {
                points.push(*q);
                weights.push(*w);
            }
            let k = (i + 1) as f64;
            knots.extend([k, k]);
        }
        NurbsCurve::new(2, points, weights, knots)
    }

    /// Exact rational representation of a circle or arc (pieces of at most 90°),
    /// starting at the arc start and running the same way.
    pub fn from_arc(arc: &CircleArc) -> NurbsCurve {
        NurbsCurve::from_beziers2(&arc_beziers(arc)).expect("non-empty arc")
    }

    /// Exact rational curve of a chain of lines and arcs (lines become degree-2
    /// pieces with a middle control point).
    pub fn from_segs(segs: &[Seg]) -> Option<NurbsCurve> {
        let mut pieces = Vec::new();
        for s in segs {
            match s {
                Seg::Line(a, b) => pieces.push([(*a, 1.0), (a.mid(*b), 1.0), (*b, 1.0)]),
                Seg::Arc(c) => pieces.extend(arc_beziers(c)),
            }
        }
        NurbsCurve::from_beziers2(&pieces)
    }

    /// Exact ellipse on `plane` (centre at its origin) with semi-axes `ra` along
    /// the plane's x axis and `rb` along its y axis.
    pub fn ellipse(plane: &Plane, ra: f64, rb: f64) -> NurbsCurve {
        let unit = NurbsCurve::from_arc(&CircleArc::circle(*plane, 1.0));
        unit.transformed(&Xform::scale_axes(plane, ra, rb, 1.0))
    }
}

/// Rational quadratic Bézier pieces of an arc, at most a quarter turn each.
fn arc_beziers(arc: &CircleArc) -> Vec<Bezier2> {
    let k = ((arc.sweep / FRAC_PI_2 - 1e-9).ceil() as usize).clamp(1, 4);
    let theta = arc.sweep.min(TAU) / k as f64;
    let w = (theta / 2.0).cos();
    (0..k)
        .map(|i| {
            let a0 = theta * i as f64;
            let am = a0 + theta / 2.0;
            let r = arc.radius / w;
            let mid = arc.plane.point_at(r * am.cos(), r * am.sin(), 0.0);
            [
                (arc.point_at_angle(a0), 1.0),
                (mid, w),
                (arc.point_at_angle(a0 + theta), 1.0),
            ]
        })
        .collect()
}

/// Knot span index for `t` (The NURBS Book A2.1) with `n` control points.
fn find_span(n: usize, p: usize, t: f64, u: &[f64]) -> usize {
    let last = n - 1;
    if t >= u[last + 1] {
        // At the end of the domain: last non-empty span.
        let mut s = last;
        while s > p && u[s] >= u[s + 1] {
            s -= 1;
        }
        return s;
    }
    if t <= u[p] {
        let mut s = p;
        while s < last && u[s + 1] <= u[s] {
            s += 1;
        }
        return s;
    }
    let (mut lo, mut hi) = (p, last + 1);
    let mut mid = (lo + hi) / 2;
    while t < u[mid] || t >= u[mid + 1] {
        if t < u[mid] {
            hi = mid;
        } else {
            lo = mid;
        }
        mid = (lo + hi) / 2;
    }
    mid
}

/// Non-zero basis functions `N[span-p..=span]` at `t` (The NURBS Book A2.2).
fn basis(span: usize, t: f64, p: usize, u: &[f64]) -> Vec<f64> {
    let mut n = vec![0.0; p + 1];
    let mut left = vec![0.0; p + 1];
    let mut right = vec![0.0; p + 1];
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = t - u[span + 1 - j];
        right[j] = u[span + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            let den = right[r + 1] + left[j - r];
            let tmp = if den.abs() < 1e-300 { 0.0 } else { n[r] / den };
            n[r] = saved + right[r + 1] * tmp;
            saved = left[j - r] * tmp;
        }
        n[j] = saved;
    }
    n
}

/// Solve `a · x = b` (dense Gaussian elimination with partial pivoting) for three
/// right-hand sides at once.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<[f64; 3]>) -> Option<Vec<[f64; 3]>> {
    let n = a.len();
    for col in 0..n {
        let piv = (col..n).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))?;
        if a[piv][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let (pivot_row, pivot_b) = (a[col].clone(), b[col]);
        for row in col + 1..n {
            let f = a[row][col] / pivot_row[col];
            if f == 0.0 {
                continue;
            }
            for (v, q) in a[row][col..].iter_mut().zip(&pivot_row[col..]) {
                *v -= f * q;
            }
            for (v, q) in b[row].iter_mut().zip(pivot_b) {
                *v -= f * q;
            }
        }
    }
    let mut x = vec![[0.0; 3]; n];
    for row in (0..n).rev() {
        for k in 0..3 {
            let s: f64 = (row + 1..n).map(|j| a[row][j] * x[j][k]).sum();
            x[row][k] = (b[row][k] - s) / a[row][row];
        }
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_circle_points_lie_on_the_circle() {
        let plane = Plane::from_normal(Point3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 1.0, 0.5));
        let c = NurbsCurve::from_arc(&CircleArc::circle(plane, 7.0));
        assert_eq!(c.points.len(), 9);
        assert_eq!(c.knots.len(), 10);
        assert!(c.is_closed());
        let (t0, t1) = c.domain();
        for i in 0..=200 {
            let p = c.point_at(t0 + (t1 - t0) * i as f64 / 200.0);
            let (u, v, w) = plane.coords(p);
            assert!(((u * u + v * v).sqrt() - 7.0).abs() < 1e-9);
            assert!(w.abs() < 1e-9);
        }
    }

    #[test]
    fn arc_conversion_keeps_ends() {
        let a = CircleArc::from_center_start_end(
            Point3::ORIGIN,
            Point3::new(5.0, 0.0, 0.0),
            Point3::new(-5.0, 0.1, 0.0),
            Vec3::Z,
        )
        .unwrap();
        let c = NurbsCurve::from_arc(&a);
        assert!(c.start().distance_to(a.start()) < 1e-9);
        assert!(c.end().distance_to(a.end()) < 1e-9);
        assert!((c.length() - a.radius * a.sweep).abs() < 1e-3);
    }

    #[test]
    fn ellipse_points_satisfy_the_equation() {
        let e = NurbsCurve::ellipse(&Plane::TOP, 10.0, 4.0);
        assert!(e.is_closed());
        let (t0, t1) = e.domain();
        for i in 0..100 {
            let p = e.point_at(t0 + (t1 - t0) * i as f64 / 100.0);
            let f = (p.x / 10.0).powi(2) + (p.y / 4.0).powi(2);
            assert!((f - 1.0).abs() < 1e-9, "{f}");
        }
    }

    #[test]
    fn interpolation_passes_through_points() {
        let pts = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 5.0, 0.0),
            Point3::new(20.0, -3.0, 2.0),
            Point3::new(35.0, 0.0, 0.0),
            Point3::new(40.0, 10.0, -1.0),
        ];
        let c = NurbsCurve::interpolate(&pts, 3).unwrap();
        assert_eq!(c.degree, 3);
        assert_eq!(c.knots.len(), c.points.len() + 2);
        let mut t = 0.0;
        for (i, q) in pts.iter().enumerate() {
            if i > 0 {
                t += pts[i - 1].distance_to(*q);
            }
            assert!(c.point_at(t).distance_to(*q) < 1e-9, "{i}");
        }
    }

    #[test]
    fn clamped_curve_ends_at_end_points_and_lowers_degree() {
        let pts = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(3.0, 2.0, 0.0),
            Point3::new(4.0, 0.0, 0.0),
            Point3::new(6.0, 1.0, 0.0),
        ];
        let c = NurbsCurve::clamped_uniform(&pts, 3).unwrap();
        assert!(c.start().distance_to(pts[0]) < 1e-12);
        assert!(c.end().distance_to(pts[4]) < 1e-12);
        let two = NurbsCurve::clamped_uniform(&pts[..2], 3).unwrap();
        assert_eq!(two.degree, 1);
        assert!(two.point_at(0.5).distance_to(Point3::new(0.5, 1.0, 0.0)) < 1e-12);
    }

    #[test]
    fn reverse_and_non_uniform_scale() {
        let c = NurbsCurve::from_arc(&CircleArc::circle(Plane::TOP, 1.0));
        let r = c.reversed();
        let (t0, t1) = c.domain();
        let t = t0 + 0.3 * (t1 - t0);
        assert!(c.point_at(t).distance_to(r.point_at(t0 + t1 - t)) < 1e-12);
        let s = c.transformed(&Xform::scale_axes(&Plane::TOP, 3.0, 1.0, 1.0));
        let p = s.point_at(t);
        assert!(((p.x / 3.0).powi(2) + p.y.powi(2) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn points_at_lengths_on_a_circle() {
        let c = NurbsCurve::from_arc(&CircleArc::circle(Plane::TOP, 10.0));
        let quarter = std::f64::consts::TAU * 10.0 / 4.0;
        let p = c.points_at_lengths(&[0.0, quarter, 2.0 * quarter]);
        assert!(p[0].distance_to(Point3::new(10.0, 0.0, 0.0)) < 1e-9);
        assert!(
            p[1].distance_to(Point3::new(0.0, 10.0, 0.0)) < 1e-3,
            "{}",
            p[1]
        );
        assert!(
            p[2].distance_to(Point3::new(-10.0, 0.0, 0.0)) < 1e-3,
            "{}",
            p[2]
        );
        // On the curve exactly.
        assert!(p
            .iter()
            .all(|q| (q.distance_to(Point3::ORIGIN) - 10.0).abs() < 1e-9));
    }
}
