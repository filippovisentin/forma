//! Forma desktop UI (egui), laid out like Rhino: four viewports, a command line,
//! a tool sidebar and a layers panel. Every modelling action goes through
//! `forma-engine`; interactive tools only collect clicks and emit command lines.
//!
//! The app state lives here; its behaviour is split by area:
//! `app` (frame loop, document sync, settings), `commands` (command line
//! dispatch and tools), `files` (open / save / unsaved-changes guard),
//! `keyboard`, `input` (viewport mouse input), `overlays` (cursor, snaps,
//! gumball, labels), `views` (viewport layout and title menus), `cmdline`
//! (history, prompt, autocomplete), `menus`, `toolbar`, `panels`, `status`.

mod app;
mod cmdline;
mod commands;
mod complete;
mod cplane;
mod files;
mod grips;
mod gumball;
mod icons;
mod index;
mod input;
mod keyboard;
mod menus;
mod overlays;
mod panels;
mod perf;
mod pick;
mod settings;
mod snap;
mod status;
mod theme;
mod toolbar;
mod tools;
mod viewport;
mod views;

use eframe::egui::{self, Pos2};
use forma_doc::ObjectId;
use forma_engine::Engine;
use forma_geom::{BoundingBox, Point3, Vec3};
use forma_render::glam::DVec3;
use forma_render::{DisplayMode, Renderer, SceneCache};
use index::SceneIndex;
use settings::Settings;
use snap::{SnapKind, SnapSettings};
use std::collections::BTreeSet;
use tools::{Tool, ToolKind};
use viewport::Viewport;

/// Start the desktop app, optionally opening a file.
pub fn run(open: Option<String>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Forma")
            .with_inner_size([1500.0, 950.0])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true)
            .with_icon(std::sync::Arc::new(app_icon())),
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

/// Window and taskbar icon, drawn in code (no image assets): a white
/// wireframe box on a blue rounded square.
fn app_icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    // Isometric box corners (pixels).
    let p = |x: f32, y: f32| (x, y);
    let (top, l, r, c) = (p(32.0, 12.0), p(13.0, 22.0), p(51.0, 22.0), p(32.0, 32.0));
    let (lb, rb, cb) = (p(13.0, 42.0), p(51.0, 42.0), p(32.0, 53.0));
    let edges = [
        (top, l),
        (top, r),
        (l, c),
        (r, c),
        (l, lb),
        (r, rb),
        (c, cb),
        (lb, cb),
        (rb, cb),
    ];
    let dist = |(px, py): (f32, f32), (a, b): ((f32, f32), (f32, f32))| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
    };
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded square, radius 12, with a soft edge.
            let qx = (fx - 32.0).abs() - 20.0;
            let qy = (fy - 32.0).abs() - 20.0;
            let outside =
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - 10.0;
            let alpha = (0.5 - outside).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                continue;
            }
            // Blue background, lighter at the top.
            let k = fy / N as f32;
            let mut col = [
                40.0 + 30.0 * (1.0 - k),
                90.0 + 40.0 * (1.0 - k),
                170.0 + 40.0 * (1.0 - k),
            ];
            let d = edges
                .iter()
                .map(|e| dist((fx, fy), *e))
                .fold(f32::INFINITY, f32::min);
            let line = (2.6 - d).clamp(0.0, 1.0);
            for c in &mut col {
                *c += (255.0 - *c) * line;
            }
            let i = (y * N + x) * 4;
            rgba[i] = col[0] as u8;
            rgba[i + 1] = col[1] as u8;
            rgba[i + 2] = col[2] as u8;
            rgba[i + 3] = (alpha * 255.0) as u8;
        }
    }
    egui::IconData {
        rgba,
        width: N as u32,
        height: N as u32,
    }
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
    /// Faces of selected solids that an extrude dot pushes / pulls (preview).
    faces: Vec<Vec<[Point3; 2]>>,
}

/// A face picked with Ctrl+Shift+click (Rhino's sub-object selection).
struct FaceSel {
    id: ObjectId,
    center: Point3,
    normal: Vec3,
    outline: Vec<[Point3; 2]>,
}

/// Dragging the push / pull arrow of a picked face.
struct FaceDrag {
    viewport: usize,
    start: f64,
    distance: f64,
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

/// An action that would lose unsaved changes, waiting for "Save changes?".
#[derive(Clone, PartialEq)]
enum Guarded {
    /// `New …` or `Open <file>`.
    Line(String),
    /// Show the Open dialog.
    OpenDialog,
    /// Close the window.
    Exit,
}

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

/// Facts about the selection that are costly on big meshes, refreshed only
/// when the document or the selection changes (not every frame).
#[derive(Default)]
struct SelInfo {
    bbox: Option<BoundingBox>,
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
    /// Snap and pick data of the visible objects.
    index: SceneIndex,
    sel_info: SelInfo,
    snap: SnapSettings,
    tool: Option<Tool>,
    last_command: Option<String>,
    command: String,
    focus_command: bool,
    /// Commands run, for Up / Down recall.
    history: complete::History,
    completer: complete::Completer,
    /// Autocomplete row chosen with Up / Down.
    ac_choice: Option<usize>,
    /// A numeric tool option being typed (`Distance`, `Radius`, `NumSides`).
    option_edit: Option<&'static str>,
    log: Vec<(LogKind, String)>,
    hover: Option<Hover>,
    drag: Option<DragSelect>,
    /// The next window drag zooms instead of selecting (`ZW`).
    zoom_window: bool,
    title: String,
    /// Saved at least once by Forma (otherwise Save asks for a name, to avoid
    /// overwriting an original Rhino file with display meshes).
    saved_by_forma: bool,
    /// Document version at the last open / save (unsaved changes otherwise).
    saved_version: u64,
    /// "Save changes?" is being asked before this action.
    confirm: Option<Guarded>,
    /// The guard already asked (or there is nothing to lose): let it through.
    guard_pass: bool,
    /// Closing was confirmed.
    allow_close: bool,
    pending_fit: bool,
    gumball_on: bool,
    gumball_hot: Option<(usize, gumball::Handle)>,
    gumball_drag: Option<GumballDrag>,
    /// A gumball handle was clicked: the next typed number moves/rotates by it.
    gumball_typed: Option<(gumball::Handle, Point3)>,
    face_sel: Option<FaceSel>,
    face_drag: Option<FaceDrag>,
    /// The face arrow was clicked: the next typed number pushes / pulls it.
    face_typed: bool,
    face_hot: bool,
    /// Control points shown with PointsOn, and the selected ones.
    grips: grips::Grips,
    /// A viewport's custom construction plane changed: re-upload the grids.
    cplane_dirty: bool,
    /// Tool values remembered between uses (Offset distance, Fillet radius…).
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
    /// Persistent user settings, and when they were last compared / written.
    settings: Settings,
    settings_checked: std::time::Instant,
    /// Panel sizes (points), remembered between runs.
    side_width: f32,
    command_height: f32,
}

const CMD_ID: &str = "forma-command-line";
