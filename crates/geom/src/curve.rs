//! Planar curve tools on chains of line and arc segments: offset, trim, extend,
//! join, fillet. All operations assume the curves lie in (or are projected to)
//! a common working plane.

use crate::{CircleArc, Plane, Point3, Vec3};
use std::f64::consts::TAU;

/// One piece of a curve: a straight segment or a circular arc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Line(Point3, Point3),
    Arc(CircleArc),
}

/// Why a curve operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurveError {
    NoIntersection,
    Degenerate(&'static str),
    Closed,
}

impl std::fmt::Display for CurveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CurveError::NoIntersection => write!(f, "no intersection found"),
            CurveError::Degenerate(s) => write!(f, "{s}"),
            CurveError::Closed => write!(f, "curve is closed"),
        }
    }
}

impl Seg {
    pub fn start(&self) -> Point3 {
        self.point_at(0.0)
    }

    pub fn end(&self) -> Point3 {
        self.point_at(1.0)
    }

    /// Point at normalised parameter `t` (0 = start, 1 = end). Values outside 0..1
    /// extrapolate along the line or around the circle.
    pub fn point_at(&self, t: f64) -> Point3 {
        match self {
            Seg::Line(a, b) => *a + (*b - *a) * t,
            Seg::Arc(c) => c.point_at_angle(c.sweep * t),
        }
    }

    /// Unit tangent at `t`, in the direction of increasing parameter.
    pub fn tangent_at(&self, t: f64) -> Vec3 {
        match self {
            Seg::Line(a, b) => (*b - *a).normalized().unwrap_or(Vec3::X),
            Seg::Arc(c) => {
                let a = c.sweep * t;
                c.plane.x * -a.sin() + c.plane.y * a.cos()
            }
        }
    }

    pub fn length(&self) -> f64 {
        match self {
            Seg::Line(a, b) => a.distance_to(*b),
            Seg::Arc(c) => c.radius * c.sweep,
        }
    }

    pub fn reversed(&self) -> Seg {
        match self {
            Seg::Line(a, b) => Seg::Line(*b, *a),
            Seg::Arc(c) => {
                let z = -c.plane.z;
                let x = c.plane.x * c.sweep.cos() + c.plane.y * c.sweep.sin();
                Seg::Arc(CircleArc {
                    plane: Plane {
                        origin: c.plane.origin,
                        x,
                        y: z.cross(x),
                        z,
                    },
                    radius: c.radius,
                    sweep: c.sweep,
                })
            }
        }
    }

    /// The part between parameters `t0 < t1` (may extend beyond 0..1).
    pub fn sub(&self, t0: f64, t1: f64) -> Seg {
        match self {
            Seg::Line(..) => Seg::Line(self.point_at(t0), self.point_at(t1)),
            Seg::Arc(c) => {
                let a0 = c.sweep * t0;
                let x = c.plane.x * a0.cos() + c.plane.y * a0.sin();
                Seg::Arc(CircleArc {
                    plane: Plane {
                        origin: c.plane.origin,
                        x,
                        y: c.plane.z.cross(x),
                        z: c.plane.z,
                    },
                    radius: c.radius,
                    sweep: (c.sweep * (t1 - t0)).min(TAU),
                })
            }
        }
    }

    /// Polyline sample including both ends.
    pub fn points(&self) -> Vec<Point3> {
        match self {
            Seg::Line(a, b) => vec![*a, *b],
            Seg::Arc(c) => c.points(96),
        }
    }

    /// Parameter (clamped to 0..1) of the point of the segment closest to `p`.
    pub fn closest(&self, p: Point3) -> f64 {
        match self {
            Seg::Line(a, b) => {
                let d = *b - *a;
                let l2 = d.dot(d);
                if l2 < 1e-24 {
                    0.0
                } else {
                    ((p - *a).dot(d) / l2).clamp(0.0, 1.0)
                }
            }
            Seg::Arc(c) => {
                let ang = arc_angle(c, p);
                if ang <= c.sweep {
                    ang / c.sweep
                } else if p.distance_to(c.start()) < p.distance_to(c.end()) {
                    0.0
                } else {
                    1.0
                }
            }
        }
    }
}

/// Angle of `p` around the arc's plane, in [0, 2π).
fn arc_angle(c: &CircleArc, p: Point3) -> f64 {
    let (u, v, _) = c.plane.coords(p);
    let a = v.atan2(u);
    if a < 0.0 {
        a + TAU
    } else {
        a
    }
}

/// Unbounded parameter of `p` on the segment's line or circle (arcs: angle/sweep,
/// in [0, 2π/sweep)).
fn param_of(s: &Seg, p: Point3) -> f64 {
    match s {
        Seg::Line(a, b) => {
            let d = *b - *a;
            (p - *a).dot(d) / d.dot(d).max(1e-24)
        }
        Seg::Arc(c) => arc_angle(c, p) / c.sweep,
    }
}

/// 2D circle or line in plane coordinates.
enum Prim {
    Line([f64; 2], [f64; 2]),
    Circle([f64; 2], f64),
}

fn to2(plane: &Plane, p: Point3) -> [f64; 2] {
    let (u, v, _) = plane.coords(p);
    [u, v]
}

fn prim(plane: &Plane, s: &Seg) -> Prim {
    match s {
        Seg::Line(a, b) => Prim::Line(to2(plane, *a), to2(plane, *b)),
        Seg::Arc(c) => Prim::Circle(to2(plane, c.center()), c.radius),
    }
}

/// Intersection points (2D) of the infinite line / full circle carriers.
fn carrier_hits(a: &Prim, b: &Prim) -> Vec<[f64; 2]> {
    match (a, b) {
        (Prim::Line(p, q), Prim::Line(r, s)) => {
            let d1 = [q[0] - p[0], q[1] - p[1]];
            let d2 = [s[0] - r[0], s[1] - r[1]];
            let den = d1[0] * d2[1] - d1[1] * d2[0];
            let l = (d1[0].hypot(d1[1]) * d2[0].hypot(d2[1])).max(1e-300);
            if den.abs() / l < 1e-12 {
                return Vec::new();
            }
            let t = ((r[0] - p[0]) * d2[1] - (r[1] - p[1]) * d2[0]) / den;
            vec![[p[0] + d1[0] * t, p[1] + d1[1] * t]]
        }
        (Prim::Line(p, q), Prim::Circle(c, r)) | (Prim::Circle(c, r), Prim::Line(p, q)) => {
            let d = [q[0] - p[0], q[1] - p[1]];
            let f = [p[0] - c[0], p[1] - c[1]];
            let aa = d[0] * d[0] + d[1] * d[1];
            if aa < 1e-24 {
                return Vec::new();
            }
            let bb = 2.0 * (f[0] * d[0] + f[1] * d[1]);
            let cc = f[0] * f[0] + f[1] * f[1] - r * r;
            let disc = bb * bb - 4.0 * aa * cc;
            let tol = 1e-9 * (bb * bb).max(1.0);
            if disc < -tol {
                return Vec::new();
            }
            let sq = disc.max(0.0).sqrt();
            let mut out = Vec::new();
            for t in [(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)] {
                out.push([p[0] + d[0] * t, p[1] + d[1] * t]);
            }
            if sq < 1e-12 {
                out.pop();
            }
            out
        }
        (Prim::Circle(c1, r1), Prim::Circle(c2, r2)) => {
            let dx = c2[0] - c1[0];
            let dy = c2[1] - c1[1];
            let d = dx.hypot(dy);
            if d < 1e-12 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
                return Vec::new();
            }
            let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
            let h = (r1 * r1 - a * a).max(0.0).sqrt();
            let m = [c1[0] + a * dx / d, c1[1] + a * dy / d];
            let mut out = vec![[m[0] - h * dy / d, m[1] + h * dx / d]];
            if h > 1e-12 {
                out.push([m[0] + h * dy / d, m[1] - h * dx / d]);
            }
            out
        }
    }
}

/// Intersections of the carriers (infinite line, full circle) of two segments in
/// `plane`, as unbounded parameters `(ta, tb)` on each segment.
pub fn carrier_intersections(a: &Seg, b: &Seg, plane: &Plane) -> Vec<(f64, f64)> {
    let w = plane.coords(a.start()).2;
    carrier_hits(&prim(plane, a), &prim(plane, b))
        .into_iter()
        .map(|[u, v]| {
            let p = plane.point_at(u, v, w);
            (param_of(a, p), param_of(b, p))
        })
        .collect()
}

/// Parameters on `a` (0..1 with tolerance) where the two segments really cross.
fn seg_hits(a: &Seg, b: &Seg, plane: &Plane, eps: f64) -> Vec<f64> {
    carrier_intersections(a, b, plane)
        .into_iter()
        .filter(|(ta, tb)| in_range(a, *ta, eps) && in_range(b, *tb, eps))
        .map(|(ta, _)| ta.clamp(0.0, 1.0))
        .collect()
}

/// `t` within 0..1 allowing `eps` model units of slack at the ends.
fn in_range(s: &Seg, t: f64, eps: f64) -> bool {
    let slack = eps / s.length().max(1e-12);
    if let Seg::Arc(c) = s {
        if c.sweep >= TAU - 1e-9 {
            return true;
        }
        // Angles just below 2π are "before the start".
        let t2 = t - TAU / c.sweep;
        return (-slack..=1.0 + slack).contains(&t) || (-slack..=0.0).contains(&t2);
    }
    (-slack..=1.0 + slack).contains(&t)
}

/// A chain of segments (consecutive segments share end points).
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    pub segs: Vec<Seg>,
}

impl Chain {
    pub fn new(segs: Vec<Seg>) -> Chain {
        Chain { segs }
    }

    /// Chain through the points of a polyline.
    pub fn from_points(p: &[Point3]) -> Chain {
        Chain::new(p.windows(2).map(|w| Seg::Line(w[0], w[1])).collect())
    }

    pub fn start(&self) -> Point3 {
        self.segs[0].start()
    }

    pub fn end(&self) -> Point3 {
        self.segs[self.segs.len() - 1].end()
    }

    pub fn is_closed(&self, tol: f64) -> bool {
        !self.segs.is_empty() && self.start().distance_to(self.end()) <= tol
    }

    pub fn reversed(&self) -> Chain {
        Chain::new(self.segs.iter().rev().map(Seg::reversed).collect())
    }

    /// Point at chain parameter `t` in 0..n (n = number of segments).
    pub fn point_at(&self, t: f64) -> Point3 {
        let n = self.segs.len();
        let i = (t.floor().max(0.0) as usize).min(n - 1);
        self.segs[i].point_at(t - i as f64)
    }

    /// Chain parameter of the point closest to `p`, and its distance.
    pub fn closest(&self, p: Point3) -> (f64, f64) {
        let mut best = (0.0, f64::INFINITY);
        for (i, s) in self.segs.iter().enumerate() {
            let t = s.closest(p);
            let d = s.point_at(t).distance_to(p);
            if d < best.1 {
                best = (i as f64 + t, d);
            }
        }
        best
    }

    /// The part between chain parameters `t0 < t1`.
    pub fn sub(&self, t0: f64, t1: f64, tol: f64) -> Chain {
        let mut out = Vec::new();
        for (i, s) in self.segs.iter().enumerate() {
            let a = (t0 - i as f64).max(0.0);
            let b = (t1 - i as f64).min(1.0);
            if b <= a {
                continue;
            }
            let piece = s.sub(a, b);
            if piece.length() > tol {
                out.push(piece);
            }
        }
        Chain::new(out)
    }

    /// Polyline sample of the whole chain.
    pub fn points(&self) -> Vec<Point3> {
        let mut out: Vec<Point3> = Vec::new();
        for s in &self.segs {
            let p = s.points();
            let skip = usize::from(!out.is_empty());
            out.extend_from_slice(&p[skip..]);
        }
        out
    }

    /// Merge adjacent collinear lines and adjacent arcs of the same circle.
    pub fn simplified(&self, tol: f64) -> Chain {
        let mut out: Vec<Seg> = Vec::new();
        for s in &self.segs {
            if let Some(last) = out.last_mut() {
                if let Some(m) = merge(last, s, tol) {
                    *last = m;
                    continue;
                }
            }
            out.push(*s);
        }
        Chain::new(out)
    }
}

fn merge(a: &Seg, b: &Seg, tol: f64) -> Option<Seg> {
    match (a, b) {
        (Seg::Line(p, q), Seg::Line(_, r)) => {
            let d1 = (*q - *p).normalized()?;
            let d2 = (*r - *q).normalized()?;
            (d1.cross(d2).length() < 1e-9 && d1.dot(d2) > 0.0).then_some(Seg::Line(*p, *r))
        }
        (Seg::Arc(c1), Seg::Arc(c2)) => {
            let same = c1.center().distance_to(c2.center()) <= tol
                && (c1.radius - c2.radius).abs() <= tol
                && c1.plane.z.dot(c2.plane.z) > 0.999_999
                && c1.end().distance_to(c2.start()) <= tol;
            same.then(|| {
                Seg::Arc(CircleArc {
                    sweep: (c1.sweep + c2.sweep).min(TAU),
                    ..*c1
                })
            })
        }
        _ => None,
    }
}

/// Left-hand normal (in `plane`) of a segment at `t`.
fn left(s: &Seg, t: f64, plane: &Plane) -> Vec3 {
    plane.z.cross(s.tangent_at(t))
}

/// Which side of the chain `p` lies on: +1 left, −1 right (looking along the chain
/// within `plane`).
pub fn side_of(chain: &Chain, p: Point3, plane: &Plane) -> f64 {
    let (t, _) = chain.closest(p);
    let n = chain.segs.len();
    let i = (t.floor() as usize).min(n - 1);
    let lt = t - i as f64;
    let q = chain.segs[i].point_at(lt);
    let mut d = (p - q).dot(left(&chain.segs[i], lt, plane));
    if d.abs() < 1e-12 {
        // On a corner: use the bisector of the two neighbouring normals.
        let j = if lt > 0.5 {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        d = (p - q).dot(left(&chain.segs[j], 0.5, plane));
    }
    if d >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

fn offset_seg(s: &Seg, d: f64, plane: &Plane) -> Result<Seg, CurveError> {
    match s {
        Seg::Line(a, b) => {
            let n = left(s, 0.0, plane) * d;
            Ok(Seg::Line(*a + n, *b + n))
        }
        Seg::Arc(c) => {
            // Counter-clockwise around the plane normal: left is towards the centre.
            let ccw = c.plane.z.dot(plane.z) > 0.0;
            let r = if ccw { c.radius - d } else { c.radius + d };
            if r <= 1e-9 {
                return Err(CurveError::Degenerate("offset distance too large for arc"));
            }
            Ok(Seg::Arc(CircleArc { radius: r, ..*c }))
        }
    }
}

/// Connect offset segment `a` (ending) to `b` (starting), trimming or extending
/// both to their intersection nearest `corner`. Returns the adjusted pair and an
/// optional bridging line when they cannot meet.
fn connect(a: &mut Seg, b: &mut Seg, corner: Point3, plane: &Plane, tol: f64) -> Option<Seg> {
    if a.end().distance_to(b.start()) <= tol {
        return None;
    }
    let hits = carrier_intersections(a, b, plane);
    let best = hits.into_iter().min_by(|x, y| {
        a.point_at(x.0)
            .distance_to(corner)
            .total_cmp(&a.point_at(y.0).distance_to(corner))
    });
    match best {
        Some((ta, tb)) => {
            let ta = unwrap_end(a, ta);
            let tb = unwrap_start(b, tb);
            if ta > 1e-9 && tb < 1.0 - 1e-9 {
                *a = a.sub(0.0, ta);
                *b = b.sub(tb, 1.0);
                return None;
            }
            Some(Seg::Line(a.end(), b.start()))
        }
        None => Some(Seg::Line(a.end(), b.start())),
    }
}

/// For an arc parameter near the end, pick the representative close to 1.
fn unwrap_end(s: &Seg, t: f64) -> f64 {
    match s {
        Seg::Arc(c) => {
            let period = TAU / c.sweep;
            if (t - period).abs() < (t - 1.0).abs() {
                t - period
            } else {
                t
            }
        }
        Seg::Line(..) => t,
    }
}

/// For an arc parameter near the start, pick the representative close to 0.
fn unwrap_start(s: &Seg, t: f64) -> f64 {
    match s {
        Seg::Arc(c) => {
            let period = TAU / c.sweep;
            if (t - period).abs() < t.abs() {
                t - period
            } else {
                t
            }
        }
        Seg::Line(..) => t,
    }
}

/// Offset a planar chain by `distance` towards `side` (+1 left, −1 right), with
/// sharp (mitred) corners like Rhino's default.
pub fn offset(
    chain: &Chain,
    distance: f64,
    side: f64,
    plane: &Plane,
    tol: f64,
) -> Result<Chain, CurveError> {
    if chain.segs.is_empty() {
        return Err(CurveError::Degenerate("empty curve"));
    }
    let d = distance * side;
    let closed = chain.is_closed(tol);
    let mut segs: Vec<Seg> = chain
        .segs
        .iter()
        .map(|s| offset_seg(s, d, plane))
        .collect::<Result<_, _>>()?;
    let n = segs.len();
    if n == 1 && closed {
        return Ok(Chain::new(segs));
    }
    let mut bridges: Vec<Option<Seg>> = vec![None; n];
    let joints = if closed { n } else { n - 1 };
    for i in 0..joints {
        let j = (i + 1) % n;
        let corner = chain.segs[i].end();
        let (mut a, mut b) = (segs[i], segs[j]);
        bridges[i] = connect(&mut a, &mut b, corner, plane, tol);
        segs[i] = a;
        segs[j] = b;
    }
    let mut out = Vec::new();
    for i in 0..n {
        out.push(segs[i]);
        if let Some(b) = bridges[i] {
            out.push(b);
        }
    }
    Ok(Chain::new(out))
}

/// Sorted, de-duplicated chain parameters where `chain` crosses `cutters`.
fn cut_params(chain: &Chain, cutters: &[Chain], plane: &Plane, tol: f64) -> Vec<f64> {
    let mut ts: Vec<f64> = Vec::new();
    for (i, s) in chain.segs.iter().enumerate() {
        for c in cutters {
            for k in &c.segs {
                for t in seg_hits(s, k, plane, tol) {
                    ts.push(i as f64 + t);
                }
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|a, b| chain.point_at(*a).distance_to(chain.point_at(*b)) <= tol);
    ts
}

/// Points where two chains cross inside `plane` (both must lie in it).
pub fn crossings(a: &Chain, b: &Chain, plane: &Plane, tol: f64) -> Vec<Point3> {
    let mut out: Vec<Point3> = Vec::new();
    for t in cut_params(a, std::slice::from_ref(b), plane, tol) {
        let p = a.point_at(t);
        if !out.iter().any(|q| q.distance_to(p) <= tol) {
            out.push(p);
        }
    }
    out
}

/// Cut `chain` at every crossing with `cutters`, keeping all the pieces (Rhino's
/// Split). Crossings at the ends of an open chain are ignored; a closed chain is
/// cut into as many pieces as it has crossings (the piece across the seam is
/// joined). Errors when there is nothing to split at.
pub fn split(
    chain: &Chain,
    cutters: &[Chain],
    plane: &Plane,
    tol: f64,
) -> Result<Vec<Chain>, CurveError> {
    let n = chain.segs.len() as f64;
    let closed = chain.is_closed(tol);
    let mut ts = cut_params(chain, cutters, plane, tol);
    if closed {
        if ts.len() > 1
            && chain
                .point_at(ts[0])
                .distance_to(chain.point_at(*ts.last().expect("len")))
                <= tol
        {
            ts.pop();
        }
        if ts.is_empty() {
            return Err(CurveError::NoIntersection);
        }
        let mut out = Vec::new();
        for w in ts.windows(2) {
            let c = chain.sub(w[0], w[1], tol);
            if !c.segs.is_empty() {
                out.push(c.simplified(tol));
            }
        }
        // Last crossing -> seam -> first crossing.
        let mut c = chain.sub(*ts.last().expect("len"), n, tol);
        c.segs.extend(chain.sub(0.0, ts[0], tol).segs);
        if !c.segs.is_empty() {
            out.push(c.simplified(tol));
        }
        return Ok(out);
    }
    ts.retain(|t| {
        let p = chain.point_at(*t);
        p.distance_to(chain.start()) > tol && p.distance_to(chain.end()) > tol
    });
    if ts.is_empty() {
        return Err(CurveError::NoIntersection);
    }
    let mut bounds = vec![0.0];
    bounds.extend(ts);
    bounds.push(n);
    Ok(bounds
        .windows(2)
        .map(|w| chain.sub(w[0], w[1], tol))
        .filter(|c| !c.segs.is_empty())
        .collect())
}

/// Remove the part of `chain` around `pick` bounded by the nearest crossings with
/// `cutters`. Returns the remaining pieces (zero, one or two chains).
pub fn trim(
    chain: &Chain,
    cutters: &[Chain],
    pick: Point3,
    plane: &Plane,
    tol: f64,
) -> Result<Vec<Chain>, CurveError> {
    let n = chain.segs.len() as f64;
    let mut ts = cut_params(chain, cutters, plane, tol);
    let (tp, _) = chain.closest(pick);
    let closed = chain.is_closed(tol);
    if closed {
        // Crossings at the seam count once.
        if ts.len() > 1
            && chain
                .point_at(ts[0])
                .distance_to(chain.point_at(*ts.last().expect("len")))
                <= tol
        {
            ts.pop();
        }
        if ts.len() < 2 {
            return Err(CurveError::NoIntersection);
        }
        let after = ts.iter().copied().find(|t| *t > tp).unwrap_or(ts[0]);
        let before = ts
            .iter()
            .copied()
            .rev()
            .find(|t| *t < tp)
            .unwrap_or(*ts.last().expect("len"));
        // Keep from `after` forward to `before`, wrapping through the seam.
        let kept = if after < before {
            chain.sub(after, before, tol)
        } else {
            let mut c = chain.sub(after, n, tol);
            c.segs.extend(chain.sub(0.0, before, tol).segs);
            c
        };
        return Ok(vec![kept.simplified(tol)]);
    }
    let before = ts.iter().copied().rev().find(|t| *t < tp - 1e-12);
    let after = ts.iter().copied().find(|t| *t > tp + 1e-12);
    if before.is_none() && after.is_none() {
        return Err(CurveError::NoIntersection);
    }
    let mut out = Vec::new();
    if let Some(b) = before {
        let c = chain.sub(0.0, b, tol);
        if !c.segs.is_empty() {
            out.push(c);
        }
    }
    if let Some(a) = after {
        let c = chain.sub(a, n, tol);
        if !c.segs.is_empty() {
            out.push(c);
        }
    }
    Ok(out)
}

/// How far to extend an open chain.
pub enum ExtendTo<'a> {
    Boundaries(&'a [Chain]),
    Length(f64),
}

/// Extend the end of an open chain nearest to `pick`: lines straight on, arcs
/// around their circle.
pub fn extend(
    chain: &Chain,
    to: ExtendTo<'_>,
    pick: Point3,
    plane: &Plane,
    tol: f64,
) -> Result<Chain, CurveError> {
    if chain.is_closed(tol) {
        return Err(CurveError::Closed);
    }
    let from_start = pick.distance_to(chain.start()) < pick.distance_to(chain.end());
    let mut c = if from_start {
        chain.reversed()
    } else {
        chain.clone()
    };
    let last = *c.segs.last().expect("non-empty chain");
    let t_new = match to {
        ExtendTo::Length(l) => {
            if l <= 0.0 {
                return Err(CurveError::Degenerate("extension length must be positive"));
            }
            1.0 + l / last.length().max(1e-12)
        }
        ExtendTo::Boundaries(bounds) => {
            let mut best = f64::INFINITY;
            for b in bounds {
                for k in &b.segs {
                    for (ta, tb) in carrier_intersections(&last, k, plane) {
                        if ta > 1.0 + tol / last.length().max(1e-12)
                            && in_range(k, tb, tol)
                            && ta < best
                        {
                            best = ta;
                        }
                    }
                }
            }
            if !best.is_finite() {
                return Err(CurveError::NoIntersection);
            }
            best
        }
    };
    let t_new = match last {
        Seg::Arc(a) => t_new.min(TAU / a.sweep),
        Seg::Line(..) => t_new,
    };
    *c.segs.last_mut().expect("non-empty") = last.sub(0.0, t_new);
    Ok(if from_start { c.reversed() } else { c })
}

/// Join chains whose end points meet within `tol` into as few chains as possible.
pub fn join(chains: Vec<Chain>, tol: f64) -> Vec<Chain> {
    let mut pool: Vec<Chain> = chains.into_iter().filter(|c| !c.segs.is_empty()).collect();
    let mut out = Vec::new();
    while let Some(mut cur) = pool.pop() {
        loop {
            if cur.is_closed(tol) {
                break;
            }
            let mut grew = false;
            let mut i = 0;
            while i < pool.len() {
                let c = &pool[i];
                let (e, s) = (cur.end(), cur.start());
                let attach = if c.start().distance_to(e) <= tol {
                    Some((false, false))
                } else if c.end().distance_to(e) <= tol {
                    Some((false, true))
                } else if c.end().distance_to(s) <= tol {
                    Some((true, false))
                } else if c.start().distance_to(s) <= tol {
                    Some((true, true))
                } else {
                    None
                };
                if let Some((front, rev)) = attach {
                    let c = pool.swap_remove(i);
                    let c = if rev { c.reversed() } else { c };
                    if front {
                        let mut segs = c.segs;
                        segs.extend(cur.segs);
                        cur.segs = segs;
                    } else {
                        cur.segs.extend(c.segs);
                    }
                    grew = true;
                } else {
                    i += 1;
                }
            }
            if !grew {
                break;
            }
        }
        out.push(cur);
    }
    out.reverse();
    out
}

/// Fillet arc between two lines `(a0, a1)` and `(b0, b1)` that meet (or would
/// meet) at a corner. Returns the trimmed lines and the arc (None for radius 0,
/// which just extends both lines to the corner).
pub fn fillet_lines(
    a: (Point3, Point3),
    b: (Point3, Point3),
    radius: f64,
    plane: &Plane,
) -> Result<(Seg, Seg, Option<CircleArc>), CurveError> {
    let la = Seg::Line(a.0, a.1);
    let lb = Seg::Line(b.0, b.1);
    let hit = carrier_intersections(&la, &lb, plane);
    let (ta, _) = *hit
        .first()
        .ok_or(CurveError::Degenerate("lines are parallel"))?;
    let x = la.point_at(ta);
    let far = |p: (Point3, Point3)| {
        if p.0.distance_to(x) > p.1.distance_to(x) {
            p.0
        } else {
            p.1
        }
    };
    let (fa, fb) = (far(a), far(b));
    let corner = fillet_corner(fa, x, fb, radius)?;
    match corner {
        None => Ok((Seg::Line(fa, x), Seg::Line(x, fb), None)),
        Some((t1, t2, arc)) => Ok((Seg::Line(fa, t1), Seg::Line(t2, fb), Some(arc))),
    }
}

/// Chamfer between two lines that meet (or would meet) at a corner: the first line
/// is cut back by `d1` from the corner, the second by `d2`. Returns the trimmed
/// lines and the chamfer segment (None when both distances are 0, which just
/// extends the lines to the corner).
pub fn chamfer_lines(
    a: (Point3, Point3),
    b: (Point3, Point3),
    d1: f64,
    d2: f64,
    plane: &Plane,
) -> Result<(Seg, Seg, Option<Seg>), CurveError> {
    if d1 < 0.0 || d2 < 0.0 {
        return Err(CurveError::Degenerate("distances must not be negative"));
    }
    let la = Seg::Line(a.0, a.1);
    let lb = Seg::Line(b.0, b.1);
    let hit = carrier_intersections(&la, &lb, plane);
    let (ta, _) = *hit
        .first()
        .ok_or(CurveError::Degenerate("lines are parallel"))?;
    let x = la.point_at(ta);
    let far = |p: (Point3, Point3)| {
        if p.0.distance_to(x) > p.1.distance_to(x) {
            p.0
        } else {
            p.1
        }
    };
    let (fa, fb) = (far(a), far(b));
    if d1 < 1e-12 && d2 < 1e-12 {
        return Ok((Seg::Line(fa, x), Seg::Line(x, fb), None));
    }
    if d1 >= fa.distance_to(x) - 1e-9 || d2 >= fb.distance_to(x) - 1e-9 {
        return Err(CurveError::Degenerate("chamfer distance too large"));
    }
    let u1 = (fa - x)
        .normalized()
        .ok_or(CurveError::Degenerate("zero-length line"))?;
    let u2 = (fb - x)
        .normalized()
        .ok_or(CurveError::Degenerate("zero-length line"))?;
    let t1 = x + u1 * d1;
    let t2 = x + u2 * d2;
    Ok((
        Seg::Line(fa, t1),
        Seg::Line(t2, fb),
        Some(Seg::Line(t1, t2)),
    ))
}

/// Tangent points and arc rounding the corner `x` between `p` and `q`.
#[allow(clippy::type_complexity)]
fn fillet_corner(
    p: Point3,
    x: Point3,
    q: Point3,
    radius: f64,
) -> Result<Option<(Point3, Point3, CircleArc)>, CurveError> {
    if radius < 0.0 {
        return Err(CurveError::Degenerate("radius must not be negative"));
    }
    if radius < 1e-12 {
        return Ok(None);
    }
    let u1 = (p - x)
        .normalized()
        .ok_or(CurveError::Degenerate("zero-length line"))?;
    let u2 = (q - x)
        .normalized()
        .ok_or(CurveError::Degenerate("zero-length line"))?;
    let cos = u1.dot(u2).clamp(-1.0, 1.0);
    let half = cos.acos() / 2.0;
    if half < 1e-6 || (std::f64::consts::FRAC_PI_2 - half) < 1e-6 {
        return Err(CurveError::Degenerate("lines are parallel"));
    }
    let d = radius / half.tan();
    if d > p.distance_to(x) + 1e-9 || d > q.distance_to(x) + 1e-9 {
        return Err(CurveError::Degenerate("radius too large"));
    }
    let t1 = x + u1 * d;
    let t2 = x + u2 * d;
    let bis = (u1 + u2)
        .normalized()
        .ok_or(CurveError::Degenerate("lines are parallel"))?;
    let c = x + bis * (radius / half.sin());
    let n = (t1 - c)
        .cross(t2 - c)
        .normalized()
        .ok_or(CurveError::Degenerate("degenerate fillet"))?;
    let arc = CircleArc::from_center_start_end(c, t1, t2, n)
        .ok_or(CurveError::Degenerate("degenerate fillet"))?;
    Ok(Some((t1, t2, arc)))
}

/// Round every corner of a polyline (Rhino's FilletCorners). Corners where the
/// radius does not fit are left sharp.
pub fn fillet_corners(points: &[Point3], radius: f64, tol: f64) -> Result<Chain, CurveError> {
    let closed = points.len() > 3 && points[0].distance_to(*points.last().expect("len")) <= tol;
    let pts: Vec<Point3> = if closed {
        points[..points.len() - 1].to_vec()
    } else {
        points.to_vec()
    };
    let n = pts.len();
    if n < 3 {
        return Err(CurveError::Degenerate("needs at least one corner"));
    }
    // For each corner: (incoming end, arc, outgoing start).
    let mut corners: Vec<Option<(Point3, Point3, CircleArc)>> = vec![None; n];
    for i in 0..n {
        if !closed && (i == 0 || i == n - 1) {
            continue;
        }
        let (p, x, q) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        // Each side can give at most half its length to a fillet.
        let mid_p = p.mid(x);
        let mid_q = x.mid(q);
        corners[i] = fillet_corner(mid_p, x, mid_q, radius).ok().flatten();
    }
    let mut segs = Vec::new();
    let edges = if closed { n } else { n - 1 };
    let start_of = |i: usize| corners[i].map_or(pts[i], |c| c.1);
    let end_of = |j: usize| corners[j].map_or(pts[j], |c| c.0);
    for i in 0..edges {
        let j = (i + 1) % n;
        let (a, b) = (start_of(i), end_of(j));
        if a.distance_to(b) > tol {
            segs.push(Seg::Line(a, b));
        }
        if let Some((_, _, arc)) = corners[j] {
            if closed || j != n - 1 {
                segs.push(Seg::Arc(arc));
            }
        }
    }
    Ok(Chain::new(segs))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-6;

    #[test]
    fn split_open_and_closed() {
        let line = Chain::from_points(&[p(0.0, 5.0), p(20.0, 5.0)]);
        let cutters = [
            Chain::from_points(&[p(5.0, 0.0), p(5.0, 10.0)]),
            Chain::from_points(&[p(12.0, 0.0), p(12.0, 10.0)]),
        ];
        let pieces = split(&line, &cutters, &Plane::TOP, TOL).unwrap();
        assert_eq!(pieces.len(), 3);
        assert!(pieces[0].end().distance_to(p(5.0, 5.0)) < TOL);
        assert!(pieces[2].start().distance_to(p(12.0, 5.0)) < TOL);
        let total: f64 = pieces
            .iter()
            .flat_map(|c| c.segs.iter().map(Seg::length))
            .sum();
        assert!((total - 20.0).abs() < 1e-9);
        // A closed square crossed twice by a line → two pieces.
        let sq = square();
        let cut = [Chain::from_points(&[p(5.0, -5.0), p(5.0, 15.0)])];
        let pieces = split(&sq, &cut, &Plane::TOP, TOL).unwrap();
        assert_eq!(pieces.len(), 2);
        let total: f64 = pieces
            .iter()
            .flat_map(|c| c.segs.iter().map(Seg::length))
            .sum();
        assert!((total - 40.0).abs() < 1e-9, "{total}");
        assert!(split(
            &line,
            &[Chain::from_points(&[p(0.0, 9.0), p(1.0, 9.0)])],
            &Plane::TOP,
            TOL
        )
        .is_err());
    }

    #[test]
    fn chamfer_two_lines() {
        let (a, b, c) = chamfer_lines(
            (p(0.0, 0.0), p(10.0, 0.0)),
            (p(10.0, 0.0), p(10.0, 10.0)),
            2.0,
            3.0,
            &Plane::TOP,
        )
        .unwrap();
        assert!(a.end().distance_to(p(8.0, 0.0)) < TOL);
        assert!(b.start().distance_to(p(10.0, 3.0)) < TOL);
        let c = c.unwrap();
        assert!((c.length() - 13f64.sqrt()).abs() < 1e-9);
        assert!(chamfer_lines(
            (p(0.0, 0.0), p(10.0, 0.0)),
            (p(10.0, 0.0), p(10.0, 10.0)),
            20.0,
            3.0,
            &Plane::TOP,
        )
        .is_err());
    }

    #[test]
    fn crossings_of_circle_and_line() {
        let c = Chain::new(vec![Seg::Arc(CircleArc::circle(Plane::TOP, 5.0))]);
        let l = Chain::from_points(&[p(-10.0, 0.0), p(10.0, 0.0)]);
        let x = crossings(&c, &l, &Plane::TOP, TOL);
        assert_eq!(x.len(), 2, "{x:?}");
    }

    fn p(x: f64, y: f64) -> Point3 {
        Point3::new(x, y, 0.0)
    }

    fn square() -> Chain {
        Chain::from_points(&[
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 0.0),
        ])
    }

    #[test]
    fn offset_square_outwards_and_inwards() {
        let sq = square();
        let side = side_of(&sq, p(20.0, 5.0), &Plane::TOP);
        let out = offset(&sq, 2.0, side, &Plane::TOP, TOL).unwrap();
        let pts = out.points();
        assert_eq!(pts.len(), 5);
        assert!(
            pts.iter().any(|q| q.distance_to(p(12.0, 12.0)) < 1e-9),
            "{pts:?}"
        );
        assert!(
            pts.iter().any(|q| q.distance_to(p(-2.0, -2.0)) < 1e-9),
            "{pts:?}"
        );
        let side = side_of(&sq, p(5.0, 5.0), &Plane::TOP);
        let inner = offset(&sq, 1.0, side, &Plane::TOP, TOL).unwrap();
        assert!(inner
            .points()
            .iter()
            .any(|q| q.distance_to(p(9.0, 9.0)) < 1e-9));
        assert!(inner.is_closed(TOL));
    }

    #[test]
    fn offset_open_polyline_and_circle() {
        let l = Chain::from_points(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        let o = offset(
            &l,
            1.0,
            side_of(&l, p(0.0, 5.0), &Plane::TOP),
            &Plane::TOP,
            TOL,
        )
        .unwrap();
        let pts = o.points();
        assert!(pts[0].distance_to(p(0.0, 1.0)) < 1e-9, "{pts:?}");
        assert!(pts[1].distance_to(p(9.0, 1.0)) < 1e-9, "{pts:?}");
        assert!(pts[2].distance_to(p(9.0, 10.0)) < 1e-9, "{pts:?}");
        let c = Chain::new(vec![Seg::Arc(CircleArc::circle(Plane::TOP, 5.0))]);
        let o = offset(
            &c,
            2.0,
            side_of(&c, p(20.0, 0.0), &Plane::TOP),
            &Plane::TOP,
            TOL,
        )
        .unwrap();
        match o.segs[0] {
            Seg::Arc(a) => assert!((a.radius - 7.0).abs() < 1e-9),
            Seg::Line(..) => panic!(),
        }
    }

    #[test]
    fn trim_line_between_two_cutters() {
        let l = Chain::from_points(&[p(0.0, 0.0), p(10.0, 0.0)]);
        let c1 = Chain::from_points(&[p(3.0, -5.0), p(3.0, 5.0)]);
        let c2 = Chain::new(vec![Seg::Arc(CircleArc::circle(
            Plane::TOP.moved_to(p(7.0, 0.0)),
            1.0,
        ))]);
        let pieces = trim(&l, &[c1, c2], p(5.0, 0.1), &Plane::TOP, TOL).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(pieces[0].end().distance_to(p(3.0, 0.0)) < 1e-9);
        assert!(pieces[1].start().distance_to(p(6.0, 0.0)) < 1e-9);
    }

    #[test]
    fn trim_circle_by_line_keeps_the_other_arc() {
        let c = Chain::new(vec![Seg::Arc(CircleArc::circle(Plane::TOP, 5.0))]);
        let cut = Chain::from_points(&[p(0.0, -10.0), p(0.0, 10.0)]);
        let pieces = trim(&c, &[cut], p(5.0, 0.0), &Plane::TOP, TOL).unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].segs.len(), 1);
        let Seg::Arc(a) = pieces[0].segs[0] else {
            panic!()
        };
        assert!((a.sweep - std::f64::consts::PI).abs() < 1e-9);
        assert!(a.mid().distance_to(p(-5.0, 0.0)) < 1e-9, "{:?}", a.mid());
    }

    #[test]
    fn extend_line_to_boundary_and_by_length() {
        let l = Chain::from_points(&[p(0.0, 0.0), p(5.0, 0.0)]);
        let b = Chain::from_points(&[p(8.0, -5.0), p(8.0, 5.0)]);
        let e = extend(
            &l,
            ExtendTo::Boundaries(&[b]),
            p(4.0, 0.0),
            &Plane::TOP,
            TOL,
        )
        .unwrap();
        assert!(e.end().distance_to(p(8.0, 0.0)) < 1e-9);
        let e = extend(&l, ExtendTo::Length(2.0), p(0.5, 0.0), &Plane::TOP, TOL).unwrap();
        assert!(e.start().distance_to(p(-2.0, 0.0)) < 1e-9);
        assert!(e.end().distance_to(p(5.0, 0.0)) < 1e-9);
    }

    #[test]
    fn extend_arc_to_line() {
        let a = CircleArc::from_center_start_end(Point3::ORIGIN, p(5.0, 0.0), p(0.0, 5.0), Vec3::Z)
            .unwrap();
        let c = Chain::new(vec![Seg::Arc(a)]);
        let b = Chain::from_points(&[p(-10.0, 0.0), p(0.0, 0.0)]);
        let e = extend(
            &c,
            ExtendTo::Boundaries(&[b]),
            p(0.0, 5.0),
            &Plane::TOP,
            TOL,
        )
        .unwrap();
        assert!(e.end().distance_to(p(-5.0, 0.0)) < 1e-9, "{:?}", e.end());
    }

    #[test]
    fn join_chains_in_any_direction() {
        let a = Chain::from_points(&[p(0.0, 0.0), p(1.0, 0.0)]);
        let b = Chain::from_points(&[p(1.0, 1.0), p(1.0, 0.0)]);
        let c = Chain::from_points(&[p(1.0, 1.0), p(0.0, 0.0)]);
        let d = Chain::from_points(&[p(5.0, 5.0), p(6.0, 5.0)]);
        let j = join(vec![a, b, c, d], TOL);
        assert_eq!(j.len(), 2);
        assert!(j.iter().any(|c| c.segs.len() == 3 && c.is_closed(TOL)));
    }

    #[test]
    fn fillet_two_lines_and_corners() {
        let (a, b, arc) = fillet_lines(
            (p(0.0, 0.0), p(10.0, 0.0)),
            (p(12.0, 2.0), p(12.0, 10.0)),
            2.0,
            &Plane::TOP,
        )
        .unwrap();
        let arc = arc.unwrap();
        assert!(a.end().distance_to(p(10.0, 0.0)) < 1e-9, "{a:?}");
        assert!(b.start().distance_to(p(12.0, 2.0)) < 1e-9, "{b:?}");
        assert!(arc.center().distance_to(p(10.0, 2.0)) < 1e-9);
        assert!((arc.sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let c = fillet_corners(&square().points(), 1.0, TOL).unwrap();
        assert_eq!(c.segs.len(), 8);
        assert!(Chain::new(c.segs.clone()).is_closed(1e-9));
        for w in c.segs.windows(2) {
            assert!(w[0].end().distance_to(w[1].start()) < 1e-9);
        }
    }
}
