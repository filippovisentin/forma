//! Control-point editing (Rhino's PointsOn / F10): grips of the chosen objects
//! are drawn as small squares; click or window-select them, then drag one to
//! move every selected grip (object snaps, grid, Ortho and Planar apply). The
//! move itself is the engine's `MoveGrips` command, so it is undoable.

use crate::viewport::{Projector, Viewport};
use crate::{snap, FormaApp, LogKind};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, StrokeKind};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{Point3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// Grips within this distance (pixels) of the cursor are hit.
const HIT: f32 = 7.0;

#[derive(Default)]
pub(crate) struct Grips {
    /// Objects showing their grips.
    pub on: BTreeSet<ObjectId>,
    /// Selected grips: (object, grip index).
    pub sel: BTreeSet<(ObjectId, usize)>,
    /// Grip positions of the objects in `on`, for the document version `version`.
    points: BTreeMap<ObjectId, Vec<Point3>>,
    version: u64,
    pub drag: Option<GripDrag>,
}

pub(crate) struct GripDrag {
    pub viewport: usize,
    /// The grip that was grabbed, and where it is being dragged to.
    pub base: Point3,
    pub to: Point3,
}

impl Grips {
    pub fn active(&self) -> bool {
        !self.on.is_empty()
    }

    pub fn points(&self, id: ObjectId) -> &[Point3] {
        self.points.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Recompute grip positions when the document changed.
    pub fn refresh(&mut self, doc: &forma_doc::Document) {
        if self.version == doc.version() && self.points.len() == self.on.len() {
            return;
        }
        self.version = doc.version();
        self.on
            .retain(|id| doc.object(*id).is_some_and(|o| doc.is_visible(o)));
        self.points = self
            .on
            .iter()
            .map(|id| (*id, doc.object(*id).expect("kept").geometry.grips()))
            .collect();
        let pts = &self.points;
        self.sel
            .retain(|(id, i)| pts.get(id).is_some_and(|p| *i < p.len()));
    }

    /// Closest grip to a screen position, within [`HIT`] pixels.
    pub fn hit(&self, pr: &Projector, pos: Pos2) -> Option<(ObjectId, usize, Point3)> {
        let mut best: Option<(f32, ObjectId, usize, Point3)> = None;
        for (id, pts) in &self.points {
            for (i, p) in pts.iter().enumerate() {
                if let Some(s) = pr.to_screen(*p) {
                    let d = s.distance(pos);
                    if d <= HIT && best.is_none_or(|b| d < b.0) {
                        best = Some((d, *id, i, *p));
                    }
                }
            }
        }
        best.map(|(_, id, i, p)| (id, i, p))
    }

    /// Grips whose screen position falls in `r`.
    pub fn in_rect(&self, pr: &Projector, r: Rect) -> Vec<(ObjectId, usize)> {
        self.points
            .iter()
            .flat_map(|(id, pts)| {
                pts.iter()
                    .enumerate()
                    .filter(|(_, p)| pr.to_screen(**p).is_some_and(|s| r.contains(s)))
                    .map(|(i, _)| (*id, i))
            })
            .collect()
    }

    /// `MoveGrips` command line moving the selected grips by `v`.
    pub fn command(&self, v: Vec3) -> Option<String> {
        if self.sel.is_empty() || v.length() <= 1e-12 {
            return None;
        }
        let mut by_obj: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();
        for (id, i) in &self.sel {
            by_obj.entry(*id).or_default().push(*i);
        }
        let mut line = String::from("MoveGrips");
        for (id, idx) in by_obj {
            let list: Vec<String> = idx.iter().map(ToString::to_string).collect();
            line.push_str(&format!(" #{} {}", id.0, list.join(",")));
        }
        line.push_str(&format!(
            " {},{},{}",
            crate::tools::round(v.x),
            crate::tools::round(v.y),
            crate::tools::round(v.z)
        ));
        Some(line)
    }
}

/// Whether grips of `g` are joined by a control polygon (curves only).
fn has_polygon(g: &Geometry) -> bool {
    g.is_curve() && !matches!(g, Geometry::Text(_) | Geometry::Dimension(_))
}

impl FormaApp {
    /// PointsOn: show the grips of the selected objects.
    pub(crate) fn points_on(&mut self) {
        let sel: Vec<ObjectId> = self.engine.ctx.selection.iter().copied().collect();
        if sel.is_empty() {
            self.log(LogKind::Error, "PointsOn: select objects first");
            return;
        }
        let doc = self.engine.doc();
        let mut shown = 0;
        let mut too_many = 0;
        for id in sel {
            let g = &doc.object(id).expect("selected").geometry;
            if g.grips().is_empty() {
                too_many += usize::from(matches!(g, Geometry::Mesh(_)));
                continue;
            }
            self.grips.on.insert(id);
            shown += 1;
        }
        self.grips.version = u64::MAX; // force a refresh
        self.grips.refresh(self.engine.doc());
        self.engine.ctx.selection.clear();
        let mut msg = format!(
            "points on for {shown} object(s) — click or window-select points, drag to move them; F11 or PointsOff to finish"
        );
        if too_many > 0 {
            msg.push_str(&format!(
                " ({too_many} mesh(es) have more than {} vertices: no points shown)",
                forma_doc::MAX_MESH_GRIPS
            ));
        }
        self.log(LogKind::Normal, msg);
        self.dirty_all();
    }

    pub(crate) fn points_off(&mut self) {
        let had = self.grips.active();
        self.grips = crate::grips::Grips::default();
        if had {
            self.log(LogKind::Normal, "points off");
        }
        self.dirty_all();
    }

    /// Grip clicks, drags and window selection. Returns true when the input was
    /// used here.
    pub(crate) fn grips_input(
        &mut self,
        ui: &egui::Ui,
        vi: usize,
        resp: &egui::Response,
        mods: egui::Modifiers,
    ) -> bool {
        if !self.grips.active() || self.tool.is_some() {
            return false;
        }
        self.grips.refresh(self.engine.doc());
        let origin = self.origin();
        let pr = self.viewports[vi].projector(origin);

        // Dragging grips.
        if let Some(d) = self.grips.drag.as_ref() {
            if d.viewport != vi {
                return false;
            }
            if let Some(pos) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                let base = d.base;
                let to = self.grip_target(vi, pos, base, mods.shift);
                if let Some(d) = self.grips.drag.as_mut() {
                    d.to = to;
                }
                self.dirty_all();
            }
            if resp.drag_stopped_by(egui::PointerButton::Primary) {
                let d = self.grips.drag.take().expect("dragging");
                if let Some(line) = self.grips.command(d.to - d.base) {
                    self.run_engine(&line);
                }
            }
            return true;
        }
        if resp.drag_started_by(egui::PointerButton::Primary) {
            let at = ui
                .input(|i| i.pointer.press_origin())
                .or(resp.hover_pos())
                .unwrap_or_default();
            if let Some((id, i, p)) = self.grips.hit(&pr, at) {
                if !self.grips.sel.contains(&(id, i)) {
                    if !mods.shift {
                        self.grips.sel.clear();
                    }
                    self.grips.sel.insert((id, i));
                }
                self.grips.drag = Some(GripDrag {
                    viewport: vi,
                    base: p,
                    to: p,
                });
                return true;
            }
            return false; // a window drag: handled by `grips_window`
        }
        if resp.clicked_by(egui::PointerButton::Primary) {
            let pos = resp.interact_pointer_pos().unwrap_or_default();
            match self.grips.hit(&pr, pos) {
                Some((id, i, _)) => {
                    let g = (id, i);
                    if mods.command {
                        self.grips.sel.remove(&g);
                    } else if mods.shift {
                        if !self.grips.sel.insert(g) {
                            self.grips.sel.remove(&g);
                        }
                    } else {
                        self.grips.sel.clear();
                        self.grips.sel.insert(g);
                    }
                    self.engine.ctx.selection.clear();
                    let n = self.grips.sel.len();
                    self.log(
                        LogKind::Normal,
                        format!("{n} point(s) selected — drag one to move them"),
                    );
                    self.dirty_all();
                    return true;
                }
                None if !(mods.shift || mods.command) => {
                    self.grips.sel.clear();
                    self.dirty_all();
                }
                None => {}
            }
        }
        false
    }

    /// Window selection of grips (instead of objects) while points are on.
    pub(crate) fn grips_window(&mut self, vi: usize, a: Pos2, b: Pos2, mods: egui::Modifiers) {
        let pr = self.viewports[vi].projector(self.origin());
        let hits = self.grips.in_rect(&pr, Rect::from_two_pos(a, b));
        if mods.command {
            for h in hits {
                self.grips.sel.remove(&h);
            }
        } else {
            if !mods.shift {
                self.grips.sel.clear();
            }
            self.grips.sel.extend(hits);
        }
        let n = self.grips.sel.len();
        self.log(LogKind::Normal, format!("{n} point(s) selected"));
        self.dirty_all();
    }

    /// Where a grip dragged from `base` lands under the cursor: an object snap,
    /// else the plane through `base` parallel to the CPlane (grid, Ortho).
    fn grip_target(&self, vi: usize, pos: Pos2, base: Point3, shift: bool) -> Point3 {
        let vp: &Viewport = &self.viewports[vi];
        let origin = self.origin();
        if let Some((p, _)) = self
            .index
            .find(vp, pos, origin, &self.snap, 14.0, Some(base))
        {
            return p;
        }
        let cp = vp.cplane();
        let h = (base - cp.origin).dot(cp.z);
        let plane = cp.moved_to(cp.origin + cp.z * h);
        let (o, d) = vp.ray(pos, origin);
        let mut p = if d.dot(plane.z).abs() > 0.02 {
            plane.intersect_line(o, d).unwrap_or(base)
        } else {
            vp.cplane_point(pos, origin)
        };
        if self.snap.grid {
            p = snap::grid_snap(p, &plane, self.snap.step);
        }
        if self.snap.ortho != shift {
            p = snap::ortho(p, base, &plane);
        }
        p
    }

    /// Grips and control polygons of the objects with points on; the selected
    /// grips in yellow, and the drag preview.
    pub(crate) fn draw_grips(&self, p: &Painter, pr: &Projector) {
        if !self.grips.active() {
            return;
        }
        let doc = self.engine.doc();
        let shift = self
            .grips
            .drag
            .as_ref()
            .map_or(Vec3::new(0.0, 0.0, 0.0), |d| d.to - d.base);
        let polygon = Stroke::new(1.0, Color32::from_rgb(70, 70, 90));
        for id in &self.grips.on {
            let pts = self.grips.points(*id);
            let Some(o) = doc.object(*id) else { continue };
            let moved: Vec<Point3> = pts
                .iter()
                .enumerate()
                .map(|(i, q)| {
                    if self.grips.sel.contains(&(*id, i)) {
                        *q + shift
                    } else {
                        *q
                    }
                })
                .collect();
            if has_polygon(&o.geometry) {
                let closed = o.geometry.is_closed_curve();
                let n = moved.len();
                let segs = if closed { n } else { n.saturating_sub(1) };
                for k in 0..segs {
                    if let (Some(a), Some(b)) =
                        (pr.to_screen(moved[k]), pr.to_screen(moved[(k + 1) % n]))
                    {
                        p.line_segment([a, b], polygon);
                    }
                }
            }
            for (i, q) in moved.iter().enumerate() {
                let Some(s) = pr.to_screen(*q) else { continue };
                let r = Rect::from_center_size(s, egui::vec2(7.0, 7.0));
                if self.grips.sel.contains(&(*id, i)) {
                    p.rect_filled(r, 0.0, Color32::from_rgb(255, 200, 0));
                } else {
                    p.rect_filled(r, 0.0, Color32::WHITE);
                }
                p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Middle);
            }
        }
        if let Some(d) = &self.grips.drag {
            if let (Some(a), Some(b)) = (pr.to_screen(d.base), pr.to_screen(d.to)) {
                p.line_segment([a, b], Stroke::new(1.0, Color32::from_rgb(200, 40, 40)));
                let v = d.to - d.base;
                crate::overlays::measure_tag(p, b, self.fmt_len(v.length()));
            }
        }
    }
}
