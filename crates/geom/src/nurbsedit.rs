//! Editing and comparing NURBS curves: conic arcs, matching an end to another
//! curve (position, tangency, curvature) and the deviation between two curves.
//!
//! Rhino 8 is the behavioural reference: the control points and distances it
//! gives on the test curves below were observed by running it (clean-room).

use crate::{NurbsCurve, Point3, Vec3};

/// Largest and smallest distance between two curves where they run alongside
/// each other, with the points where they are measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveDeviation {
    pub max: f64,
    pub max_a: Point3,
    pub max_b: Point3,
    pub min: f64,
    pub min_a: Point3,
    pub min_b: Point3,
}

impl NurbsCurve {
    /// Conic arc from `start` to `end` whose end tangents meet at `apex`: a
    /// rational quadratic with one span. `rho` in (0, 1) is how far the arc
    /// reaches from the chord towards the apex: below 0.5 an ellipse, 0.5 a
    /// parabola, above 0.5 a hyperbola.
    pub fn conic(start: Point3, apex: Point3, end: Point3, rho: f64) -> Option<NurbsCurve> {
        if !(rho > 0.0 && rho < 1.0) {
            return None;
        }
        if (apex - start).cross(end - start).length() < 1e-12 {
            return None;
        }
        NurbsCurve::new(
            2,
            vec![start, apex, end],
            vec![1.0, rho / (1.0 - rho), 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
    }

    /// The curve with its start moved to `point`. With `leave` (a unit vector)
    /// it also starts in that direction: the second control point goes on that
    /// line, as far from the start as before. With `curvature` (a curvature
    /// vector, see [`NurbsCurve::curvature_at`]) the third control point moves
    /// too, so the curve starts with that curvature; it keeps its distance
    /// along the start direction. Only the first one, two or three control
    /// points change. `None` when the curve has too few control points, is not
    /// clamped at its start or has no direction there.
    pub fn matched_start(
        &self,
        point: Point3,
        leave: Option<Vec3>,
        curvature: Option<Vec3>,
    ) -> Option<NurbsCurve> {
        let p = self.degree;
        let n = self.points.len();
        let (a, b) = self.domain();
        let eps = (b - a).abs() * 1e-12;
        if (self.knots[p - 1] - self.knots[0]).abs() > eps {
            return None;
        }
        let mut c = self.clone();
        c.points[0] = point;
        let Some(dir) = leave else {
            return Some(c);
        };
        let dir = dir.normalized()?;
        if n < 3 {
            return None;
        }
        let old = self.points[1] - self.points[0];
        let len = old.length();
        let t_old = old.normalized()?;
        c.points[1] = point + dir * len;
        let Some(k) = curvature else {
            return Some(c);
        };
        if n < 4 || p < 2 {
            return None;
        }
        // C′ = p·(w1/w0)·(P1 − P0)/d1, and the part of C″ across the curve is
        // p(p−1)/d12 · (w2/w0) · across(P2 − P1)/d2 (textbook knots u).
        let u = self.full_knots();
        let (d1, d2, d12) = (u[p + 1] - u[1], u[p + 2] - u[2], u[p + 1] - u[2]);
        if d1 <= 0.0 || d2 <= 0.0 || d12 <= 0.0 {
            return None;
        }
        let (w0, w1, w2) = (self.weights[0], self.weights[1], self.weights[2]);
        let (pf, speed) = (p as f64, p as f64 * (w1 / w0) * len / d1);
        let along = (self.points[2] - self.points[1]).dot(t_old);
        let across =
            (k - dir * k.dot(dir)) * (speed * speed * d2 * d12 / (pf * (pf - 1.0)) * (w0 / w2));
        c.points[2] = c.points[1] + dir * along + across;
        Some(c)
    }

    /// [`NurbsCurve::matched_start`] at the other end; `leave` points from the
    /// matched end into the curve (the way the curve it is matched to goes on).
    pub fn matched_end(
        &self,
        point: Point3,
        leave: Option<Vec3>,
        curvature: Option<Vec3>,
    ) -> Option<NurbsCurve> {
        Some(
            self.reversed()
                .matched_start(point, leave, curvature)?
                .reversed(),
        )
    }

    /// Distances from this curve to `other`, where the two run alongside: for
    /// points of this curve, the distance to the closest point of `other`,
    /// leaving out those that only see an end of `other` at an angle. Returns
    /// (largest, its points, smallest, its points).
    fn one_sided(&self, other: &NurbsCurve) -> Option<[(f64, Point3, Point3); 2]> {
        let (a, b) = self.domain();
        let (oa, ob) = other.domain();
        let open = !other.is_closed();
        let measure = |t: f64| -> Option<(f64, Point3, Point3)> {
            let p = self.point_at(t);
            let s = other.closest_param(p);
            let q = other.point_at(s);
            let d = p.distance_to(q);
            let at_end = (s - oa).abs() <= (ob - oa) * 1e-6 || (ob - s).abs() <= (ob - oa) * 1e-6;
            if open && at_end && d > 1e-12 {
                let tan = other.tangent_at(s);
                if ((p - q) * (1.0 / d)).dot(tan).abs() > 1e-3 {
                    return None;
                }
            }
            Some((d, p, q))
        };
        let mut knots = vec![a];
        knots.extend(self.interior_knots());
        knots.push(b);
        let mut ts: Vec<f64> = Vec::new();
        for w in knots.windows(2) {
            for i in 0..48 {
                ts.push(w[0] + (w[1] - w[0]) * f64::from(i) / 48.0);
            }
        }
        ts.push(b);
        let vals: Vec<Option<(f64, Point3, Point3)>> = ts.iter().map(|t| measure(*t)).collect();
        let pick = |sign: f64| -> Option<(f64, Point3, Point3)> {
            let i = (0..ts.len())
                .filter(|i| vals[*i].is_some())
                .max_by(|i, j| {
                    (sign * vals[*i].expect("some").0)
                        .total_cmp(&(sign * vals[*j].expect("some").0))
                })?;
            // Golden-section search around the best sample.
            let score = |t: f64| measure(t).map_or(f64::NEG_INFINITY, |m| sign * m.0);
            let (mut lo, mut hi) = (ts[i.saturating_sub(1)], ts[(i + 1).min(ts.len() - 1)]);
            let g = (5f64.sqrt() - 1.0) / 2.0;
            for _ in 0..60 {
                let (m1, m2) = (hi - g * (hi - lo), lo + g * (hi - lo));
                if score(m1) > score(m2) {
                    hi = m2;
                } else {
                    lo = m1;
                }
            }
            let refined = measure((lo + hi) / 2.0);
            let best = vals[i].expect("some");
            Some(match refined {
                Some(r) if sign * r.0 >= sign * best.0 => r,
                _ => best,
            })
        };
        Some([pick(1.0)?, pick(-1.0)?])
    }

    /// Largest and smallest distance between the two curves over the part where
    /// they run alongside each other (measured from both). `None` when no part
    /// of one faces the other.
    pub fn deviation(&self, other: &NurbsCurve) -> Option<CurveDeviation> {
        let ab = self.one_sided(other);
        let ba = other.one_sided(self);
        let mut best: Option<CurveDeviation> = None;
        let sides = [
            ab.map(|[mx, mn]| (mx, mn)),
            ba.map(|[mx, mn]| ((mx.0, mx.2, mx.1), (mn.0, mn.2, mn.1))),
        ];
        for (mx, mn) in sides.into_iter().flatten() {
            let d = best.get_or_insert(CurveDeviation {
                max: mx.0,
                max_a: mx.1,
                max_b: mx.2,
                min: mn.0,
                min_a: mn.1,
                min_b: mn.2,
            });
            if mx.0 > d.max {
                (d.max, d.max_a, d.max_b) = mx;
            }
            if mn.0 < d.min {
                (d.min, d.min_a, d.min_b) = mn;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cubic(pts: &[(f64, f64)]) -> NurbsCurve {
        let p: Vec<Point3> = pts.iter().map(|q| Point3::new(q.0, q.1, 0.0)).collect();
        NurbsCurve::clamped_uniform(&p, 3).unwrap()
    }

    fn near(c: &NurbsCurve, want: &[(f64, f64)]) {
        assert_eq!(c.points.len(), want.len());
        for (p, r) in c.points.iter().zip(want) {
            assert!(
                p.distance_to(Point3::new(r.0, r.1, 0.0)) < 1e-6,
                "{p} {r:?}"
            );
        }
    }

    #[test]
    fn conic_reaches_rho_of_the_way_to_the_apex() {
        let (s, a, e) = (
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 20.0, 0.0),
            Point3::new(20.0, 0.0, 0.0),
        );
        for rho in [0.25, 0.5, 0.8] {
            let c = NurbsCurve::conic(s, a, e, rho).unwrap();
            assert!(c.start().distance_to(s) < 1e-12 && c.end().distance_to(e) < 1e-12);
            // The shoulder: on the median, rho of the way from the chord.
            let mid = c.point_at(0.5);
            assert!(
                mid.distance_to(Point3::new(10.0, 20.0 * rho, 0.0)) < 1e-9,
                "{mid}"
            );
            // Tangent to the legs at both ends.
            let t0 = c.tangent_at(0.0);
            assert!(t0.cross((a - s).normalized().unwrap()).length() < 1e-4);
        }
        // rho = 0.5 is the parabola y = 2x − x²/10.
        let c = NurbsCurve::conic(s, a, e, 0.5).unwrap();
        for i in 0..=20 {
            let p = c.point_at(f64::from(i) / 20.0);
            assert!((p.y - (2.0 * p.x - p.x * p.x / 10.0)).abs() < 1e-9);
        }
        assert!(NurbsCurve::conic(s, a, e, 1.0).is_none());
        assert!(NurbsCurve::conic(s, Point3::new(10.0, 0.0, 0.0), e, 0.5).is_none());
    }

    /// Rhino 8.36, `Curve.CreateMatchCurve`: a six-point cubic matched to the
    /// start of a four-point cubic, at its own start and at its own end.
    #[test]
    fn matching_an_end_gives_rhinos_control_points() {
        let c0 = cubic(&[
            (0.0, 0.0),
            (10.0, 10.0),
            (20.0, 5.0),
            (30.0, 12.0),
            (40.0, 0.0),
            (50.0, 6.0),
        ]);
        let c1 = cubic(&[(-40.0, -10.0), (-30.0, 10.0), (-15.0, 5.0), (-5.0, -3.0)]);
        let (t0, _) = c1.domain();
        let (at, d1, _) = c1.derivatives_at(t0);
        // Leaving the start of c1 backwards: the matched curve goes on from it.
        let leave = (d1 * -1.0).normalized().unwrap();
        let k = c1.curvature_at(t0).unwrap();

        near(
            &c0.matched_start(at, None, None).unwrap(),
            &[
                (-40.0, -10.0),
                (10.0, 10.0),
                (20.0, 5.0),
                (30.0, 12.0),
                (40.0, 0.0),
                (50.0, 6.0),
            ],
        );
        near(
            &c0.matched_start(at, Some(leave), None).unwrap(),
            &[
                (-40.0, -10.0),
                (-46.324_555_32, -22.649_110_641),
                (20.0, 5.0),
                (30.0, 12.0),
                (40.0, 0.0),
                (50.0, 6.0),
            ],
        );
        let g2 = c0.matched_start(at, Some(leave), Some(k)).unwrap();
        near(
            &g2,
            &[
                (-40.0, -10.0),
                (-46.324_555_32, -22.649_110_641),
                (-36.705_694_15, -31.411_388_301),
                (30.0, 12.0),
                (40.0, 0.0),
                (50.0, 6.0),
            ],
        );
        assert!((g2.curvature_at(g2.domain().0).unwrap() - k).length() < 1e-9);
        near(
            &c0.matched_end(at, Some(leave), None).unwrap(),
            &[
                (0.0, 0.0),
                (10.0, 10.0),
                (20.0, 5.0),
                (30.0, 12.0),
                (-45.215_361_924, -20.430_723_848),
                (-40.0, -10.0),
            ],
        );
        let e2 = c0.matched_end(at, Some(leave), Some(k)).unwrap();
        near(
            &e2,
            &[
                (0.0, 0.0),
                (10.0, 10.0),
                (20.0, 5.0),
                (-38.673_112_909, -26.386_225_817),
                (-45.215_361_924, -20.430_723_848),
                (-40.0, -10.0),
            ],
        );
        assert!((e2.curvature_at(e2.domain().1).unwrap() - k).length() < 1e-9);
    }

    #[test]
    fn matching_curvature_holds_with_weights_and_uneven_knots() {
        let mut c = cubic(&[
            (0.0, 0.0),
            (10.0, 10.0),
            (20.0, 5.0),
            (30.0, 12.0),
            (40.0, 0.0),
            (50.0, 6.0),
        ]);
        c.weights = vec![1.5, 0.7, 2.0, 1.0, 1.0, 1.0];
        c.knots = vec![0.0, 0.0, 0.0, 0.4, 2.5, 3.0, 3.0, 3.0];
        let k = Vec3::new(0.0, 0.0, 0.03);
        let leave = Vec3::new(1.0, 1.0, 0.0).normalized().unwrap();
        let m = c
            .matched_start(Point3::new(5.0, 5.0, 5.0), Some(leave), Some(k))
            .unwrap();
        let t0 = m.domain().0;
        assert!(m.start().distance_to(Point3::new(5.0, 5.0, 5.0)) < 1e-12);
        assert!((m.derivatives_at(t0).1.normalized().unwrap() - leave).length() < 1e-9);
        assert!((m.curvature_at(t0).unwrap() - k).length() < 1e-9);
        // Too few points for curvature; a line has none to spare for tangency.
        assert!(cubic(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0)])
            .matched_start(Point3::ORIGIN, Some(leave), Some(k))
            .is_some());
        let line = NurbsCurve::new(
            1,
            vec![Point3::ORIGIN, Point3::new(1.0, 0.0, 0.0)],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
        )
        .unwrap();
        assert!(line
            .matched_start(Point3::ORIGIN, Some(leave), None)
            .is_none());
        assert!(line
            .matched_start(Point3::new(0.0, 2.0, 0.0), None, None)
            .is_some());
    }

    /// Rhino 8.36, `Curve.GetDistancesBetweenCurves`.
    #[test]
    fn deviation_matches_rhino() {
        let zig: Vec<(f64, f64)> = (0..8)
            .map(|i| (f64::from(i) * 10.0, if i % 2 == 0 { 0.0 } else { 8.0 }))
            .collect();
        let z = cubic(&zig);
        let z2 = z.without_knot(2.0).unwrap();
        let d = z.deviation(&z2).unwrap();
        assert!((d.max - 2.478_371_706).abs() < 1e-4, "{}", d.max);
        assert!(d.min < 1e-6, "{}", d.min);

        let a = cubic(&[
            (0.0, 0.0),
            (30.0, 40.0),
            (70.0, -20.0),
            (100.0, 20.0),
            (130.0, 0.0),
        ]);
        let line = NurbsCurve::new(
            1,
            vec![Point3::new(20.0, -10.0, 0.0), Point3::new(90.0, 30.0, 0.0)],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
        )
        .unwrap();
        for d in [a.deviation(&line).unwrap(), line.deviation(&a).unwrap()] {
            assert!((d.max - 24.453_210_339).abs() < 1e-4, "{}", d.max);
            // The curves cross, so the smallest distance is 0. (Rhino's API
            // reports 0.464 here, the smallest of the samples it takes.)
            assert!(d.min < 1e-6, "{}", d.min);
        }
        let d = a.deviation(&line).unwrap();
        assert!(d.max_a.distance_to(a.point_at(0.208_428_971)) < 1e-2);
        assert!((d.max_a.distance_to(d.max_b) - d.max).abs() < 1e-9);
        // Curves end to end, at an angle: no part of one faces the other.
        let far = NurbsCurve::new(
            1,
            vec![Point3::new(200.0, 0.0, 0.0), Point3::new(300.0, 0.0, 0.0)],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
        )
        .unwrap();
        let up = NurbsCurve::new(
            1,
            vec![Point3::new(310.0, 10.0, 0.0), Point3::new(310.0, 90.0, 0.0)],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
        )
        .unwrap();
        assert!(far.deviation(&up).is_none());
    }
}
