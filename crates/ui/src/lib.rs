//! Forma desktop UI (egui), laid out like Rhino: four viewports, a command line,
//! a tool sidebar and a layers panel. Every modelling action goes through
//! `forma-engine`; interactive tools only collect clicks and emit command lines.

mod icons;
mod snap;
mod tools;
mod viewport;

use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, Stroke, StrokeKind};
use eframe::egui_wgpu;
use forma_doc::{LengthUnit, ObjectId};
use forma_engine::Engine;
use forma_geom::{Point3, Vec3};
use forma_render::glam::DVec3;
use forma_render::{grid_plane_for, model_center, Renderer, SceneCache, StandardView};
use icons::Icon;
use snap::{SnapKind, SnapPoints, SnapSettings};
use std::collections::BTreeSet;
use tools::{parse_typed_point, Step, Tool, ToolKind, Want};
use viewport::{closest_on_line, to_d, Viewport};

/// Start the desktop app, optionally opening a file.
pub fn run(open: Option<String>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Forma")
            .with_inner_size([1500.0, 950.0])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Forma",
        options,
        Box::new(move |cc| {
            let mut app = FormaApp::new(cc);
            if let Some(path) = open {
                app.run_engine(&format!("Open {path}"));
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum LogKind {
    Normal,
    Command,
    Error,
}

struct Hover {
    viewport: usize,
    point: Point3,
    snap: Option<SnapKind>,
    pos: Pos2,
}

struct DragSelect {
    viewport: usize,
    start: Pos2,
}

struct FormaApp {
    engine: Engine,
    renderer: Option<Renderer>,
    cache: SceneCache,
    viewports: Vec<Viewport>,
    active: usize,
    maximized: Option<usize>,
    seen_version: u64,
    seen_selection: BTreeSet<ObjectId>,
    force_rebuild: bool,
    scene_min: DVec3,
    scene_max: DVec3,
    triangles: usize,
    snaps: SnapPoints,
    snap: SnapSettings,
    tool: Option<Tool>,
    last_command: Option<String>,
    command: String,
    focus_command: bool,
    log: Vec<(LogKind, String)>,
    hover: Option<Hover>,
    drag: Option<DragSelect>,
    title: String,
    /// Saved at least once by Forma (otherwise Save asks for a name, to avoid
    /// overwriting an original Rhino file with display meshes).
    saved_by_forma: bool,
    pending_fit: bool,
}

const CMD_ID: &str = "forma-command-line";

fn min_grid_extent(u: LengthUnit) -> f64 {
    match u {
        LengthUnit::Millimeters => 3000.0,
        LengthUnit::Centimeters => 300.0,
        LengthUnit::Meters => 3.0,
        LengthUnit::Inches => 120.0,
        LengthUnit::Feet => 10.0,
    }
}

impl FormaApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let renderer = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| Renderer::new(&rs.device));
        let mut app = FormaApp {
            engine: Engine::new(),
            renderer,
            cache: SceneCache::default(),
            viewports: vec![
                Viewport::new(StandardView::Top),
                Viewport::new(StandardView::Perspective),
                Viewport::new(StandardView::Front),
                Viewport::new(StandardView::Right),
            ],
            active: 1,
            maximized: None,
            seen_version: u64::MAX,
            seen_selection: BTreeSet::new(),
            force_rebuild: true,
            scene_min: DVec3::splat(-1000.0),
            scene_max: DVec3::splat(1000.0),
            triangles: 0,
            snaps: SnapPoints::default(),
            snap: SnapSettings::default(),
            tool: None,
            last_command: None,
            command: String::new(),
            focus_command: false,
            log: Vec::new(),
            hover: None,
            drag: None,
            title: String::new(),
            saved_by_forma: false,
            pending_fit: true,
        };
        app.reset_document_view();
        app.log(
            LogKind::Normal,
            "Forma — type a command or pick a tool on the left. Enter / Space / right click confirms or repeats, Esc cancels. Help lists everything.",
        );
        app
    }

    // ----------------------------------------------------------------- log

    fn log(&mut self, kind: LogKind, msg: impl Into<String>) {
        let msg = msg.into();
        if msg.is_empty() {
            return;
        }
        self.log.push((kind, msg));
        if self.log.len() > 300 {
            self.log.remove(0);
        }
    }

    // ----------------------------------------------------------------- document

    /// A different document was loaded: new scene origin, snap step and camera fit.
    fn reset_document_view(&mut self) {
        self.cache.reset(model_center(self.engine.doc()));
        self.snap.step = match self.engine.doc().units {
            LengthUnit::Millimeters => 10.0,
            LengthUnit::Centimeters => 1.0,
            LengthUnit::Meters => 0.01,
            LengthUnit::Inches => 1.0,
            LengthUnit::Feet => 0.1,
        };
        self.force_rebuild = true;
        self.pending_fit = true;
        self.tool = None;
        self.hover = None;
    }

    fn origin(&self) -> DVec3 {
        self.cache.origin()
    }

    /// Rebuild GPU buffers when the document or selection changed.
    fn sync(&mut self, frame: &eframe::Frame) {
        let Some(rs) = frame.wgpu_render_state() else {
            return;
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let doc = &self.engine.ctx.doc;
        let mut changed = false;
        if self.force_rebuild || doc.version() != self.seen_version {
            let scene = self.cache.scene(doc);
            self.triangles = scene.triangle_count();
            let origin = self.cache.origin();
            if scene.is_empty() {
                let e = min_grid_extent(doc.units) / 2.0;
                self.scene_min = DVec3::new(-e, -e, 0.0) - origin;
                self.scene_max = DVec3::new(e, e, 0.0) - origin;
            } else {
                self.scene_min = scene.min;
                self.scene_max = scene.max;
            }
            renderer.set_scene(&rs.device, &scene, min_grid_extent(doc.units));
            self.snaps = SnapPoints::build(doc);
            self.seen_version = doc.version();
            self.force_rebuild = false;
            self.seen_selection.clear();
            self.seen_selection.insert(ObjectId(u64::MAX)); // force a highlight refresh
            changed = true;
        }
        if self.engine.ctx.selection != self.seen_selection {
            let hl = self.cache.highlight(doc, &self.engine.ctx.selection);
            renderer.set_highlight(&rs.device, &hl);
            self.seen_selection = self.engine.ctx.selection.clone();
            changed = true;
        }
        if changed {
            for v in &mut self.viewports {
                v.dirty = true;
            }
        }
    }

    fn fit(&mut self, which: Option<usize>) {
        let (min, max) = (self.scene_min, self.scene_max);
        for (i, v) in self.viewports.iter_mut().enumerate() {
            if which.is_some_and(|w| w != i) {
                continue;
            }
            let aspect = if v.rect.is_positive() {
                v.aspect()
            } else {
                1.5
            };
            v.camera.fit(min, max, aspect);
            v.dirty = true;
        }
    }

    // ----------------------------------------------------------------- commands

    /// Run an engine command line and log the result.
    fn run_engine(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let first = line.split_whitespace().next().unwrap_or("").to_lowercase();
        self.log(LogKind::Command, format!("Command: {line}"));
        match self.engine.run_line(line) {
            Ok(msg) => {
                self.log(LogKind::Normal, msg);
                if first == "open" || first == "new" {
                    self.reset_document_view();
                    self.saved_by_forma = false;
                }
                if first == "save" || first == "saveas" {
                    self.saved_by_forma = true;
                }
                if !matches!(
                    first.as_str(),
                    "undo" | "redo" | "u" | "open" | "save" | "saveas"
                ) {
                    self.last_command = Some(line.to_string());
                }
            }
            Err(e) => self.log(LogKind::Error, e.to_string()),
        }
    }

    fn has_selection(&self) -> bool {
        !self.engine.ctx.selection.is_empty()
    }

    fn start_tool(&mut self, kind: ToolKind) {
        self.tool = None;
        let plane = self.viewports[self.active].cplane();
        let doc = self.engine.doc();
        let sel = &self.engine.ctx.selection;
        let anchor = snap::selection_center(doc, sel).unwrap_or(Point3::ORIGIN);
        let skeleton = if kind.needs_selection() {
            snap::selection_skeleton(doc, sel)
        } else {
            Vec::new()
        };
        let mut tool = Tool::new(kind, plane, self.has_selection(), anchor, skeleton);
        if kind == ToolKind::Extrude {
            tool.anchor = self.extrude_anchor().unwrap_or(anchor);
        }
        self.log(LogKind::Command, format!("Command: {}", kind.name()));
        self.last_command = Some(kind.name().to_string());
        self.tool = Some(tool);
    }

    /// First point of the first selected curve (Extrude height line).
    fn extrude_anchor(&self) -> Option<Point3> {
        let doc = self.engine.doc();
        self.engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .find_map(|o| o.geometry.curve_points().first().copied())
    }

    fn refresh_tool_selection(&mut self) {
        let doc = self.engine.doc();
        let sel = &self.engine.ctx.selection;
        let skeleton = snap::selection_skeleton(doc, sel);
        let anchor = snap::selection_center(doc, sel);
        let ext = self.extrude_anchor();
        if let Some(t) = self.tool.as_mut() {
            t.skeleton = skeleton;
            if let Some(a) = anchor {
                t.anchor = a;
            }
            if t.kind == ToolKind::Extrude {
                if let Some(a) = ext {
                    t.anchor = a;
                }
            }
        }
    }

    fn handle_step(&mut self, step: Step) {
        match step {
            Step::Continue => {}
            Step::Done(cmds) => {
                self.tool = None;
                for c in cmds {
                    self.run_engine_from_tool(&c);
                }
            }
            Step::Emit(cmds) => {
                for c in cmds {
                    self.run_engine_from_tool(&c);
                }
            }
            Step::Cancel(msg) => {
                self.tool = None;
                self.log(LogKind::Normal, msg);
            }
        }
    }

    fn run_engine_from_tool(&mut self, line: &str) {
        let keep = self.last_command.clone();
        self.run_engine(line);
        // Repeating should restart the tool, not replay its coordinates.
        self.last_command = keep;
    }

    /// Enter, Space or right click.
    fn enter_action(&mut self) {
        let has_sel = self.has_selection();
        if let Some(t) = self.tool.as_mut() {
            let was_selecting = t.selecting;
            let step = t.enter(has_sel);
            if was_selecting && matches!(step, Step::Continue) {
                self.refresh_tool_selection();
            }
            self.handle_step(step);
            return;
        }
        if let Some(last) = self.last_command.clone() {
            self.submit(&last);
        }
    }

    fn cancel(&mut self) {
        if self.tool.take().is_some() {
            self.log(LogKind::Normal, "cancelled");
        } else if !self.command.is_empty() {
            self.command.clear();
        } else {
            self.engine.ctx.selection.clear();
        }
    }

    /// Text from the command line (one or more tokens).
    fn submit(&mut self, text: &str) {
        let toks: Vec<String> = text.split_whitespace().map(String::from).collect();
        if toks.is_empty() {
            self.enter_action();
            return;
        }
        if self.tool.is_some() {
            for t in &toks {
                self.feed_token(t);
                if self.tool.is_none() {
                    break;
                }
            }
            return;
        }
        let first = toks[0].to_lowercase();
        if let Some(kind) = ToolKind::from_name(&first) {
            self.start_tool(kind);
            for t in &toks[1..] {
                self.feed_token(t);
                if self.tool.is_none() {
                    break;
                }
            }
            return;
        }
        match first.as_str() {
            "ze" | "zoomextents" | "zea" => {
                let which = if first == "ze" {
                    Some(self.active)
                } else {
                    None
                };
                self.fit(which);
            }
            "top" => self.set_active_view(StandardView::Top),
            "front" => self.set_active_view(StandardView::Front),
            "right" => self.set_active_view(StandardView::Right),
            "perspective" | "persp" => self.set_active_view(StandardView::Perspective),
            "help" | "?" => {
                self.show_help();
                return;
            }
            "ortho" => self.snap.ortho = !self.snap.ortho,
            "snap" | "gridsnap" => self.snap.grid = !self.snap.grid,
            _ => {
                self.run_engine(text);
                return;
            }
        }
        self.log(LogKind::Command, format!("Command: {text}"));
    }

    fn set_active_view(&mut self, v: StandardView) {
        let vp = &mut self.viewports[self.active];
        let target = vp.camera.target;
        vp.camera = forma_render::Camera::view(v);
        vp.camera.target = target;
        vp.kind = v;
        self.fit(Some(self.active));
    }

    fn show_help(&mut self) {
        self.log(LogKind::Command, "Command: Help");
        let mut lines: Vec<String> = Vec::new();
        lines.push(
            "Drawing: click points in a view, or type x,y · @dx,dy · @dist<angle · a length:"
                .into(),
        );
        for k in ToolKind::CURVES
            .iter()
            .chain(&ToolKind::SOLIDS)
            .chain(&ToolKind::TRANSFORMS)
        {
            lines.push(format!("  {}", k.tooltip()));
        }
        lines.push("Other: Delete · SelAll · SelNone · Layer <name> · ChangeLayer <name> · Undo · Redo · Save · Open · New [mm|cm|m] · ZE · ZEA".into());
        lines.push("Keys: Enter/Space/right click = confirm or repeat · Esc = cancel · Del = delete · F8 = Ortho (or hold Shift) · F9 = grid snap · F7 = grid · Ctrl+Z/Y/A/S/O/N".into());
        lines.push("Mouse: left = pick/select (drag: window →, crossing ←) · right drag = rotate (pan in Top/Front/Right) · Shift+right / middle = pan · wheel = zoom · double-click a view title = maximize".into());
        for l in lines {
            self.log(LogKind::Normal, l);
        }
    }

    fn feed_token(&mut self, tok: &str) {
        let Some(t) = self.tool.as_mut() else { return };
        if let Some(step) = t.option(tok) {
            self.handle_step(step);
            return;
        }
        let want = t.want();
        let toward = self.hover.as_ref().map(|h| h.point);
        let step = match want {
            Want::Selection => {
                self.log(
                    LogKind::Normal,
                    "select objects in a view, then press Enter",
                );
                return;
            }
            Want::Height { .. } => match tok.parse::<f64>() {
                Ok(x) => t.feed_number(x),
                Err(_) => {
                    self.log(LogKind::Error, format!("a number is expected: {tok}"));
                    return;
                }
            },
            Want::PointOrNumber if tok.parse::<f64>().is_ok() => {
                t.feed_number(tok.parse().expect("checked"))
            }
            Want::Point | Want::PointOrNumber => {
                let plane = t.plane;
                match parse_typed_point(tok, &plane, t.base(), toward) {
                    Some(p) => t.feed_point(p),
                    None => {
                        self.log(LogKind::Error, format!("cannot read a point from: {tok}"));
                        return;
                    }
                }
            }
        };
        self.handle_step(step);
    }

    // ----------------------------------------------------------------- files

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Rhino 3D model", &["3dm"])
            .pick_file()
        {
            self.run_engine(&format!("Open {}", p.display()));
        }
    }

    fn save(&mut self, save_as: bool) {
        let path = self.engine.doc().path.clone();
        if !save_as && self.saved_by_forma && path.is_some() {
            self.run_engine("Save");
            return;
        }
        let suggested = path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map(|s| format!("{}-forma.3dm", s.to_string_lossy()))
            .unwrap_or_else(|| "senza-titolo.3dm".into());
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Rhino 3D model", &["3dm"])
            .set_file_name(suggested);
        if let Some(dir) = path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).parent())
        {
            dialog = dialog.set_directory(dir);
        }
        if let Some(p) = dialog.save_file() {
            if path.is_some() && !self.saved_by_forma {
                self.log(
                    LogKind::Normal,
                    "note: surfaces imported from Rhino are saved as meshes in this version",
                );
            }
            self.run_engine(&format!("Save {}", p.display()));
        }
    }

    // ----------------------------------------------------------------- input

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let cmd_id = egui::Id::new(CMD_ID);
        let focused = ctx.memory(|m| m.focused());
        let cmd_focused = focused == Some(cmd_id);
        let other_focused = focused.is_some() && !cmd_focused;
        let empty = self.command.is_empty();

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.cancel();
        }
        // Enter in the command line submits and keeps the focus there.
        if cmd_focused && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            let text = std::mem::take(&mut self.command);
            self.submit(&text);
            self.focus_command = true;
        }
        if empty && !other_focused {
            let mut actions: Vec<&str> = Vec::new();
            ctx.input_mut(|i| {
                if i.consume_key(Modifiers::COMMAND, Key::Z) {
                    actions.push("undo");
                }
                if i.consume_key(Modifiers::COMMAND, Key::Y) {
                    actions.push("redo");
                }
                if i.consume_key(Modifiers::COMMAND, Key::A) {
                    actions.push("selall");
                }
                if i.consume_key(Modifiers::NONE, Key::Delete) {
                    actions.push("delete");
                }
            });
            for a in actions {
                match a {
                    "undo" => self.run_engine("Undo"),
                    "redo" => self.run_engine("Redo"),
                    "selall" => self.run_engine("SelAll"),
                    "delete" if self.has_selection() && self.tool.is_none() => {
                        self.run_engine("Delete")
                    }
                    _ => {}
                }
            }
        }
        let (save_as, save, open, new) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::N),
            )
        });
        if save_as {
            self.save(true);
        } else if save {
            self.save(false);
        }
        if open {
            self.open_dialog();
        }
        if new {
            self.run_engine("New");
        }
        let (f7, f8, f9) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F7),
                i.consume_key(Modifiers::NONE, Key::F8),
                i.consume_key(Modifiers::NONE, Key::F9),
            )
        });
        if f8 {
            self.snap.ortho = !self.snap.ortho;
        }
        if f9 {
            self.snap.grid = !self.snap.grid;
        }
        if f7 {
            if let Some(r) = self.renderer.as_mut() {
                r.show_grid = !r.show_grid;
            }
            for v in &mut self.viewports {
                v.dirty = true;
            }
        }

        // Keystrokes typed while nothing has focus go to the command line (Rhino).
        if focused.is_none() {
            let mut typed = String::new();
            let mut enter = false;
            ctx.input_mut(|i| {
                i.events.retain(|e| match e {
                    egui::Event::Text(t) => {
                        typed.push_str(t);
                        false
                    }
                    egui::Event::Key {
                        key: Key::Enter,
                        pressed: true,
                        ..
                    } => {
                        enter = true;
                        false
                    }
                    _ => true,
                });
            });
            for ch in typed.chars() {
                if ch == ' ' {
                    let text = std::mem::take(&mut self.command);
                    self.submit(&text);
                } else {
                    self.command.push(ch);
                    self.focus_command = true;
                }
            }
            if enter {
                let text = std::mem::take(&mut self.command);
                self.submit(&text);
            }
        }
    }

    /// Cursor point in a viewport with snaps, grid and ortho applied.
    fn compute_hover(&self, vi: usize, pos: Pos2, shift: bool) -> Hover {
        let vp = &self.viewports[vi];
        let origin = self.origin();
        let want = self.tool.as_ref().map(|t| t.want());
        let picking = matches!(
            want,
            Some(Want::Point | Want::PointOrNumber | Want::Height { .. })
        );
        let osnap = if picking {
            self.snaps.find(vp, pos, origin, &self.snap, 14.0)
        } else {
            None
        };
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
            };
        }
        if let Some((p, k)) = osnap {
            return Hover {
                viewport: vi,
                point: p,
                snap: Some(k),
                pos,
            };
        }
        let tool_plane = self
            .tool
            .as_ref()
            .filter(|t| !t.pts.is_empty())
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
        if self.snap.grid {
            p = snap::grid_snap(p, &plane, self.snap.step);
        }
        if let Some(base) = self.tool.as_ref().and_then(|t| t.base()) {
            if self.snap.ortho != shift {
                p = snap::ortho(p, base, &plane);
            }
        }
        Hover {
            viewport: vi,
            point: p,
            snap: None,
            pos,
        }
    }

    fn click_select(&mut self, vi: usize, pos: Pos2, mods: Modifiers) {
        let id = snap::pick(self.engine.doc(), &self.viewports[vi], pos, self.origin());
        let selecting_for_tool = self.tool.as_ref().is_some_and(|t| t.selecting);
        let sel = &mut self.engine.ctx.selection;
        match id {
            Some(id) if mods.command => {
                sel.remove(&id);
            }
            Some(id) if mods.shift || selecting_for_tool => {
                sel.insert(id);
            }
            Some(id) => {
                sel.clear();
                sel.insert(id);
            }
            None if !(mods.shift || mods.command || selecting_for_tool) => sel.clear(),
            None => {}
        }
    }

    fn window_select(&mut self, vi: usize, a: Pos2, b: Pos2, mods: Modifiers) {
        let rect = Rect::from_two_pos(a, b);
        let crossing = b.x < a.x;
        let ids = snap::window_select(
            self.engine.doc(),
            &self.viewports[vi],
            rect,
            crossing,
            self.origin(),
        );
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

    fn viewport_input(&mut self, ui: &egui::Ui, vi: usize, resp: &egui::Response) {
        let mods = ui.input(|i| i.modifiers);
        let origin = self.origin();
        let ppp = ui.ctx().pixels_per_point() as f64;

        if resp.clicked_by(PointerButton::Primary)
            || resp.clicked_by(PointerButton::Secondary)
            || resp.drag_started()
        {
            self.active = vi;
        }

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

        // Cursor point (snaps etc).
        if let Some(pos) = resp.hover_pos() {
            self.hover = Some(self.compute_hover(vi, pos, mods.shift));
        }

        // Window / crossing selection.
        let tool_wants_points = self.tool.as_ref().is_some_and(|t| !t.selecting);
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
                let h = self.compute_hover(vi, pos, mods.shift);
                if let Some(t) = self.tool.as_mut() {
                    let step = t.feed_point(h.point);
                    self.handle_step(step);
                }
            } else {
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

    // ----------------------------------------------------------------- drawing

    fn render_viewports(&mut self, frame: &eframe::Frame, ppp: f32) {
        let Some(rs) = frame.wgpu_render_state() else {
            return;
        };
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let visible: Vec<usize> = match self.maximized {
            Some(m) => vec![m],
            None => (0..self.viewports.len()).collect(),
        };
        for i in visible {
            let vp = &mut self.viewports[i];
            if !vp.rect.is_positive() {
                continue;
            }
            let px = (
                (vp.rect.width() * ppp).round().max(1.0) as u32,
                (vp.rect.height() * ppp).round().max(1.0) as u32,
            );
            if !(vp.dirty || px != vp.px || vp.texture.is_none()) {
                continue;
            }
            let view = vp.view.get_or_insert_with(|| renderer.new_view(&rs.device));
            let tex = renderer.render(
                &rs.device,
                &rs.queue,
                view,
                px,
                &vp.camera,
                grid_plane_for(vp.kind),
            );
            let mut egui_renderer = rs.renderer.write();
            match vp.texture {
                Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    tex,
                    egui_wgpu::wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    vp.texture = Some(egui_renderer.register_native_texture(
                        &rs.device,
                        tex,
                        egui_wgpu::wgpu::FilterMode::Linear,
                    ))
                }
            }
            vp.px = px;
            vp.dirty = false;
        }
    }

    fn draw_overlays(&self, painter: &egui::Painter, vi: usize) {
        let vp = &self.viewports[vi];
        let p = painter.with_clip_rect(vp.rect);
        let origin = self.origin();
        let seg = |a: Point3, b: Point3, s: Stroke| {
            if let (Some(sa), Some(sb)) = (vp.to_screen(a, origin), vp.to_screen(b, origin)) {
                p.line_segment([sa, sb], s);
            }
        };
        if let (Some(t), Some(h)) = (self.tool.as_ref(), self.hover.as_ref()) {
            let st = Stroke::new(1.3, Color32::from_rgb(20, 20, 24));
            for [a, b] in t.preview(h.point) {
                seg(a, b, st);
            }
            if let Some(s) = vp.to_screen(h.point, origin) {
                if !t.selecting {
                    p.circle_filled(s, 3.0, Color32::from_rgb(20, 20, 24));
                }
                if let Some(k) = h.snap {
                    let r = Rect::from_center_size(s, egui::vec2(10.0, 10.0));
                    p.rect_stroke(r, 0.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Middle);
                    if h.viewport == vi {
                        p.text(
                            s + egui::vec2(9.0, -9.0),
                            egui::Align2::LEFT_BOTTOM,
                            k.label(),
                            egui::FontId::proportional(13.0),
                            Color32::WHITE,
                        );
                    }
                }
            }
        }
        if let (Some(d), Some(h)) = (self.drag.as_ref(), self.hover.as_ref()) {
            if d.viewport == vi {
                let r = Rect::from_two_pos(d.start, h.pos);
                let crossing = h.pos.x < d.start.x;
                let (fill, stroke) = if crossing {
                    (
                        Color32::from_rgba_unmultiplied(80, 200, 80, 30),
                        Stroke::new(1.0, Color32::from_rgb(30, 110, 30)),
                    )
                } else {
                    (
                        Color32::from_rgba_unmultiplied(80, 120, 220, 30),
                        Stroke::new(1.0, Color32::from_rgb(30, 50, 140)),
                    )
                };
                p.rect_filled(r, 0.0, fill);
                if crossing {
                    for e in [
                        [r.left_top(), r.right_top()],
                        [r.right_top(), r.right_bottom()],
                        [r.right_bottom(), r.left_bottom()],
                        [r.left_bottom(), r.left_top()],
                    ] {
                        p.extend(egui::Shape::dashed_line(&e, stroke, 5.0, 4.0));
                    }
                } else {
                    p.rect_stroke(r, 0.0, stroke, StrokeKind::Inside);
                }
            }
        }
        // Title and frame.
        let active = vi == self.active;
        let color = if active {
            Color32::WHITE
        } else {
            Color32::from_gray(225)
        };
        let r = p.text(
            vp.rect.left_top() + egui::vec2(8.0, 5.0),
            egui::Align2::LEFT_TOP,
            vp.name(),
            egui::FontId::proportional(13.0),
            color,
        );
        if active {
            p.line_segment(
                [
                    r.left_bottom() + egui::vec2(0.0, 1.0),
                    r.right_bottom() + egui::vec2(0.0, 1.0),
                ],
                Stroke::new(1.5, Color32::WHITE),
            );
        }
        p.rect_stroke(
            vp.rect,
            0.0,
            Stroke::new(
                if active { 1.5 } else { 1.0 },
                if active {
                    Color32::from_gray(235)
                } else {
                    Color32::from_gray(90)
                },
            ),
            StrokeKind::Inside,
        );
    }

    fn ui_viewports(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let full = ui.available_rect_before_wrap();
        let gap = 2.0;
        let rects: Vec<(usize, Rect)> = match self.maximized {
            Some(m) => vec![(m, full)],
            None => {
                let w = (full.width() - gap) / 2.0;
                let h = (full.height() - gap) / 2.0;
                let at = |c: f32, r: f32| {
                    Rect::from_min_size(
                        full.left_top() + egui::vec2(c * (w + gap), r * (h + gap)),
                        egui::vec2(w, h),
                    )
                };
                vec![
                    (0, at(0.0, 0.0)),
                    (1, at(1.0, 0.0)),
                    (2, at(0.0, 1.0)),
                    (3, at(1.0, 1.0)),
                ]
            }
        };
        for (i, r) in &rects {
            self.viewports[*i].rect = *r;
        }
        if self.pending_fit && self.renderer.is_some() && self.seen_version != u64::MAX {
            self.fit(None);
            self.pending_fit = false;
        }
        let mut any_hover = false;
        for (i, r) in rects.clone() {
            let resp = ui.interact(
                r,
                egui::Id::new(("viewport", i)),
                egui::Sense::click_and_drag(),
            );
            if resp.hovered() {
                any_hover = true;
            }
            // Double-click on the title toggles maximize.
            let title_rect = Rect::from_min_size(r.left_top(), egui::vec2(110.0, 22.0));
            if resp.double_clicked_by(PointerButton::Primary)
                && resp
                    .interact_pointer_pos()
                    .is_some_and(|p| title_rect.contains(p))
            {
                self.maximized = if self.maximized.is_some() {
                    None
                } else {
                    Some(i)
                };
                self.active = i;
                for v in &mut self.viewports {
                    v.dirty = true;
                }
                continue;
            }
            self.viewport_input(ui, i, &resp);
        }
        if !any_hover && self.drag.is_none() {
            self.hover = None;
        }
        self.render_viewports(frame, ui.ctx().pixels_per_point());
        let painter = ui.painter();
        for (i, r) in &rects {
            if let Some(id) = self.viewports[*i].texture {
                painter.image(
                    id,
                    *r,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
        for (i, _) in &rects {
            self.draw_overlays(painter, *i);
        }
        if self.viewports.iter().any(|v| v.dirty) {
            ui.ctx().request_repaint();
        }
    }

    fn icon_button(&mut self, ui: &mut egui::Ui, icon: Icon, tip: &str, active: bool) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::click());
        let visuals = ui.style().interact_selectable(&resp, active);
        if resp.hovered() || active {
            ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
        }
        icons::paint(ui.painter(), rect, icon, visuals.fg_stroke.color);
        resp.on_hover_text(tip).clicked()
    }

    fn ui_tool_group(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        kinds: &[ToolKind],
    ) -> Option<ToolKind> {
        let current = self.tool.as_ref().map(|t| t.kind);
        let mut start = None;
        ui.label(egui::RichText::new(title).small().weak());
        egui::Grid::new(title).spacing([2.0, 2.0]).show(ui, |ui| {
            for (k, kind) in kinds.iter().enumerate() {
                if self.icon_button(
                    ui,
                    Icon::Tool(*kind),
                    kind.tooltip(),
                    current == Some(*kind),
                ) {
                    start = Some(*kind);
                }
                if k % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.add_space(6.0);
        start
    }

    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        let mut start = None;
        start = start.or(self.ui_tool_group(ui, "Curve", &ToolKind::CURVES));
        start = start.or(self.ui_tool_group(ui, "Solid", &ToolKind::SOLIDS));
        start = start.or(self.ui_tool_group(ui, "Transform", &ToolKind::TRANSFORMS));
        let mut action: Option<&str> = None;
        ui.label(egui::RichText::new("Edit").small().weak());
        egui::Grid::new("edit").spacing([2.0, 2.0]).show(ui, |ui| {
            if self.icon_button(ui, Icon::Delete, "Delete selected (Del)", false) {
                action = Some("Delete");
            }
            if self.icon_button(ui, Icon::SelectAll, "Select all (Ctrl+A)", false) {
                action = Some("SelAll");
            }
        });
        if let Some(k) = start {
            self.start_tool(k);
        }
        if let Some(a) = action {
            self.run_engine(a);
        }
    }

    fn ui_menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                for (label, cmd) in [
                    ("New (mm)       Ctrl+N", "New mm"),
                    ("New (cm)", "New cm"),
                    ("New (m)", "New m"),
                ] {
                    if ui.button(label).clicked() {
                        ui.close();
                        self.run_engine(cmd);
                    }
                }
                ui.separator();
                if ui.button("Open…          Ctrl+O").clicked() {
                    ui.close();
                    self.open_dialog();
                }
                if ui.button("Save           Ctrl+S").clicked() {
                    ui.close();
                    self.save(false);
                }
                if ui.button("Save As…  Ctrl+Shift+S").clicked() {
                    ui.close();
                    self.save(true);
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                for (label, cmd) in [
                    ("Undo          Ctrl+Z", "Undo"),
                    ("Redo          Ctrl+Y", "Redo"),
                    ("Select All    Ctrl+A", "SelAll"),
                    ("Select None   Esc", "SelNone"),
                    ("Delete        Del", "Delete"),
                ] {
                    if ui.button(label).clicked() {
                        ui.close();
                        self.run_engine(cmd);
                    }
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Zoom Extents (all views)").clicked() {
                    ui.close();
                    self.fit(None);
                }
                let label = if self.maximized.is_some() {
                    "Show 4 views"
                } else {
                    "Maximize active view"
                };
                if ui.button(label).clicked() {
                    ui.close();
                    self.maximized = if self.maximized.is_some() {
                        None
                    } else {
                        Some(self.active)
                    };
                    for v in &mut self.viewports {
                        v.dirty = true;
                    }
                }
                ui.separator();
                if let Some(r) = &mut self.renderer {
                    if ui.checkbox(&mut r.show_grid, "Grid  (F7)").changed() {
                        for v in &mut self.viewports {
                            v.dirty = true;
                        }
                    }
                }
            });
            for (title, kinds) in [
                ("Curve", &ToolKind::CURVES[..]),
                ("Solid", &ToolKind::SOLIDS[..]),
                ("Transform", &ToolKind::TRANSFORMS[..]),
            ] {
                ui.menu_button(title, |ui| {
                    for k in kinds {
                        if ui.button(k.tooltip()).clicked() {
                            ui.close();
                            self.start_tool(*k);
                        }
                    }
                });
            }
            ui.menu_button("Help", |ui| {
                if ui.button("Commands, keys and mouse").clicked() {
                    ui.close();
                    self.show_help();
                }
            });
            ui.separator();
            let mut go: Option<&str> = None;
            for (icon, tip, cmd) in [
                (Icon::New, "New (Ctrl+N)", "new"),
                (Icon::Open, "Open (Ctrl+O)", "open"),
                (Icon::Save, "Save (Ctrl+S)", "save"),
                (Icon::Undo, "Undo (Ctrl+Z)", "undo"),
                (Icon::Redo, "Redo (Ctrl+Y)", "redo"),
                (Icon::ZoomExtents, "Zoom extents, all views", "zea"),
            ] {
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
                let v = ui.style().interact(&resp);
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 3.0, v.weak_bg_fill);
                }
                icons::paint(ui.painter(), rect.expand(2.0), icon, v.fg_stroke.color);
                if resp.on_hover_text(tip).clicked() {
                    go = Some(cmd);
                }
            }
            match go {
                Some("new") => self.run_engine("New"),
                Some("open") => self.open_dialog(),
                Some("save") => self.save(false),
                Some("undo") => self.run_engine("Undo"),
                Some("redo") => self.run_engine("Redo"),
                Some("zea") => self.fit(None),
                _ => {}
            }
        });
    }

    fn ui_layers(&mut self, ui: &mut egui::Ui) {
        let mut new_layer = false;
        ui.horizontal(|ui| {
            ui.heading("Layers");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("+ New")
                    .on_hover_text("New layer (becomes current)")
                    .clicked()
                {
                    new_layer = true;
                }
            });
        });
        if new_layer {
            let mut k = self.engine.doc().layers.len();
            let name = loop {
                let candidate = format!("Layer {k:02}");
                if self.engine.doc().find_layer(&candidate).is_none() {
                    break candidate;
                }
                k += 1;
            };
            self.run_engine(&format!("Layer {name}"));
        }
        ui.separator();
        let doc = self.engine.doc();
        let mut counts = vec![0usize; doc.layers.len()];
        for o in doc.objects() {
            counts[o.layer.0] += 1;
        }
        let current = doc.current_layer.0;
        let has_sel = self.has_selection();
        let mut toggled = false;
        let mut make_current: Option<String> = None;
        let mut move_here: Option<String> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, layer) in self.engine.ctx.doc.layers.iter_mut().enumerate() {
                let depth = layer.name.matches("::").count();
                let short = layer
                    .name
                    .rsplit("::")
                    .next()
                    .unwrap_or(&layer.name)
                    .to_string();
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 12.0);
                    if ui
                        .checkbox(&mut layer.visible, "")
                        .on_hover_text("Show / hide")
                        .changed()
                    {
                        toggled = true;
                    }
                    let [r, g, b] = layer.color;
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                    let text = if i == current {
                        egui::RichText::new(format!("✔ {short}")).strong()
                    } else {
                        egui::RichText::new(short)
                    };
                    let resp = ui
                        .add(egui::Label::new(text).sense(egui::Sense::click()))
                        .on_hover_text(format!(
                            "{} — {} objects\nClick: make current · Right click: more",
                            layer.name, counts[i]
                        ));
                    if resp.clicked() {
                        make_current = Some(layer.name.clone());
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Make current").clicked() {
                            make_current = Some(layer.name.clone());
                            ui.close();
                        }
                        if ui
                            .add_enabled(has_sel, egui::Button::new("Move selected objects here"))
                            .clicked()
                        {
                            move_here = Some(layer.name.clone());
                            ui.close();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(counts[i].to_string());
                    });
                });
            }
        });
        if toggled {
            self.force_rebuild = true;
        }
        if let Some(n) = make_current {
            self.run_engine(&format!("Layer {n}"));
        }
        if let Some(n) = move_here {
            self.run_engine(&format!("ChangeLayer {n}"));
        }
    }

    fn ui_command_line(&mut self, ui: &mut egui::Ui) {
        let log_height = (ui.available_height() - 30.0).max(20.0);
        egui::ScrollArea::vertical()
            .max_height(log_height)
            .min_scrolled_height(log_height)
            .stick_to_bottom(true)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (kind, line) in &self.log {
                    let t = egui::RichText::new(line).monospace();
                    match kind {
                        LogKind::Normal => ui.label(t),
                        LogKind::Command => ui.label(t.weak()),
                        LogKind::Error => ui.colored_label(ui.visuals().error_fg_color, t),
                    };
                }
            });
        ui.horizontal(|ui| {
            let prompt = match &self.tool {
                Some(t) => format!("{}:", t.prompt()),
                None => "Command:".to_string(),
            };
            ui.label(egui::RichText::new(prompt).monospace().strong());
            let id = egui::Id::new(CMD_ID);
            let edit = egui::TextEdit::singleline(&mut self.command)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY);
            let resp = ui.add(edit);
            if self.focus_command {
                resp.request_focus();
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                    let end = egui::text::CCursor::new(self.command.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(end)));
                    state.store(ui.ctx(), id);
                }
                self.focus_command = false;
            }
            if resp.changed() && self.command.ends_with(' ') {
                let text = std::mem::take(&mut self.command);
                self.submit(&text);
            }
        });
    }

    fn ui_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let coords = match &self.hover {
                Some(h) => {
                    let (u, v, w) = self.viewports[h.viewport].cplane().coords(h.point);
                    format!(
                        "{}  x {:.2}  y {:.2}  z {:.2}",
                        self.viewports[h.viewport].name(),
                        u,
                        v,
                        w
                    )
                }
                None => "—".to_string(),
            };
            ui.monospace(coords);
            ui.separator();
            ui.weak(self.engine.doc().units.abbreviation());
            ui.separator();
            ui.toggle_value(&mut self.snap.grid, "Grid Snap")
                .on_hover_text("F9");
            ui.toggle_value(&mut self.snap.ortho, "Ortho")
                .on_hover_text("F8, or hold Shift");
            ui.add(
                egui::DragValue::new(&mut self.snap.step)
                    .speed(0.1)
                    .range(0.0001..=1.0e6)
                    .prefix("step "),
            )
            .on_hover_text("Grid snap step");
            ui.separator();
            ui.weak("Osnap:");
            ui.toggle_value(&mut self.snap.end, "End");
            ui.toggle_value(&mut self.snap.mid, "Mid");
            ui.toggle_value(&mut self.snap.cen, "Cen");
            ui.toggle_value(&mut self.snap.quad, "Quad");
            ui.separator();
            let doc = self.engine.doc();
            ui.weak(format!("Layer: {}", doc.layer(doc.current_layer).name));
            ui.separator();
            ui.weak(format!(
                "{} objects · {} selected · {} triangles",
                doc.len(),
                self.engine.ctx.selection.len(),
                self.triangles
            ));
        });
    }
}

impl eframe::App for FormaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().display().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        });
        if let Some(p) = dropped.into_iter().next() {
            self.run_engine(&format!("Open {p}"));
        }
        self.handle_keyboard(&ctx);
        self.sync(frame);

        let title = match &self.engine.doc().path {
            Some(p) => format!(
                "Forma — {}",
                std::path::Path::new(p)
                    .file_name()
                    .map_or(p.clone(), |n| n.to_string_lossy().into_owned())
            ),
            None => "Forma — senza titolo".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("menu").show(ui, |ui| self.ui_menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.ui_status(ui));
        egui::Panel::bottom("command")
            .resizable(true)
            .default_size(130.0)
            .size_range(60.0..=500.0)
            .show(ui, |ui| self.ui_command_line(ui));
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(80.0)
            .show(ui, |ui| self.ui_toolbar(ui));
        egui::Panel::right("layers")
            .resizable(true)
            .default_size(230.0)
            .show(ui, |ui| self.ui_layers(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_gray(60)))
            .show(ui, |ui| self.ui_viewports(ui, frame));

        // Changes made by panels this frame (commands, layer toggles) need a redraw.
        if self.force_rebuild
            || self.engine.doc().version() != self.seen_version
            || self.engine.ctx.selection != self.seen_selection
            || self.tool.is_some()
        {
            ctx.request_repaint();
        }
    }
}
