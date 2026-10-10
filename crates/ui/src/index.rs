//! Per-object snap and pick data of the visible objects, kept up to date
//! incrementally (only objects whose revision changed are rebuilt) and searched
//! with screen-space culling, so big models stay responsive while the cursor
//! moves.

use crate::snap::{SnapKind, SnapSettings};
use crate::viewport::{Projector, Viewport};
use eframe::egui::Pos2;
use forma_doc::{Document, Geometry, ObjectId};
use forma_geom::{BoundingBox, CircleArc, Point3, Seg};
use forma_render::glam::DVec3;
use std::collections::HashMap;

/// Points of one snap kind, small enough to be culled as a block.
struct PointGroup {
    kind: SnapKind,
    min: Point3,
    max: Point3,
    pts: Vec<Point3>,
}

/// Group size: a block of points is culled with one 8-corner projection.
const GROUP: usize = 256;
/// Meshes with more vertices offer no Vertex snaps (keeps memory bounded).
const MAX_VERTEX_SNAPS: usize = 250_000;

/// What the index knows about one object.
pub struct Entry {
    rev: u64,
    pub min: Point3,
    pub max: Point3,
    groups: Vec<PointGroup>,
    /// Display polyline of a curve (also used to pick and window-select it).
    pub curve: Option<Vec<Point3>>,
    /// Arcs of a curve, for Tan / Perp.
    arcs: Vec<CircleArc>,
}

/// Snap and pick data of the visible objects.
#[derive(Default)]
pub struct SceneIndex {
    entries: HashMap<ObjectId, Entry>,
    /// Visible objects, in document order.
    visible: Vec<ObjectId>,
}

fn groups_of(points: Vec<(Point3, SnapKind)>) -> Vec<PointGroup> {
    let mut out: Vec<PointGroup> = Vec::new();
    for k in SnapKind::ALL {
        let pts: Vec<Point3> = points
            .iter()
            .filter(|(_, kk)| *kk == k)
            .map(|(p, _)| *p)
            .collect();
        for chunk in pts.chunks(GROUP) {
            let bb = BoundingBox::from_points(chunk).expect("non-empty chunk");
            out.push(PointGroup {
                kind: k,
                min: bb.min,
                max: bb.max,
                pts: chunk.to_vec(),
            });
        }
    }
    out
}

fn build_entry(o: &forma_doc::Object) -> Entry {
    let mut points = Vec::new();
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
            if m.positions.len() <= MAX_VERTEX_SNAPS {
                points.extend(m.positions.iter().map(|p| (*p, SnapKind::Vertex)));
            }
        }
        #[allow(unreachable_patterns)] // geometry kinds added later: no snaps yet
        _ => {}
    }
    let curve = o
        .geometry
        .is_curve()
        .then(|| o.geometry.curve_points())
        .filter(|p| p.len() >= 2);
    let bb = match (&o.geometry, &curve) {
        (Geometry::Point(p), _) => BoundingBox { min: *p, max: *p },
        (_, Some(pts)) => BoundingBox::from_points(pts).expect("two points"),
        (g, None) => g.bounding_box(),
    };
    let groups = groups_of(points);
    // The box covers every snap point too (an arc's centre lies outside it).
    let bb = groups.iter().fold(bb, |b, g| {
        b.union(BoundingBox {
            min: g.min,
            max: g.max,
        })
    });
    Entry {
        rev: o.rev,
        min: bb.min,
        max: bb.max,
        groups,
        curve,
        arcs,
    }
}

impl SceneIndex {
    /// Bring the index up to date with the document: objects whose revision
    /// changed are rebuilt, deleted ones dropped.
    pub fn update(&mut self, doc: &Document) {
        self.entries.retain(|id, _| doc.object(*id).is_some());
        self.visible.clear();
        for o in doc.objects() {
            if !doc.is_visible(o) {
                continue;
            }
            let stale = self.entries.get(&o.id).is_none_or(|e| e.rev != o.rev);
            if stale {
                self.entries.insert(o.id, build_entry(o));
            }
            self.visible.push(o.id);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.visible.clear();
    }

    pub fn get(&self, id: ObjectId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// Visible objects with their data, in document order.
    pub fn visible(&self) -> impl Iterator<Item = (ObjectId, &Entry)> {
        self.visible
            .iter()
            .filter_map(|id| self.entries.get(id).map(|e| (*id, e)))
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
        let pr = vp.projector(origin);
        let mut best: Option<(f32, Point3, SnapKind)> = None;
        let consider = |p: Point3, k: SnapKind, best: &mut Option<(f32, Point3, SnapKind)>| {
            let Some(s) = pr.to_screen(p) else {
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
        let want_curves = [SnapKind::Near, SnapKind::Perp, SnapKind::Tan, SnapKind::Int]
            .iter()
            .any(|k| settings.enabled(*k));
        // Curves near the cursor, as screen polylines.
        let mut near: Vec<(&Entry, Vec<Option<Pos2>>)> = Vec::new();
        for (_, e) in self.visible() {
            if !pr.box_near(e.min, e.max, pos, radius) {
                continue;
            }
            for g in &e.groups {
                if !settings.enabled(g.kind) {
                    continue;
                }
                if g.pts.len() > 8 && !pr.box_near(g.min, g.max, pos, radius) {
                    continue;
                }
                for p in &g.pts {
                    consider(*p, g.kind, &mut best);
                }
            }
            if let (true, Some(pts), true) = (want_curves, &e.curve, near.len() < 12) {
                let sc: Vec<Option<Pos2>> = pts.iter().map(|p| pr.to_screen(*p)).collect();
                let close = sc.windows(2).any(|w| match (w[0], w[1]) {
                    (Some(a), Some(b)) => dist_point_segment(pos, a, b) <= radius,
                    _ => false,
                });
                if close {
                    near.push((e, sc));
                }
            }
        }
        for (e, sc) in &near {
            let pts = e.curve.as_deref().unwrap_or_default();
            // Near: closest point on the curve.
            if settings.enabled(SnapKind::Near) {
                if let Some(p) = closest_on_screen_polyline(pts, sc, pos) {
                    consider(p, SnapKind::Near, &mut best);
                }
            }
            if let Some(b) = base {
                if settings.enabled(SnapKind::Perp) {
                    for w in pts.windows(2) {
                        let d = w[1] - w[0];
                        let l2 = d.dot(d);
                        if l2 > 1e-18 {
                            let t = (b - w[0]).dot(d) / l2;
                            if (0.0..=1.0).contains(&t) {
                                consider(w[0] + d * t, SnapKind::Perp, &mut best);
                            }
                        }
                    }
                    for a in &e.arcs {
                        let (u, v, _) = a.plane.coords(b);
                        let ang = v.atan2(u).rem_euclid(std::f64::consts::TAU);
                        if ang <= a.sweep {
                            consider(a.point_at_angle(ang), SnapKind::Perp, &mut best);
                        }
                    }
                }
                if settings.enabled(SnapKind::Tan) {
                    for a in &e.arcs {
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
        // Int: apparent intersections between the nearby curves (a curve with
        // itself is skipped).
        if settings.enabled(SnapKind::Int) {
            for i in 0..near.len() {
                for j in i + 1..near.len() {
                    let (a, sa) = &near[i];
                    let (_, sb) = &near[j];
                    let apts = a.curve.as_deref().unwrap_or_default();
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
                                let w = apts[ka] + (apts[ka + 1] - apts[ka]) * t;
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
        let (d, t) = segment_param(pos, a, b);
        if best.is_none_or(|bb| d < bb.0) {
            best = Some((d, pts[k] + (pts[k + 1] - pts[k]) * f64::from(t)));
        }
    }
    best.map(|b| b.1)
}

/// Distance from `p` to the screen segment `a→b` and the parameter of the
/// closest point.
pub fn segment_param(p: Pos2, a: Pos2, b: Pos2) -> (f32, f32) {
    let ab = b - a;
    let len2 = ab.length_sq();
    let t = if len2 < 1e-6 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    (p.distance(a + ab * t), t)
}

pub fn dist_point_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    segment_param(p, a, b).0
}

/// Screen points of a polyline (None where behind the camera).
pub fn screen_polyline(pr: &Projector, pts: &[Point3]) -> Vec<Option<Pos2>> {
    pts.iter().map(|p| pr.to_screen(*p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_render::StandardView;

    fn top_view() -> Viewport {
        let mut vp = Viewport::new(StandardView::Top);
        vp.rect = eframe::egui::Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(1000.0, 1000.0));
        vp.camera.distance = 1000.0;
        vp
    }

    fn doc_with_lines() -> Document {
        let mut d = Document::new();
        let mut t = d.begin();
        t.add(Geometry::Line(forma_geom::LineCurve {
            from: Point3::new(0.0, 0.0, 0.0),
            to: Point3::new(100.0, 0.0, 0.0),
        }));
        t.add(Geometry::Line(forma_geom::LineCurve {
            from: Point3::new(50.0, -50.0, 0.0),
            to: Point3::new(50.0, 50.0, 0.0),
        }));
        t.commit();
        d
    }

    #[test]
    fn finds_end_mid_and_int_through_culling() {
        let d = doc_with_lines();
        let mut idx = SceneIndex::default();
        idx.update(&d);
        let vp = top_view();
        let o = DVec3::ZERO;
        let s = SnapSettings::default();
        let at = |p: Point3| vp.to_screen(p, o).expect("in front");
        let (p, k) = idx
            .find(&vp, at(Point3::new(1.0, 0.5, 0.0)), o, &s, 14.0, None)
            .expect("end");
        assert_eq!(k, SnapKind::End);
        assert!(p.distance_to(Point3::ORIGIN) < 1e-9);
        // Mid of the first line and Int with the second are the same point:
        // Int wins on priority.
        let (p, k) = idx
            .find(&vp, at(Point3::new(50.5, 0.3, 0.0)), o, &s, 14.0, None)
            .expect("int");
        assert_eq!(k, SnapKind::Int);
        // Apparent intersection: found in screen space (f32), then mapped back.
        assert!(p.distance_to(Point3::new(50.0, 0.0, 0.0)) < 0.01);
        // Far from everything: nothing.
        assert!(idx
            .find(&vp, at(Point3::new(300.0, 300.0, 0.0)), o, &s, 14.0, None)
            .is_none());
    }

    #[test]
    fn update_is_incremental() {
        let mut d = doc_with_lines();
        let mut idx = SceneIndex::default();
        idx.update(&d);
        assert_eq!(idx.visible().count(), 2);
        let first = d.objects().next().expect("line").id;
        let mut t = d.begin();
        t.remove(first);
        t.commit();
        idx.update(&d);
        assert_eq!(idx.visible().count(), 1);
        assert!(idx.get(first).is_none());
    }

    #[test]
    fn mesh_vertices_are_grouped_and_culled() {
        let mut d = Document::new();
        let mut t = d.begin();
        let mut m = forma_geom::Mesh::default();
        for i in 0..1000 {
            m.positions.push(Point3::new(i as f64, 0.0, 0.0));
        }
        t.add(Geometry::Mesh(m));
        t.commit();
        let mut idx = SceneIndex::default();
        idx.update(&d);
        let e = idx.visible().next().expect("mesh").1;
        assert_eq!(e.groups.len(), 4); // 1000 vertices in blocks of 256
        let mut vp = top_view();
        vp.camera.target = DVec3::new(700.0, 0.0, 0.0);
        vp.camera.distance = 50.0; // ~27 px per unit
        let mut s = SnapSettings::default();
        *s.flag(SnapKind::Vertex) = true;
        let at = vp
            .to_screen(Point3::new(700.2, 0.1, 0.0), DVec3::ZERO)
            .expect("in front");
        let (p, k) = idx
            .find(&vp, at, DVec3::ZERO, &s, 14.0, None)
            .expect("vertex");
        assert_eq!(k, SnapKind::Vertex);
        assert!(p.distance_to(Point3::new(700.0, 0.0, 0.0)) < 1e-9);
    }
}
