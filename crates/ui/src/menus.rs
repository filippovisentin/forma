//! The menu bar (File, Edit, View, Curve, Surface, Solid, Mesh, Dimension,
//! Transform, Tools, Analyze, Help), in the order of Rhino 8.

use crate::snap::SnapKind;
use crate::tools::seq as q;
use crate::tools::ToolKind;
use crate::{Act, FormaApp, SidePanel};
use eframe::egui;
use forma_render::DisplayMode;

/// Standard views: (label, command).
pub(crate) const SET_VIEWS: [(&str, &str); 7] = [
    ("Top", "top"),
    ("Front", "front"),
    ("Right", "right"),
    ("Perspective", "perspective"),
    ("Bottom", "bottom"),
    ("Back", "back"),
    ("Left", "left"),
];

/// Align options: (label, command line).
pub(crate) const ALIGN: [(&str, &str); 7] = [
    ("Left", "Align left"),
    ("Right", "Align right"),
    ("Top", "Align top"),
    ("Bottom", "Align bottom"),
    ("Horizontal Centers", "Align hcenter"),
    ("Vertical Centers", "Align vcenter"),
    ("Centers", "Align center"),
];

/// A Rectangular array asks for its numbers on the command line.
pub(crate) const ARRAY_PREFILL: Act = Act::Prefill(
    "Array ",
    "Array <nx> <ny> <nz> <dx,dy,dz> — e.g. Array 4 2 1 600,400,0 copies the selection 4×2 times",
);

impl FormaApp {
    pub(crate) fn ui_menu(&mut self, ui: &mut egui::Ui) {
        let mut act: Option<Act> = None;
        let mut open_recent: Option<String> = None;
        let mut clear_recent = false;
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
        use ToolKind as K;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.menu_button("New", |ui| {
                    item(ui, &mut act, "Centimetres", "Ctrl+N", Act::Cmd("New cm"));
                    item(ui, &mut act, "Millimetres", "", Act::Cmd("New mm"));
                    item(ui, &mut act, "Metres", "", Act::Cmd("New m"));
                });
                item(ui, &mut act, "Open…", "Ctrl+O", Act::Open);
                let recent = self.settings.recent();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button("Recent Files", |ui| {
                        for (i, path) in recent.iter().enumerate() {
                            let name = std::path::Path::new(path)
                                .file_name()
                                .map_or(path.clone(), |n| n.to_string_lossy().into_owned());
                            let label = format!("{}  {name}", i + 1);
                            if ui.button(label).on_hover_text(path).clicked() {
                                open_recent = Some(path.clone());
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("Clear List").clicked() {
                            clear_recent = true;
                            ui.close();
                        }
                    });
                });
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
                item(ui, &mut act, "Incremental Save", "", Act::Cmd("IncrementalSave"));
                item(
                    ui,
                    &mut act,
                    "Export Selected…",
                    "",
                    Act::Prefill(
                        "Export ",
                        "Export <file.3dm|.obj|.stl> — save only the selected objects",
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
                    item(ui, &mut act, "Visible Objects", "", Act::Cmd("SelVisible"));
                    item(
                        ui,
                        &mut act,
                        "Last Created Objects",
                        "",
                        Act::Cmd("SelLast"),
                    );
                    item(ui, &mut act, "Previous Selection", "", Act::Cmd("SelPrev"));
                    ui.separator();
                    item(ui, &mut act, "Curves", "", Act::Cmd("SelCrv"));
                    item(ui, &mut act, "Surfaces / Meshes", "", Act::Cmd("SelMesh"));
                    item(ui, &mut act, "Points", "", Act::Cmd("SelPt"));
                    ui.menu_button("Curve Filters", |ui| {
                        item(ui, &mut act, "Open Curves", "", Act::Cmd("SelOpenCrv"));
                        item(ui, &mut act, "Closed Curves", "", Act::Cmd("SelClosedCrv"));
                        item(ui, &mut act, "Polylines", "", Act::Cmd("SelPolyline"));
                        item(ui, &mut act, "Lines", "", Act::Cmd("SelLine"));
                    });
                    item(ui, &mut act, "Closed Meshes (Solids)", "", Act::Cmd("SelClosedMesh"));
                    item(ui, &mut act, "Open Meshes (Surfaces)", "", Act::Cmd("SelOpenMesh"));
                    item(ui, &mut act, "Text and Dots", "", Act::Cmd("SelText"));
                    item(ui, &mut act, "Dimensions", "", Act::Cmd("SelDim"));
                    ui.separator();
                    item(ui, &mut act, "By Layer…", "", Act::Prefill("SelLayer ", "SelLayer <layer> — select all objects on a layer"));
                    item(ui, &mut act, "By Colour…", "", Act::Prefill("SelColor ", "SelColor <r,g,b | #rrggbb | name> — select objects shown in that colour"));
                    item(ui, &mut act, "By Name…", "", Act::Prefill("SelName ", "SelName <name> — select objects by name (* wildcards)"));
                    item(ui, &mut act, "Small Objects…", "", Act::Prefill("SelSmall ", "SelSmall <size> — select objects smaller than size"));
                    item(ui, &mut act, "Duplicates", "", Act::Cmd("SelDup"));
                    tool(ui, &mut act, "Inside Boundary Curve", K::Seq(&q::SEL_BOUNDARY));
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
                ui.menu_button("Control Points", |ui| {
                    item(ui, &mut act, "Points On", "F10", Act::Submit("pointson"));
                    item(ui, &mut act, "Points Off", "F11", Act::Submit("pointsoff"));
                });
                ui.menu_button("Blocks", |ui| {
                    tool(ui, &mut act, "Create Block", K::Seq(&q::BLOCK));
                    tool(ui, &mut act, "Insert Block", K::Seq(&q::INSERT));
                    item(
                        ui,
                        &mut act,
                        "Explode Block",
                        "",
                        Act::Tool(K::OnSel("ExplodeBlock")),
                    );
                });
                ui.menu_button("Visibility", |ui| {
                    item(ui, &mut act, "Hide", "Ctrl+H", Act::Tool(K::OnSel("Hide")));
                    item(ui, &mut act, "Show", "Ctrl+Alt+H", Act::Cmd("Show"));
                    item(ui, &mut act, "Isolate", "", Act::Tool(K::OnSel("Isolate")));
                    item(ui, &mut act, "Show and Select Hidden", "", Act::Cmd("ShowSelected"));
                    item(ui, &mut act, "Swap Hidden and Visible", "", Act::Cmd("HideSwap"));
                    item(ui, &mut act, "Lock", "Ctrl+L", Act::Tool(K::OnSel("Lock")));
                    item(ui, &mut act, "Unlock", "Ctrl+Alt+L", Act::Cmd("Unlock"));
                    item(ui, &mut act, "Unlock and Select Locked", "", Act::Cmd("UnlockSelected"));
                    item(ui, &mut act, "Swap Locked and Unlocked", "", Act::Cmd("LockSwap"));
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
                    tool(ui, &mut act, "Change Object to Current Layer", K::Seq(&q::CHANGE_TO_CURRENT));
                    item(ui, &mut act, "Copy Objects to Layer…", "", Act::Prefill("CopyObjectsToLayer ", "CopyObjectsToLayer <layer> — copy the selection to a layer"));
                    ui.separator();
                    item(ui, &mut act, "One Layer On…", "", Act::Prefill("OneLayerOn ", "OneLayerOn <layer> — show only this layer"));
                    item(ui, &mut act, "One Layer Off…", "", Act::Prefill("OneLayerOff ", "OneLayerOff <layer> — hide one layer"));
                    item(ui, &mut act, "All Layers On", "", Act::Cmd("AllLayersOn"));
                    item(ui, &mut act, "Rename Layer…", "", Act::Prefill("RenameLayer ", "RenameLayer <old name> <new name>"));
                    item(ui, &mut act, "Delete Layer…", "", Act::Prefill("DeleteLayer ", "DeleteLayer <layer> — delete a layer and its objects"));
                    item(ui, &mut act, "Purge Empty Layers", "", Act::Cmd("Purge"));
                });
                item(ui, &mut act, "Object Name…", "", Act::Prefill("SetObjectName ", "SetObjectName <name> — name the selected objects"));
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
                    item(ui, &mut act, "Zoom Window", "ZW", Act::Submit("zw"));
                });
                ui.menu_button("Set View", |ui| {
                    for (label, cmd) in SET_VIEWS {
                        item(ui, &mut act, label, "", Act::Submit(cmd));
                    }
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
                let grid_changed = self
                    .renderer
                    .as_mut()
                    .is_some_and(|r| ui.checkbox(&mut r.show_grid, "Grid  (F7)").changed());
                if grid_changed {
                    self.dirty_all();
                }
                ui.checkbox(&mut self.gumball_on, "Gumball");
                ui.checkbox(&mut self.show_osnap, "Osnap Toolbar");
            });
            ui.menu_button("Curve", |ui| {
                ui.menu_button("Point Object", |ui| {
                    tool(ui, &mut act, "Single Point", K::Point);
                    ui.menu_button("Divide Curve By", |ui| {
                        tool(ui, &mut act, "Number of Segments", K::Seq(&q::DIVIDE));
                        tool(ui, &mut act, "Length of Segments", K::Seq(&q::DIVIDE_LENGTH));
                    });
                    tool(ui, &mut act, "Extract Points", K::Seq(&q::EXTRACTPT));
                });
                ui.menu_button("Line", |ui| {
                    tool(ui, &mut act, "Single Line", K::Line);
                    tool(ui, &mut act, "Line Segments", K::Seq(&q::LINES));
                });
                tool(ui, &mut act, "Polyline", K::Polyline);
                ui.menu_button("Free-Form", |ui| {
                    tool(ui, &mut act, "Control Points", K::Curve);
                    tool(ui, &mut act, "Interpolate Points", K::InterpCrv);
                });
                ui.menu_button("Rectangle", |ui| {
                    tool(ui, &mut act, "Corner to Corner", K::Rectangle);
                    tool(ui, &mut act, "Center, Corner", K::Seq(&q::RECTANGLE_CENTER));
                    tool(ui, &mut act, "3 Points", K::Seq(&q::RECTANGLE3PT));
                    tool(ui, &mut act, "Rounded", K::Seq(&q::ROUNDED_RECTANGLE));
                });
                tool(ui, &mut act, "Polygon", K::Polygon);
                ui.menu_button("Circle", |ui| {
                    tool(ui, &mut act, "Center, Radius", K::Circle);
                    tool(ui, &mut act, "2 Points", K::Seq(&q::CIRCLE2PT));
                    tool(ui, &mut act, "3 Points", K::Seq(&q::CIRCLE3PT));
                });
                ui.menu_button("Arc", |ui| {
                    tool(ui, &mut act, "Center, Start, Angle", K::Arc);
                    tool(ui, &mut act, "Start, End, Point on Arc", K::Seq(&q::ARC3PT));
                });
                tool(ui, &mut act, "Ellipse", K::Ellipse);
                tool(ui, &mut act, "Slot", K::Seq(&q::SLOT));
                tool(ui, &mut act, "Helix", K::Seq(&q::HELIX));
                tool(ui, &mut act, "Spiral", K::Seq(&q::SPIRAL));
                ui.separator();
                tool(ui, &mut act, "Fillet Curves", K::Fillet);
                tool(ui, &mut act, "Chamfer Curves", K::Chamfer);
                tool(ui, &mut act, "Fillet Corners", K::FilletCorners);
                tool(ui, &mut act, "Offset Curve", K::Offset);
                tool(ui, &mut act, "Extend Curve", K::Extend);
                tool(ui, &mut act, "Curve Boolean", K::Seq(&q::CURVE_BOOLEAN));
                ui.separator();
                ui.menu_button("Curve Edit Tools", |ui| {
                    tool(ui, &mut act, "Join", K::Join);
                    tool(ui, &mut act, "Explode", K::Explode);
                    tool(ui, &mut act, "Trim", K::Trim);
                    tool(ui, &mut act, "Split", K::Split);
                    tool(ui, &mut act, "Flip Direction", K::OnSel("Flip"));
                    tool(ui, &mut act, "Rebuild", K::Seq(&q::REBUILD));
                    tool(ui, &mut act, "Convert to Polyline", K::Seq(&q::CONVERT));
                    tool(ui, &mut act, "Close Open Curve", K::Seq(&q::CLOSECRV));
                });
                ui.menu_button("Curve From Objects", |ui| {
                    tool(ui, &mut act, "Project", K::Seq(&q::PROJECT));
                    tool(ui, &mut act, "Pull", K::Seq(&q::PULL));
                    tool(ui, &mut act, "Duplicate Edge", K::Seq(&q::DUPEDGE));
                    tool(ui, &mut act, "Duplicate Border", K::Seq(&q::DUPBORDER));
                    tool(ui, &mut act, "Duplicate Face Border", K::Seq(&q::DUP_FACE_BORDER));
                    tool(ui, &mut act, "Intersection", K::OnSel("Intersect"));
                    tool(ui, &mut act, "Contour", K::Seq(&q::CONTOUR));
                    tool(ui, &mut act, "Section", K::Seq(&q::SECTION));
                    tool(
                        ui,
                        &mut act,
                        "Project To CPlane",
                        K::OnSel("ProjectToCPlane"),
                    );
                });
            });
            ui.menu_button("Surface", |ui| {
                tool(ui, &mut act, "Corner Points", K::Seq(&q::SRFPT));
                tool(ui, &mut act, "Edge Curves", K::Seq(&q::EDGESRF));
                tool(ui, &mut act, "Planar Curves", K::OnSel("PlanarSrf"));
                tool(ui, &mut act, "Rectangle (Plane)", K::Seq(&q::PLANE));
                ui.menu_button("Extrude Curve", |ui| {
                    tool(ui, &mut act, "Straight", K::Extrude);
                    tool(ui, &mut act, "Along Curve", K::Seq(&q::EXTRUDE_ALONG));
                    tool(ui, &mut act, "Tapered", K::Seq(&q::EXTRUDE_TAPERED));
                    tool(ui, &mut act, "To Point", K::Seq(&q::EXTRUDE_TO_POINT));
                });
                tool(ui, &mut act, "Loft", K::OnSel("Loft"));
                tool(ui, &mut act, "Revolve", K::Revolve);
                tool(ui, &mut act, "Sweep 1 Rail", K::Sweep1);
                tool(ui, &mut act, "Pipe", K::Seq(&q::PIPE));
                ui.separator();
                tool(ui, &mut act, "Offset Surface (solids)", K::Seq(&q::OFFSET_SRF));
                ui.label(
                    egui::RichText::new("Surfaces are meshes until the NURBS kernel lands")
                        .small()
                        .weak(),
                );
            });
            ui.menu_button("Solid", |ui| {
                tool(ui, &mut act, "Box", K::Box);
                tool(ui, &mut act, "Sphere", K::Sphere);
                tool(ui, &mut act, "Ellipsoid", K::Seq(&q::ELLIPSOID));
                tool(ui, &mut act, "Cone", K::Seq(&q::CONE));
                tool(ui, &mut act, "Truncated Cone", K::Seq(&q::TCONE));
                tool(ui, &mut act, "Pyramid", K::Seq(&q::PYRAMID));
                tool(ui, &mut act, "Cylinder", K::Cylinder);
                tool(ui, &mut act, "Tube", K::Seq(&q::TUBE));
                tool(ui, &mut act, "Pipe", K::Seq(&q::PIPE));
                tool(ui, &mut act, "Torus", K::Seq(&q::TORUS));
                tool(ui, &mut act, "Wall / Slab from Curves", K::Seq(&q::SLAB));
                ui.separator();
                ui.menu_button("Extrude Planar Curve", |ui| {
                    tool(ui, &mut act, "Straight", K::Extrude);
                });
                tool(ui, &mut act, "Extrude Surface", K::ExtrudeSrf);
                if ui
                    .button("Push / Pull Face…")
                    .on_hover_text("Ctrl+Shift+click a face of a solid, then drag its orange arrow (MoveFace)")
                    .clicked()
                {
                    act = Some(Act::Prefill(
                        "",
                        "Push / pull: Ctrl+Shift+click a flat face of a solid, then drag the orange arrow or click it and type a distance",
                    ));
                    ui.close();
                }
                tool(ui, &mut act, "Cap Planar Holes", K::OnSel("Cap"));
                ui.separator();
                ui.menu_button("Boolean", |ui| {
                    tool(ui, &mut act, "Union", K::Seq(&q::BOOLEAN_UNION));
                    tool(ui, &mut act, "Difference", K::Seq(&q::BOOLEAN_DIFFERENCE));
                    tool(ui, &mut act, "Intersection", K::Seq(&q::BOOLEAN_INTERSECTION));
                    tool(ui, &mut act, "Split", K::Seq(&q::BOOLEAN_SPLIT));
                });
                ui.menu_button("Edge Tools", |ui| {
                    tool(ui, &mut act, "Fillet Edge", K::Seq(&q::FILLET_EDGE));
                    tool(ui, &mut act, "Chamfer Edge", K::Seq(&q::CHAMFER_EDGE));
                });
                tool(ui, &mut act, "Shell", K::Seq(&q::SHELL));
                tool(ui, &mut act, "Offset Solid", K::Seq(&q::OFFSET_SRF));
            });
            ui.menu_button("Mesh", |ui| {
                tool(ui, &mut act, "Weld (smooth)", K::Seq(&q::WELD));
                tool(ui, &mut act, "Unweld (show edges)", K::Seq(&q::UNWELD));
                tool(ui, &mut act, "Explode into Faces", K::Explode);
                tool(ui, &mut act, "Join Meshes", K::Join);
                ui.separator();
                tool(ui, &mut act, "Duplicate Mesh Edges", K::Seq(&q::DUPEDGE));
                tool(ui, &mut act, "Duplicate Mesh Border", K::Seq(&q::DUPBORDER));
                tool(ui, &mut act, "Unify Normals", K::Seq(&q::UNIFY_MESH_NORMALS));
                tool(ui, &mut act, "Contour", K::Seq(&q::CONTOUR));
                tool(ui, &mut act, "Section", K::Seq(&q::SECTION));
            });
            ui.menu_button("Dimension", |ui| {
                tool(ui, &mut act, "Linear Dimension", K::Seq(&q::DIM));
                tool(ui, &mut act, "Aligned Dimension", K::Seq(&q::DIM_ALIGNED));
                tool(ui, &mut act, "Radial Dimension", K::Seq(&q::DIM_RADIUS));
                tool(ui, &mut act, "Diameter Dimension", K::Seq(&q::DIM_DIAMETER));
                tool(ui, &mut act, "Angle Dimension", K::Seq(&q::DIM_ANGLE));
                tool(ui, &mut act, "Leader", K::Seq(&q::LEADER));
                ui.separator();
                tool(ui, &mut act, "Text", K::Seq(&q::TEXT));
                tool(ui, &mut act, "Text Dot", K::Seq(&q::DOT));
                tool(ui, &mut act, "Edit Text", K::Seq(&q::EDIT_TEXT));
                tool(ui, &mut act, "Hatch", K::Seq(&q::HATCH));
                ui.separator();
                item(
                    ui,
                    &mut act,
                    "Annotation Style…",
                    "",
                    Act::Prefill(
                        "DimStyle ",
                        "DimStyle [text height] [decimals] — style of new text and dimensions (Enter shows it)",
                    ),
                );
            });
            ui.menu_button("Transform", |ui| {
                tool(ui, &mut act, "Move", K::Move);
                tool(ui, &mut act, "Copy", K::Copy);
                tool(ui, &mut act, "Rotate", K::Rotate);
                tool(ui, &mut act, "Rotate 3-D", K::Seq(&q::ROTATE3D));
                ui.menu_button("Scale", |ui| {
                    tool(ui, &mut act, "Scale 3-D", K::Scale);
                    tool(ui, &mut act, "Scale 2-D", K::Scale2D);
                    tool(ui, &mut act, "Scale 1-D", K::Scale1D);
                    tool(ui, &mut act, "Non-Uniform Scale", K::Seq(&q::SCALENU));
                });
                ui.menu_button("Mirror", |ui| {
                    tool(ui, &mut act, "Mirror (line)", K::Mirror);
                    tool(ui, &mut act, "Mirror 3 Points", K::Seq(&q::MIRROR3PT));
                });
                tool(ui, &mut act, "Orient: 2 Points", K::Orient);
                tool(ui, &mut act, "Orient: 3 Points", K::Seq(&q::ORIENT3PT));
                tool(ui, &mut act, "Shear", K::Seq(&q::SHEAR));
                tool(ui, &mut act, "Bend", K::Seq(&q::BEND));
                tool(ui, &mut act, "Twist", K::Seq(&q::TWIST));
                tool(ui, &mut act, "Taper", K::Seq(&q::TAPER));
                tool(ui, &mut act, "Stretch", K::Seq(&q::STRETCH));
                ui.separator();
                ui.menu_button("Array", |ui| {
                    item(ui, &mut act, "Rectangular", "", ARRAY_PREFILL);
                    tool(ui, &mut act, "Linear", K::ArrayLinear);
                    tool(ui, &mut act, "Polar", K::ArrayPolar);
                    tool(ui, &mut act, "Along Curve", K::Seq(&q::ARRAYCRV));
                });
                ui.menu_button("Align", |ui| {
                    for (label, cmd) in ALIGN {
                        item(ui, &mut act, label, "", Act::Submit(cmd));
                    }
                });
                tool(ui, &mut act, "Distribute", K::Seq(&q::DISTRIBUTE));
                ui.separator();
                tool(ui, &mut act, "Set Points", K::Seq(&q::SETPT));
                item(
                    ui,
                    &mut act,
                    "Box Edit…",
                    "",
                    Act::Prefill(
                        "BoxEdit ",
                        "BoxEdit [x=<size>] [y=<size>] [z=<size>] [uniform] [center] [at=x,y,z] — exact size and position of the selection",
                    ),
                );
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
                tool(ui, &mut act, "Point (Evaluate)", K::Seq(&q::EVALUATE_PT));
                tool(ui, &mut act, "Distance", K::Distance);
                tool(ui, &mut act, "Closest Point", K::Seq(&q::CLOSEST_PT));
                tool(ui, &mut act, "Angle", K::Seq(&q::ANGLE));
                tool(ui, &mut act, "Radius", K::Seq(&q::RADIUS));
                tool(ui, &mut act, "Length", K::OnSel("Length"));
                ui.menu_button("Mass Properties", |ui| {
                    tool(ui, &mut act, "Area", K::OnSel("Area"));
                    tool(ui, &mut act, "Area Centroid", K::Seq(&q::AREA_CENTROID));
                    tool(ui, &mut act, "Volume", K::OnSel("Volume"));
                    tool(ui, &mut act, "Volume Centroid", K::Seq(&q::VOLUME_CENTROID));
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
        if let Some(p) = open_recent {
            self.run_engine(&format!("Open {p}"));
        }
        if clear_recent {
            for p in self.settings.recent() {
                self.settings.remove_recent(&p);
            }
            self.save_settings(true);
        }
    }
}
