//! Object snaps, grid snap, ortho, picking and window selection.

use crate::viewport::Viewport;
use eframe::egui::{Pos2, Rect};
use forma_doc::{Document, Geometry, ObjectId};
use forma_geom::{Plane, Point3, Vec3};
use forma_render::glam::DVec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    End,
    Mid,
    Cen,
    Quad,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::End => "End",
            SnapKind::Mid => "Mid",
            SnapKind::Cen => "Cen",
            SnapKind::Quad => "Quad",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapSettings {
    pub end: bool,
    pub mid: bool,
    pub cen: bool,
    pub quad: bool,
    pub grid: bool,
    pub ortho: bool,
    /// Grid snap step in model units.
    pub step: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        SnapSettings {
            end: true,
            mid: true,
            cen: true,
            quad: true,
            grid: true,
            ortho: false,
            step: 10.0,
        }
    }
}

impl SnapSettings {
    fn enabled(&self, k: SnapKind) -> bool {
        match k {
            SnapKind::End => self.end,
            SnapKind::Mid => self.mid,
            SnapKind::Cen => self.cen,
            SnapKind::Quad => self.quad,
        }
    }
}

/// Snap candidates of the visible objects.
#[derive(Default)]
pub struct SnapPoints {
    points: Vec<(Point3, SnapKind)>,
}

impl SnapPoints {
    pub fn build(doc: &Document) -> SnapPoints {
        let mut points = Vec::new();
        for o in doc.objects() {
            if !doc.layer(o.layer).visible {
                continue;
            }
            match &o.geometry {
                Geometry::Line(l) => {
                    points.push((l.from, SnapKind::End));
                    points.push((l.to, SnapKind::End));
                    points.push((l.from.mid(l.to), SnapKind::Mid));
                }
                Geometry::Polyline(p) => {
                    for q in p {
                        points.push((*q, SnapKind::End));
                    }
                    for w in p.windows(2) {
                        points.push((w[0].mid(w[1]), SnapKind::Mid));
                    }
                }
                Geometry::Arc(a) => {
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
                    }
                }
                Geometry::PolyCurve(segs) => {
                    for s in segs {
                        points.push((s.start(), SnapKind::End));
                        points.push((s.end(), SnapKind::End));
                        points.push((s.point_at(0.5), SnapKind::Mid));
                        if let forma_geom::Seg::Arc(a) = s {
                            points.push((a.center(), SnapKind::Cen));
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
                }
            }
        }
        SnapPoints { points }
    }

    /// Nearest enabled snap within `radius` screen points of `pos`.
    pub fn find(
        &self,
        vp: &Viewport,
        pos: Pos2,
        origin: DVec3,
        settings: &SnapSettings,
        radius: f32,
    ) -> Option<(Point3, SnapKind)> {
        let mut best: Option<(f32, Point3, SnapKind)> = None;
        for (p, k) in &self.points {
            if !settings.enabled(*k) {
                continue;
            }
            let Some(s) = vp.to_screen(*p, origin) else {
                continue;
            };
            let d = s.distance(pos);
            if d <= radius
                && best.is_none_or(|b| {
                    d < b.0 - 0.5 || (d < b.0 + 0.5 && priority(*k) < priority(b.2))
                })
            {
                best = Some((d, *p, *k));
            }
        }
        best.map(|(_, p, k)| (p, k))
    }
}

fn priority(k: SnapKind) -> u8 {
    match k {
        SnapKind::End => 0,
        SnapKind::Cen => 1,
        SnapKind::Quad => 2,
        SnapKind::Mid => 3,
    }
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

fn selectable(doc: &Document, layer: forma_doc::LayerId) -> bool {
    let l = doc.layer(layer);
    l.visible && !l.locked
}

/// Object under the cursor: curves within a few points win, otherwise the nearest
/// surface hit by the ray.
pub fn pick(doc: &Document, vp: &Viewport, pos: Pos2, origin: DVec3) -> Option<ObjectId> {
    const CURVE_RADIUS: f32 = 6.0;
    let mut best_curve: Option<(f32, ObjectId)> = None;
    let (ro, rd) = vp.ray(pos, origin);
    let mut best_mesh: Option<(f64, ObjectId)> = None;
    for o in doc.objects() {
        if !selectable(doc, o.layer) {
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
        if !selectable(doc, o.layer) || !o.geometry.is_curve() {
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
        if !selectable(doc, o.layer) {
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
