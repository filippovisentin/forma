//! The "New model" dialog: units for a new document, shown at start-up (unless
//! turned off) and for File ▸ New, Ctrl+N or a bare `New` on the command line.

use crate::{theme, FormaApp};
use eframe::egui::{self, Key, RichText};

/// Units offered, as `New` arguments, with their labels.
pub(crate) const UNITS: [(&str, &str, &str); 5] = [
    ("mm", "Millimetres", "mm — joinery, furniture details"),
    ("cm", "Centimetres", "cm — interiors, stands"),
    ("m", "Metres", "m — buildings, sites"),
    ("in", "Inches", "in"),
    ("ft", "Feet", "ft"),
];

/// Settings keys.
const LAST_UNITS: &str = "new.units";
const ASK_AT_START: &str = "start.ask_units";

pub(crate) struct NewDialog {
    /// Index into [`UNITS`].
    choice: usize,
    /// Shown at start-up (offers Open too, and "ask at every start").
    at_start: bool,
    ask_at_start: bool,
    /// Opened this frame: the key that opened it (Enter on the command line)
    /// must not also confirm it.
    fresh: bool,
}

impl FormaApp {
    /// The dialog to show at start-up, unless the user turned it off.
    pub(crate) fn start_dialog(&self) -> Option<NewDialog> {
        let ask = self.settings.get_bool(ASK_AT_START).unwrap_or(true);
        ask.then(|| self.new_dialog(true))
    }

    pub(crate) fn new_dialog(&self, at_start: bool) -> NewDialog {
        let last = self.settings.get(LAST_UNITS).unwrap_or("cm");
        NewDialog {
            choice: UNITS.iter().position(|u| u.0 == last).unwrap_or(1),
            at_start,
            ask_at_start: self.settings.get_bool(ASK_AT_START).unwrap_or(true),
            fresh: true,
        }
    }

    /// Units of the new document at start-up when the dialog is turned off.
    pub(crate) fn default_units(&self) -> String {
        let last = self.settings.get(LAST_UNITS).unwrap_or("cm");
        let known = UNITS.iter().any(|u| u.0 == last);
        if known { last } else { "cm" }.to_string()
    }

    pub(crate) fn ui_new_dialog(&mut self, ctx: &egui::Context) {
        if self.autosave.offer.is_some() || self.confirm.is_some() {
            return; // recovery and "Save changes?" come first
        }
        let Some(d) = self.new_model.as_mut() else {
            return;
        };
        let mut create = false;
        let mut open = false;
        let modal = egui::Modal::new(egui::Id::new("new-model")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(RichText::new("New model").strong().size(15.0));
            ui.add_space(4.0);
            ui.label(RichText::new("Units of the new model").color(theme::weak()));
            ui.add_space(6.0);
            for (i, (_, name, hint)) in UNITS.iter().enumerate() {
                let r = ui.radio_value(&mut d.choice, i, *name);
                if r.double_clicked() {
                    create = true;
                }
                r.on_hover_text(*hint);
            }
            ui.add_space(8.0);
            if d.at_start {
                ui.checkbox(&mut d.ask_at_start, "Ask at every start");
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                let w = egui::vec2(96.0, 26.0);
                if ui
                    .add(egui::Button::new(RichText::new("Create").strong()).min_size(w))
                    .clicked()
                {
                    create = true;
                }
                if d.at_start
                    && ui
                        .add(egui::Button::new("Open file…").min_size(w))
                        .clicked()
                {
                    open = true;
                }
            });
            ui.label(
                RichText::new("Enter = Create · ↑↓ change · Esc = centimetres")
                    .small()
                    .color(theme::weak()),
            );
        });
        let fresh = std::mem::take(&mut d.fresh);
        ctx.input_mut(|i| {
            if fresh {
                return;
            }
            if i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) {
                d.choice = (d.choice + 1) % UNITS.len();
            }
            if i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) {
                d.choice = (d.choice + UNITS.len() - 1) % UNITS.len();
            }
            if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                create = true;
            }
        });
        let cancelled = !create && !open && modal.should_close();
        if !(create || open || cancelled) {
            return;
        }
        let d = self.new_model.take().expect("shown");
        if d.at_start {
            self.settings.set(ASK_AT_START, d.ask_at_start);
            self.save_settings(true);
        }
        if open {
            self.open_dialog();
            return;
        }
        if cancelled {
            // At start-up the default centimetre model is already open.
            return;
        }
        let unit = UNITS[d.choice].0;
        self.settings.set(LAST_UNITS, unit);
        self.save_settings(true);
        self.run_engine(&format!("New {unit}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_unit_is_accepted_by_new() {
        let mut e = forma_engine::Engine::new();
        for (arg, _, _) in UNITS {
            e.run_line(&format!("New {arg}")).unwrap();
            assert_eq!(e.doc().units.abbreviation(), arg);
        }
    }
}
