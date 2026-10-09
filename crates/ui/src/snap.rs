//! Object snaps, grid snap, ortho, picking and window selection.

use crate::viewport::Viewport;
use eframe::egui::{Pos2, Rect};
use forma_doc::{Document, Geometry, ObjectId};
use forma_geom::{CircleArc, Plane, Point3, Seg, Vec3};
use forma_render::glam::DVec3;

/// Rhino's object snaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    End,
    Near,
    Point,
    Mid,
    Cen,
    Int,
    Perp,
    Tan,
    Quad,
    Knot,
    Vertex,
}

impl SnapKind {
    /// In the order of Rhino's Osnap bar.
    pub const ALL: [SnapKind; 11] = [
        SnapKind::End,
        SnapKind::Near,
        SnapKind::Point,
        SnapKind::Mid,
        SnapKind::Cen,
        SnapKind::Int,
        SnapKind::Perp,
        SnapKind::Tan,
        SnapKind::Quad,
        SnapKind::Knot,
        SnapKind::Vertex,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SnapKind::End => "End",
            SnapKind::Near => "Near",
            SnapKind::Point => "Point",
            SnapKind::Mid => "Mid",
            SnapKind::Cen => "Cen",
            SnapKind::Int => "Int",
            SnapKind::Perp => "Perp",
            SnapKind::Tan => "Tan",
            SnapKind::Quad => "Quad",
            SnapKind::Knot => "Knot",
            SnapKind::Vertex => "Vertex",
        }
    }

    fn index(self) -> usize {
        SnapKind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// Lower wins when two snaps are about as close.
    fn priority(self) -> u8 {
        match self {
            SnapKind::End | SnapKind::Point => 0,
            SnapKind::Int => 1,
            SnapKind::Cen => 2,
            SnapKind::Quad | SnapKind::Knot | SnapKind::Vertex => 3,
            SnapKind::Mid => 4,
            SnapKind::Perp | SnapKind::Tan => 5,
            SnapKind::Near => 9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapSettings {
    /// One flag per [`SnapKind::ALL`].
    pub on: [bool; 11],
    /// Project snapped points onto the construction plane.
    pub project: bool,
    /// All object snaps off (Rhino's "Disable").
    pub disabled: bool,
    pub grid: bool,
    pub ortho: bool,
    /// New points stay on the plane of the first point of the command.
    pub planar: bool,
    /// SmartTrack: horizontal / vertical alignment with recent snap points.
    pub smart: bool,
    /// Grid snap step in model units.
    pub step: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        let mut on = [false; 11];
        for k in [
            SnapKind::End,
            SnapKind::Point,
            SnapKind::Mid,
            SnapKind::Cen,
            SnapKind::Int,
            SnapKind::Quad,
        ] {
            on[k.index()] = true;
        }
        SnapSettings {
            on,
            project: false,
            disabled: false,
            grid: true,
            ortho: false,
            planar: true,
            smart: true,
            step: 10.0,
        }
    }
}

impl SnapSettings {
    pub fn enabled(&self, k: SnapKind) -> bool {
        !self.disabled && self.on[k.index()]
    }

    pub fn flag(&mut self, k: SnapKind) -> &mut bool {
        &mut self.on[k.index()]
    }
}

/// Snap data of the visible objects: fixed points and curves for the
/// cursor-dependent snaps (Near, Int, Perp, Tan).
#[derive(Default)]
pub struct SnapPoints {
    points: Vec<(Point3, SnapKind)>,
    /// Polylines of visible curves (and arcs for Tan / Perp).
    curves: Vec<CurveSnap>,
}

struct CurveSnap {
    pts: Vec<Point3>,
    arcs: Vec<CircleArc>,
}

impl SnapPoints {
    pub fn build(doc: &Document) -> SnapPoints {
        let mut points = Vec::new();
        let mut curves = Vec::new();
        for o in doc.objects() {
            if !doc.is_visible(o) {
                continue;
            }
            let mut arcs = Vec::new();
            match &o.geometry {
                Geometry::Line(l) => {
                    points.push((l.from, SnapKind::End));
                    points.push((l.to, SnapKind::End));
                    points.push((l.from.mid(l.to), SnapKind::Mid));
                }
                Geometry::Polyline(p) => {
                    if let (Some(a), Some(b)) = (p.first(), p.last()) {
                        points.push((*a, SnapKind::End));
                        points.push((*b, SnapKind::End));
                    }
                    for q in p.iter().skip(1).take(p.len().saturating_sub(2)) {
                        // Rhino snaps End on polyline corners too.
                        points.push((*q, SnapKind::End));
                        points.push((*q, SnapKind::Knot));
                    }
                    for w in p.windows(2) {
                        points.push((w[0].mid(w[1]), SnapKind::Mid));
                    }
                }
                Geometry::Arc(a) => {
                    arcs.push(*a);
                    points.push((a.center(), SnapKind::Cen));
                    if a.is_closed() {
                        for k in 0..4 {
                            points.push((
                                a.point_at_angle(k as f64 * std::f64::consts::FRAC_PI_2),
                                SnapKind::Quad,
                            ));
                        }
                    } else {
                        points.push((a.start(), SnapKind::End));
                        points.push((a.end(), SnapKind::End));
                        points.push((a.mid(), SnapKind::Mid));
                        for k in 0..4 {
                            let ang = k as f64 * std::f64::consts::FRAC_PI_2;
                            if ang > 1e-9 && ang < a.sweep - 1e-9 {
                                points.push((a.point_at_angle(ang), SnapKind::Quad));
                            }
                        }
                    }
                }
                Geometry::PolyCurve(segs) => {
                    for s in segs {
                        points.push((s.start(), SnapKind::End));
                        points.push((s.end(), SnapKind::End));
                        points.push((s.point_at(0.5), SnapKind::Mid));
                        if let Seg::Arc(a) = s {
                            arcs.push(*a);
                            points.push((a.center(), SnapKind::Cen));
                        }
                    }
                }
                Geometry::Point(p) => points.push((*p, SnapKind::Point)),
                Geometry::Nurbs(n) => {
                    points.push((n.start(), SnapKind::End));
                    points.push((n.end(), SnapKind::End));
                    let (t0, t1) = n.domain();
                    points.push((n.point_at((t0 + t1) / 2.0), SnapKind::Mid));
                    for k in n.knots.windows(2) {
                        if k[1] > k[0] + 1e-12 && k[1] < t1 - 1e-12 {
                            points.push((n.point_at(k[1]), SnapKind::Knot));
                        }
                    }
                }
                Geometry::Mesh(m) => {
                    // Corners and edge midpoints of small solids (boxes, extruded
                    // rectangles); big imported meshes would only add noise.
                    if m.positions.len() <= 64 {
                        for [a, b] in m.boundary_edges() {
                            let (pa, pb) = (m.positions[a as usize], m.positions[b as usize]);
                            points.push((pa, SnapKind::End));
                            points.push((pa.mid(pb), SnapKind::Mid));
                        }
                    }
                    if m.positions.len() <= 20_000 {
                        points.extend(m.positions.iter().map(|p| (*p, SnapKind::Vertex)));
                    }
                }
            }
            if o.geometry.is_curve() {
                let pts = o.geometry.curve_points();
                if pts.len() >= 2 {
                    curves.push(CurveSnap { pts, arcs });
                }
            }
        }
        SnapPoints { points, curves }
    }

    /// Nearest enabled snap within `radius` screen points of `pos`. `base` is the
    /// previous point of the command (for Perp and Tan).
    pub fn find(
        &self,
        vp: &Viewport,
        pos: Pos2,
        origin: DVec3,
        settings: &SnapSettings,
        radius: f32,
        base: Option<Point3>,
    ) -> Option<(Point3, SnapKind)> {
        if settings.disabled {
            return None;
        }
        let mut best: Option<(f32, Point3, SnapKind)> = None;
        let consider = |p: Point3, k: SnapKind, best: &mut Option<(f32, Point3, SnapKind)>| {
            if !settings.enabled(k) {
                return;
            }
            let Some(s) = vp.to_screen(p, origin) else {
                return;
            };
            let d = s.distance(pos);
            if d <= radius
                && best.is_none_or(|b| {
                    d < b.0 - 1.5 || (d < b.0 + 1.5 && k.priority() < b.2.priority())
                })
            {
                *best = Some((d, p, k));
            }
        };
        for (p, k) in &self.points {
            consider(*p, *k, &mut best);
        }
        // Curves near the cursor, as screen polylines.
        let near: Vec<(&CurveSnap, Vec<Option<Pos2>>)> = self
            .curves
            .iter()
            .filter_map(|c| {
                let sc: Vec<Option<Pos2>> =
                    c.pts.iter().map(|p| vp.to_screen(*p, origin)).collect();
                let close = sc.windows(2).any(|w| match (w[0], w[1]) {
                    (Some(a), Some(b)) => dist_point_segment(pos, a, b) <= radius,
                    _ => false,
                });
                close.then_some((c, sc))
            })
            .take(12)
            .collect();
        for (c, sc) in &near {
            // Near: closest point on the curve.
            if settings.enabled(SnapKind::Near) {
                if let Some(p) = closest_on_screen_polyline(&c.pts, sc, pos) {
                    consider(p, SnapKind::Near, &mut best);
                }
            }
            if let Some(b) = base {
                if settings.enabled(SnapKind::Perp) {
                    for w in c.pts.windows(2) {
                        let d = w[1] - w[0];
                        let l2 = d.dot(d);
                        if l2 > 1e-18 {
                            let t = (b - w[0]).dot(d) / l2;
                            if (0.0..=1.0).contains(&t) {
                                consider(w[0] + d * t, SnapKind::Perp, &mut best);
                            }
                        }
                    }
                    for a in &c.arcs {
                        let (u, v, _) = a.plane.coords(b);
                        let ang = v.atan2(u).rem_euclid(std::f64::consts::TAU);
                        if ang <= a.sweep {
                            consider(a.point_at_angle(ang), SnapKind::Perp, &mut best);
                        }
                    }
                }
                if settings.enabled(SnapKind::Tan) {
                    for a in &c.arcs {
                        let (u, v, _) = a.plane.coords(b);
                        let d = u.hypot(v);
                        if d > a.radius + 1e-9 {
                            let phi = v.atan2(u);
                            let off = (a.radius / d).acos();
                            for ang in [phi + off, phi - off] {
                                let ang = ang.rem_euclid(std::f64::consts::TAU);
                                if ang <= a.sweep {
                                    consider(a.point_at_angle(ang), SnapKind::Tan, &mut best);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Int: apparent intersections between the nearby curves (and a curve with
        // itself is skipped).
        if settings.enabled(SnapKind::Int) {
            for i in 0..near.len() {
                for j in i + 1..near.len() {
                    let (a, sa) = &near[i];
                    let (_, sb) = &near[j];
                    for (ka, wa) in sa.windows(2).enumerate() {
                        let (Some(p0), Some(p1)) = (wa[0], wa[1]) else {
                            continue;
                        };
                        if dist_point_segment(pos, p0, p1) > radius * 2.0 {
                            continue;
                        }
                        for wb in sb.windows(2) {
                            let (Some(q0), Some(q1)) = (wb[0], wb[1]) else {
                                continue;
                            };
                            if let Some(t) = screen_cross(p0, p1, q0, q1) {
                                let w = a.pts[ka] + (a.pts[ka + 1] - a.pts[ka]) * t;
                                consider(w, SnapKind::Int, &mut best);
                            }
                        }
                    }
                }
            }
        }
        best.map(|(_, p, k)| (p, k))
    }
}

/// SmartTrack: align `p` (in `plane`) with the construction-plane axes through
/// the tracking points, when the cursor is within a few points of such a line.
/// `lock` keeps one coordinate (Ortho from a base point: `true` = moving along
/// the plane's x). Returns the aligned point and the tracking lines to draw.
pub fn smart_track(
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
    plane: &Plane,
    p: Point3,
    tracks: &[Point3],
    lock: Option<(Point3, bool)>,
) -> Option<(Point3, Vec<[Point3; 2]>)> {
    const RADIUS: f32 = 8.0;
    let (pu, pv, pw) = plane.coords(p);
    let screen_d = |u: f64, v: f64| {
        vp.to_screen(plane.point_at(u, v, pw), origin)
            .map_or(f32::INFINITY, |s| s.distance(pos))
    };
    // Best horizontal line (fixes v) and vertical line (fixes u).
    let mut best_h: Option<(f32, f64, Point3)> = None;
    let mut best_v: Option<(f32, f64, Point3)> = None;
    for t in tracks {
        let (tu, tv, _) = plane.coords(*t);
        if (tv - pv).abs() > 1e-9 || (tu - pu).abs() > 1e-9 {
            let dh = screen_d(pu, tv);
            if dh < RADIUS && best_h.is_none_or(|b| dh < b.0) {
                best_h = Some((dh, tv, *t));
            }
            let dv = screen_d(tu, pv);
            if dv < RADIUS && best_v.is_none_or(|b| dv < b.0) {
                best_v = Some((dv, tu, *t));
            }
        }
    }
    let (u, v) = match lock {
        Some((b, true)) => {
            let (_, bv, _) = plane.coords(b);
            (best_v?.1, bv)
        }
        Some((b, false)) => {
            let (bu, _, _) = plane.coords(b);
            (bu, best_h?.1)
        }
        None => match (best_h, best_v) {
            (Some(h), Some(vv)) => (vv.1, h.1),
            (Some(h), None) => (pu, h.1),
            (None, Some(vv)) => (vv.1, pv),
            (None, None) => return None,
        },
    };
    let q = plane.point_at(u, v, pw);
    let mut lines = Vec::new();
    for (b, fixed_v) in [(best_h, true), (best_v, false)] {
        if let Some((_, c, t)) = b {
            let on = if fixed_v {
                (v - c).abs() < 1e-9
            } else {
                (u - c).abs() < 1e-9
            };
            if on {
                lines.push([t, q]);
            }
        }
    }
    Some((q, lines))
}

/// Parameter on `a0→a1` where it crosses `b0→b1` in screen space.
fn screen_cross(a0: Pos2, a1: Pos2, b0: Pos2, b1: Pos2) -> Option<f64> {
    let r = a1 - a0;
    let s = b1 - b0;
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-6 {
        return None;
    }
    let qp = b0 - a0;
    let t = (qp.x * s.y - qp.y * s.x) / den;
    let u = (qp.x * r.y - qp.y * r.x) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(f64::from(t))
}

fn closest_on_screen_polyline(pts: &[Point3], sc: &[Option<Pos2>], pos: Pos2) -> Option<Point3> {
    let mut best: Option<(f32, Point3)> = None;
    for (k, w) in sc.windows(2).enumerate() {
        let (Some(a), Some(b)) = (w[0], w[1]) else {
            continue;
        };
        let ab = b - a;
        let t = if ab.length_sq() < 1e-6 {
            0.0
        } else {
            ((pos - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
        };
        let d = pos.distance(a + ab * t);
        if best.is_none_or(|bb| d < bb.0) {
            best = Some((d, pts[k] + (pts[k + 1] - pts[k]) * f64::from(t)));
        }
    }
    best.map(|b| b.1)
}

/// Round a point to the grid of `plane` (in-plane coordinates only).
pub fn grid_snap(p: Point3, plane: &Plane, step: f64) -> Point3 {
    if step <= 0.0 {
        return p;
    }
    let (u, v, w) = plane.coords(p);
    plane.point_at((u / step).round() * step, (v / step).round() * step, w)
}

/// Keep only the dominant in-plane direction from `base` (Rhino's Ortho).
pub fn ortho(p: Point3, base: Point3, plane: &Plane) -> Point3 {
    let d = p - base;
    let (u, v) = (d.dot(plane.x), d.dot(plane.y));
    if u.abs() >= v.abs() {
        base + plane.x * u
    } else {
        base + plane.y * v
    }
}

fn dist_point_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn ray_box(o: Point3, d: Vec3, min: Point3, max: Point3) -> bool {
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for (oo, dd, lo, hi) in [
        (o.x, d.x, min.x, max.x),
        (o.y, d.y, min.y, max.y),
        (o.z, d.z, min.z, max.z),
    ] {
        if dd.abs() < 1e-15 {
            if oo < lo || oo > hi {
                return false;
            }
        } else {
            let (a, b) = ((lo - oo) / dd, (hi - oo) / dd);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    t1 >= t0
}

/// Möller–Trumbore; returns the ray parameter.
fn ray_triangle(o: Point3, d: Vec3, a: Point3, b: Point3, c: Point3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-14 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

fn selectable(doc: &Document, o: &forma_doc::Object) -> bool {
    doc.is_selectable(o)
}

/// Surface under the cursor: (object, hit point, triangle), nearest to the eye.
pub fn pick_face(
    doc: &Document,
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
) -> Option<(ObjectId, Point3, usize)> {
    let (ro, rd) = vp.ray(pos, origin);
    let mut best: Option<(f64, ObjectId, usize)> = None;
    for o in doc.objects() {
        if !doc.is_selectable(o) {
            continue;
        }
        let Geometry::Mesh(m) = &o.geometry else {
            continue;
        };
        let Some(bb) = m.bounding_box() else { continue };
        if !ray_box(ro, rd, bb.min, bb.max) {
            continue;
        }
        for (i, t) in m.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|k| m.positions[k as usize]);
            if let Some(tt) = ray_triangle(ro, rd, a, b, c) {
                if best.is_none_or(|bb| tt < bb.0) {
                    best = Some((tt, o.id, i));
                }
            }
        }
    }
    best.map(|(t, id, tri)| (id, ro + rd * t, tri))
}

/// Object under the cursor: curves within a few points win, otherwise the nearest
/// surface hit by the ray.
pub fn pick(doc: &Document, vp: &Viewport, pos: Pos2, origin: DVec3) -> Option<ObjectId> {
    const CURVE_RADIUS: f32 = 6.0;
    let mut best_curve: Option<(f32, ObjectId)> = None;
    let (ro, rd) = vp.ray(pos, origin);
    let mut best_mesh: Option<(f64, ObjectId)> = None;
    for o in doc.objects() {
        if !selectable(doc, o) {
            continue;
        }
        match &o.geometry {
            Geometry::Mesh(m) => {
                let Some(bb) = m.bounding_box() else { continue };
                if !ray_box(ro, rd, bb.min, bb.max) {
                    continue;
                }
                for t in &m.triangles {
                    let [a, b, c] = t.map(|i| m.positions[i as usize]);
                    if let Some(tt) = ray_triangle(ro, rd, a, b, c) {
                        if best_mesh.is_none_or(|bm| tt < bm.0) {
                            best_mesh = Some((tt, o.id));
                        }
                    }
                }
            }
            Geometry::Point(p) => {
                if let Some(sp) = vp.to_screen(*p, origin) {
                    let d = sp.distance(pos);
                    if d <= CURVE_RADIUS && best_curve.is_none_or(|bc| d < bc.0) {
                        best_curve = Some((d, o.id));
                    }
                }
            }
            g => {
                let pts = g.curve_points();
                let screen: Vec<Option<Pos2>> =
                    pts.iter().map(|p| vp.to_screen(*p, origin)).collect();
                for w in screen.windows(2) {
                    if let (Some(a), Some(b)) = (w[0], w[1]) {
                        let d = dist_point_segment(pos, a, b);
                        if d <= CURVE_RADIUS && best_curve.is_none_or(|bc| d < bc.0) {
                            best_curve = Some((d, o.id));
                        }
                    }
                }
            }
        }
    }
    best_curve.map(|c| c.1).or(best_mesh.map(|m| m.1))
}

/// Point on the curve under the cursor (within a few screen points), used by
/// Trim, Extend and Fillet to know which curve and which part was clicked.
pub fn pick_curve_point(doc: &Document, vp: &Viewport, pos: Pos2, origin: DVec3) -> Option<Point3> {
    const RADIUS: f32 = 8.0;
    let mut best: Option<(f32, Point3)> = None;
    for o in doc.objects() {
        if !selectable(doc, o) || !o.geometry.is_curve() {
            continue;
        }
        let pts = o.geometry.curve_points();
        for w in pts.windows(2) {
            let (Some(a), Some(b)) = (vp.to_screen(w[0], origin), vp.to_screen(w[1], origin))
            else {
                continue;
            };
            let ab = b - a;
            let len2 = ab.length_sq();
            let t = if len2 < 1e-6 {
                0.0
            } else {
                ((pos - a).dot(ab) / len2).clamp(0.0, 1.0)
            };
            let d = pos.distance(a + ab * t);
            if d <= RADIUS && best.is_none_or(|bb| d < bb.0) {
                let p = w[0] + (w[1] - w[0]) * f64::from(t);
                best = Some((d, p));
            }
        }
    }
    best.map(|b| b.1)
}

/// Window (left→right: fully inside) or crossing (right→left: touching) selection.
pub fn window_select(
    doc: &Document,
    vp: &Viewport,
    rect: Rect,
    crossing: bool,
    origin: DVec3,
) -> Vec<ObjectId> {
    let mut out = Vec::new();
    for o in doc.objects() {
        if !selectable(doc, o) {
            continue;
        }
        let bb = o.geometry.bounding_box();
        let corners: Vec<Option<Pos2>> = bb
            .corners()
            .iter()
            .map(|c| vp.to_screen(*c, origin))
            .collect();
        if corners.iter().any(Option::is_none) {
            continue;
        }
        let sc: Vec<Pos2> = corners.into_iter().flatten().collect();
        let bbox = Rect::from_points(&sc);
        if !crossing {
            if rect.contains_rect(bbox) {
                out.push(o.id);
            }
            continue;
        }
        if !rect.intersects(bbox) {
            continue;
        }
        if rect.contains_rect(bbox) {
            out.push(o.id);
            continue;
        }
        let hit = match &o.geometry {
            Geometry::Mesh(m) => m
                .positions
                .iter()
                .any(|p| vp.to_screen(*p, origin).is_some_and(|s| rect.contains(s))),
            g => {
                let pts: Vec<Option<Pos2>> = g
                    .curve_points()
                    .iter()
                    .map(|p| vp.to_screen(*p, origin))
                    .collect();
                pts.iter().flatten().any(|s| rect.contains(*s))
                    || pts.windows(2).any(|w| match (w[0], w[1]) {
                        (Some(a), Some(b)) => segment_hits_rect(a, b, rect),
                        _ => false,
                    })
            }
        };
        if hit {
            out.push(o.id);
        }
    }
    out
}

fn segment_hits_rect(a: Pos2, b: Pos2, r: Rect) -> bool {
    let edges = [
        (r.left_top(), r.right_top()),
        (r.right_top(), r.right_bottom()),
        (r.right_bottom(), r.left_bottom()),
        (r.left_bottom(), r.left_top()),
    ];
    edges.iter().any(|(c, d)| segments_cross(a, b, *c, *d))
}

fn segments_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    let o = |p: Pos2, q: Pos2, r: Pos2| (q - p).x * (r - p).y - (q - p).y * (r - p).x;
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Wireframe of the selection used to preview transforms: curve polylines and mesh
/// edges (or bounding boxes for very large meshes).
pub fn selection_skeleton(
    doc: &Document,
    ids: &std::collections::BTreeSet<ObjectId>,
) -> Vec<[Point3; 2]> {
    let mut out = Vec::new();
    for id in ids {
        let Some(o) = doc.object(*id) else { continue };
        match &o.geometry {
            Geometry::Mesh(m) => {
                let edges = m.boundary_edges();
                if edges.len() <= 4000 {
                    out.extend(
                        edges
                            .iter()
                            .map(|[a, b]| [m.positions[*a as usize], m.positions[*b as usize]]),
                    );
                } else if let Some(bb) = m.bounding_box() {
                    let c = bb.corners();
                    for (i, j) in [
                        (0, 1),
                        (1, 2),
                        (2, 3),
                        (3, 0),
                        (4, 5),
                        (5, 6),
                        (6, 7),
                        (7, 4),
                        (0, 4),
                        (1, 5),
                        (2, 6),
                        (3, 7),
                    ] {
                        out.push([c[i], c[j]]);
                    }
                }
            }
            g => {
                let pts = g.curve_points();
                let step = (pts.len() / 2000).max(1);
                let pts: Vec<Point3> = pts.iter().step_by(step).copied().collect();
                out.extend(pts.windows(2).map(|w| [w[0], w[1]]));
            }
        }
        if out.len() > 50_000 {
            break;
        }
    }
    out
}

/// Centre of the selection's bounding box.
pub fn selection_center(
    doc: &Document,
    ids: &std::collections::BTreeSet<ObjectId>,
) -> Option<Point3> {
    let pts: Vec<Point3> = ids
        .iter()
        .filter_map(|id| doc.object(*id))
        .flat_map(|o| {
            let b = o.geometry.bounding_box();
            [b.min, b.max]
        })
        .collect();
    forma_geom::BoundingBox::from_points(&pts).map(|b| b.center())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_ortho() {
        let p = grid_snap(Point3::new(12.4, 7.6, 3.0), &Plane::TOP, 5.0);
        assert_eq!(p, Point3::new(10.0, 10.0, 3.0));
        let q = ortho(Point3::new(10.0, 3.0, 0.0), Point3::ORIGIN, &Plane::TOP);
        assert_eq!(q, Point3::new(10.0, 0.0, 0.0));
        let f = grid_snap(Point3::new(12.0, 0.0, 26.0), &Plane::FRONT, 10.0);
        assert_eq!(f, Point3::new(10.0, 0.0, 30.0));
    }

    #[test]
    fn ray_hits_triangle() {
        let t = ray_triangle(
            Point3::new(0.2, 0.2, 5.0),
            Vec3::new(0.0, 0.0, -1.0),
            Point3::ORIGIN,
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        );
        assert!((t.unwrap() - 5.0).abs() < 1e-12);
    }
}
