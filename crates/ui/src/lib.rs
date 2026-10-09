//! Forma desktop UI (egui), laid out like Rhino: four viewports, a command line,
//! a tool sidebar and a layers panel. Every modelling action goes through
//! `forma-engine`; interactive tools only collect clicks and emit command lines.

mod gumball;
mod icons;
mod snap;
mod theme;
mod tools;
mod viewport;

use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, Stroke, StrokeKind};
use eframe::egui_wgpu;
use forma_doc::{LengthUnit, ObjectId};
use forma_engine::Engine;
use forma_geom::{Point3, Vec3, Xform};
use forma_render::glam::DVec3;
use forma_render::{grid_plane_for, model_center, DisplayMode, Renderer, SceneCache, StandardView};
use icons::Icon;
use snap::{SnapKind, SnapPoints, SnapSettings};
use std::collections::BTreeSet;
use tools::{parse_typed_point, Step, Tool, ToolKind, Want};
use viewport::{closest_on_line, to_d, to_p, Viewport};

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
    /// SmartTrack lines to draw (from a tracking point to the cursor).
    tracks: Vec<[Point3; 2]>,
}

struct DragSelect {
    viewport: usize,
    start: Pos2,
}

struct GumballDrag {
    viewport: usize,
    handle: gumball::Handle,
    center: Point3,
    start: gumball::Grip,
    motion: Option<gumball::Motion>,
    skeleton: Vec<[Point3; 2]>,
    /// Object snap the drag is following, if any.
    snapped: Option<(Point3, SnapKind)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidePanel {
    Properties,
    Layers,
    Help,
}

/// A menu / toolbar action, applied after the UI pass.
#[derive(Clone, Copy)]
enum Act {
    Tool(ToolKind),
    Cmd(&'static str),
    Submit(&'static str),
    Prefill(&'static str, &'static str),
    Open,
    Save,
    SaveAs,
    Exit,
    Help,
    Maximize,
    Panel(SidePanel),
    /// Set the active viewport's display mode.
    Mode(DisplayMode),
    /// Copy / paste through the engine clipboard.
    Clip(&'static str),
}

const ARRAY_PREFILL: Act = Act::Prefill(
    "Array ",
    "Array <nx> <ny> <nz> <dx,dy,dz> — e.g. Array 4 2 1 600,400,0 copies the selection 4×2 times",
);

/// Quick colours in the Properties panel.
const SWATCHES: [[u8; 3]; 14] = [
    [0, 0, 0],
    [128, 128, 128],
    [255, 255, 255],
    [255, 0, 0],
    [255, 127, 0],
    [255, 220, 0],
    [0, 160, 0],
    [0, 200, 200],
    [0, 0, 255],
    [150, 60, 200],
    [255, 0, 255],
    [130, 80, 40],
    [190, 160, 120],
    [70, 110, 140],
];

/// Rhino-style toolbar tabs above the viewports.
const TABS: [&str; 10] = [
    "Standard",
    "Set View",
    "Display",
    "Select",
    "Visibility",
    "Transform",
    "Curve Tools",
    "Surface Tools",
    "Solid Tools",
    "Analyze",
];

/// What a click can select (Rhino's selection Filter).
#[derive(Clone, Copy)]
struct SelFilter {
    curves: bool,
    surfaces: bool,
    points: bool,
}

impl SelFilter {
    fn accepts(self, g: &forma_doc::Geometry) -> bool {
        match g {
            forma_doc::Geometry::Point(_) => self.points,
            forma_doc::Geometry::Mesh(_) => self.surfaces,
            _ => self.curves,
        }
    }
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
    gumball_on: bool,
    gumball_hot: Option<(usize, gumball::Handle)>,
    gumball_drag: Option<GumballDrag>,
    /// A gumball handle was clicked: the next typed number moves/rotates by it.
    gumball_typed: Option<(gumball::Handle, Point3)>,
    offset_distance: f64,
    fillet_radius: f64,
    chamfer_distance: f64,
    side_panel: SidePanel,
    tab: usize,
    /// Colour being edited in the Properties panel.
    color_edit: [u8; 3],
    /// Viewport whose title menu is open.
    title_menu: Option<(usize, Pos2)>,
    filter: SelFilter,
    show_osnap: bool,
    /// Last open / save, for "minutes from last save".
    saved_at: std::time::Instant,
    /// An engine command waiting for its arguments (run on Enter), and what it asks for.
    pending: Option<(String, &'static str)>,
    /// SmartTrack points: object snaps the cursor rested on during this command.
    track_points: Vec<Point3>,
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
            gumball_on: true,
            gumball_hot: None,
            gumball_drag: None,
            gumball_typed: None,
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
            saved_at: std::time::Instant::now(),
            pending: None,
            track_points: Vec::new(),
        };
        theme::apply(&cc.egui_ctx);
        // Interior design in centimetres, like Filippo's Rhino files.
        let _ = app.engine.run_line("New cm");
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
                    self.saved_at = std::time::Instant::now();
                }
                if first == "save" || first == "saveas" {
                    self.saved_by_forma = true;
                    self.saved_at = std::time::Instant::now();
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
        self.gumball_typed = None;
        let plane = self.viewports[self.active].cplane();
        if kind.instant() && self.has_selection() {
            let line = Tool::new(kind, plane, true, Point3::ORIGIN, Vec::new()).instant_line();
            self.last_command = Some(kind.name().to_string());
            self.run_engine(&line);
            return;
        }
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
        tool.distance = match kind {
            ToolKind::Offset => self.offset_distance,
            ToolKind::Fillet => self.fillet_radius,
            ToolKind::Chamfer => self.chamfer_distance,
            _ => tool.distance,
        };
        tool.curves = self.selected_curves();
        self.log(LogKind::Command, format!("Command: {}", kind.name()));
        self.last_command = Some(kind.name().to_string());
        self.tool = Some(tool);
    }

    /// Selected curves with their own plane normals (Offset preview).
    fn selected_curves(&self) -> Vec<(forma_geom::Chain, Option<Vec3>)> {
        let doc = self.engine.doc();
        self.engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .filter_map(|o| {
                o.geometry
                    .to_chain()
                    .map(|c| (c, o.geometry.curve_normal()))
            })
            .collect()
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
        let curves = self.selected_curves();
        if let Some(t) = self.tool.as_mut() {
            t.skeleton = skeleton;
            t.curves = curves;
            if let Some(a) = anchor {
                t.anchor = a;
            }
            if matches!(t.kind, ToolKind::Extrude) {
                if let Some(a) = ext {
                    t.anchor = a;
                }
            }
        }
    }

    fn handle_step(&mut self, step: Step) {
        if matches!(step, Step::Done(_) | Step::Cancel(_)) {
            self.track_points.clear();
        }
        if let Some(t) = &self.tool {
            match t.kind {
                ToolKind::Offset => self.offset_distance = t.distance,
                ToolKind::Fillet => self.fillet_radius = t.distance,
                ToolKind::Chamfer => self.chamfer_distance = t.distance,
                _ => {}
            }
        }
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
        // Split: first the objects to split, then the cutting objects.
        if let Some(t) = self.tool.as_mut().filter(|t| t.kind == ToolKind::Split) {
            if t.phase == 0 {
                if !has_sel {
                    self.tool = None;
                    self.log(LogKind::Normal, "nothing selected");
                    return;
                }
                t.stash = self.engine.ctx.selection.iter().map(|i| i.0).collect();
                t.phase = 1;
                self.engine.ctx.selection.clear();
                return;
            }
            let ids: Vec<String> = t.stash.iter().map(|i| format!("#{i}")).collect();
            let cutters: Vec<String> = self
                .engine
                .ctx
                .selection
                .iter()
                .map(|i| format!("#{}", i.0))
                .collect();
            let cmds = vec![
                "SelNone".to_string(),
                format!("Select {}", ids.join(" ")),
                format!("Split {}", cutters.join(" ")).trim().to_string(),
            ];
            self.handle_step(Step::Done(cmds));
            return;
        }
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
        let gumball = self.gumball_typed.take().is_some()
            | self.gumball_drag.take().is_some()
            | self.pending.take().is_some();
        if gumball || self.tool.take().is_some() {
            self.log(LogKind::Normal, "cancelled");
        } else if !self.command.is_empty() {
            self.command.clear();
        } else {
            self.engine.ctx.selection.clear();
        }
    }

    /// Text from the command line (one or more tokens).
    /// Text ended by a space (Rhino: space works like Enter, except that a
    /// command that needs arguments keeps collecting them until Enter).
    fn submit_space(&mut self, text: &str) {
        let toks: Vec<&str> = text.split_whitespace().collect();
        if toks.is_empty() {
            self.submit(text);
            return;
        }
        if let Some((line, _)) = self.pending.as_mut() {
            line.push(' ');
            line.push_str(&toks.join(" "));
            return;
        }
        if self.pending.is_none() && self.tool.is_none() && self.gumball_typed.is_none() {
            let first = toks[0];
            let is_tool = ToolKind::from_name(first).is_some()
                || ToolKind::selection_command(first).is_some();
            if !is_tool {
                if let Some(what) = self.engine.missing_input(first) {
                    self.pending = Some((toks.join(" "), what));
                    return;
                }
            }
        }
        self.submit(text);
    }

    fn submit(&mut self, text: &str) {
        let toks: Vec<String> = text.split_whitespace().map(String::from).collect();
        // A command waiting for arguments: collect them until Enter.
        if let Some((mut line, _)) = self.pending.take() {
            if !toks.is_empty() {
                line.push(' ');
                line.push_str(&toks.join(" "));
            }
            self.run_engine(&line);
            return;
        }
        if toks.is_empty() {
            if self.gumball_typed.take().is_some() {
                return;
            }
            self.enter_action();
            return;
        }
        if let Some((h, center)) = self.gumball_typed.take() {
            match toks[0].parse::<f64>() {
                Ok(v) => {
                    for line in self.gumball_lines(gumball::typed_motion(h, v), center) {
                        self.run_engine(&line);
                    }
                }
                Err(_) => self.log(LogKind::Error, format!("a number is expected: {}", toks[0])),
            }
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
        if toks.len() == 1 && !self.has_selection() {
            if let Some(kind) = ToolKind::selection_command(&first) {
                self.start_tool(kind);
                return;
            }
        }
        if toks.len() == 1 && first == "projecttocplane" {
            self.start_tool(ToolKind::OnSel("ProjectToCPlane"));
            return;
        }
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
            "planar" => self.snap.planar = !self.snap.planar,
            "osnap" => self.snap.disabled = !self.snap.disabled,
            "gumball" => self.gumball_on = !self.gumball_on,
            "zs" | "zoomselected" => self.zoom_selected(),
            "wireframe" | "setdisplaymode-wireframe" => self.set_mode(DisplayMode::Wireframe),
            "shaded" => self.set_mode(DisplayMode::Shaded),
            "ghosted" => self.set_mode(DisplayMode::Ghosted),
            "xray" | "x-ray" => self.set_mode(DisplayMode::XRay),
            "maxviewport" | "maximize" => self.toggle_maximize(),
            _ => {
                // A bare engine command that needs arguments waits for them.
                if toks.len() == 1 {
                    if let Some(what) = self.engine.missing_input(text.trim()) {
                        self.pending = Some((text.trim().to_string(), what));
                        return;
                    }
                }
                self.run_engine(text);
                return;
            }
        }
        self.log(LogKind::Command, format!("Command: {text}"));
    }

    fn set_mode(&mut self, m: DisplayMode) {
        let vp = &mut self.viewports[self.active];
        vp.mode = m;
        vp.dirty = true;
    }

    /// Fit the selection in the active view (Rhino's ZS).
    fn zoom_selected(&mut self) {
        let doc = self.engine.doc();
        let pts: Vec<Point3> = self
            .engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .flat_map(|o| {
                let b = o.geometry.bounding_box();
                [b.min, b.max]
            })
            .collect();
        let Some(bb) = forma_geom::BoundingBox::from_points(&pts) else {
            self.log(LogKind::Normal, "nothing selected");
            return;
        };
        let o = self.origin();
        let vp = &mut self.viewports[self.active];
        let aspect = if vp.rect.is_positive() {
            vp.aspect()
        } else {
            1.5
        };
        vp.camera.fit(to_d(bb.min) - o, to_d(bb.max) - o, aspect);
        vp.dirty = true;
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
        lines.push("Curve tools / edit:".into());
        for k in ToolKind::CURVE_TOOLS.iter().chain(&ToolKind::EDIT) {
            lines.push(format!("  {}", k.tooltip()));
        }
        lines.push("Attributes: SetObjectColor <r,g,b|#hex|rosso…|ByLayer> · LayerColor <colour> [layer] · LayerVisible on|off [layer] · LayerLock on|off [layer] · Layer <name> · ChangeLayer <name>".into());
        lines.push("Other: Array nx ny nz dx,dy,dz · Delete · SelAll · SelNone · Undo · Redo · Save · Open · New [mm|cm|m] · ZE · ZEA".into());
        lines.push("Gumball: drag an arrow to move along an axis, a square to move in a plane, an arc to rotate; click a handle to type an exact value".into());
        lines.push("Keys: Enter/Space/right click = confirm or repeat · Esc = cancel · Del = delete · F3 = properties · F8 = Ortho (or hold Shift) · F9 = grid snap · F7 = grid · Ctrl+Z/Y/A/S/O/N".into());
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
            Want::Number | Want::Pick => match tok.parse::<f64>() {
                Ok(x) => t.feed_number(x),
                Err(_) => {
                    let msg = if want == Want::Pick {
                        "click on a curve in a view (or type a number to change the value)"
                    } else {
                        "a number is expected"
                    };
                    self.log(LogKind::Error, format!("{msg}: {tok}"));
                    return;
                }
            },
            Want::PickObject => match tok.trim_start_matches('#').parse::<u64>() {
                Ok(id) => t.feed_object(id),
                Err(_) => {
                    self.log(LogKind::Error, "click on an object in a view (or type #id)");
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
                let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
                let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
                for (m, k, a) in [
                    (ctrl_alt, Key::H, "Show"),
                    (ctrl_alt, Key::L, "Unlock"),
                    (ctrl_shift, Key::G, "Ungroup"),
                    (Modifiers::COMMAND, Key::H, "Hide"),
                    (Modifiers::COMMAND, Key::L, "Lock"),
                    (Modifiers::COMMAND, Key::G, "Group"),
                    (Modifiers::COMMAND, Key::J, "Join"),
                ] {
                    if i.consume_key(m, k) {
                        actions.push(a);
                    }
                }
                // Copy / cut / paste arrive as clipboard events, not keys.
                i.events.retain(|e| match e {
                    egui::Event::Copy => {
                        actions.push("CopyToClipboard");
                        false
                    }
                    egui::Event::Cut => {
                        actions.push("Cut");
                        false
                    }
                    egui::Event::Paste(_) => {
                        actions.push("Paste");
                        false
                    }
                    _ => true,
                });
                if i.consume_key(Modifiers::NONE, Key::F1) {
                    actions.push("help");
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
                    "help" => self.act(ctx, Act::Help),
                    "Show" | "Unlock" | "Paste" => self.run_engine(a),
                    "Hide" | "Lock" | "Group" | "Ungroup" | "Join" | "CopyToClipboard" | "Cut"
                        if self.tool.is_none() =>
                    {
                        self.act(ctx, Act::Clip(a));
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
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F3)) {
            self.side_panel = SidePanel::Properties;
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
                    self.submit_space(&text);
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
        let base = self.tool.as_ref().and_then(Tool::base);
        let osnap = if picking {
            self.snaps
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
            let on_curve = snap::pick_curve_point(doc, vp, pos, origin);
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
        let id = snap::pick(self.engine.doc(), &self.viewports[vi], pos, self.origin());
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
        let ids = snap::window_select(
            self.engine.doc(),
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
            let h = self.compute_hover(vi, pos, mods.shift);
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

        // Gumball handles (only without a running tool).
        let tool_wants_points = self.tool.as_ref().is_some_and(|t| !t.selecting);
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
                        self.snaps.find(vp, pos, origin, &self.snap, 14.0, None)
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
                return;
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
                            skeleton: snap::selection_skeleton(
                                self.engine.doc(),
                                &self.engine.ctx.selection,
                            ),
                            snapped: None,
                        });
                    }
                    return;
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
                    return;
                }
            }
        }

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
                    let id = snap::pick(self.engine.doc(), &self.viewports[vi], pos, origin);
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
                    && snap::pick_curve_point(self.engine.doc(), &self.viewports[vi], pos, origin)
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

    /// Engine lines for a gumball motion. Extrusion makes solids from the
    /// selected curves (ExtrudeCrv) and from open surfaces (ExtrudeSrf).
    fn gumball_lines(&self, m: gumball::Motion, center: Point3) -> Vec<String> {
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
        let surfaces: Vec<String> = sel
            .iter()
            .filter(|o| matches!(o.geometry, forma_doc::Geometry::Mesh(_)))
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
        out
    }

    /// A length in document units, for live readouts.
    fn fmt_len(&self, v: f64) -> String {
        let u = self.engine.doc().units.abbreviation();
        let decimals = match self.engine.doc().units {
            LengthUnit::Millimeters => 1,
            LengthUnit::Meters => 3,
            _ => 2,
        };
        format!("{v:.decimals$} {u}")
    }

    /// Where the gumball sits: centre of the selection (when enabled).
    fn gumball_center(&self) -> Option<Point3> {
        if !self.gumball_on || self.engine.ctx.selection.is_empty() {
            return None;
        }
        snap::selection_center(self.engine.doc(), &self.engine.ctx.selection)
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
                vp.mode,
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

    /// Screen rectangle of a viewport's title (click: menu, double-click: maximize).
    fn title_rect(vp: &Viewport) -> Rect {
        let w = 12.0 + vp.name().len() as f32 * 7.6 + 16.0;
        Rect::from_min_size(
            vp.rect.left_top() + egui::vec2(4.0, 3.0),
            egui::vec2(w, 20.0),
        )
    }

    fn draw_overlays(&self, painter: &egui::Painter, vi: usize) {
        let vp = &self.viewports[vi];
        let p = painter.with_clip_rect(vp.rect);
        let origin = self.origin();
        let ink = Color32::from_rgb(20, 20, 24);
        let seg = |a: Point3, b: Point3, s: Stroke| {
            if let (Some(sa), Some(sb)) = (vp.to_screen(a, origin), vp.to_screen(b, origin)) {
                p.line_segment([sa, sb], s);
            }
        };
        // Point objects: small squares, like Rhino's point display.
        let doc = self.engine.doc();
        for o in doc.objects() {
            if let forma_doc::Geometry::Point(q) = &o.geometry {
                if !doc.is_visible(o) {
                    continue;
                }
                if let Some(sp) = vp.to_screen(*q, origin) {
                    let selected = self.engine.ctx.selection.contains(&o.id);
                    let [r, g, b] = doc.display_color(o);
                    let c = if selected {
                        Color32::from_rgb(255, 200, 0)
                    } else {
                        Color32::from_rgb(r, g, b)
                    };
                    let r = Rect::from_center_size(sp, egui::vec2(6.0, 6.0));
                    p.rect_filled(r, 0.0, c);
                    p.rect_stroke(r, 0.0, Stroke::new(1.0, ink), StrokeKind::Outside);
                }
            }
        }
        if let (Some(t), Some(h)) = (self.tool.as_ref(), self.hover.as_ref()) {
            let st = Stroke::new(1.3, ink);
            for [a, b] in t.preview(h.point) {
                seg(a, b, st);
            }
            // SmartTrack lines and points.
            let track = Stroke::new(1.0, Color32::WHITE);
            for [a, b] in &h.tracks {
                if let (Some(sa), Some(sb)) = (vp.to_screen(*a, origin), vp.to_screen(*b, origin)) {
                    p.extend(egui::Shape::dashed_line(&[sa, sb], track, 4.0, 3.0));
                }
            }
            for q in &self.track_points {
                if let Some(sq) = vp.to_screen(*q, origin) {
                    p.line_segment(
                        [sq - egui::vec2(4.0, 0.0), sq + egui::vec2(4.0, 0.0)],
                        track,
                    );
                    p.line_segment(
                        [sq - egui::vec2(0.0, 4.0), sq + egui::vec2(0.0, 4.0)],
                        track,
                    );
                }
            }
            // Live measurement next to the cursor (Rhino's dynamic readout).
            if h.viewport == vi {
                let text: Vec<String> = t
                    .measure(h.point)
                    .into_iter()
                    .map(|m| match m {
                        tools::Measure::Len("", v) => self.fmt_len(v),
                        tools::Measure::Len(l, v) => format!("{l} {}", self.fmt_len(v)),
                        tools::Measure::Angle(a) => format!("∠ {a:.1}°"),
                        tools::Measure::Factor(f) => format!("× {f:.3}"),
                    })
                    .collect();
                if let (false, Some(s)) = (text.is_empty(), vp.to_screen(h.point, origin)) {
                    let font = egui::FontId::proportional(12.5);
                    let galley = p.layout_no_wrap(text.join("   "), font, Color32::BLACK);
                    let at = s + egui::vec2(14.0, 10.0);
                    let bg = Rect::from_min_size(at, galley.size() + egui::vec2(10.0, 5.0));
                    p.rect_filled(bg, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
                    p.rect_stroke(
                        bg,
                        3.0,
                        Stroke::new(1.0, Color32::from_gray(120)),
                        StrokeKind::Inside,
                    );
                    p.galley(at + egui::vec2(5.0, 2.5), galley, Color32::BLACK);
                }
            }
            if let Some(s) = vp.to_screen(h.point, origin) {
                if !t.selecting {
                    p.circle_filled(s, 3.0, ink);
                }
                if let Some(k) = h.snap {
                    let r = Rect::from_center_size(s, egui::vec2(10.0, 10.0));
                    p.rect_stroke(r, 0.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Middle);
                    if h.viewport == vi {
                        // Rhino-like osnap tag: a small label box next to the cursor.
                        let font = egui::FontId::proportional(12.0);
                        let galley = p.layout_no_wrap(k.label().to_string(), font, Color32::BLACK);
                        let at = s + egui::vec2(10.0, -22.0);
                        let bg = Rect::from_min_size(at, galley.size() + egui::vec2(8.0, 4.0));
                        p.rect_filled(bg, 2.0, Color32::from_rgb(255, 255, 225));
                        p.rect_stroke(
                            bg,
                            2.0,
                            Stroke::new(1.0, Color32::from_gray(90)),
                            StrokeKind::Inside,
                        );
                        p.galley(at + egui::vec2(4.0, 2.0), galley, Color32::BLACK);
                    }
                }
            }
        }
        if let Some(d) = &self.gumball_drag {
            if let Some((q, k)) = d.snapped {
                if let Some(sq) = vp.to_screen(q, origin) {
                    let r = Rect::from_center_size(sq, egui::vec2(10.0, 10.0));
                    p.rect_stroke(r, 0.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Middle);
                    p.text(
                        sq + egui::vec2(9.0, -9.0),
                        egui::Align2::LEFT_BOTTOM,
                        k.label(),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }
            }
            if let Some(m) = d.motion {
                let x: Xform = m.xform(d.center);
                let st = Stroke::new(1.2, ink);
                for [a, b] in &d.skeleton {
                    seg(x.point(*a), x.point(*b), st);
                }
                if matches!(m, gumball::Motion::Extrude(..)) {
                    // Side edges of the extrusion.
                    let step = (d.skeleton.len() / 24).max(1);
                    for [a, _] in d.skeleton.iter().step_by(step) {
                        seg(*a, x.point(*a), st);
                    }
                }
                if d.viewport == vi {
                    if let Some(s) = vp.to_screen(x.point(d.center), origin) {
                        let label = match m {
                            gumball::Motion::Translate(v) => self.fmt_len(v.length()),
                            gumball::Motion::Rotate(a, _) => format!("{a:.1}°"),
                            gumball::Motion::Extrude(h, _) => {
                                format!("Extrude {}", self.fmt_len(h))
                            }
                        };
                        let font = egui::FontId::proportional(13.0);
                        let galley = p.layout_no_wrap(label, font, Color32::BLACK);
                        let at = s + egui::vec2(12.0, -26.0);
                        let bg = Rect::from_min_size(at, galley.size() + egui::vec2(8.0, 4.0));
                        p.rect_filled(bg, 2.0, Color32::from_rgb(255, 255, 225));
                        p.galley(at + egui::vec2(4.0, 2.0), galley, Color32::BLACK);
                    }
                }
            }
        } else if self.tool.is_none() {
            if let Some(c) = self.gumball_center() {
                if let Some(l) = gumball::layout(vp, c, origin) {
                    let hot = self
                        .gumball_hot
                        .filter(|(v, _)| *v == vi)
                        .map(|(_, h)| h)
                        .or(self.gumball_typed.map(|g| g.0));
                    gumball::draw(&p, vp, &l, origin, hot);
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
        // World axes icon, lower left (Rhino shows one in every view).
        let corner = vp.rect.left_bottom() + egui::vec2(28.0, -28.0);
        if let Some(c0) = vp.to_screen(to_p(vp.camera.target + origin), origin) {
            for (axis, color, label) in [
                (Vec3::X, Color32::from_rgb(200, 40, 40), "x"),
                (Vec3::Y, Color32::from_rgb(30, 140, 40), "y"),
                (Vec3::Z, Color32::from_rgb(40, 70, 200), "z"),
            ] {
                let t = to_p(vp.camera.target + origin);
                let Some(c1) = vp.to_screen(t + axis * (vp.camera.distance * 0.05), origin) else {
                    continue;
                };
                let d = c1 - c0;
                if d.length() < 1e-3 {
                    continue;
                }
                let len = 18.0 * (d.length() / (vp.rect.height() * 0.05 + 1e-3)).min(1.0);
                if len < 3.0 {
                    continue;
                }
                let dir = d.normalized();
                let tip = corner + dir * len;
                p.line_segment([corner, tip], Stroke::new(1.8, color));
                p.text(
                    tip + dir * 7.0,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    color,
                );
            }
        }
        // Title with the drop-down arrow, Rhino style.
        let active = vi == self.active;
        let tr = Self::title_rect(vp);
        if active {
            p.rect_filled(tr, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 150));
        }
        let title = format!("{} ▾", vp.name());
        p.text(
            tr.left_center() + egui::vec2(6.0, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            if active {
                egui::FontId::proportional(13.5)
            } else {
                egui::FontId::proportional(13.0)
            },
            if active {
                Color32::BLACK
            } else {
                Color32::from_gray(45)
            },
        );
        if vp.mode != DisplayMode::Wireframe || active {
            p.text(
                vp.rect.right_top() + egui::vec2(-8.0, 6.0),
                egui::Align2::RIGHT_TOP,
                vp.mode.name(),
                egui::FontId::proportional(11.5),
                Color32::from_gray(55),
            );
        }
        p.rect_stroke(
            vp.rect,
            0.0,
            Stroke::new(
                if active { 2.0 } else { 1.0 },
                if active {
                    theme::ACCENT
                } else {
                    Color32::from_gray(110)
                },
            ),
            StrokeKind::Inside,
        );
    }

    fn toggle_maximize(&mut self) {
        self.maximized = if self.maximized.is_some() {
            None
        } else {
            Some(self.active)
        };
        self.dirty_all();
    }

    fn ui_viewports(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let outer = ui.available_rect_before_wrap();
        let tabs_h = 24.0;
        let full = Rect::from_min_max(outer.min, egui::pos2(outer.max.x, outer.max.y - tabs_h));
        let gap = 3.0;
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
            let title = Self::title_rect(&self.viewports[i]);
            let on_title = resp
                .interact_pointer_pos()
                .is_some_and(|p| title.contains(p));
            if on_title && resp.double_clicked_by(PointerButton::Primary) {
                self.active = i;
                self.title_menu = None;
                self.toggle_maximize();
                continue;
            }
            if on_title
                && (resp.clicked_by(PointerButton::Primary)
                    || resp.clicked_by(PointerButton::Secondary))
            {
                self.active = i;
                self.title_menu = Some((i, title.left_bottom()));
                continue;
            }
            self.viewport_input(ui, i, &resp);
        }
        if !any_hover && self.drag.is_none() {
            self.hover = None;
        }
        self.render_viewports(frame, ui.ctx().pixels_per_point());
        let painter = ui.painter();
        painter.rect_filled(outer, 0.0, Color32::from_gray(150));
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
        self.ui_title_menu(ui);
        // Viewport tabs under the views.
        let tabs = Rect::from_min_max(egui::pos2(outer.min.x, full.max.y), outer.max);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(tabs.shrink2(egui::vec2(4.0, 2.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.painter().rect_filled(tabs, 0.0, theme::STRIP);
        for i in [1usize, 0, 2, 3] {
            let name = self.viewports[i].name();
            let selected = self.active == i;
            if child.selectable_label(selected, name).clicked() {
                self.active = i;
                if self.maximized.is_some() {
                    self.maximized = Some(i);
                }
                self.dirty_all();
            }
        }
        if self.viewports.iter().any(|v| v.dirty) {
            ui.ctx().request_repaint();
        }
    }

    /// Drop-down menu of a viewport title (Rhino's "Top ▾").
    fn ui_title_menu(&mut self, ui: &mut egui::Ui) {
        let Some((vi, at)) = self.title_menu else {
            return;
        };
        let mut close = false;
        let mut act: Option<Act> = None;
        let area = egui::Area::new(egui::Id::new("viewport-title-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + egui::vec2(0.0, 2.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(170.0);
                    ui.label(egui::RichText::new("Set View").small().weak());
                    for (label, cmd) in [
                        ("Top", "top"),
                        ("Front", "front"),
                        ("Right", "right"),
                        ("Perspective", "perspective"),
                    ] {
                        if ui.selectable_label(false, label).clicked() {
                            act = Some(Act::Submit(cmd));
                            close = true;
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Display Mode").small().weak());
                    let current = self.viewports[vi].mode;
                    for m in DisplayMode::ALL {
                        if ui.selectable_label(current == m, m.name()).clicked() {
                            act = Some(Act::Mode(m));
                            close = true;
                        }
                    }
                    ui.separator();
                    let label = if self.maximized.is_some() {
                        "Restore Viewport Layout"
                    } else {
                        "Maximize"
                    };
                    if ui.selectable_label(false, label).clicked() {
                        act = Some(Act::Maximize);
                        close = true;
                    }
                    if ui.selectable_label(false, "Zoom Extents").clicked() {
                        act = Some(Act::Submit("ze"));
                        close = true;
                    }
                    if ui.selectable_label(false, "Zoom Selected").clicked() {
                        act = Some(Act::Submit("zs"));
                        close = true;
                    }
                });
            });
        let title = Self::title_rect(&self.viewports[vi]);
        let press = ui.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let clicked_elsewhere =
            press.is_some_and(|p| !area.response.rect.contains(p) && !title.contains(p));
        if close || clicked_elsewhere || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.title_menu = None;
        }
        if let Some(a) = act {
            self.active = vi;
            self.act(ui.ctx(), a);
        }
    }

    fn icon_button(&mut self, ui: &mut egui::Ui, icon: Icon, tip: &str, active: bool) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
        if resp.hovered() || active {
            let (fill, stroke) = if active {
                (theme::ACCENT_BG, theme::ACCENT)
            } else {
                (
                    Color32::from_rgb(229, 241, 251),
                    Color32::from_rgb(160, 200, 236),
                )
            };
            ui.painter().rect_filled(rect, 3.0, fill);
            ui.painter()
                .rect_stroke(rect, 3.0, Stroke::new(1.0, stroke), StrokeKind::Inside);
        }
        icons::paint(ui.painter(), rect.shrink(3.0), icon, theme::TEXT);
        resp.on_hover_text(tip).clicked()
    }

    /// Left sidebar: Rhino's main palette, two columns of tools.
    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        use ToolKind as K;
        let groups: [&[ToolKind]; 7] = [
            &[
                K::Point,
                K::Line,
                K::Polyline,
                K::Curve,
                K::Circle,
                K::Arc,
                K::Ellipse,
                K::Rectangle,
                K::Polygon,
                K::InterpCrv,
            ],
            &[
                K::OnSel("PlanarSrf"),
                K::Extrude,
                K::OnSel("Loft"),
                K::Revolve,
                K::Sweep1,
                K::OnSel("Cap"),
            ],
            &[K::Box, K::Sphere, K::Cylinder, K::ExtrudeSrf],
            &[
                K::Move,
                K::Copy,
                K::Rotate,
                K::Scale,
                K::Mirror,
                K::Orient,
                K::ArrayLinear,
                K::ArrayPolar,
            ],
            &[
                K::Join,
                K::Explode,
                K::Trim,
                K::Split,
                K::Extend,
                K::Offset,
                K::Fillet,
                K::Chamfer,
            ],
            &[
                K::OnSel("Group"),
                K::OnSel("Hide"),
                K::OnSel("Lock"),
                K::MatchProperties,
            ],
            &[K::Distance, K::OnSel("Area")],
        ];
        let current = self.tool.as_ref().map(|t| t.kind);
        let mut start = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);
            for (g, kinds) in groups.iter().enumerate() {
                egui::Grid::new(("sidebar", g))
                    .spacing([1.0, 1.0])
                    .show(ui, |ui| {
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
                ui.add_space(3.0);
                ui.separator();
            }
        });
        if let Some(k) = start {
            self.start_tool(k);
        }
    }

    fn act(&mut self, ctx: &egui::Context, a: Act) {
        match a {
            Act::Tool(k) => self.start_tool(k),
            Act::Cmd(c) => self.run_engine(c),
            Act::Submit(c) => self.submit(c),
            Act::Prefill(text, usage) => {
                self.tool = None;
                self.command = text.to_string();
                self.focus_command = true;
                self.log(LogKind::Normal, usage);
            }
            Act::Open => self.open_dialog(),
            Act::Save => self.save(false),
            Act::SaveAs => self.save(true),
            Act::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Act::Help => {
                self.side_panel = SidePanel::Help;
                self.show_help();
            }
            Act::Maximize => self.toggle_maximize(),
            Act::Panel(p) => self.side_panel = p,
            Act::Mode(m) => self.set_mode(m),
            Act::Clip(c) => match c {
                "Paste" => self.run_engine("Paste"),
                other => {
                    if self.has_selection() {
                        self.run_engine(other);
                    } else {
                        self.start_tool(ToolKind::OnSel(other));
                    }
                }
            },
        }
    }

    fn dirty_all(&mut self) {
        for v in &mut self.viewports {
            v.dirty = true;
        }
    }

    /// Buttons of a toolbar tab: (icon, tooltip, action); `None` = separator.
    fn tab_items(&self) -> Vec<Option<(Icon, &'static str, Act)>> {
        use ToolKind as K;
        let tool = |k: ToolKind| Some((Icon::Tool(k), k.tooltip(), Act::Tool(k)));
        let named = |n: &'static str, tip: &'static str, a: Act| Some((Icon::Named(n), tip, a));
        let tools = |ks: &[ToolKind]| ks.iter().map(|k| tool(*k)).collect::<Vec<_>>();
        let mut v = Vec::new();
        match self.tab {
            0 => {
                v.push(named("New", "New (Ctrl+N)", Act::Cmd("New")));
                v.push(named("Open", "Open (Ctrl+O)", Act::Open));
                v.push(named("Save", "Save (Ctrl+S)", Act::Save));
                v.push(None);
                v.push(named("Cut", "Cut (Ctrl+X)", Act::Clip("Cut")));
                v.push(named(
                    "CopyToClipboard",
                    "Copy to clipboard (Ctrl+C)",
                    Act::Clip("CopyToClipboard"),
                ));
                v.push(named("Paste", "Paste (Ctrl+V)", Act::Clip("Paste")));
                v.push(None);
                v.push(named("Undo", "Undo (Ctrl+Z)", Act::Cmd("Undo")));
                v.push(named("Redo", "Redo (Ctrl+Y)", Act::Cmd("Redo")));
                v.push(None);
                v.push(named("SelAll", "Select all (Ctrl+A)", Act::Cmd("SelAll")));
                v.push(tool(K::OnSel("Delete")));
                v.push(None);
                v.push(tool(K::OnSel("Hide")));
                v.push(named(
                    "Show",
                    "Show hidden objects (Ctrl+Alt+H)",
                    Act::Cmd("Show"),
                ));
                v.push(tool(K::OnSel("Lock")));
                v.push(named(
                    "Unlock",
                    "Unlock all (Ctrl+Alt+L)",
                    Act::Cmd("Unlock"),
                ));
                v.push(None);
                v.push(named(
                    "ZoomExtents",
                    "Zoom extents, all views",
                    Act::Submit("zea"),
                ));
                v.push(named("ZoomSelected", "Zoom selected", Act::Submit("zs")));
                v.push(None);
                v.push(named(
                    "Layer",
                    "Layers panel",
                    Act::Panel(SidePanel::Layers),
                ));
                v.push(named(
                    "What",
                    "Properties panel (F3)",
                    Act::Panel(SidePanel::Properties),
                ));
                v.push(named("Help", "Help", Act::Help));
            }
            1 => {
                for (n, c) in [
                    ("Top", "top"),
                    ("Front", "front"),
                    ("Right", "right"),
                    ("Perspective", "perspective"),
                ] {
                    v.push(named(n, n, Act::Submit(c)));
                }
                v.push(None);
                v.push(named("ZoomExtents", "Zoom extents (ZE)", Act::Submit("ze")));
                v.push(named(
                    "ZoomSelected",
                    "Zoom selected (ZS)",
                    Act::Submit("zs"),
                ));
                v.push(named(
                    "Isolate",
                    "Maximize / restore the active view",
                    Act::Maximize,
                ));
            }
            2 => {
                for (m, n) in [
                    (DisplayMode::Wireframe, "Wireframe"),
                    (DisplayMode::Shaded, "Shaded"),
                    (DisplayMode::Ghosted, "Ghosted"),
                    (DisplayMode::XRay, "XRay"),
                ] {
                    v.push(named(n, m.name(), Act::Mode(m)));
                }
            }
            3 => {
                for (n, tip) in [
                    ("SelAll", "Select all (Ctrl+A)"),
                    ("SelNone", "Select none (Esc)"),
                    ("Invert", "Invert selection"),
                    ("SelLast", "Select last created objects"),
                    ("SelCrv", "Select all curves"),
                    ("SelMesh", "Select all surfaces / meshes"),
                    ("SelPt", "Select all points"),
                ] {
                    v.push(named(n, tip, Act::Cmd(n)));
                }
                v.push(tool(K::OnSel("SelGroup")));
            }
            4 => {
                v.push(tool(K::OnSel("Hide")));
                v.push(named("Show", "Show all hidden objects", Act::Cmd("Show")));
                v.push(tool(K::OnSel("Isolate")));
                v.push(named(
                    "Unisolate",
                    "Unisolate (show all)",
                    Act::Cmd("Unisolate"),
                ));
                v.push(None);
                v.push(tool(K::OnSel("Lock")));
                v.push(named("Unlock", "Unlock all", Act::Cmd("Unlock")));
                v.push(None);
                v.push(tool(K::OnSel("Group")));
                v.push(tool(K::OnSel("Ungroup")));
            }
            5 => {
                v.extend(tools(&ToolKind::TRANSFORMS));
                v.push(named("Array", "Rectangular array", ARRAY_PREFILL));
            }
            6 => {
                v.extend(tools(&ToolKind::CURVES));
                v.push(None);
                v.extend(tools(&ToolKind::CURVE_TOOLS));
            }
            7 => v.extend(tools(&ToolKind::SURFACES)),
            8 => v.extend(tools(&ToolKind::SOLIDS)),
            _ => {
                v.extend(tools(&ToolKind::ANALYZE));
                v.push(None);
                v.push(tool(K::MatchProperties));
            }
        }
        v
    }

    /// Tab strip and its command row (Rhino 8 style).
    fn ui_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, name) in TABS.iter().enumerate() {
                let selected = self.tab == i;
                let text = egui::RichText::new(*name).size(12.5);
                let b = egui::Button::new(if selected { text.strong() } else { text })
                    .fill(if selected {
                        Color32::WHITE
                    } else {
                        theme::STRIP
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if selected {
                            theme::BORDER
                        } else {
                            theme::STRIP
                        },
                    ))
                    .corner_radius(egui::CornerRadius {
                        nw: 3,
                        ne: 3,
                        sw: 0,
                        se: 0,
                    });
                if ui.add(b).clicked() {
                    self.tab = i;
                }
            }
        });
        let mut act: Option<Act> = None;
        let current = self.tool.as_ref().map(|t| t.kind);
        let items = self.tab_items();
        let frame = egui::Frame::NONE
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .inner_margin(egui::Margin::symmetric(4, 2));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                for it in items {
                    match it {
                        None => {
                            ui.add_space(3.0);
                            ui.separator();
                            ui.add_space(3.0);
                        }
                        Some((icon, tip, a)) => {
                            let active =
                                matches!((icon, current), (Icon::Tool(k), Some(c)) if k == c);
                            if self.icon_button(ui, icon, tip, active) {
                                act = Some(a);
                            }
                        }
                    }
                }
                if self.tab == 8 {
                    ui.separator();
                    for b in [
                        "BooleanUnion",
                        "BooleanDifference",
                        "BooleanIntersection",
                        "FilletEdge",
                    ] {
                        ui.add_enabled(false, egui::Button::new(b))
                            .on_disabled_hover_text(
                                "Needs the solid kernel (OpenCascade), planned for a next version",
                            );
                    }
                }
                if self.tab == 5 {
                    ui.separator();
                    ui.menu_button("Align ▾", |ui| {
                        for (label, opt) in [
                            ("Left", "left"),
                            ("Right", "right"),
                            ("Top", "top"),
                            ("Bottom", "bottom"),
                            ("Horizontal centers", "hcenter"),
                            ("Vertical centers", "vcenter"),
                            ("Centers", "center"),
                        ] {
                            if ui.button(label).clicked() {
                                act = Some(Act::Submit(align_cmd(opt)));
                                ui.close();
                            }
                        }
                    });
                }
            });
        });
        if let Some(a) = act {
            self.act(ui.ctx(), a);
        }
    }

    fn ui_menu(&mut self, ui: &mut egui::Ui) {
        let mut act: Option<Act> = None;
        let item = |ui: &mut egui::Ui, act: &mut Option<Act>, label: &str, key: &str, a: Act| {
            let b = egui::Button::new(label).shortcut_text(key);
            if ui.add(b).clicked() {
                *act = Some(a);
                ui.close();
            }
        };
        let tool = |ui: &mut egui::Ui, act: &mut Option<Act>, label: &str, k: ToolKind| {
            if ui.button(label).on_hover_text(k.tooltip()).clicked() {
                *act = Some(Act::Tool(k));
                ui.close();
            }
        };
        let kernel = |ui: &mut egui::Ui, label: &str| {
            ui.add_enabled(false, egui::Button::new(label))
                .on_disabled_hover_text(
                    "Needs the solid kernel (OpenCascade), planned for a next version",
                );
        };
        use ToolKind as K;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.menu_button("New", |ui| {
                    item(ui, &mut act, "Centimetres", "Ctrl+N", Act::Cmd("New cm"));
                    item(ui, &mut act, "Millimetres", "", Act::Cmd("New mm"));
                    item(ui, &mut act, "Metres", "", Act::Cmd("New m"));
                });
                item(ui, &mut act, "Open…", "Ctrl+O", Act::Open);
                item(
                    ui,
                    &mut act,
                    "Import…",
                    "",
                    Act::Prefill(
                        "Import ",
                        "Import <file.3dm> — merge a Rhino file into this model",
                    ),
                );
                ui.separator();
                item(ui, &mut act, "Save", "Ctrl+S", Act::Save);
                item(ui, &mut act, "Save As…", "Ctrl+Shift+S", Act::SaveAs);
                item(
                    ui,
                    &mut act,
                    "Export Selected…",
                    "",
                    Act::Prefill(
                        "Export ",
                        "Export <file.3dm> — save only the selected objects",
                    ),
                );
                ui.separator();
                item(ui, &mut act, "Exit", "", Act::Exit);
            });
            ui.menu_button("Edit", |ui| {
                item(ui, &mut act, "Undo", "Ctrl+Z", Act::Cmd("Undo"));
                item(ui, &mut act, "Redo", "Ctrl+Y", Act::Cmd("Redo"));
                ui.separator();
                item(ui, &mut act, "Cut", "Ctrl+X", Act::Clip("Cut"));
                item(ui, &mut act, "Copy", "Ctrl+C", Act::Clip("CopyToClipboard"));
                item(ui, &mut act, "Paste", "Ctrl+V", Act::Clip("Paste"));
                item(ui, &mut act, "Delete", "Del", Act::Tool(K::OnSel("Delete")));
                ui.separator();
                ui.menu_button("Select Objects", |ui| {
                    item(ui, &mut act, "All Objects", "Ctrl+A", Act::Cmd("SelAll"));
                    item(ui, &mut act, "None", "Esc", Act::Cmd("SelNone"));
                    item(ui, &mut act, "Invert", "", Act::Cmd("Invert"));
                    item(
                        ui,
                        &mut act,
                        "Last Created Objects",
                        "",
                        Act::Cmd("SelLast"),
                    );
                    ui.separator();
                    item(ui, &mut act, "Curves", "", Act::Cmd("SelCrv"));
                    item(ui, &mut act, "Surfaces / Meshes", "", Act::Cmd("SelMesh"));
                    item(ui, &mut act, "Points", "", Act::Cmd("SelPt"));
                });
                ui.separator();
                tool(ui, &mut act, "Join", K::Join);
                tool(ui, &mut act, "Explode", K::Explode);
                tool(ui, &mut act, "Trim", K::Trim);
                tool(ui, &mut act, "Split", K::Split);
                ui.separator();
                ui.menu_button("Groups", |ui| {
                    item(
                        ui,
                        &mut act,
                        "Group",
                        "Ctrl+G",
                        Act::Tool(K::OnSel("Group")),
                    );
                    item(
                        ui,
                        &mut act,
                        "Ungroup",
                        "Ctrl+Shift+G",
                        Act::Tool(K::OnSel("Ungroup")),
                    );
                    item(
                        ui,
                        &mut act,
                        "Select Group",
                        "",
                        Act::Tool(K::OnSel("SelGroup")),
                    );
                });
                ui.menu_button("Visibility", |ui| {
                    item(ui, &mut act, "Hide", "Ctrl+H", Act::Tool(K::OnSel("Hide")));
                    item(ui, &mut act, "Show", "Ctrl+Alt+H", Act::Cmd("Show"));
                    item(ui, &mut act, "Isolate", "", Act::Tool(K::OnSel("Isolate")));
                    item(ui, &mut act, "Lock", "Ctrl+L", Act::Tool(K::OnSel("Lock")));
                    item(ui, &mut act, "Unlock", "Ctrl+Alt+L", Act::Cmd("Unlock"));
                });
                ui.menu_button("Layers", |ui| {
                    item(
                        ui,
                        &mut act,
                        "Layer Panel",
                        "",
                        Act::Panel(SidePanel::Layers),
                    );
                    item(
                        ui,
                        &mut act,
                        "Change Object Layer…",
                        "",
                        Act::Panel(SidePanel::Properties),
                    );
                });
                tool(ui, &mut act, "Match Properties", K::MatchProperties);
                item(
                    ui,
                    &mut act,
                    "Object Properties",
                    "F3",
                    Act::Panel(SidePanel::Properties),
                );
            });
            ui.menu_button("View", |ui| {
                ui.menu_button("Zoom", |ui| {
                    item(ui, &mut act, "Zoom Extents", "ZE", Act::Submit("ze"));
                    item(ui, &mut act, "Zoom Extents All", "ZEA", Act::Submit("zea"));
                    item(ui, &mut act, "Zoom Selected", "ZS", Act::Submit("zs"));
                });
                ui.menu_button("Set View", |ui| {
                    item(ui, &mut act, "Top", "", Act::Submit("top"));
                    item(ui, &mut act, "Front", "", Act::Submit("front"));
                    item(ui, &mut act, "Right", "", Act::Submit("right"));
                    item(ui, &mut act, "Perspective", "", Act::Submit("perspective"));
                });
                ui.menu_button("Display Mode", |ui| {
                    for m in DisplayMode::ALL {
                        item(ui, &mut act, m.name(), "", Act::Mode(m));
                    }
                });
                let label = if self.maximized.is_some() {
                    "Restore Viewport Layout"
                } else {
                    "Maximize Active Viewport"
                };
                item(ui, &mut act, label, "", Act::Maximize);
                ui.separator();
                if let Some(r) = &mut self.renderer {
                    if ui.checkbox(&mut r.show_grid, "Grid  (F7)").changed() {
                        for v in &mut self.viewports {
                            v.dirty = true;
                        }
                    }
                }
                ui.checkbox(&mut self.gumball_on, "Gumball");
                ui.checkbox(&mut self.show_osnap, "Osnap Toolbar");
            });
            ui.menu_button("Curve", |ui| {
                tool(ui, &mut act, "Point Object", K::Point);
                ui.menu_button("Line", |ui| {
                    tool(ui, &mut act, "Single Line", K::Line);
                });
                tool(ui, &mut act, "Polyline", K::Polyline);
                ui.menu_button("Free-Form", |ui| {
                    tool(ui, &mut act, "Control Points", K::Curve);
                    tool(ui, &mut act, "Interpolate Points", K::InterpCrv);
                });
                tool(ui, &mut act, "Rectangle", K::Rectangle);
                tool(ui, &mut act, "Polygon", K::Polygon);
                tool(ui, &mut act, "Circle", K::Circle);
                tool(ui, &mut act, "Arc", K::Arc);
                tool(ui, &mut act, "Ellipse", K::Ellipse);
                ui.separator();
                tool(ui, &mut act, "Fillet Curves", K::Fillet);
                tool(ui, &mut act, "Chamfer Curves", K::Chamfer);
                tool(ui, &mut act, "Fillet Corners", K::FilletCorners);
                tool(ui, &mut act, "Offset Curve", K::Offset);
                tool(ui, &mut act, "Extend Curve", K::Extend);
                ui.separator();
                ui.menu_button("Curve Edit Tools", |ui| {
                    tool(ui, &mut act, "Join", K::Join);
                    tool(ui, &mut act, "Explode", K::Explode);
                    tool(ui, &mut act, "Trim", K::Trim);
                    tool(ui, &mut act, "Split", K::Split);
                    tool(ui, &mut act, "Flip Direction", K::OnSel("Flip"));
                });
                ui.menu_button("Curve From Objects", |ui| {
                    tool(ui, &mut act, "Intersection", K::OnSel("Intersect"));
                    tool(
                        ui,
                        &mut act,
                        "Project To CPlane",
                        K::OnSel("ProjectToCPlane"),
                    );
                });
            });
            ui.menu_button("Surface", |ui| {
                tool(ui, &mut act, "Planar Curves", K::OnSel("PlanarSrf"));
                ui.menu_button("Extrude Curve", |ui| {
                    tool(ui, &mut act, "Straight", K::Extrude);
                });
                tool(ui, &mut act, "Loft", K::OnSel("Loft"));
                tool(ui, &mut act, "Revolve", K::Revolve);
                tool(ui, &mut act, "Sweep 1 Rail", K::Sweep1);
                ui.separator();
                kernel(ui, "Offset Surface");
                ui.label(
                    egui::RichText::new("Surfaces are meshes until the NURBS kernel lands")
                        .small()
                        .weak(),
                );
            });
            ui.menu_button("Solid", |ui| {
                tool(ui, &mut act, "Box", K::Box);
                tool(ui, &mut act, "Sphere", K::Sphere);
                tool(ui, &mut act, "Cylinder", K::Cylinder);
                ui.separator();
                ui.menu_button("Extrude Planar Curve", |ui| {
                    tool(ui, &mut act, "Straight", K::Extrude);
                });
                tool(ui, &mut act, "Extrude Surface", K::ExtrudeSrf);
                tool(ui, &mut act, "Cap Planar Holes", K::OnSel("Cap"));
                ui.separator();
                kernel(ui, "Union");
                kernel(ui, "Difference");
                kernel(ui, "Intersection");
                kernel(ui, "Fillet Edge");
            });
            ui.menu_button("Transform", |ui| {
                tool(ui, &mut act, "Move", K::Move);
                tool(ui, &mut act, "Copy", K::Copy);
                tool(ui, &mut act, "Rotate", K::Rotate);
                ui.menu_button("Scale", |ui| {
                    tool(ui, &mut act, "Scale 3-D", K::Scale);
                    tool(ui, &mut act, "Scale 2-D", K::Scale2D);
                    tool(ui, &mut act, "Scale 1-D", K::Scale1D);
                });
                tool(ui, &mut act, "Mirror", K::Mirror);
                tool(ui, &mut act, "Orient: 2 Points", K::Orient);
                ui.separator();
                ui.menu_button("Array", |ui| {
                    item(ui, &mut act, "Rectangular", "", ARRAY_PREFILL);
                    tool(ui, &mut act, "Linear", K::ArrayLinear);
                    tool(ui, &mut act, "Polar", K::ArrayPolar);
                });
                ui.menu_button("Align", |ui| {
                    for (label, opt) in [
                        ("Left", "left"),
                        ("Right", "right"),
                        ("Top", "top"),
                        ("Bottom", "bottom"),
                        ("Horizontal Centers", "hcenter"),
                        ("Vertical Centers", "vcenter"),
                        ("Centers", "center"),
                    ] {
                        item(ui, &mut act, label, "", Act::Submit(align_cmd(opt)));
                    }
                });
                ui.separator();
                tool(
                    ui,
                    &mut act,
                    "Project To CPlane",
                    K::OnSel("ProjectToCPlane"),
                );
            });
            ui.menu_button("Tools", |ui| {
                ui.menu_button("Object Snap", |ui| {
                    for k in SnapKind::ALL {
                        ui.checkbox(self.snap.flag(k), k.label());
                    }
                    ui.checkbox(&mut self.snap.project, "Project");
                    ui.checkbox(&mut self.snap.disabled, "Disable");
                });
                ui.checkbox(&mut self.snap.grid, "Grid Snap (F9)");
                ui.checkbox(&mut self.snap.ortho, "Ortho (F8)");
                ui.checkbox(&mut self.snap.planar, "Planar");
            });
            ui.menu_button("Analyze", |ui| {
                tool(ui, &mut act, "Distance", K::Distance);
                tool(ui, &mut act, "Length", K::OnSel("Length"));
                ui.menu_button("Mass Properties", |ui| {
                    tool(ui, &mut act, "Area", K::OnSel("Area"));
                    tool(ui, &mut act, "Volume", K::OnSel("Volume"));
                });
                tool(ui, &mut act, "Bounding Box", K::OnSel("BoundingBox"));
                tool(ui, &mut act, "Object Details (What)", K::OnSel("What"));
            });
            ui.menu_button("Help", |ui| {
                item(ui, &mut act, "Commands, keys and mouse", "F1", Act::Help);
            });
        });
        if let Some(a) = act {
            self.act(ui.ctx(), a);
        }
    }

    /// Right panel with Properties, Layers and Help tabs.
    fn ui_side(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (p, name) in [
                (SidePanel::Properties, "Properties"),
                (SidePanel::Layers, "Layers"),
                (SidePanel::Help, "Help"),
            ] {
                let selected = self.side_panel == p;
                let text = egui::RichText::new(name).size(12.5);
                let b = egui::Button::new(if selected { text.strong() } else { text })
                    .fill(if selected {
                        Color32::WHITE
                    } else {
                        theme::PANEL
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if selected {
                            theme::BORDER
                        } else {
                            theme::PANEL
                        },
                    ));
                if ui.add(b).clicked() {
                    self.side_panel = p;
                }
            }
        });
        ui.separator();
        match self.side_panel {
            SidePanel::Properties => self.ui_properties(ui),
            SidePanel::Layers => self.ui_layers(ui),
            SidePanel::Help => self.ui_help(ui),
        }
    }

    fn ui_help(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(t) = &self.tool {
                ui.heading(t.kind.name());
                ui.label(t.kind.tooltip());
                ui.label(egui::RichText::new(t.prompt()).weak());
                ui.separator();
            }
            ui.label(egui::RichText::new("Mouse").strong());
            ui.label("Left: pick / select · drag →: window · drag ←: crossing\nRight drag: rotate (pan in Top/Front/Right) · Shift+right / middle: pan · wheel: zoom\nRight click: Enter (repeat last command)");
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Keys").strong());
            ui.label("Enter / Space: confirm or repeat · Esc: cancel\nCtrl+Z/Y undo/redo · Ctrl+C/X/V clipboard · Ctrl+G group · Ctrl+H hide · Ctrl+L lock\nF3 properties · F7 grid · F8 Ortho · F9 Grid Snap");
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Commands").strong());
            for k in ToolKind::CURVES
                .iter()
                .chain(&ToolKind::CURVE_TOOLS)
                .chain(&ToolKind::SURFACES)
                .chain(&ToolKind::SOLIDS)
                .chain(&ToolKind::TRANSFORMS)
                .chain(&ToolKind::VISIBILITY)
                .chain(&ToolKind::ANALYZE)
            {
                ui.label(egui::RichText::new(k.tooltip()).small());
            }
        });
    }

    fn ui_properties(&mut self, ui: &mut egui::Ui) {
        let doc = self.engine.doc();
        let sel: Vec<_> = self
            .engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .collect();
        if sel.is_empty() {
            ui.weak("No objects selected.");
            ui.add_space(6.0);
            ui.weak("Select objects in a view to see and change their layer and colour.");
            return;
        }
        let mut cmd: Option<String> = None;
        let kinds: std::collections::BTreeSet<&str> =
            sel.iter().map(|o| o.geometry.kind()).collect();
        let kind = if kinds.len() == 1 {
            kinds.iter().next().copied().unwrap_or("")
        } else {
            "varies"
        };
        egui::Grid::new("props")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.weak("Selected");
                ui.label(sel.len().to_string());
                ui.end_row();
                ui.weak("Type");
                ui.label(kind);
                ui.end_row();

                // Layer.
                let layers: std::collections::BTreeSet<usize> =
                    sel.iter().map(|o| o.layer.0).collect();
                let current = if layers.len() == 1 {
                    doc.layers[*layers.iter().next().expect("one")].name.clone()
                } else {
                    "varies".into()
                };
                ui.weak("Layer");
                egui::ComboBox::from_id_salt("obj-layer")
                    .selected_text(current)
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for l in &doc.layers {
                            if ui.selectable_label(false, &l.name).clicked() {
                                cmd = Some(format!("ChangeLayer {}", l.name));
                            }
                        }
                    });
                ui.end_row();

                // Colour.
                let colors: std::collections::BTreeSet<Option<[u8; 3]>> =
                    sel.iter().map(|o| o.color).collect();
                ui.weak("Display colour");
                ui.vertical(|ui| {
                    let by_layer = colors.len() == 1 && colors.contains(&None);
                    let label = match (colors.len(), colors.iter().next()) {
                        (1, Some(None)) => "By Layer".to_string(),
                        (1, Some(Some([r, g, b]))) => format!("{r}, {g}, {b}"),
                        _ => "varies".into(),
                    };
                    ui.horizontal(|ui| {
                        if let (1, Some(c)) = (colors.len(), colors.iter().next()) {
                            let [r, g, b] = c.unwrap_or(doc.layer(sel[0].layer).color);
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                        }
                        ui.label(label);
                    });
                    if !by_layer && ui.button("Use layer colour").clicked() {
                        cmd = Some("SetObjectColor ByLayer".into());
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                        for c in SWATCHES {
                            let (rect, resp) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                Color32::from_rgb(c[0], c[1], c[2]),
                            );
                            if resp.hovered() {
                                ui.painter().rect_stroke(
                                    rect,
                                    2.0,
                                    Stroke::new(1.5, Color32::WHITE),
                                    StrokeKind::Outside,
                                );
                            }
                            if resp.clicked() {
                                cmd = Some(format!("SetObjectColor {},{},{}", c[0], c[1], c[2]));
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.color_edit_button_srgb(&mut self.color_edit);
                        if ui
                            .button("Apply")
                            .on_hover_text("Set this colour on the selection")
                            .clicked()
                        {
                            let [r, g, b] = self.color_edit;
                            cmd = Some(format!("SetObjectColor {r},{g},{b}"));
                        }
                    });
                });
                ui.end_row();

                // Size.
                let pts: Vec<Point3> = sel
                    .iter()
                    .flat_map(|o| {
                        let b = o.geometry.bounding_box();
                        [b.min, b.max]
                    })
                    .collect();
                if let Some(bb) = forma_geom::BoundingBox::from_points(&pts) {
                    let u = doc.units.abbreviation();
                    ui.weak("Size");
                    ui.label(format!(
                        "{:.2} × {:.2} × {:.2} {u}",
                        bb.max.x - bb.min.x,
                        bb.max.y - bb.min.y,
                        bb.max.z - bb.min.z
                    ));
                    ui.end_row();
                }
                let groups: std::collections::BTreeSet<Option<u32>> =
                    sel.iter().map(|o| o.group).collect();
                if !groups.contains(&None) || groups.len() > 1 {
                    ui.weak("Group");
                    ui.label(match groups.iter().next() {
                        Some(Some(g)) if groups.len() == 1 => format!("group {g}"),
                        _ => "varies".into(),
                    });
                    ui.end_row();
                }
                if sel.iter().any(|o| o.locked) {
                    ui.weak("Locked");
                    ui.label("yes");
                    ui.end_row();
                }
                if sel.len() == 1 {
                    if let Some(c) = sel[0].geometry.to_chain() {
                        let len: f64 = c.segs.iter().map(forma_geom::Seg::length).sum();
                        ui.weak("Length");
                        ui.label(format!("{len:.3} {}", doc.units.abbreviation()));
                        ui.end_row();
                    }
                    if let forma_doc::Geometry::Arc(a) = &sel[0].geometry {
                        ui.weak("Radius");
                        ui.label(format!("{:.3}", a.radius));
                        ui.end_row();
                    }
                }
            });
        if let Some(c) = cmd {
            self.run_engine(&c);
        }
    }

    fn ui_layers(&mut self, ui: &mut egui::Ui) {
        let mut cmd: Option<String> = None;
        ui.horizontal(|ui| {
            if ui
                .button("＋ New Layer")
                .on_hover_text("New layer (becomes current)")
                .clicked()
            {
                let mut k = self.engine.doc().layers.len();
                let name = loop {
                    let candidate = format!("Layer {k:02}");
                    if self.engine.doc().find_layer(&candidate).is_none() {
                        break candidate;
                    }
                    k += 1;
                };
                cmd = Some(format!("Layer {name}"));
            }
            if ui
                .add_enabled(
                    self.has_selection(),
                    egui::Button::new("Move Selection Here"),
                )
                .on_hover_text("Move the selected objects to the current layer")
                .clicked()
            {
                let doc = self.engine.doc();
                cmd = Some(format!("ChangeLayer {}", doc.layer(doc.current_layer).name));
            }
        });
        ui.add_space(4.0);
        let doc = self.engine.doc();
        let mut counts = vec![0usize; doc.layers.len()];
        for o in doc.objects() {
            counts[o.layer.0] += 1;
        }
        let current = doc.current_layer.0;
        let has_sel = self.has_selection();
        let layers = doc.layers.clone();
        // Header like Rhino's layer panel.
        let header = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [130.0, 16.0],
                    egui::Label::new(egui::RichText::new("Name").small().weak()),
                );
                ui.add_sized(
                    [18.0, 16.0],
                    egui::Label::new(egui::RichText::new("").small()),
                );
                ui.add_sized(
                    [18.0, 16.0],
                    egui::Label::new(egui::RichText::new("On").small().weak()),
                );
                ui.add_sized(
                    [18.0, 16.0],
                    egui::Label::new(egui::RichText::new("Lk").small().weak()),
                );
                ui.add_sized(
                    [24.0, 16.0],
                    egui::Label::new(egui::RichText::new("Color").small().weak()),
                );
            });
        };
        header(ui);
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, layer) in layers.iter().enumerate() {
                let depth = layer.name.matches("::").count();
                let short = layer
                    .name
                    .rsplit("::")
                    .next()
                    .unwrap_or(&layer.name)
                    .to_string();
                let row = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let (name_rect, name_resp) =
                        ui.allocate_exact_size(egui::vec2(130.0, 20.0), egui::Sense::click());
                    if i == current {
                        ui.painter().rect_filled(name_rect, 2.0, theme::ACCENT_BG);
                    } else if name_resp.hovered() {
                        ui.painter()
                            .rect_filled(name_rect, 2.0, Color32::from_rgb(235, 243, 251));
                    }
                    let indent = depth as f32 * 12.0;
                    if depth > 0 {
                        ui.painter().text(
                            name_rect.left_center() + egui::vec2(indent - 8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            "└",
                            egui::FontId::proportional(11.0),
                            theme::WEAK,
                        );
                    }
                    ui.painter().text(
                        name_rect.left_center() + egui::vec2(indent + 4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &short,
                        egui::FontId::proportional(13.0),
                        if layer.visible {
                            theme::TEXT
                        } else {
                            theme::WEAK
                        },
                    );
                    let name_resp = name_resp.on_hover_text(format!(
                        "{} — {} objects\nDouble-click: make current · Right click: more",
                        layer.name, counts[i]
                    ));
                    if name_resp.double_clicked() || name_resp.clicked() {
                        cmd = Some(format!("Layer {}", layer.name));
                    }
                    name_resp.context_menu(|ui| {
                        if ui.button("Set Current").clicked() {
                            cmd = Some(format!("Layer {}", layer.name));
                            ui.close();
                        }
                        if ui
                            .add_enabled(has_sel, egui::Button::new("Change Object Layer"))
                            .clicked()
                        {
                            cmd = Some(format!("ChangeLayer {}", layer.name));
                            ui.close();
                        }
                        if ui.button("Select Objects").clicked() {
                            let ids: Vec<String> = doc
                                .objects()
                                .filter(|o| o.layer.0 == i)
                                .map(|o| format!("#{}", o.id.0))
                                .collect();
                            if !ids.is_empty() {
                                cmd = Some(format!("SelNone; Select {}", ids.join(" ")));
                            }
                            ui.close();
                        }
                    });
                    // Current check mark.
                    let (r, resp) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                    if i == current {
                        ui.painter().text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            "✔",
                            egui::FontId::proportional(13.0),
                            theme::TEXT,
                        );
                    }
                    if resp.on_hover_text("Current layer").clicked() {
                        cmd = Some(format!("Layer {}", layer.name));
                    }
                    // On / off: a light bulb.
                    let (r, resp) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                    paint_bulb(ui.painter(), r, layer.visible);
                    if resp.on_hover_text("Show / hide").clicked() {
                        let on = if layer.visible { "off" } else { "on" };
                        cmd = Some(format!("LayerVisible {on} {}", layer.name));
                    }
                    // Lock: a padlock.
                    let (r, resp) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                    paint_lock(ui.painter(), r, layer.locked);
                    if resp.on_hover_text("Lock / unlock").clicked() {
                        let on = if layer.locked { "off" } else { "on" };
                        cmd = Some(format!("LayerLock {on} {}", layer.name));
                    }
                    let mut c = layer.color;
                    if ui.color_edit_button_srgb(&mut c).changed() {
                        cmd = Some(format!(
                            "LayerColor {},{},{} {}",
                            c[0], c[1], c[2], layer.name
                        ));
                    }
                    ui.label(egui::RichText::new(counts[i].to_string()).small().weak());
                });
                let _ = row;
            }
        });
        if let Some(c) = cmd {
            if let Some(rest) = c.strip_prefix("SelNone; ") {
                self.engine.ctx.selection.clear();
                self.run_engine(rest);
            } else {
                // Layer edits from the panel are not worth a log line each.
                let quiet = c.starts_with("LayerColor")
                    || c.starts_with("LayerVisible")
                    || c.starts_with("LayerLock");
                if quiet {
                    if let Err(e) = self.engine.run_line(&c) {
                        self.log(LogKind::Error, e.to_string());
                    }
                } else {
                    self.run_engine(&c);
                }
            }
        }
    }

    /// Command history and prompt, docked at the top like Rhino.
    fn ui_command_line(&mut self, ui: &mut egui::Ui) {
        ui.set_min_height(ui.available_height());
        let log_height = (ui.available_height() - 28.0).max(16.0);
        let frame = egui::Frame::NONE
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .inner_margin(egui::Margin::symmetric(6, 2));
        frame.show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(log_height)
                .min_scrolled_height(log_height)
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (kind, line) in &self.log {
                        let t = egui::RichText::new(line).monospace().size(12.5);
                        match kind {
                            LogKind::Normal => ui.label(t),
                            LogKind::Command => ui.label(t.color(theme::WEAK)),
                            LogKind::Error => ui.label(t.color(Color32::from_rgb(190, 30, 30))),
                        };
                    }
                });
        });
        ui.horizontal(|ui| {
            let prompt = match (&self.tool, &self.gumball_typed) {
                _ if self.pending.is_some() => {
                    let (line, what) = self.pending.as_ref().expect("pending");
                    format!("{line} — {what} (Enter to run):")
                }
                (_, Some((h, _))) => format!("Gumball — {}:", h.describe()),
                (Some(t), None) => format!("{}:", t.prompt()),
                (None, None) => "Command:".to_string(),
            };
            ui.label(egui::RichText::new(prompt).monospace().size(13.0));
            let id = egui::Id::new(CMD_ID);
            let edit = egui::TextEdit::singleline(&mut self.command)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .frame(egui::Frame::NONE)
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
                self.submit_space(&text);
            }
        });
    }

    /// Rhino's Osnap bar: one checkbox per object snap, plus Project and Disable.
    fn ui_osnap(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.spacing_mut().icon_spacing = 3.0;
            let disabled = self.snap.disabled;
            for k in SnapKind::ALL {
                ui.add_enabled_ui(!disabled, |ui| {
                    ui.checkbox(self.snap.flag(k), egui::RichText::new(k.label()).size(12.5));
                });
            }
            ui.add_enabled_ui(!disabled, |ui| {
                ui.checkbox(
                    &mut self.snap.project,
                    egui::RichText::new("Project").size(12.5),
                );
            });
            ui.checkbox(
                &mut self.snap.disabled,
                egui::RichText::new("Disable").size(12.5),
            );
        });
    }

    fn ui_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (plane_name, coords) = match &self.hover {
                Some(h) => {
                    let (u, v, w) = self.viewports[h.viewport].cplane().coords(h.point);
                    ("CPlane", format!("x {u:>9.3}   y {v:>9.3}   z {w:>8.3}"))
                }
                None => ("CPlane", "x —   y —   z —".to_string()),
            };
            ui.label(egui::RichText::new(plane_name).size(12.5));
            let dist = match (&self.hover, self.tool.as_ref().and_then(Tool::base)) {
                (Some(h), Some(b)) => Some(h.point.distance_to(b)),
                _ => None,
            };
            ui.add_sized(
                [250.0, 18.0],
                egui::Label::new(egui::RichText::new(coords).monospace().size(12.5)),
            );
            if let Some(d) = dist {
                ui.label(egui::RichText::new(format!("Distance {}", self.fmt_len(d))).size(12.5));
            }
            ui.separator();
            let units = match self.engine.doc().units {
                LengthUnit::Millimeters => "Millimeters",
                LengthUnit::Centimeters => "Centimeters",
                LengthUnit::Meters => "Meters",
                LengthUnit::Inches => "Inches",
                LengthUnit::Feet => "Feet",
            };
            ui.label(egui::RichText::new(units).size(12.5));
            ui.separator();
            // Current layer: colour swatch and a drop-down to change it.
            let doc = self.engine.doc();
            let cur = doc.layer(doc.current_layer);
            let [r, g, b] = cur.color;
            let (sw, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(sw, 1.0, Color32::from_rgb(r, g, b));
            ui.painter()
                .rect_stroke(sw, 1.0, Stroke::new(1.0, theme::WEAK), StrokeKind::Inside);
            let mut pick: Option<String> = None;
            egui::ComboBox::from_id_salt("status-layer")
                .selected_text(egui::RichText::new(cur.name.clone()).size(12.5))
                .width(130.0)
                .show_ui(ui, |ui| {
                    for l in &doc.layers {
                        if ui.selectable_label(false, &l.name).clicked() {
                            pick = Some(l.name.clone());
                        }
                    }
                });
            if let Some(n) = pick {
                self.run_engine(&format!("Layer {n}"));
            }
            ui.separator();
            let pane = |ui: &mut egui::Ui, on: &mut bool, label: &str, tip: &str| {
                let text = egui::RichText::new(label).size(12.5);
                let text = if *on {
                    text.strong().color(Color32::BLACK)
                } else {
                    text.color(theme::WEAK)
                };
                if ui
                    .add(egui::Button::new(text).frame(false))
                    .on_hover_text(tip)
                    .clicked()
                {
                    *on = !*on;
                }
            };
            pane(
                ui,
                &mut self.snap.grid,
                "Grid Snap",
                "F9 — snap to the grid",
            );
            pane(
                ui,
                &mut self.snap.ortho,
                "Ortho",
                "F8 or hold Shift — horizontal / vertical only",
            );
            pane(
                ui,
                &mut self.snap.planar,
                "Planar",
                "Keep points on the plane of the first point",
            );
            let mut osnap_on = !self.snap.disabled;
            pane(ui, &mut osnap_on, "Osnap", "Object snaps on / off");
            self.snap.disabled = !osnap_on;
            pane(
                ui,
                &mut self.snap.smart,
                "SmartTrack",
                "Line up with points you rested on with an object snap",
            );
            pane(
                ui,
                &mut self.gumball_on,
                "Gumball",
                "Axis handles on the selection",
            );
            ui.add_enabled(
                false,
                egui::Button::new(egui::RichText::new("Record History").size(12.5)).frame(false),
            )
            .on_disabled_hover_text("Not available yet");
            let filter = &mut self.filter;
            ui.menu_button(egui::RichText::new("Filter").size(12.5), |ui| {
                ui.checkbox(&mut filter.curves, "Curves");
                ui.checkbox(&mut filter.surfaces, "Surfaces / meshes");
                ui.checkbox(&mut filter.points, "Points");
            });
            ui.separator();
            if let Some(r) = self.renderer.as_ref() {
                let _ = r;
            }
            let mins = self.saved_at.elapsed().as_secs() / 60;
            ui.label(
                egui::RichText::new(format!("Minutes from last save: {mins}"))
                    .size(12.5)
                    .color(theme::WEAK),
            );
            ui.separator();
            let doc = self.engine.doc();
            ui.label(
                egui::RichText::new(format!(
                    "{} objects · {} selected",
                    doc.len(),
                    self.engine.ctx.selection.len()
                ))
                .size(12.5)
                .color(theme::WEAK),
            );
        });
    }
}

/// Rhino-style Align command for the active construction plane.
fn align_cmd(opt: &str) -> &'static str {
    match opt {
        "left" => "Align left",
        "right" => "Align right",
        "top" => "Align top",
        "bottom" => "Align bottom",
        "hcenter" => "Align hcenter",
        "vcenter" => "Align vcenter",
        _ => "Align center",
    }
}

fn paint_bulb(p: &egui::Painter, r: Rect, on: bool) {
    let c = r.center() - egui::vec2(0.0, 2.0);
    let fill = if on {
        Color32::from_rgb(255, 214, 40)
    } else {
        Color32::from_rgb(225, 225, 225)
    };
    p.circle_filled(c, 5.0, fill);
    p.circle_stroke(c, 5.0, Stroke::new(1.0, Color32::from_gray(90)));
    let base = Rect::from_center_size(c + egui::vec2(0.0, 6.5), egui::vec2(5.0, 3.0));
    p.rect_filled(base, 0.5, Color32::from_gray(110));
}

fn paint_lock(p: &egui::Painter, r: Rect, locked: bool) {
    let body = Rect::from_center_size(r.center() + egui::vec2(0.0, 2.5), egui::vec2(10.0, 7.0));
    let fill = if locked {
        Color32::from_rgb(230, 170, 30)
    } else {
        Color32::from_rgb(235, 235, 235)
    };
    p.rect_filled(body, 1.0, fill);
    p.rect_stroke(
        body,
        1.0,
        Stroke::new(1.0, Color32::from_gray(90)),
        StrokeKind::Inside,
    );
    let cx = if locked {
        body.center().x
    } else {
        body.center().x + 3.0
    };
    let pts: Vec<Pos2> = (0..=12)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 12.0;
            egui::pos2(cx + 3.2 * a.cos(), body.top() - 3.8 * a.sin())
        })
        .collect();
    p.add(egui::Shape::line(
        pts,
        Stroke::new(1.3, Color32::from_gray(90)),
    ));
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
                "{} — Forma",
                std::path::Path::new(p)
                    .file_name()
                    .map_or(p.clone(), |n| n.to_string_lossy().into_owned())
            ),
            None => "Untitled — Forma".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

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
        egui::Panel::top("command")
            .resizable(true)
            .default_size(92.0)
            .size_range(56.0..=400.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(4, 3)),
            )
            .show(ui, |ui| self.ui_command_line(ui));
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
        egui::Panel::right("side")
            .resizable(true)
            .default_size(280.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(6)),
            )
            .show(ui, |ui| self.ui_side(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_gray(150)))
            .show(ui, |ui| self.ui_viewports(ui, frame));

        // Changes made by panels this frame (commands, layer toggles) need a redraw.
        if self.force_rebuild
            || self.engine.doc().version() != self.seen_version
            || self.engine.ctx.selection != self.seen_selection
            || self.tool.is_some()
        {
            ctx.request_repaint();
        }
        // Minutes-from-last-save counter.
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
    }
}
