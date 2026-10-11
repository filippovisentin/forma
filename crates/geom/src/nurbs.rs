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
/// Homogeneous control point (x·w, y·w, z·w, w).
type Hom = [f64; 4];

fn hom(p: Point3, w: f64) -> Hom {
    [p.x * w, p.y * w, p.z * w, w]
}

fn lerp_h(a: Hom, b: Hom, t: f64) -> Hom {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

impl NurbsCurve {
    fn homogeneous(&self) -> Vec<Hom> {
        self.points
            .iter()
            .zip(&self.weights)
            .map(|(p, w)| hom(*p, *w))
            .collect()
    }

    /// Curve from textbook knots and homogeneous points.
    fn from_hom(degree: usize, pw: &[Hom], full: &[f64]) -> Option<NurbsCurve> {
        let points = pw
            .iter()
            .map(|h| Point3::new(h[0] / h[3], h[1] / h[3], h[2] / h[3]))
            .collect();
        let weights = pw.iter().map(|h| h[3]).collect();
        NurbsCurve::new(degree, points, weights, full[1..full.len() - 1].to_vec())
    }

    /// How many times `t` is a knot (within 1e-12 of the domain size).
    fn multiplicity(&self, t: f64) -> usize {
        let (a, b) = self.domain();
        let eps = (b - a).abs() * 1e-12;
        self.knots.iter().filter(|k| (**k - t).abs() <= eps).count()
    }

    /// The same curve with the knot `t` inserted `times` more times (Boehm's
    /// algorithm, The NURBS Book A5.1): exact, only the representation changes.
    /// The total multiplicity is capped at the degree.
    pub fn with_knot(&self, t: f64, times: usize) -> NurbsCurve {
        let p = self.degree;
        let (a, b) = self.domain();
        if t <= a || t >= b {
            return self.clone();
        }
        let room = p.saturating_sub(self.multiplicity(t));
        let mut u = self.full_knots();
        let mut pw = self.homogeneous();
        for _ in 0..times.min(room) {
            let n = pw.len();
            let k = find_span(n, p, t, &u);
            let mut q = Vec::with_capacity(n + 1);
            for i in 0..=n {
                if i + p <= k {
                    q.push(pw[i]);
                } else if i > k {
                    q.push(pw[i - 1]);
                } else {
                    let den = u[i + p] - u[i];
                    let alpha = if den.abs() < 1e-300 {
                        0.0
                    } else {
                        (t - u[i]) / den
                    };
                    q.push(lerp_h(pw[i - 1], pw[i], alpha));
                }
            }
            u.insert(k + 1, t);
            pw = q;
        }
        NurbsCurve::from_hom(p, &pw, &u).unwrap_or_else(|| self.clone())
    }

    /// The two halves at parameter `t` (strictly inside the domain), exact.
    pub fn split_at(&self, t: f64) -> Option<(NurbsCurve, NurbsCurve)> {
        let (a, b) = self.domain();
        let eps = (b - a) * 1e-9;
        if t <= a + eps || t >= b - eps {
            return None;
        }
        let p = self.degree;
        let c = self.with_knot(t, p);
        let u = c.full_knots();
        let pw = c.homogeneous();
        let tol = (b - a) * 1e-12;
        // First knot equal to t: the left half ends there.
        let r = u.iter().position(|k| (k - t).abs() <= tol)?;
        if r == 0 || r > pw.len() {
            return None;
        }
        let mut lk: Vec<f64> = u[..r].to_vec();
        lk.extend(std::iter::repeat_n(t, p + 1));
        let mut rk: Vec<f64> = vec![t; p + 1];
        rk.extend_from_slice(&u[r + p..]);
        let left = NurbsCurve::from_hom(p, &pw[..r], &lk)?;
        let right = NurbsCurve::from_hom(p, &pw[r - 1..], &rk)?;
        Some((left, right))
    }

    /// The part of the curve between parameters `t0 < t1`, exact.
    pub fn sub(&self, t0: f64, t1: f64) -> Option<NurbsCurve> {
        let (a, b) = self.domain();
        let (t0, t1) = (t0.clamp(a, b), t1.clamp(a, b));
        if t1 - t0 <= (b - a) * 1e-9 {
            return None;
        }
        let right = match self.split_at(t0) {
            Some((_, r)) => r,
            None => self.clone(),
        };
        Some(match right.split_at(t1) {
            Some((l, _)) => l,
            None => right,
        })
    }

    /// Parameter of the point of the curve closest to `p` (sampling, then a
    /// golden-section search around the best sample).
    pub fn closest_param(&self, p: Point3) -> f64 {
        let mut params = Vec::new();
        for (a, b) in self.spans() {
            for i in 0..32 {
                params.push(a + (b - a) * f64::from(i) / 32.0);
            }
        }
        params.push(self.domain().1);
        let d = |t: f64| self.point_at(t).distance_to(p);
        let best = (0..params.len())
            .min_by(|i, j| d(params[*i]).total_cmp(&d(params[*j])))
            .unwrap_or(0);
        let mut lo = params[best.saturating_sub(1)];
        let mut hi = params[(best + 1).min(params.len() - 1)];
        let g = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..80 {
            let m1 = hi - g * (hi - lo);
            let m2 = lo + g * (hi - lo);
            if d(m1) < d(m2) {
                hi = m2;
            } else {
                lo = m1;
            }
        }
        (lo + hi) / 2.0
    }

    /// The same curve raised to degree `degree` (exact): split into Bézier
    /// pieces, each raised (The NURBS Book eq. 5.36), joined with knots of full
    /// multiplicity.
    pub fn elevated(&self, degree: usize) -> NurbsCurve {
        let p = self.degree;
        if degree <= p {
            return self.clone();
        }
        // Bézier pieces: every interior knot to multiplicity p.
        let mut c = self.clone();
        let (a, b) = self.domain();
        let breaks: Vec<f64> = {
            let mut v: Vec<f64> = self
                .knots
                .iter()
                .copied()
                .filter(|k| *k > a && *k < b)
                .collect();
            v.dedup_by(|x, y| (*x - *y).abs() < 1e-14);
            v
        };
        for &k in &breaks {
            c = c.with_knot(k, p);
        }
        let pw = c.homogeneous();
        let pieces = (pw.len() - 1) / p;
        let mut out: Vec<Hom> = Vec::new();
        for s in 0..pieces {
            let mut seg: Vec<Hom> = pw[s * p..=s * p + p].to_vec();
            for q in p..degree {
                // Degree q -> q + 1.
                let mut up = Vec::with_capacity(q + 2);
                up.push(seg[0]);
                for i in 1..=q {
                    let al = i as f64 / (q + 1) as f64;
                    up.push(lerp_h(seg[i], seg[i - 1], al));
                }
                up.push(seg[q]);
                seg = up;
            }
            if s == 0 {
                out.extend(seg);
            } else {
                out.extend(seg.into_iter().skip(1));
            }
        }
        let mut full = vec![a; degree + 1];
        for &k in &breaks {
            full.extend(std::iter::repeat_n(k, degree));
        }
        full.extend(std::iter::repeat_n(b, degree + 1));
        NurbsCurve::from_hom(degree, &out, &full).unwrap_or_else(|| self.clone())
    }

    /// Curves joined end to end into one (exact): each must start where the
    /// previous one ends (within `tol`). Degrees are raised to the highest; the
    /// joins become knots of full multiplicity (corners stay corners). The
    /// parameter of each piece keeps its length.
    pub fn join(curves: &[NurbsCurve], tol: f64) -> Option<NurbsCurve> {
        let first = curves.first()?;
        let q = curves.iter().map(|c| c.degree).max()?;
        let mut pw: Vec<Hom> = Vec::new();
        let mut full: Vec<f64> = Vec::new();
        let mut offset = 0.0;
        for (i, c) in curves.iter().enumerate() {
            if i > 0 && curves[i - 1].end().distance_to(c.start()) > tol {
                return None;
            }
            let e = c.elevated(q);
            let (a, b) = e.domain();
            if b - a <= 0.0 {
                return None;
            }
            // Map the parameter onto [offset, offset + length].
            let len = e.length().max(1e-9);
            let map = |k: f64| offset + (k - a) / (b - a) * len;
            let mut h = e.homogeneous();
            let u: Vec<f64> = e.full_knots().into_iter().map(map).collect();
            if i == 0 {
                pw = h;
                full = u;
            } else {
                // Scale the weights so the shared point has one weight.
                let wl = pw.last().expect("points")[3];
                let f = wl / h[0][3];
                for x in &mut h {
                    for v in x.iter_mut() {
                        *v *= f;
                    }
                }
                full.pop();
                full.extend_from_slice(&u[q + 1..]);
                pw.extend(h.into_iter().skip(1));
            }
            offset += len;
        }
        let _ = first;
        NurbsCurve::from_hom(q, &pw, &full)
    }

    /// The curve cut at its corners: interior knots of full multiplicity where
    /// the direction changes (as left by [`NurbsCurve::join`]). One piece when
    /// the curve is smooth.
    pub fn split_at_kinks(&self) -> Vec<NurbsCurve> {
        let (a, b) = self.domain();
        let p = self.degree;
        let mut cuts: Vec<f64> = Vec::new();
        for &k in &self.knots {
            if k > a
                && k < b
                && cuts.last().is_none_or(|c| (k - c).abs() > 1e-12)
                && self.multiplicity(k) >= p
            {
                let h = (b - a) * 1e-7;
                let (d0, d1) = (
                    self.point_at(k) - self.point_at(k - h),
                    self.point_at(k + h) - self.point_at(k),
                );
                if let (Some(u), Some(v)) = (d0.normalized(), d1.normalized()) {
                    if u.dot(v) < 1.0 - 1e-9 {
                        cuts.push(k);
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut rest = self.clone();
        for k in cuts {
            match rest.split_at(k) {
                Some((l, r)) => {
                    out.push(l);
                    rest = r;
                }
                None => break,
            }
        }
        out.push(rest);
        out
    }

    /// The planar offset of the curve by `distance` towards `side` (a unit
    /// normal of the plane crossed with the tangent gives the left side): a
    /// cubic through points of the exact offset, refined until it stays within
    /// `tol` of it. `None` where the offset folds over itself (radius of
    /// curvature smaller than the distance).
    pub fn offset_planar(
        &self,
        distance: f64,
        normal: Vec3,
        left: bool,
        tol: f64,
    ) -> Option<NurbsCurve> {
        let (a, b) = self.domain();
        let sign = if left { 1.0 } else { -1.0 };
        let at = |t: f64| -> Point3 {
            let n = normal
                .cross(self.tangent_at(t))
                .normalized()
                .unwrap_or(Vec3::X);
            self.point_at(t) + n * (distance * sign)
        };
        // Folds: a sample of the offset running against the curve.
        let probe = 64 * self.spans().len();
        for i in 0..probe {
            let (t0, t1) = (
                a + (b - a) * i as f64 / probe as f64,
                a + (b - a) * (i + 1) as f64 / probe as f64,
            );
            let (dc, doff) = (self.point_at(t1) - self.point_at(t0), at(t1) - at(t0));
            if dc.dot(doff) <= 0.0 {
                return None;
            }
        }
        let closed = self.is_closed();
        let mut count = (8 * self.spans().len()).max(16);
        for _ in 0..4 {
            let params: Vec<f64> = (0..=count)
                .map(|i| a + (b - a) * i as f64 / count as f64)
                .collect();
            let mut pts: Vec<Point3> = params.iter().map(|t| at(*t)).collect();
            if closed {
                let last = pts.len() - 1;
                pts[last] = pts[0];
            }
            let c = NurbsCurve::interpolate(&pts, 3)?;
            let worst = params
                .windows(2)
                .map(|w| {
                    let m = at((w[0] + w[1]) / 2.0);
                    c.point_at(c.closest_param(m)).distance_to(m)
                })
                .fold(0.0, f64::max);
            if worst <= tol {
                return Some(c);
            }
            count *= 2;
        }
        None
    }
}

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

    fn wavy() -> NurbsCurve {
        NurbsCurve::clamped_uniform(
            &[
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(30.0, 40.0, 0.0),
                Point3::new(70.0, -20.0, 0.0),
                Point3::new(100.0, 20.0, 0.0),
                Point3::new(130.0, 0.0, 0.0),
            ],
            3,
        )
        .unwrap()
    }

    /// Largest distance from points of `a` to the curve `b`.
    fn deviation(a: &NurbsCurve, b: &NurbsCurve) -> f64 {
        let (t0, t1) = a.domain();
        (0..=100)
            .map(|i| {
                let p = a.point_at(t0 + (t1 - t0) * f64::from(i) / 100.0);
                b.point_at(b.closest_param(p)).distance_to(p)
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn knots_split_and_sub_curves_are_exact() {
        let c = wavy();
        let k = c.with_knot(0.7, 2);
        assert_eq!(k.points.len(), c.points.len() + 2);
        assert!(deviation(&c, &k) < 1e-9);
        let (l, r) = c.split_at(1.3).unwrap();
        let p = c.point_at(1.3);
        assert!(l.end().distance_to(p) < 1e-9 && r.start().distance_to(p) < 1e-9);
        assert!(deviation(&l, &c) < 1e-9 && deviation(&r, &c) < 1e-9);
        assert!(((l.length() + r.length()) - c.length()).abs() < 1e-3);
        let s = c.sub(0.4, 1.6).unwrap();
        assert!(s.start().distance_to(c.point_at(0.4)) < 1e-9);
        assert!(s.end().distance_to(c.point_at(1.6)) < 1e-9);
        assert!(c.split_at(0.0).is_none());
        // Rational: a circle cut in two keeps its radius.
        let circle = NurbsCurve::from_arc(&CircleArc::circle(
            Plane::from_normal(Point3::ORIGIN, Vec3::Z),
            5.0,
        ));
        let (a, _) = circle.split_at(1.5).unwrap();
        for i in 0..=50 {
            let (t0, t1) = a.domain();
            let q = a.point_at(t0 + (t1 - t0) * f64::from(i) / 50.0);
            assert!((q.distance_to(Point3::ORIGIN) - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn closest_points() {
        let c = wavy();
        let t = 1.234;
        let p = c.point_at(t) + Vec3::new(0.0, 0.0, 3.0);
        assert!((c.closest_param(p) - t).abs() < 1e-6);
    }

    #[test]
    fn elevation_and_join_keep_the_shape() {
        let c = wavy();
        let e = c.elevated(5);
        assert_eq!(e.degree, 5);
        assert!(deviation(&c, &e) < 1e-9 && deviation(&e, &c) < 1e-9);
        let arc = NurbsCurve::from_arc(
            &crate::curvetools::arc_3pt(
                c.end(),
                Point3::new(170.0, 0.0, 0.0),
                Point3::new(150.0, 20.0, 0.0),
            )
            .unwrap(),
        );
        let line = NurbsCurve::new(
            1,
            vec![arc.end(), Point3::new(200.0, 0.0, 0.0)],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
        )
        .unwrap();
        let j = NurbsCurve::join(&[c.clone(), arc.clone(), line.clone()], 1e-9).unwrap();
        assert_eq!(j.degree, 3);
        for part in [&c, &arc, &line] {
            assert!(deviation(part, &j) < 1e-8, "{}", deviation(part, &j));
        }
        assert!((j.length() - (c.length() + arc.length() + line.length())).abs() < 1e-3);
        assert!(NurbsCurve::join(&[line, c], 1e-9).is_none(), "gap");
    }

    #[test]
    fn planar_offsets() {
        let circle = NurbsCurve::from_arc(&CircleArc::circle(
            Plane::from_normal(Point3::ORIGIN, Vec3::Z),
            20.0,
        ));
        // Counter-clockwise: the left side is inside.
        let inner = circle.offset_planar(5.0, Vec3::Z, true, 1e-4).unwrap();
        let outer = circle.offset_planar(5.0, Vec3::Z, false, 1e-4).unwrap();
        for (c, r) in [(&inner, 15.0), (&outer, 25.0)] {
            let (t0, t1) = c.domain();
            for i in 0..=200 {
                let q = c.point_at(t0 + (t1 - t0) * f64::from(i) / 200.0);
                assert!((q.distance_to(Point3::ORIGIN) - r).abs() < 2e-4, "{r}");
            }
        }
        // Deeper than the radius: folds.
        assert!(circle.offset_planar(25.0, Vec3::Z, true, 1e-4).is_none());
    }

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
