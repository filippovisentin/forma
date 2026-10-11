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

mod agent_link;
mod app;
mod ask;
mod autosave;
mod capture;
mod cmdline;
mod commands;
mod complete;
mod cplane;
mod dialogs;
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
mod scripting;
mod settings;
mod snap;
mod start;
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
    autosave::install_crash_log();
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
                app.new_model = None; // a file was given: no "New model" dialog
                app.run_engine(&format!("Open {path}"));
            }
            Ok(Box::new(app))
        }),
    )
}

/// Window and taskbar icon: `packaging/icon/forma-256.png` (made by
/// `packaging/icon/make_icon.py`; the same picture is in `forma.ico` for the
/// Windows executable).
fn app_icon() -> egui::IconData {
    decode_icon(include_bytes!("../../../packaging/icon/forma-256.png")).unwrap_or_default()
}

fn decode_icon(bytes: &[u8]) -> Option<egui::IconData> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        _ => return None,
    };
    Some(egui::IconData {
        rgba,
        width: info.width,
        height: info.height,
    })
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
    /// Open a command dialog.
    Form(&'static dialogs::Form),
    /// File dialogs for Import and Export Selected.
    Import,
    Export,
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
            forma_doc::Geometry::Mesh(_) | forma_doc::Geometry::Brep(_) => self.surfaces,
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
    /// A different interface look was chosen: applied at the start of the next frame.
    pending_look: Option<theme::Look>,
    gumball_on: bool,
    gumball_hot: Option<(usize, gumball::Handle)>,
    gumball_drag: Option<GumballDrag>,
    /// A gumball handle was clicked: the next typed number moves/rotates by it.
    gumball_typed: Option<(gumball::Handle, Point3)>,
    /// A typed command line with arguments: the tool to start (with those
    /// arguments) if the engine says some required input is missing.
    typed_fallback: Option<(ToolKind, Vec<String>)>,
    /// Log lines already shown (the history scrolls to new ones).
    log_seen: usize,
    /// Lines logged so far (the log itself keeps the last 300).
    log_count: usize,
    face_sel: Option<FaceSel>,
    face_drag: Option<FaceDrag>,
    /// The face arrow was clicked: the next typed number pushes / pulls it.
    face_typed: bool,
    face_hot: bool,
    /// Control points shown with PointsOn, and the selected ones.
    grips: grips::Grips,
    /// A viewport's custom construction plane changed: re-upload the grids.
    cplane_dirty: bool,
    /// The open command dialog (Array, Box Edit, layer operations…).
    form: Option<dialogs::OpenForm>,
    /// A view capture to save (View → Capture View to Image).
    capture: Option<capture::Capture>,
    /// Values being edited in the Properties panel.
    prop_edit: Option<panels::PropEdit>,
    /// Section cut shown in every view (ClippingPlane): point and normal.
    clip: Option<(forma_geom::Point3, forma_geom::Vec3)>,
    /// Named views of this session: name, view kind, camera, CPlane.
    named_views: Vec<(
        String,
        forma_render::StandardView,
        forma_render::Camera,
        Option<forma_geom::Plane>,
    )>,
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
    /// What the settings file holds (written when the settings differ from it).
    settings_on_disk: Settings,
    /// Periodic copy of unsaved changes, and recovery after a crash.
    autosave: autosave::Autosave,
    /// AI assistants' connection to this window (Tools ▸ Agent Connection), when on.
    agent: Option<agent_link::AgentLink>,
    /// The Ask window (AI writes a command as a Python script).
    ask: ask::AskWindow,
    /// For work that needs the context outside the frame callbacks.
    egui_ctx: egui::Context,
    /// The "New model" (units) dialog, when open.
    new_model: Option<start::NewDialog>,
    /// Panel sizes (points), remembered between runs.
    side_width: f32,
    command_height: f32,
    /// A height to give the docked command area next frame (double-click on
    /// the grip, `CommandArea Compact|Tall`).
    command_resize: Option<f32>,
    /// Where the command area is (View ▸ Command Area).
    cmd_dock: cmdline::CmdDock,
}

const CMD_ID: &str = "forma-command-line";

#[cfg(test)]
mod icon_tests {
    #[test]
    fn the_app_icon_decodes() {
        let icon = super::app_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        // Transparent corner, opaque middle.
        assert_eq!(icon.rgba[3], 0);
        assert_eq!(icon.rgba[(128 * 256 + 128) * 4 + 3], 255);
    }
}
