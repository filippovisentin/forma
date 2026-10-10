//! The menu bar (File, Edit, View, Curve, Surface, Solid, Transform, Tools,
//! Analyze, Help), in the order and wording of Rhino 8.

use crate::snap::SnapKind;
use crate::tools::ToolKind;
use crate::{Act, FormaApp, SidePanel};
use eframe::egui;
use forma_render::DisplayMode;

/// Standard views: (label, command).
pub(crate) const SET_VIEWS: [(&str, &str); 4] = [
    ("Top", "top"),
    ("Front", "front"),
    ("Right", "right"),
    ("Perspective", "perspective"),
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

/// Disabled menu entry for what needs the solid kernel.
pub(crate) const KERNEL_TIP: &str =
    "Needs the solid kernel (OpenCascade), planned for a next version";

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
        let kernel = |ui: &mut egui::Ui, label: &str| {
            ui.add_enabled(false, egui::Button::new(label))
                .on_disabled_hover_text(KERNEL_TIP);
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
                    for (label, cmd) in ALIGN {
                        item(ui, &mut act, label, "", Act::Submit(cmd));
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
