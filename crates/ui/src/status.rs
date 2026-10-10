//! The bottom bars: Rhino's Osnap bar and the status bar (coordinates,
//! units, current layer, Grid Snap / Ortho / Planar / Osnap / SmartTrack /
//! Gumball panes, selection filter).

use crate::snap::SnapKind;
use crate::tools::Tool;
use crate::{theme, FormaApp};
use eframe::egui::{self, Color32, Stroke, StrokeKind};
use forma_doc::LengthUnit;

impl FormaApp {
    /// Rhino's Osnap bar: one checkbox per object snap, plus Project and Disable.
    pub(crate) fn ui_osnap(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn ui_status(&mut self, ui: &mut egui::Ui) {
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
                .truncate()
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
