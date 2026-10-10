//! Mouse input in the viewports: cursor point with snaps, navigation,
//! selection, gumball and face push / pull, and clicks fed to the running tool.

use crate::tools::{Tool, Want};
use crate::viewport::{closest_on_line, to_d, Viewport};
use crate::{
    gumball, perf, pick, snap, DragSelect, FaceDrag, FaceSel, FormaApp, GumballDrag, Hover, LogKind,
};
use eframe::egui::{self, Modifiers, PointerButton, Pos2, Rect};
use forma_doc::ObjectId;
use forma_geom::{Point3, Vec3};
use std::collections::BTreeSet;

impl FormaApp {
    fn compute_hover(&self, vi: usize, pos: Pos2, shift: bool) -> Hover {
        let vp = &self.viewports[vi];
        let origin = self.origin();
        let want = self.tool.as_ref().map(|t| t.want());
        let picking = matches!(
            want,
            Some(Want::Point | Want::PointOrNumber | Want::Height { .. })
        );
        let base = self.tool.as_ref().and_then(Tool::base);
        let osnap = if picking {
            self.index
                .find(vp, pos, origin, &self.snap, 14.0, base)
                .map(|(p, k)| {
                    if self.snap.project {
                        (vp.cplane().project(p), k)
                    } else {
                        (p, k)
                    }
                })
        } else {
            None
        };
        if want == Some(Want::Pick) {
            let doc = self.engine.doc();
            let on_curve = pick::pick_curve_point(doc, &self.index, vp, pos, origin);
            return Hover {
                viewport: vi,
                point: on_curve.unwrap_or_else(|| vp.cplane_point(pos, origin)),
                snap: None,
                pos,
                tracks: Vec::new(),
            };
        }
        if let Some(Want::Height { from, dir }) = want {
            let (o, d) = vp.ray(pos, origin);
            let mut p = closest_on_line(from, dir, o, d);
            let n = dir.normalized().unwrap_or(Vec3::Z);
            let mut kind = None;
            if let Some((s, k)) = osnap {
                p = from + n * (s - from).dot(n);
                kind = Some(k);
            } else if self.snap.grid && self.snap.step > 0.0 {
                let h = (p - from).dot(n);
                p = from + n * ((h / self.snap.step).round() * self.snap.step);
            }
            return Hover {
                viewport: vi,
                point: p,
                snap: kind,
                pos,
                tracks: Vec::new(),
            };
        }
        if let Some((p, k)) = osnap {
            return Hover {
                viewport: vi,
                point: p,
                snap: Some(k),
                pos,
                tracks: Vec::new(),
            };
        }
        let tool_plane = self
            .tool
            .as_ref()
            .filter(|t| !t.pts.is_empty() && self.snap.planar)
            .map(|t| t.plane.moved_to(t.pts[0]));
        let plane = tool_plane.unwrap_or_else(|| vp.cplane());
        let mut p = vp.cplane_point(pos, origin);
        if let Some(tp) = tool_plane {
            // Later points stay on the plane of the first one.
            let (o, d) = vp.ray(pos, origin);
            if d.dot(tp.z).abs() > 0.02 {
                if let Some(q) = tp.intersect_line(o, d) {
                    p = q;
                }
            }
        }
        let raw = p;
        if self.snap.grid {
            p = snap::grid_snap(p, &plane, self.snap.step);
        }
        let base = self.tool.as_ref().and_then(|t| t.base());
        let mut lock = None;
        if let Some(b) = base {
            if self.snap.ortho != shift {
                p = snap::ortho(p, b, &plane);
                let d = p - b;
                lock = Some((b, d.dot(plane.x).abs() >= d.dot(plane.y).abs()));
            }
        }
        // SmartTrack: line up with recent snap points (and the base point).
        let mut tracks = Vec::new();
        if self.snap.smart && !self.snap.disabled && picking {
            let mut pts = self.track_points.clone();
            pts.extend(base);
            if let Some((q, lines)) = snap::smart_track(vp, pos, origin, &plane, raw, &pts, lock) {
                p = q;
                tracks = lines;
            }
        }
        Hover {
            viewport: vi,
            point: p,
            snap: None,
            pos,
            tracks,
        }
    }

    /// Objects plus the other members of their groups, through the filter.
    fn expand_groups(&self, ids: Vec<ObjectId>) -> Vec<ObjectId> {
        let doc = self.engine.doc();
        let ids: Vec<ObjectId> = ids
            .into_iter()
            .filter(|id| {
                doc.object(*id)
                    .is_some_and(|o| self.filter.accepts(&o.geometry))
            })
            .collect();
        let groups: BTreeSet<u32> = ids
            .iter()
            .filter_map(|id| doc.object(*id).and_then(|o| o.group))
            .collect();
        if groups.is_empty() {
            return ids;
        }
        let mut out: BTreeSet<ObjectId> = ids.into_iter().collect();
        for o in doc.objects() {
            if o.group.is_some_and(|g| groups.contains(&g)) && doc.is_selectable(o) {
                out.insert(o.id);
            }
        }
        out.into_iter().collect()
    }

    fn click_select(&mut self, vi: usize, pos: Pos2, mods: Modifiers) {
        let t = perf::span("pick");
        let id = pick::pick(
            self.engine.doc(),
            &self.index,
            &self.viewports[vi],
            pos,
            self.origin(),
        );
        drop(t);
        let ids = self.expand_groups(id.into_iter().collect());
        let id = ids.first().copied();
        let selecting_for_tool = self.tool.as_ref().is_some_and(|t| t.selecting);
        let sel = &mut self.engine.ctx.selection;
        match id {
            Some(_) if mods.command => {
                for i in &ids {
                    sel.remove(i);
                }
            }
            Some(_) if mods.shift || selecting_for_tool => {
                sel.extend(ids);
            }
            Some(_) => {
                sel.clear();
                sel.extend(ids);
            }
            None if !(mods.shift || mods.command || selecting_for_tool) => sel.clear(),
            None => {}
        }
    }

    fn window_select(&mut self, vi: usize, a: Pos2, b: Pos2, mods: Modifiers) {
        let rect = Rect::from_two_pos(a, b);
        let crossing = b.x < a.x;
        let _t = perf::span("window select");
        let ids = pick::window_select(
            self.engine.doc(),
            &self.index,
            &self.viewports[vi],
            rect,
            crossing,
            self.origin(),
        );
        let ids = self.expand_groups(ids);
        let selecting_for_tool = self.tool.as_ref().is_some_and(|t| t.selecting);
        let sel = &mut self.engine.ctx.selection;
        if mods.command {
            for id in ids {
                sel.remove(&id);
            }
        } else {
            if !(mods.shift || selecting_for_tool) {
                sel.clear();
            }
            sel.extend(ids);
        }
    }

    pub(crate) fn viewport_input(&mut self, ui: &egui::Ui, vi: usize, resp: &egui::Response) {
        let mods = ui.input(|i| i.modifiers);
        let origin = self.origin();

        if resp.clicked_by(PointerButton::Primary)
            || resp.clicked_by(PointerButton::Secondary)
            || resp.drag_started()
        {
            self.active = vi;
        }

        self.navigate(ui, vi, resp, mods);
        self.update_hover(vi, resp, mods);
        if self.face_input(ui, vi, resp) || self.gumball_input(ui, vi, resp, mods) {
            return;
        }
        let tool_wants_points = self.tool.as_ref().is_some_and(|t| !t.selecting);

        // Window / crossing selection.
        if resp.drag_started_by(PointerButton::Primary) && !tool_wants_points {
            if let Some(p) = resp.interact_pointer_pos() {
                self.drag = Some(DragSelect {
                    viewport: vi,
                    start: p,
                });
            }
        }
        if resp.drag_stopped_by(PointerButton::Primary) {
            if let (Some(d), Some(end)) = (self.drag.take(), resp.interact_pointer_pos()) {
                if d.viewport == vi && d.start.distance(end) > 3.0 {
                    self.window_select(vi, d.start, end, mods);
                }
            }
        }

        // Clicks.
        if resp.clicked_by(PointerButton::Primary) {
            let pos = resp.interact_pointer_pos().unwrap_or_default();
            if tool_wants_points {
                if let Some(t) = self.tool.as_mut() {
                    if t.pts.is_empty() && !matches!(t.want(), Want::Height { .. }) {
                        t.plane = self.viewports[vi].cplane();
                    }
                }
                if self.tool.as_ref().is_some_and(|t| t.want() == Want::Number) {
                    self.log(LogKind::Error, "type a number in the command line");
                    return;
                }
                if self
                    .tool
                    .as_ref()
                    .is_some_and(|t| t.want() == Want::PickObject)
                {
                    let id = pick::pick(
                        self.engine.doc(),
                        &self.index,
                        &self.viewports[vi],
                        pos,
                        origin,
                    );
                    match (id, self.tool.as_mut()) {
                        (Some(id), Some(t)) => {
                            let step = t.feed_object(id.0);
                            self.handle_step(step);
                        }
                        _ => self.log(LogKind::Error, "no object there — click on an object"),
                    }
                    return;
                }
                if self.tool.as_ref().is_some_and(|t| t.want() == Want::Pick)
                    && pick::pick_curve_point(
                        self.engine.doc(),
                        &self.index,
                        &self.viewports[vi],
                        pos,
                        origin,
                    )
                    .is_none()
                {
                    self.log(LogKind::Error, "no curve there — click on a curve");
                    return;
                }
                let h = self.compute_hover(vi, pos, mods.shift);
                if let Some(t) = self.tool.as_mut() {
                    let step = t.feed_point(h.point);
                    self.handle_step(step);
                }
            } else if mods.command && mods.shift {
                // Sub-object selection: a face of a solid.
                if !self.pick_face(vi, pos) {
                    self.face_sel = None;
                }
            } else {
                self.face_sel = None;
                self.click_select(vi, pos, mods);
            }
        }
        if resp.clicked_by(PointerButton::Secondary) {
            self.enter_action();
        }
        if resp.double_clicked_by(PointerButton::Middle) {
            self.fit(Some(vi));
        }
    }

    /// Orbit, pan and zoom.
    fn navigate(&mut self, ui: &egui::Ui, vi: usize, resp: &egui::Response, mods: Modifiers) {
        let origin = self.origin();
        let ppp = ui.ctx().pixels_per_point() as f64;
        // Navigation.
        let delta = resp.drag_delta();
        let vp = &mut self.viewports[vi];
        let height = vp.rect.height() as f64 * ppp;
        let pan = |vp: &mut Viewport| {
            vp.camera
                .pan(delta.x as f64 * ppp, delta.y as f64 * ppp, height);
            vp.dirty = true;
        };
        if resp.dragged_by(PointerButton::Middle) {
            pan(vp);
        } else if resp.dragged_by(PointerButton::Secondary) {
            if mods.command {
                vp.camera.zoom((delta.y as f64 * 0.01).exp());
                vp.dirty = true;
            } else if vp.is_parallel() || mods.shift {
                pan(vp);
            } else {
                vp.camera.orbit(delta.x as f64, delta.y as f64);
                vp.dirty = true;
            }
        }
        if let Some(pos) = resp.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let anchor = to_d(vp.cplane_point(pos, origin)) - origin;
                vp.camera.zoom_at((-scroll as f64 * 0.0025).exp(), anchor);
                vp.dirty = true;
            }
        }
    }

    /// Cursor point with snaps; rested-on snaps become SmartTrack points.
    fn update_hover(&mut self, vi: usize, resp: &egui::Response, mods: Modifiers) {
        // Cursor point (snaps etc).
        if let Some(pos) = resp.hover_pos() {
            let t = perf::span_min("hover", 2.0);
            let h = self.compute_hover(vi, pos, mods.shift);
            drop(t);
            if let (Some(_), true) = (h.snap, self.snap.smart) {
                if !self
                    .track_points
                    .iter()
                    .any(|q| q.distance_to(h.point) < 1e-9)
                {
                    self.track_points.push(h.point);
                    if self.track_points.len() > 6 {
                        self.track_points.remove(0);
                    }
                }
            }
            self.hover = Some(h);
        }
    }

    /// The push / pull arrow of a picked face. `true` when it used the input.
    fn face_input(&mut self, ui: &egui::Ui, vi: usize, resp: &egui::Response) -> bool {
        let origin = self.origin();
        // Push / pull arrow of a picked face.
        if let Some((fc, fnorm)) = self.face_sel.as_ref().map(|f| (f.center, f.normal)) {
            let step = self.snap.step;
            let grid = self.snap.grid;
            if let Some(d) = self.face_drag.as_mut() {
                if d.viewport == vi {
                    if let Some(pos) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                        let vp = &self.viewports[vi];
                        let snapped = self.index.find(vp, pos, origin, &self.snap, 14.0, None);
                        let q = match snapped {
                            Some((q, _)) => q,
                            None => {
                                let (o, dd) = vp.ray(pos, origin);
                                closest_on_line(fc, fnorm, o, dd)
                            }
                        };
                        let mut t = (q - fc).dot(fnorm);
                        if snapped.is_none() {
                            t -= d.start;
                            if grid && step > 0.0 {
                                t = (t / step).round() * step;
                            }
                        }
                        d.distance = t;
                        d.snapped = snapped;
                    }
                    if resp.drag_stopped_by(PointerButton::Primary) {
                        let d = self.face_drag.take().expect("dragging");
                        if d.distance.abs() > 1e-9 {
                            if let Some(f) = self.face_sel.as_ref() {
                                let line = face_command(f, d.distance);
                                self.run_engine(&line);
                                self.refresh_face_sel();
                            }
                        }
                    }
                    return true;
                }
            }
            if self.tool.is_none() {
                let at = if resp.drag_started_by(PointerButton::Primary) {
                    ui.input(|i| i.pointer.press_origin()).or(resp.hover_pos())
                } else {
                    resp.hover_pos()
                };
                let vp = &self.viewports[vi];
                if let Some((base, tip)) = gumball::face_handle(vp, fc, fnorm, origin) {
                    let hot = at.is_some_and(|p| gumball::face_handle_hit(base, tip, p));
                    if resp.hovered() {
                        self.face_hot = hot;
                    }
                    if hot && resp.drag_started_by(PointerButton::Primary) {
                        let (o, dd) = vp.ray(at.unwrap_or_default(), origin);
                        let start = (closest_on_line(fc, fnorm, o, dd) - fc).dot(fnorm);
                        self.face_drag = Some(FaceDrag {
                            viewport: vi,
                            start,
                            distance: 0.0,
                            snapped: None,
                        });
                        return true;
                    }
                    if hot && resp.clicked_by(PointerButton::Primary) {
                        let unit = self.engine.doc().units.abbreviation();
                        self.log(
                            LogKind::Normal,
                            format!("Push / pull face — type the distance ({unit}, negative pushes in) and press Enter"),
                        );
                        self.face_typed = true;
                        self.focus_command = true;
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Gumball handles (only without a running tool). `true` when it used the
    /// input.
    fn gumball_input(
        &mut self,
        ui: &egui::Ui,
        vi: usize,
        resp: &egui::Response,
        mods: Modifiers,
    ) -> bool {
        let origin = self.origin();
        // Gumball handles (only without a running tool).
        let gumball_center = self.gumball_center();
        if let Some(d) = self.gumball_drag.as_mut() {
            if d.viewport == vi {
                if let Some(pos) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                    let vp = &self.viewports[vi];
                    // Object snaps steer the drag: the snapped point is projected
                    // on the axis (or plane) being dragged.
                    let snapped = if matches!(d.handle, gumball::Handle::Rotate(_)) {
                        None
                    } else {
                        self.index.find(vp, pos, origin, &self.snap, 14.0, None)
                    };
                    d.snapped = snapped;
                    let g = match snapped {
                        Some((q, _)) => Some(gumball::project_grip(d.center, d.handle, q)),
                        None => gumball::grip(vp, d.center, d.handle, origin, pos),
                    };
                    let step = if self.snap.grid && snapped.is_none() {
                        self.snap.step
                    } else {
                        0.0
                    };
                    let angle_step = if self.snap.grid != mods.shift {
                        5.0
                    } else {
                        0.0
                    };
                    if let Some(g) = g {
                        d.motion = gumball::motion(d.handle, &d.start, &g, step, angle_step);
                    }
                }
                if resp.drag_stopped_by(PointerButton::Primary) {
                    let d = self.gumball_drag.take().expect("dragging");
                    if let Some(m) = d.motion {
                        for line in self.gumball_lines(m, d.center) {
                            self.run_engine(&line);
                        }
                    }
                }
                return true;
            }
        }
        if let (Some(center), None) = (gumball_center, self.tool.as_ref()) {
            // A drag starts where the button went down, not where it is now.
            let at = if resp.drag_started_by(PointerButton::Primary) {
                ui.input(|i| i.pointer.press_origin()).or(resp.hover_pos())
            } else {
                resp.hover_pos()
            };
            let hot = at.and_then(|pos| {
                let l = gumball::layout(&self.viewports[vi], center, origin)?;
                gumball::hit(&self.viewports[vi], &l, origin, pos)
            });
            if resp.hovered() {
                self.gumball_hot = hot.map(|h| (vi, h));
            }
            if let Some(h) = hot {
                if resp.drag_started_by(PointerButton::Primary) {
                    let pos = at.unwrap_or_default();
                    if let Some(start) = gumball::grip(&self.viewports[vi], center, h, origin, pos)
                    {
                        self.gumball_drag = Some(GumballDrag {
                            viewport: vi,
                            handle: h,
                            center,
                            start,
                            motion: None,
                            skeleton: pick::selection_skeleton(
                                self.engine.doc(),
                                &self.engine.ctx.selection,
                            ),
                            snapped: None,
                            faces: self.push_pull_preview(h),
                        });
                    }
                    return true;
                }
                if resp.clicked_by(PointerButton::Primary) {
                    let unit = self.engine.doc().units.abbreviation();
                    let what = match h {
                        gumball::Handle::Rotate(_) => "angle in degrees".to_string(),
                        gumball::Handle::Extrude(_) => format!("extrusion distance ({unit})"),
                        _ => format!("distance ({unit})"),
                    };
                    self.log(
                        LogKind::Normal,
                        format!(
                            "Gumball: {} — type the {what} and press Enter",
                            h.describe()
                        ),
                    );
                    self.gumball_typed = Some((h, center));
                    self.focus_command = true;
                    return true;
                }
            }
        }

        false
    }

    /// Engine lines for a gumball motion. Extrusion makes solids from the
    /// selected curves (ExtrudeCrv) and from open surfaces (ExtrudeSrf).
    pub(crate) fn gumball_lines(&self, m: gumball::Motion, center: Point3) -> Vec<String> {
        let gumball::Motion::Extrude(d, ax) = m else {
            return vec![m.command(center)];
        };
        let doc = self.engine.doc();
        let sel: Vec<_> = self
            .engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .collect();
        let curves: Vec<String> = sel
            .iter()
            .filter(|o| o.geometry.is_curve())
            .map(|o| format!("#{}", o.id.0))
            .collect();
        let tol = doc.absolute_tolerance.max(1e-6);
        let is_solid =
            |g: &forma_doc::Geometry| matches!(g, forma_doc::Geometry::Mesh(m) if m.is_closed(tol));
        let surfaces: Vec<String> = sel
            .iter()
            .filter(|o| {
                matches!(o.geometry, forma_doc::Geometry::Mesh(_)) && !is_solid(&o.geometry)
            })
            .map(|o| format!("#{}", o.id.0))
            .collect();
        let solids: Vec<String> = sel
            .iter()
            .filter(|o| is_solid(&o.geometry))
            .map(|o| format!("#{}", o.id.0))
            .collect();
        let dir = format!("{},{},{}", ax.x, ax.y, ax.z);
        let d = (d * 1e6).round() / 1e6;
        let mut out = Vec::new();
        if !curves.is_empty() {
            out.push("SelNone".to_string());
            out.push(format!("Select {}", curves.join(" ")));
            out.push(format!("Extrude {d} {dir}"));
        }
        if !surfaces.is_empty() {
            out.push("SelNone".to_string());
            out.push(format!("Select {}", surfaces.join(" ")));
            out.push(format!("ExtrudeSrf {d} {dir}"));
        }
        if !solids.is_empty() {
            // A solid grows or shrinks: its face on that side is pushed / pulled.
            out.push("SelNone".to_string());
            out.push(format!("Select {}", solids.join(" ")));
            out.push(format!("PushPull {dir} {d}"));
        }
        out
    }

    /// Outlines of the faces an extrude dot would push / pull, for the preview.
    fn push_pull_preview(&self, h: gumball::Handle) -> Vec<Vec<[Point3; 2]>> {
        let gumball::Handle::Extrude(i) = h else {
            return Vec::new();
        };
        let dir = [Vec3::X, Vec3::Y, Vec3::Z][i];
        let doc = self.engine.doc();
        let tol = doc.absolute_tolerance.max(1e-6);
        self.engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .filter_map(|o| match &o.geometry {
                forma_doc::Geometry::Mesh(m) if m.is_closed(tol) => {
                    m.extreme_face(dir, tol).map(|f| m.face_outline(&f, tol))
                }
                _ => None,
            })
            .collect()
    }

    /// Pick the face of a solid under the cursor (Ctrl+Shift+click).
    fn pick_face(&mut self, vi: usize, pos: Pos2) -> bool {
        let doc = self.engine.doc();
        let Some((id, _hit, tri)) =
            pick::pick_face(doc, &self.index, &self.viewports[vi], pos, self.origin())
        else {
            return false;
        };
        let Some(forma_doc::Geometry::Mesh(m)) = doc.object(id).map(|o| &o.geometry) else {
            return false;
        };
        let tol = doc.absolute_tolerance.max(1e-6);
        let Some(face) = m.planar_face(tri, tol) else {
            return false;
        };
        self.face_sel = Some(FaceSel {
            id,
            center: face.center,
            normal: face.normal,
            outline: m.face_outline(&face, tol),
        });
        self.engine.ctx.selection.clear();
        self.log(
            LogKind::Normal,
            "Face selected — drag the orange arrow to push / pull it, or click it and type a distance",
        );
        true
    }

    /// Re-read the picked face after it moved (it keeps its normal).
    pub(crate) fn refresh_face_sel(&mut self) {
        let Some(f) = self.face_sel.as_ref() else {
            return;
        };
        let doc = self.engine.doc();
        let tol = doc.absolute_tolerance.max(1e-6);
        let Some(forma_doc::Geometry::Mesh(m)) = doc.object(f.id).map(|o| &o.geometry) else {
            self.face_sel = None;
            return;
        };
        // The face moved along its normal: look for it near the old centre.
        let best = m
            .planar_faces(tol)
            .into_iter()
            .filter(|g| g.normal.dot(f.normal) > 0.999)
            .min_by(|a, b| {
                let da = (a.center - f.center).cross(f.normal).length();
                let db = (b.center - f.center).cross(f.normal).length();
                da.total_cmp(&db)
            });
        match best {
            Some(g) => {
                let outline = m.face_outline(&g, tol);
                self.face_sel = Some(FaceSel {
                    id: f.id,
                    center: g.center,
                    normal: g.normal,
                    outline,
                });
            }
            None => self.face_sel = None,
        }
    }

    /// Where the gumball sits: centre of the selection (when enabled).
    pub(crate) fn gumball_center(&self) -> Option<Point3> {
        if !self.gumball_on || self.engine.ctx.selection.is_empty() {
            return None;
        }
        self.sel_info.bbox.map(|b| b.center())
    }
}

/// Engine line pushing / pulling a picked face by `d`.
pub(crate) fn face_command(f: &FaceSel, d: f64) -> String {
    let c = f.center;
    let r = |x: f64| (x * 1e6).round() / 1e6;
    format!(
        "MoveFace #{} {},{},{} {}",
        f.id.0,
        r(c.x),
        r(c.y),
        r(c.z),
        r(d)
    )
}
