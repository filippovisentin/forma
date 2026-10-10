//! App construction, the frame loop, document → GPU sync and persistent
//! settings.

use crate::complete::{Completer, UI_COMMANDS};
use crate::settings::Settings;
use crate::snap::{SnapKind, SnapSettings};
use crate::tools::{ToolKind, SELECTION_COMMANDS};
use crate::viewport::Viewport;
use crate::{perf, pick, theme, FormaApp, LogKind, SelFilter, SelInfo, SidePanel};
use eframe::egui::{self, Color32};
use forma_doc::{LengthUnit, ObjectId};
use forma_engine::Engine;
use forma_render::glam::DVec3;
use forma_render::{model_center, DisplayMode, Renderer, SceneCache, StandardView};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(crate) fn min_grid_extent(u: LengthUnit) -> f64 {
    match u {
        LengthUnit::Millimeters => 3000.0,
        LengthUnit::Centimeters => 300.0,
        LengthUnit::Meters => 3.0,
        LengthUnit::Inches => 120.0,
        LengthUnit::Feet => 10.0,
    }
}

/// Every command name the command line knows, for autocomplete.
fn completer(engine: &Engine) -> Completer {
    let engine_cmds = engine.command_list().into_iter().map(|(n, h)| {
        (
            n.to_string(),
            h.split_once(" — ").map_or(h, |(_, d)| d).to_string(),
        )
    });
    let tools = ToolKind::CURVES
        .iter()
        .chain(&ToolKind::CURVE_TOOLS)
        .chain(&ToolKind::SURFACES)
        .chain(&ToolKind::SOLIDS)
        .chain(&ToolKind::TRANSFORMS)
        .chain(&ToolKind::VISIBILITY)
        .chain(&ToolKind::ANALYZE)
        .chain(&ToolKind::EDIT)
        .map(|k| (k.name().to_string(), k.description().to_string()));
    let sel = SELECTION_COMMANDS
        .iter()
        .filter_map(|n| ToolKind::selection_command(n))
        .map(|k| (k.name().to_string(), k.description().to_string()));
    let ui = UI_COMMANDS
        .iter()
        .map(|(n, h)| (n.to_string(), h.to_string()));
    // UI descriptions first: they describe the interactive tool.
    Completer::new(tools.chain(sel).chain(ui).chain(engine_cmds))
}

impl FormaApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let renderer = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| Renderer::new(&rs.device));
        let engine = Engine::new();
        let completer = completer(&engine);
        let mut app = FormaApp {
            engine,
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
            index: Default::default(),
            sel_info: SelInfo::default(),
            snap: SnapSettings::default(),
            tool: None,
            last_command: None,
            command: String::new(),
            focus_command: false,
            history: Default::default(),
            completer,
            ac_choice: None,
            option_edit: None,
            log: Vec::new(),
            hover: None,
            drag: None,
            zoom_window: false,
            title: String::new(),
            saved_by_forma: false,
            saved_version: 0,
            confirm: None,
            guard_pass: false,
            allow_close: false,
            pending_fit: true,
            gumball_on: true,
            gumball_hot: None,
            gumball_drag: None,
            gumball_typed: None,
            face_sel: None,
            grips: crate::grips::Grips::default(),
            cplane_dirty: false,
            face_drag: None,
            face_typed: false,
            face_hot: false,
            offset_distance: 10.0,
            fillet_radius: 5.0,
            chamfer_distance: 5.0,
            side_panel: SidePanel::Properties,
            tab: 0,
            color_edit: [200, 60, 60],
            title_menu: None,
            filter: SelFilter {
                curves: true,
                surfaces: true,
                points: true,
            },
            show_osnap: true,
            saved_at: Instant::now(),
            pending: None,
            track_points: Vec::new(),
            settings: Settings::load(),
            settings_checked: Instant::now(),
            side_width: 280.0,
            command_height: 92.0,
        };
        theme::apply(&cc.egui_ctx);
        app.apply_settings();
        // Interior design in centimetres, like Filippo's Rhino files.
        let _ = app.engine.run_line("New cm");
        app.saved_version = app.engine.doc().version();
        app.reset_document_view();
        app.log(
            LogKind::Normal,
            "Forma — type a command or pick a tool on the left. Enter / Space / right click confirms or repeats, Esc cancels. Help lists everything.",
        );
        app
    }

    // ----------------------------------------------------------------- log

    pub(crate) fn log(&mut self, kind: LogKind, msg: impl Into<String>) {
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
    pub(crate) fn reset_document_view(&mut self) {
        self.cache.reset(model_center(self.engine.doc()));
        self.index.clear();
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
        self.option_edit = None;
        self.hover = None;
        self.face_sel = None;
    }

    pub(crate) fn origin(&self) -> DVec3 {
        self.cache.origin()
    }

    /// The document changed since it was opened or saved.
    pub(crate) fn is_modified(&self) -> bool {
        self.engine.doc().version() != self.saved_version
    }

    pub(crate) fn has_selection(&self) -> bool {
        !self.engine.ctx.selection.is_empty()
    }

    /// Rebuild GPU buffers, snap data and selection facts when the document or
    /// the selection changed.
    fn sync(&mut self, frame: &eframe::Frame) {
        self.sync_cplanes(frame);
        let doc = &self.engine.ctx.doc;
        let mut changed = false;
        let doc_changed = self.force_rebuild || doc.version() != self.seen_version;
        if doc_changed {
            let _t = perf::span("sync: snap index");
            self.index.update(doc);
        }
        let renderer = frame.wgpu_render_state().zip(self.renderer.as_mut());
        if let (true, Some((rs, renderer))) = (doc_changed, renderer) {
            let t = perf::span("sync: scene build");
            let scene = self.cache.scene(doc);
            drop(t);
            let origin = self.cache.origin();
            if scene.is_empty() {
                let e = min_grid_extent(doc.units) / 2.0;
                self.scene_min = DVec3::new(-e, -e, 0.0) - origin;
                self.scene_max = DVec3::new(e, e, 0.0) - origin;
            } else {
                self.scene_min = scene.min;
                self.scene_max = scene.max;
            }
            let _t = perf::span("sync: scene upload");
            renderer.set_scene(&rs.device, &scene, min_grid_extent(doc.units));
        }
        if doc_changed {
            self.grips.refresh(doc);
            self.seen_version = doc.version();
            self.force_rebuild = false;
            self.seen_selection.clear();
            self.seen_selection.insert(ObjectId(u64::MAX)); // force a highlight refresh
            changed = true;
        }
        if self.engine.ctx.selection != self.seen_selection {
            // Remember what the user had selected before deselecting (SelPrev).
            if self.engine.ctx.selection.is_empty()
                && !self.seen_selection.is_empty()
                && !self.seen_selection.contains(&ObjectId(u64::MAX))
            {
                self.engine
                    .ctx
                    .prev_selection
                    .clone_from(&self.seen_selection);
            }
            if let Some((rs, renderer)) = frame.wgpu_render_state().zip(self.renderer.as_mut()) {
                let _t = perf::span("sync: highlight");
                let hl = self.cache.highlight(doc, &self.engine.ctx.selection);
                renderer.set_highlight(&rs.device, &hl);
            }
            self.sel_info = SelInfo {
                bbox: pick::selection_box(doc, &self.engine.ctx.selection),
            };
            self.seen_selection = self.engine.ctx.selection.clone();
            changed = true;
        }
        if changed {
            self.dirty_all();
        }
    }

    pub(crate) fn fit(&mut self, which: Option<usize>) {
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

    pub(crate) fn dirty_all(&mut self) {
        for v in &mut self.viewports {
            v.dirty = true;
        }
    }

    /// A length in document units, for live readouts.
    pub(crate) fn fmt_len(&self, v: f64) -> String {
        let u = self.engine.doc().units.abbreviation();
        let decimals = match self.engine.doc().units {
            LengthUnit::Millimeters => 1,
            LengthUnit::Meters => 3,
            _ => 2,
        };
        format!("{v:.decimals$} {u}")
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = match &self.engine.doc().path {
            Some(p) => std::path::Path::new(p)
                .file_name()
                .map_or(p.clone(), |n| n.to_string_lossy().into_owned()),
            None => "Untitled".to_string(),
        };
        let star = if self.is_modified() { "*" } else { "" };
        let title = format!("{name}{star} — Forma");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    // ----------------------------------------------------------------- settings

    fn apply_settings(&mut self) {
        let s = &self.settings;
        for k in SnapKind::ALL {
            s.load_bool(&format!("osnap.{}", k.label()), self.snap.flag(k));
        }
        s.load_bool("osnap.project", &mut self.snap.project);
        s.load_bool("osnap.disabled", &mut self.snap.disabled);
        s.load_bool("snap.grid", &mut self.snap.grid);
        s.load_bool("snap.ortho", &mut self.snap.ortho);
        s.load_bool("snap.planar", &mut self.snap.planar);
        s.load_bool("snap.smarttrack", &mut self.snap.smart);
        s.load_bool("gumball", &mut self.gumball_on);
        s.load_bool("osnap_bar", &mut self.show_osnap);
        s.load_bool("filter.curves", &mut self.filter.curves);
        s.load_bool("filter.surfaces", &mut self.filter.surfaces);
        s.load_bool("filter.points", &mut self.filter.points);
        if let (Some(r), Some(g)) = (self.renderer.as_mut(), s.get_bool("grid")) {
            r.show_grid = g;
        }
        for (i, v) in self.viewports.iter_mut().enumerate() {
            let mode = s.get(&format!("view.{i}.mode"));
            if let Some(m) = DisplayMode::ALL
                .into_iter()
                .find(|m| Some(m.name()) == mode)
            {
                v.mode = m;
            }
        }
        if let Some(t) = s.get("tab").and_then(|t| t.parse::<usize>().ok()) {
            self.tab = t.min(crate::toolbar::TABS.len() - 1);
        }
        if let Some(w) = s.get_f32("panel.side") {
            self.side_width = w.clamp(160.0, 900.0);
        }
        if let Some(h) = s.get_f32("panel.command") {
            self.command_height = h.clamp(56.0, 400.0);
        }
    }

    /// Current settings (keeps keys this version does not know, and the
    /// recent-files list).
    fn collect_settings(&self) -> Settings {
        let mut s = self.settings.clone();
        for k in SnapKind::ALL {
            s.set(&format!("osnap.{}", k.label()), self.snap.is_on(k));
        }
        s.set("osnap.project", self.snap.project);
        s.set("osnap.disabled", self.snap.disabled);
        s.set("snap.grid", self.snap.grid);
        s.set("snap.ortho", self.snap.ortho);
        s.set("snap.planar", self.snap.planar);
        s.set("snap.smarttrack", self.snap.smart);
        s.set("gumball", self.gumball_on);
        s.set("osnap_bar", self.show_osnap);
        s.set("filter.curves", self.filter.curves);
        s.set("filter.surfaces", self.filter.surfaces);
        s.set("filter.points", self.filter.points);
        if let Some(r) = &self.renderer {
            s.set("grid", r.show_grid);
        }
        for (i, v) in self.viewports.iter().enumerate() {
            s.set(&format!("view.{i}.mode"), v.mode.name());
        }
        s.set("tab", self.tab);
        s.set("panel.side", self.side_width.round());
        s.set("panel.command", self.command_height.round());
        s
    }

    /// Write the settings file when something changed (checked about once a
    /// second, or right away when `now`).
    pub(crate) fn save_settings(&mut self, now: bool) {
        if !now && self.settings_checked.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.settings_checked = Instant::now();
        let s = self.collect_settings();
        if s != self.settings {
            if let Err(e) = s.save() {
                self.log(LogKind::Error, format!("could not save the settings: {e}"));
            }
            self.settings = s;
        }
    }
}

impl eframe::App for FormaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let _frame_span = perf::span_min("frame", 4.0);

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
        self.handle_close(&ctx);
        if self.confirm.is_none() {
            self.handle_keyboard(&ctx);
        }
        self.sync(frame);
        self.update_title(&ctx);

        let strip = egui::Frame::NONE
            .fill(theme::STRIP)
            .inner_margin(egui::Margin::symmetric(6, 2));
        egui::Panel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(4, 1)),
            )
            .show(ui, |ui| self.ui_menu(ui));
        let r = egui::Panel::top("command")
            .resizable(true)
            .default_size(self.command_height)
            .size_range(56.0..=400.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(4, 3)),
            )
            .show(ui, |ui| self.ui_command_line(ui));
        self.command_height = r.response.rect.height();
        egui::Panel::top("tabs")
            .frame(
                egui::Frame::NONE
                    .fill(theme::STRIP)
                    .inner_margin(egui::Margin {
                        left: 4,
                        right: 4,
                        top: 3,
                        bottom: 3,
                    }),
            )
            .show(ui, |ui| self.ui_tabs(ui));
        egui::Panel::bottom("status")
            .frame(strip)
            .show(ui, |ui| self.ui_status(ui));
        if self.show_osnap {
            egui::Panel::bottom("osnap")
                .frame(strip)
                .show(ui, |ui| self.ui_osnap(ui));
        }
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(72.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(2, 4)),
            )
            .show(ui, |ui| self.ui_toolbar(ui));
        let r = egui::Panel::right("side")
            .resizable(true)
            .default_size(self.side_width)
            .size_range(160.0..=900.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(6)),
            )
            .show(ui, |ui| self.ui_side(ui));
        self.side_width = r.response.rect.width();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_gray(150)))
            .show(ui, |ui| self.ui_viewports(ui, frame));
        self.ui_confirm(&ctx);

        // Changes made by panels this frame (commands, layer toggles) need a
        // redraw. Mouse and keyboard input repaint by themselves: no continuous
        // repaint while idle, even with a tool running.
        if self.force_rebuild
            || self.engine.doc().version() != self.seen_version
            || self.engine.ctx.selection != self.seen_selection
            || self.viewports.iter().any(|v| v.dirty)
        {
            ctx.request_repaint();
        }
        // Minutes-from-last-save counter.
        ctx.request_repaint_after(Duration::from_secs(30));
        self.save_settings(false);
    }
}
