//! The command line docked at the top (Rhino): history, prompt with
//! clickable options, the input field and the autocomplete list.

use crate::complete::Entry;
use crate::tools::OptAction;
use crate::{theme, FormaApp, LogKind, CMD_ID};
use eframe::egui::{self, Color32, RichText, Stroke};

/// Rows shown by the autocomplete list.
const AC_ROWS: usize = 12;

impl FormaApp {
    /// Commands matching the typed word, when the list should be shown.
    pub(crate) fn completions(&self) -> Vec<&Entry> {
        if !self.at_command_prompt() || self.history.is_browsing() {
            return Vec::new();
        }
        self.completer.matches(&self.command, AC_ROWS)
    }

    /// The prompt text and the options to offer.
    fn prompt(&self) -> (String, Vec<(String, OptAction)>) {
        if let (Some(name), Some(t)) = (self.option_edit, self.tool.as_ref()) {
            let cur = t.option_value(name).unwrap_or_default();
            let cur = (cur * 1e6).round() / 1e6;
            return (format!("{name} <{cur}>"), Vec::new());
        }
        if let Some((line, what)) = &self.pending {
            return (format!("{line} — {what} (Enter to run)"), Vec::new());
        }
        if let Some((h, _)) = &self.gumball_typed {
            return (format!("Gumball — {}", h.describe()), Vec::new());
        }
        if self.face_typed {
            return ("Push / pull distance".to_string(), Vec::new());
        }
        match &self.tool {
            Some(t) => (
                t.prompt(),
                t.options()
                    .into_iter()
                    .map(|o| (o.label, o.action))
                    .collect(),
            ),
            None => ("Command".to_string(), Vec::new()),
        }
    }

    /// Command history and prompt, docked at the top like Rhino.
    pub(crate) fn ui_command_line(&mut self, ui: &mut egui::Ui) {
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
                        let t = RichText::new(line).monospace().size(12.5);
                        match kind {
                            LogKind::Normal => ui.label(t),
                            LogKind::Command => ui.label(t.color(theme::WEAK)),
                            LogKind::Error => ui.label(t.color(Color32::from_rgb(190, 30, 30))),
                        };
                    }
                });
        });
        let mut clicked: Option<OptAction> = None;
        let mut edit_rect = egui::Rect::NOTHING;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let (prompt, options) = self.prompt();
            let mono = |t: &str| RichText::new(t).monospace().size(13.0);
            if options.is_empty() {
                ui.label(mono(&format!("{prompt}:")));
            } else {
                // Rhino: `Next point ( Close Undo ):` — each option is a link.
                ui.label(mono(&format!("{prompt} (")));
                for (label, a) in &options {
                    let link = egui::Link::new(mono(label).color(theme::ACCENT));
                    let tip = match a {
                        OptAction::Word(w) => format!("{w} (or type {})", &w[..1]),
                        OptAction::Value(n) => format!("Click, or type {n}=value"),
                    };
                    if ui.add(link).on_hover_text(tip).clicked() {
                        clicked = Some(*a);
                    }
                }
                ui.label(mono("):"));
            }
            let id = egui::Id::new(CMD_ID);
            let hint = self
                .last_command
                .as_ref()
                .filter(|_| self.at_command_prompt())
                .map(|last| {
                    let short: String = last.chars().take(40).collect();
                    RichText::new(format!("Enter or right click: repeat {short}"))
                        .color(Color32::from_gray(160))
                });
            let mut edit = egui::TextEdit::singleline(&mut self.command)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .frame(egui::Frame::NONE)
                .desired_width(f32::INFINITY);
            if let Some(h) = hint {
                edit = edit.hint_text(h);
            }
            let resp = ui.add(edit);
            edit_rect = resp.rect;
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
            if resp.changed() {
                self.history.reset();
                if self.command.ends_with(' ') {
                    let typed = self.command.trim_end().to_string();
                    self.command = typed;
                    let text = self.take_command();
                    self.submit_space(&text);
                } else {
                    self.ac_choice = None;
                }
            }
            if resp.secondary_clicked() && self.command.is_empty() {
                // Rhino: right click on the command line repeats the last command.
                self.enter_action();
            }
        });
        if let Some(a) = clicked {
            self.click_option(a);
        }
        self.ui_autocomplete(ui, edit_rect);
    }

    /// The list of matching commands under the input field.
    fn ui_autocomplete(&mut self, ui: &egui::Ui, below: egui::Rect) {
        let rows: Vec<(String, String)> = self
            .completions()
            .iter()
            .map(|e| (e.name.clone(), e.help.clone()))
            .collect();
        if rows.is_empty() || !below.is_positive() {
            return;
        }
        let mut run: Option<String> = None;
        egui::Area::new(egui::Id::new("command-autocomplete"))
            .order(egui::Order::Foreground)
            .fixed_pos(below.left_bottom() + egui::vec2(-2.0, 2.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(380.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, (name, help)) in rows.iter().enumerate() {
                        let chosen = self.ac_choice == Some(i);
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(380.0, 20.0), egui::Sense::click());
                        if chosen || resp.hovered() {
                            let fill = if chosen {
                                theme::ACCENT_BG
                            } else {
                                Color32::from_rgb(235, 243, 251)
                            };
                            ui.painter().rect_filled(rect, 2.0, fill);
                        }
                        let p = ui.painter();
                        p.text(
                            rect.left_center() + egui::vec2(6.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(13.0),
                            theme::TEXT,
                        );
                        let help: String = help.chars().take(48).collect();
                        p.text(
                            rect.left_center() + egui::vec2(130.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            help,
                            egui::FontId::proportional(11.5),
                            theme::WEAK,
                        );
                        if resp.clicked() {
                            run = Some(name.clone());
                        }
                    }
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("↑↓ choose · Enter / Tab / Space accept · Esc clear")
                            .small()
                            .color(theme::WEAK),
                    );
                });
            });
        if let Some(name) = run {
            self.command.clear();
            self.ac_choice = None;
            self.submit(&name);
            self.focus_command = true;
        }
    }
}
