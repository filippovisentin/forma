//! Picking (click and window / crossing selection) and selection helpers. Curve
//! polylines and object boxes come from the [`SceneIndex`] cache instead of
//! being re-sampled on every click.

use crate::index::{dist_point_segment, screen_polyline, segment_param, SceneIndex};
use crate::viewport::Viewport;
use eframe::egui::{Pos2, Rect};
use forma_doc::{Document, Geometry, ObjectId};
use forma_geom::{BoundingBox, Point3, Vec3};
use forma_render::glam::DVec3;
use std::collections::BTreeSet;

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

/// Nearest triangle of a selectable mesh hit by the cursor ray:
/// (ray parameter, object, triangle index).
fn ray_mesh(
    doc: &Document,
    index: &SceneIndex,
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
) -> Option<(f64, ObjectId, usize, Point3)> {
    let (ro, rd) = vp.ray(pos, origin);
    let mut best: Option<(f64, ObjectId, usize)> = None;
    for (id, e) in index.visible() {
        if !ray_box(ro, rd, e.min, e.max) {
            continue;
        }
        let Some(o) = doc.object(id) else { continue };
        let Geometry::Mesh(m) = &o.geometry else {
            continue;
        };
        if !doc.is_selectable(o) {
            continue;
        }
        for (i, t) in m.triangles.iter().enumerate() {
            if t.iter().any(|&k| k as usize >= m.positions.len()) {
                continue;
            }
            let [a, b, c] = t.map(|k| m.positions[k as usize]);
            if let Some(tt) = ray_triangle(ro, rd, a, b, c) {
                if best.is_none_or(|bb| tt < bb.0) {
                    best = Some((tt, id, i));
                }
            }
        }
    }
    best.map(|(t, id, tri)| (t, id, tri, ro + rd * t))
}

/// Surface under the cursor: (object, hit point, triangle), nearest to the eye.
pub fn pick_face(
    doc: &Document,
    index: &SceneIndex,
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
) -> Option<(ObjectId, Point3, usize)> {
    ray_mesh(doc, index, vp, pos, origin).map(|(_, id, tri, hit)| (id, hit, tri))
}

const CURVE_RADIUS: f32 = 6.0;

/// Object under the cursor: curves and points within a few points win,
/// otherwise the nearest surface hit by the ray.
pub fn pick(
    doc: &Document,
    index: &SceneIndex,
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
) -> Option<ObjectId> {
    let pr = vp.projector(origin);
    let mut best_curve: Option<(f32, ObjectId)> = None;
    for (id, e) in index.visible() {
        if !pr.box_near(e.min, e.max, pos, CURVE_RADIUS) {
            continue;
        }
        let Some(o) = doc.object(id).filter(|o| doc.is_selectable(o)) else {
            continue;
        };
        let mut offer = |d: f32| {
            if d <= CURVE_RADIUS && best_curve.is_none_or(|bc| d < bc.0) {
                best_curve = Some((d, id));
            }
        };
        if let Geometry::Point(p) = &o.geometry {
            if let Some(sp) = pr.to_screen(*p) {
                offer(sp.distance(pos));
            }
        } else if let Some(pts) = &e.curve {
            let sc = screen_polyline(&pr, pts);
            for w in sc.windows(2) {
                if let (Some(a), Some(b)) = (w[0], w[1]) {
                    offer(dist_point_segment(pos, a, b));
                }
            }
        }
    }
    best_curve
        .map(|c| c.1)
        .or_else(|| ray_mesh(doc, index, vp, pos, origin).map(|m| m.1))
}

/// Point on the curve under the cursor (within a few screen points), used by
/// Trim, Extend and Fillet to know which curve and which part was clicked.
pub fn pick_curve_point(
    doc: &Document,
    index: &SceneIndex,
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
) -> Option<Point3> {
    const RADIUS: f32 = 8.0;
    let pr = vp.projector(origin);
    let mut best: Option<(f32, Point3)> = None;
    for (id, e) in index.visible() {
        let Some(pts) = &e.curve else { continue };
        if !pr.box_near(e.min, e.max, pos, RADIUS)
            || !doc.object(id).is_some_and(|o| doc.is_selectable(o))
        {
            continue;
        }
        let sc = screen_polyline(&pr, pts);
        for (k, w) in sc.windows(2).enumerate() {
            let (Some(a), Some(b)) = (w[0], w[1]) else {
                continue;
            };
            let (d, t) = segment_param(pos, a, b);
            if d <= RADIUS && best.is_none_or(|bb| d < bb.0) {
                best = Some((d, pts[k] + (pts[k + 1] - pts[k]) * f64::from(t)));
            }
        }
    }
    best.map(|b| b.1)
}

/// Window (left→right: fully inside) or crossing (right→left: touching) selection.
pub fn window_select(
    doc: &Document,
    index: &SceneIndex,
    vp: &Viewport,
    rect: Rect,
    crossing: bool,
    origin: DVec3,
) -> Vec<ObjectId> {
    let pr = vp.projector(origin);
    let mut out = Vec::new();
    for o in doc.objects() {
        if !doc.is_selectable(o) {
            continue;
        }
        let entry = index.get(o.id);
        let (min, max) = match entry {
            Some(e) => (e.min, e.max),
            None => {
                let b = o.geometry.bounding_box();
                (b.min, b.max)
            }
        };
        let Some(bbox) = pr.screen_box(min, max) else {
            continue;
        };
        if rect.contains_rect(bbox) {
            out.push(o.id);
            continue;
        }
        if !crossing || !rect.intersects(bbox) {
            continue;
        }
        let hit = match &o.geometry {
            Geometry::Mesh(m) => m
                .positions
                .iter()
                .any(|p| pr.to_screen(*p).is_some_and(|s| rect.contains(s))),
            Geometry::Point(_) => false,
            g => {
                let sampled;
                let pts: &[Point3] = match entry.and_then(|e| e.curve.as_deref()) {
                    Some(p) => p,
                    None => {
                        sampled = g.curve_points();
                        &sampled
                    }
                };
                let sc = screen_polyline(&pr, pts);
                sc.iter().flatten().any(|s| rect.contains(*s))
                    || sc.windows(2).any(|w| match (w[0], w[1]) {
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
pub fn selection_skeleton(doc: &Document, ids: &BTreeSet<ObjectId>) -> Vec<[Point3; 2]> {
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
                    out.extend(box_edges(&bb));
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

fn box_edges(bb: &BoundingBox) -> impl Iterator<Item = [Point3; 2]> {
    let c = bb.corners();
    [
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
    ]
    .into_iter()
    .map(move |(i, j)| [c[i], c[j]])
}

/// Bounding box of the selection.
pub fn selection_box(doc: &Document, ids: &BTreeSet<ObjectId>) -> Option<BoundingBox> {
    ids.iter()
        .filter_map(|id| doc.object(*id))
        .map(|o| o.geometry.bounding_box())
        .reduce(BoundingBox::union)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn crossing_segments() {
        let r = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0));
        assert!(segment_hits_rect(
            Pos2::new(-5.0, 5.0),
            Pos2::new(5.0, 5.0),
            r
        ));
        assert!(!segment_hits_rect(
            Pos2::new(-5.0, -5.0),
            Pos2::new(-1.0, 20.0),
            r
        ));
    }
}
