//! Toolbars: the tab strip with its command row (Rhino 8 style) and the main
//! sidebar palette. Every button has a tooltip with the command name, what it
//! does, and its alias / keyboard shortcut.

use crate::icons::{self, Icon};
use crate::menus::{ALIGN, ARRAY_PREFILL, KERNEL_TIP, SET_VIEWS};
use crate::tools::ToolKind;
use crate::{theme, Act, FormaApp, SidePanel};
use eframe::egui::{self, Color32, RichText, Stroke, StrokeKind};
use forma_render::DisplayMode;

/// Rhino-style toolbar tabs above the viewports.
pub(crate) const TABS: [&str; 10] = [
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

/// A toolbar button.
#[derive(Clone, Copy)]
struct Item {
    icon: Icon,
    /// Command name (bold first line of the tooltip).
    name: &'static str,
    desc: &'static str,
    /// Alias typed in the command line.
    alias: Option<&'static str>,
    /// Keyboard shortcut.
    key: Option<&'static str>,
    act: Act,
}

fn tool(k: ToolKind) -> Option<Item> {
    Some(Item {
        icon: Icon::Tool(k),
        name: k.name(),
        desc: k.description(),
        alias: k.alias(),
        key: k.shortcut(),
        act: Act::Tool(k),
    })
}

fn named(
    n: &'static str,
    name: &'static str,
    desc: &'static str,
    key: &'static str,
    a: Act,
) -> Option<Item> {
    Some(Item {
        icon: Icon::Named(n),
        name,
        desc,
        alias: None,
        key: (!key.is_empty()).then_some(key),
        act: a,
    })
}

fn tools(ks: &[ToolKind]) -> Vec<Option<Item>> {
    ks.iter().map(|k| tool(*k)).collect()
}

/// The tooltip of a button: name, description, alias and shortcut.
fn tooltip(ui: &mut egui::Ui, it: &Item) {
    ui.set_max_width(320.0);
    ui.label(RichText::new(it.name).strong());
    if !it.desc.is_empty() && it.desc != it.name {
        let mut d = it.desc.chars();
        let first = d.next().map(|c| c.to_uppercase().to_string());
        ui.label(format!("{}{}", first.unwrap_or_default(), d.as_str()));
    }
    let mut keys = Vec::new();
    if let Some(a) = it.alias {
        keys.push(format!("Command: {} or {a}", it.name));
    }
    if let Some(k) = it.key {
        keys.push(format!("Shortcut: {k}"));
    }
    if !keys.is_empty() {
        ui.label(RichText::new(keys.join("  ·  ")).small().color(theme::WEAK));
    }
}

impl FormaApp {
    fn icon_button(&self, ui: &mut egui::Ui, it: &Item, active: bool) -> bool {
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
        icons::paint(ui.painter(), rect.shrink(3.0), it.icon, theme::TEXT);
        resp.on_hover_ui(|ui| tooltip(ui, it)).clicked()
    }

    fn is_running(&self, it: &Item) -> bool {
        matches!((it.icon, self.tool.as_ref()), (Icon::Tool(k), Some(t)) if k == t.kind)
    }

    /// Left sidebar: Rhino's main palette, two columns of tools.
    pub(crate) fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
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
        let mut start = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);
            for (g, kinds) in groups.iter().enumerate() {
                egui::Grid::new(("sidebar", g))
                    .spacing([1.0, 1.0])
                    .show(ui, |ui| {
                        for (k, kind) in kinds.iter().enumerate() {
                            let it = tool(*kind).expect("tool item");
                            if self.icon_button(ui, &it, self.is_running(&it)) {
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

    /// Buttons of a toolbar tab; `None` = separator.
    fn tab_items(&self) -> Vec<Option<Item>> {
        use ToolKind as K;
        let mut v = Vec::new();
        match self.tab {
            0 => {
                v.push(named("New", "New", "New model", "Ctrl+N", Act::Cmd("New")));
                v.push(named(
                    "Open",
                    "Open",
                    "Open a Rhino model",
                    "Ctrl+O",
                    Act::Open,
                ));
                v.push(named("Save", "Save", "Save the model", "Ctrl+S", Act::Save));
                v.push(None);
                v.push(named(
                    "Cut",
                    "Cut",
                    "Cut to the clipboard",
                    "Ctrl+X",
                    Act::Clip("Cut"),
                ));
                v.push(named(
                    "CopyToClipboard",
                    "CopyToClipboard",
                    "Copy to the clipboard",
                    "Ctrl+C",
                    Act::Clip("CopyToClipboard"),
                ));
                v.push(named(
                    "Paste",
                    "Paste",
                    "Paste from the clipboard",
                    "Ctrl+V",
                    Act::Clip("Paste"),
                ));
                v.push(None);
                v.push(named(
                    "Undo",
                    "Undo",
                    "Undo the last action",
                    "Ctrl+Z",
                    Act::Cmd("Undo"),
                ));
                v.push(named("Redo", "Redo", "Redo", "Ctrl+Y", Act::Cmd("Redo")));
                v.push(None);
                v.push(named(
                    "SelAll",
                    "SelAll",
                    "Select all objects",
                    "Ctrl+A",
                    Act::Cmd("SelAll"),
                ));
                v.push(tool(K::OnSel("Delete")));
                v.push(None);
                v.push(tool(K::OnSel("Hide")));
                v.push(named(
                    "Show",
                    "Show",
                    "Show hidden objects",
                    "Ctrl+Alt+H",
                    Act::Cmd("Show"),
                ));
                v.push(tool(K::OnSel("Lock")));
                v.push(named(
                    "Unlock",
                    "Unlock",
                    "Unlock all objects",
                    "Ctrl+Alt+L",
                    Act::Cmd("Unlock"),
                ));
                v.push(None);
                v.push(named(
                    "ZoomExtents",
                    "ZEA",
                    "Zoom extents in all views",
                    "",
                    Act::Submit("zea"),
                ));
                v.push(named(
                    "ZoomSelected",
                    "ZS",
                    "Zoom to the selected objects",
                    "",
                    Act::Submit("zs"),
                ));
                v.push(None);
                v.push(named(
                    "Layer",
                    "Layers",
                    "Layers panel",
                    "",
                    Act::Panel(SidePanel::Layers),
                ));
                v.push(named(
                    "What",
                    "Properties",
                    "Properties panel",
                    "F3",
                    Act::Panel(SidePanel::Properties),
                ));
                v.push(named(
                    "Help",
                    "Help",
                    "Commands, keys and mouse",
                    "",
                    Act::Help,
                ));
            }
            1 => {
                for (n, c) in SET_VIEWS {
                    v.push(named(n, n, "Set the active view", "", Act::Submit(c)));
                }
                v.push(None);
                v.push(named(
                    "ZoomExtents",
                    "ZE",
                    "Zoom extents in the active view",
                    "",
                    Act::Submit("ze"),
                ));
                v.push(named(
                    "ZoomSelected",
                    "ZS",
                    "Zoom to the selected objects",
                    "",
                    Act::Submit("zs"),
                ));
                v.push(named(
                    "Isolate",
                    "MaxViewport",
                    "Maximize / restore the active view (double-click a view title)",
                    "",
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
                    v.push(named(
                        n,
                        m.name(),
                        "Display mode of the active view",
                        "",
                        Act::Mode(m),
                    ));
                }
            }
            3 => {
                for (n, desc, key) in [
                    ("SelAll", "Select all objects", "Ctrl+A"),
                    ("SelNone", "Select none", "Esc"),
                    ("Invert", "Invert the selection", ""),
                    ("SelLast", "Select the last created objects", ""),
                    ("SelCrv", "Select all curves", ""),
                    ("SelMesh", "Select all surfaces / meshes", ""),
                    ("SelPt", "Select all points", ""),
                ] {
                    v.push(named(n, n, desc, key, Act::Cmd(n)));
                }
                v.push(tool(K::OnSel("SelGroup")));
            }
            4 => {
                v.push(tool(K::OnSel("Hide")));
                v.push(named(
                    "Show",
                    "Show",
                    "Show all hidden objects",
                    "Ctrl+Alt+H",
                    Act::Cmd("Show"),
                ));
                v.push(tool(K::OnSel("Isolate")));
                v.push(named(
                    "Unisolate",
                    "Unisolate",
                    "Show everything again",
                    "",
                    Act::Cmd("Unisolate"),
                ));
                v.push(None);
                v.push(tool(K::OnSel("Lock")));
                v.push(named(
                    "Unlock",
                    "Unlock",
                    "Unlock all objects",
                    "Ctrl+Alt+L",
                    Act::Cmd("Unlock"),
                ));
                v.push(None);
                v.push(tool(K::OnSel("Group")));
                v.push(tool(K::OnSel("Ungroup")));
            }
            5 => {
                v.extend(tools(&ToolKind::TRANSFORMS));
                v.push(named(
                    "Array",
                    "Array",
                    "Rectangular array (nx ny nz spacing)",
                    "",
                    ARRAY_PREFILL,
                ));
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
    pub(crate) fn ui_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, name) in TABS.iter().enumerate() {
                let selected = self.tab == i;
                let text = RichText::new(*name).size(12.5);
                let (fill, border) = if selected {
                    (Color32::WHITE, theme::BORDER)
                } else {
                    (theme::STRIP, theme::STRIP)
                };
                let b = egui::Button::new(if selected { text.strong() } else { text })
                    .fill(fill)
                    .stroke(Stroke::new(1.0, border))
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
                        Some(it) => {
                            if self.icon_button(ui, &it, self.is_running(&it)) {
                                act = Some(it.act);
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
                            .on_disabled_hover_text(KERNEL_TIP);
                    }
                }
                if self.tab == 5 {
                    ui.separator();
                    ui.menu_button("Align ▾", |ui| {
                        for (label, cmd) in ALIGN {
                            if ui.button(label).clicked() {
                                act = Some(Act::Submit(cmd));
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
}
