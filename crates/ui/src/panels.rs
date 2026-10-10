//! The right-hand panel: Properties (F3), Layers and Help.

use crate::tools::ToolKind;
use crate::{theme, FormaApp, LogKind, SidePanel};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind};

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

/// Editable values of the Properties panel for one selection and document state.
pub(crate) struct PropEdit {
    key: (Vec<forma_doc::ObjectId>, u64),
    name: String,
    size: [String; 3],
    pos: [String; 3],
    text: String,
    radius: String,
}

impl FormaApp {
    /// Right panel with Properties, Layers and Help tabs.
    pub(crate) fn ui_side(&mut self, ui: &mut egui::Ui) {
        // Long texts are cut, never widen the panel (which would resize and
        // re-render all four views).
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
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
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
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
            ui.label(egui::RichText::new("Command line").strong());
            ui.label("Type a few letters: a list of matching commands appears (↑/↓ choose, Enter / Tab / Space accept).\n↑/↓ on an empty line: recent commands.\nOptions in ( ) can be clicked, or typed (C, U, Distance=10).");
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
        // Editable values, refreshed when the selection or the document changes.
        let key = (sel.iter().map(|o| o.id).collect::<Vec<_>>(), doc.version());
        if self.prop_edit.as_ref().is_none_or(|p| p.key != key) {
            let names: std::collections::BTreeSet<Option<&String>> =
                sel.iter().map(|o| o.name.as_ref()).collect();
            let fmt = |v: f64| crate::tools::round(v).to_string();
            let (size, pos) = match self.sel_info.bbox {
                Some(b) => (
                    [
                        fmt(b.max.x - b.min.x),
                        fmt(b.max.y - b.min.y),
                        fmt(b.max.z - b.min.z),
                    ],
                    [fmt(b.min.x), fmt(b.min.y), fmt(b.min.z)],
                ),
                None => Default::default(),
            };
            self.prop_edit = Some(PropEdit {
                key: key.clone(),
                name: match names.iter().next() {
                    Some(Some(n)) if names.len() == 1 => (*n).clone(),
                    _ => String::new(),
                },
                size,
                pos,
                text: match (sel.len(), &sel[0].geometry) {
                    (1, forma_doc::Geometry::Text(t)) => t.text.clone(),
                    (1, forma_doc::Geometry::Dimension(d)) => d.text.clone().unwrap_or_default(),
                    _ => String::new(),
                },
                radius: match (sel.len(), &sel[0].geometry) {
                    (1, forma_doc::Geometry::Arc(a)) => fmt(a.radius),
                    _ => String::new(),
                },
            });
        }
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

                // Name (Enter or leaving the field applies it).
                let pe = self.prop_edit.as_mut().expect("set above");
                ui.weak("Name");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut pe.name)
                        .desired_width(150.0)
                        .hint_text("(none)"),
                );
                if r.lost_focus() && r.changed()
                    || r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    let n = pe.name.trim();
                    cmd = Some(if n.is_empty() {
                        "SetObjectName \"\"".into()
                    } else {
                        format!("SetObjectName {n}")
                    });
                }
                ui.end_row();
                if kind == "text" || kind == "textdot" || kind == "dimension" {
                    ui.weak("Text");
                    let r = ui.add(egui::TextEdit::singleline(&mut pe.text).desired_width(150.0));
                    if r.lost_focus() && !pe.text.trim().is_empty() {
                        cmd = Some(format!("EditText #{} {}", sel[0].id.0, pe.text.trim()));
                    }
                    ui.end_row();
                }
                if !pe.radius.is_empty() {
                    ui.weak("Radius");
                    let r = ui.add(egui::TextEdit::singleline(&mut pe.radius).desired_width(80.0));
                    if r.lost_focus() {
                        if let (forma_doc::Geometry::Arc(a), Ok(nr)) = (
                            &sel[0].geometry,
                            pe.radius.trim().replace(',', ".").parse::<f64>(),
                        ) {
                            if nr > 0.0 && (nr - a.radius).abs() > 1e-12 {
                                cmd = Some(format!(
                                    "Scale {} {}",
                                    crate::tools::fmt_p(a.center()),
                                    crate::tools::round(nr / a.radius)
                                ));
                            }
                        }
                    }
                    ui.end_row();
                }

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
                    .truncate()
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

                // Size and position of the bounding box, editable (Box Edit).
                if self.sel_info.bbox.is_some() {
                    let u = doc.units.abbreviation();
                    let pe = self.prop_edit.as_mut().expect("set above");
                    let row = |ui: &mut egui::Ui, label: &str, vals: &mut [String; 3]| -> bool {
                        ui.weak(label);
                        let mut out = None;
                        ui.horizontal(|ui| {
                            for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                                let r = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut vals[i])
                                            .desired_width(52.0),
                                    )
                                    .on_hover_text(format!("{label} {axis} ({u}) — Enter applies"));
                                if r.lost_focus() && r.changed() {
                                    out = Some(i);
                                }
                                if r.lost_focus()
                                    && ui.input(|inp| inp.key_pressed(egui::Key::Enter))
                                {
                                    out = Some(i);
                                }
                            }
                        });
                        ui.end_row();
                        out.is_some()
                    };
                    let size_changed = row(ui, "Size", &mut pe.size);
                    let pos_changed = row(ui, "Position", &mut pe.pos);
                    let num = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok();
                    if size_changed {
                        let parts: Vec<String> = ["x", "y", "z"]
                            .iter()
                            .zip(&pe.size)
                            .filter_map(|(a, v)| {
                                num(v).filter(|v| *v > 0.0).map(|v| format!("{a}={v}"))
                            })
                            .collect();
                        if !parts.is_empty() {
                            cmd = Some(format!("BoxEdit {}", parts.join(" ")));
                        }
                    }
                    if pos_changed {
                        if let (Some(x), Some(y), Some(z)) =
                            (num(&pe.pos[0]), num(&pe.pos[1]), num(&pe.pos[2]))
                        {
                            cmd = Some(format!("BoxEdit at={x},{y},{z}"));
                        }
                    }
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
                }
            });
        if let Some(c) = cmd {
            self.run_engine(&c);
        }
    }

    fn ui_layers(&mut self, ui: &mut egui::Ui) {
        let mut cmd: Option<String> = None;
        let mut save_state = false;
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
            let states: Vec<String> = self
                .engine
                .ctx
                .layer_states
                .iter()
                .map(|s| s.0.clone())
                .collect();
            ui.menu_button("States ▾", |ui| {
                if ui.button("Save Layer State…").clicked() {
                    save_state = true;
                    ui.close();
                }
                if !states.is_empty() {
                    ui.separator();
                }
                for n in &states {
                    if ui
                        .button(n)
                        .on_hover_text("Restore which layers are on and locked")
                        .clicked()
                    {
                        cmd = Some(format!("LayerState Restore \"{n}\""));
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Layer states: save and restore which layers are on / locked");
        });
        if save_state {
            self.open_form(&crate::dialogs::SAVE_LAYER_STATE);
        }
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
        let mut form: Option<(&'static crate::dialogs::Form, Vec<String>)> = None;
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
                        ui.separator();
                        if ui.button("Show Only This Layer").clicked() {
                            cmd = Some(format!("OneLayerOn {}", layer.name));
                            ui.close();
                        }
                        if ui.button("New Sub-layer").clicked() {
                            let mut k = 1;
                            while layers
                                .iter()
                                .any(|l| l.name == format!("{}::Layer {k:02}", layer.name))
                            {
                                k += 1;
                            }
                            cmd = Some(format!("Layer {}::Layer {k:02}", layer.name));
                            ui.close();
                        }
                        if ui.button("Rename…").clicked() {
                            form = Some((
                                &crate::dialogs::RENAME_LAYER,
                                vec![layer.name.clone(), layer.name.clone()],
                            ));
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            form = Some((&crate::dialogs::DELETE_LAYER, vec![layer.name.clone()]));
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
        if let Some((f, values)) = form {
            self.open_form_with(f, values);
        }
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
