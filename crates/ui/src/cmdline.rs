//! The command line docked at the top (Rhino): history, prompt with
//! clickable options, the input field and the autocomplete list.

use crate::complete::Entry;
use crate::tools::OptAction;
use crate::{theme, FormaApp, LogKind, CMD_ID};
use eframe::egui::{self, RichText, Stroke};

/// Rows shown by the autocomplete list.
const AC_ROWS: usize = 12;

/// Where the command area is (View ▸ Command Area): docked above the toolbars
/// (Rhino's default), docked above the status bar, or a floating window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CmdDock {
    Top,
    Bottom,
    Float,
}

impl CmdDock {
    pub(crate) fn name(self) -> &'static str {
        match self {
            CmdDock::Top => "Top",
            CmdDock::Bottom => "Bottom",
            CmdDock::Float => "Float",
        }
    }
    pub(crate) fn parse(s: &str) -> Option<CmdDock> {
        match s.to_ascii_lowercase().as_str() {
            "top" => Some(CmdDock::Top),
            "bottom" => Some(CmdDock::Bottom),
            "float" | "floating" => Some(CmdDock::Float),
            _ => None,
        }
    }
}

/// Heights of the docked command area: smallest, compact, tall, largest.
pub(crate) const CMD_MIN: f32 = 56.0;
pub(crate) const CMD_COMPACT: f32 = 92.0;
pub(crate) const CMD_TALL: f32 = 260.0;
pub(crate) const CMD_MAX: f32 = 700.0;

impl FormaApp {
    /// The docked command area, resizable by dragging its inner edge (the grip
    /// shows where); double-click the grip to switch between compact and tall.
    pub(crate) fn ui_command_panel(&mut self, ui: &mut egui::Ui, top: bool) {
        let panel = if top {
            egui::Panel::top("command")
        } else {
            egui::Panel::bottom("command-bottom")
        };
        let mut panel = panel.resizable(true).size_range(CMD_MIN..=CMD_MAX).frame(
            egui::Frame::NONE
                .fill(theme::panel())
                .inner_margin(egui::Margin::symmetric(4, 3)),
        );
        panel = match self.command_resize.take() {
            Some(h) => panel.exact_size(h),
            None => panel.default_size(self.command_height),
        };
        let r = panel.show(ui, |ui| self.ui_command_line(ui));
        let rect = r.response.rect;
        self.command_height = rect.height();
        // Grip on the edge that drags.
        let y = if top { rect.bottom() } else { rect.top() };
        let grip =
            egui::Rect::from_center_size(egui::pos2(rect.center().x, y), egui::vec2(60.0, 6.0));
        let id = egui::Id::new(("command-grip", top));
        let resp = ui.interact(grip, id, egui::Sense::click());
        let p = ui
            .ctx()
            .layer_painter(egui::LayerId::new(egui::Order::Foreground, id));
        let c = if resp.hovered() {
            theme::accent()
        } else {
            theme::weak()
        };
        for k in -2..=2 {
            p.circle_filled(egui::pos2(rect.center().x + k as f32 * 6.0, y), 1.4, c);
        }
        let resp = resp.on_hover_text(
            "Drag the edge to resize the command area · double-click: compact / tall · \
             View ▸ Command Area: top, bottom or floating",
        );
        if resp.double_clicked() {
            self.command_resize = Some(if self.command_height > (CMD_COMPACT + CMD_TALL) / 2.0 {
                CMD_COMPACT
            } else {
                CMD_TALL
            });
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
    }

    /// The floating command area: a window to move and resize anywhere.
    pub(crate) fn ui_command_window(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let mut open = true;
        egui::Window::new("Command")
            .id(egui::Id::new("command-float"))
            .open(&mut open)
            .resizable(true)
            .collapsible(true)
            .default_size(egui::vec2(720.0, 170.0))
            .default_pos(egui::pos2(
                screen.center().x - 360.0,
                screen.bottom() - 260.0,
            ))
            .min_size(egui::vec2(320.0, 80.0))
            .show(ctx, |ui| {
                ui.set_min_size(ui.available_size());
                self.ui_command_line(ui);
            });
        if !open {
            // Closing the window docks it back at the top (it is never lost).
            self.cmd_dock = CmdDock::Top;
        }
    }

    /// `CommandArea Top|Bottom|Float|Compact|Tall`.
    pub(crate) fn command_area(&mut self, line: &str) {
        let arg = line.split_whitespace().nth(1).unwrap_or("");
        if let Some(d) = CmdDock::parse(arg) {
            self.cmd_dock = d;
            self.log(LogKind::Normal, format!("command area: {}", d.name()));
            return;
        }
        match arg.to_ascii_lowercase().as_str() {
            "compact" => self.command_resize = Some(CMD_COMPACT),
            "tall" => self.command_resize = Some(CMD_TALL),
            _ => self.log(
                LogKind::Error,
                format!(
                    "CommandArea Top|Bottom|Float|Compact|Tall (now {})",
                    self.cmd_dock.name()
                ),
            ),
        }
    }

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
            .fill(theme::input())
            .stroke(Stroke::new(1.0, theme::border()))
            .inner_margin(egui::Margin::symmetric(6, 2));
        // New output always shows, even after scrolling back (Rhino).
        let grew = self.log_count != self.log_seen;
        self.log_seen = self.log_count;
        frame.show(ui, |ui| {
            let area = egui::ScrollArea::vertical()
                .max_height(log_height)
                .min_scrolled_height(log_height)
                .stick_to_bottom(true)
                .auto_shrink([false, false]);
            area.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (kind, line) in &self.log {
                    let t = RichText::new(line).monospace().size(12.5);
                    match kind {
                        LogKind::Normal => ui.label(t),
                        LogKind::Command => ui.label(t.color(theme::weak())),
                        LogKind::Error => ui.label(t.color(theme::error())),
                    };
                }
                if grew {
                    // Scroll to the last line (an offset of f32::MAX overshot and
                    // left the history blank).
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
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
                // Graphite shows the options as chips instead.
                let chips = theme::look() == theme::Look::Graphite;
                ui.label(mono(&if chips {
                    format!("{prompt}:")
                } else {
                    format!("{prompt} (")
                }));
                for (label, a) in &options {
                    let tip = match a {
                        OptAction::Word(w) => format!("{w} (or type {})", &w[..1]),
                        OptAction::Value(n) => format!("Click, or type {n}=value"),
                    };
                    let r = if chips {
                        ui.add(
                            egui::Button::new(mono(label).size(12.0))
                                .corner_radius(egui::CornerRadius::same(4))
                                .stroke(Stroke::new(1.0, theme::border())),
                        )
                    } else {
                        ui.add(egui::Link::new(mono(label).color(theme::accent())))
                    };
                    if r.on_hover_text(tip).clicked() {
                        clicked = Some(*a);
                    }
                }
                if !chips {
                    ui.label(mono("):"));
                }
            }
            let id = egui::Id::new(CMD_ID);
            let hint = self
                .last_command
                .as_ref()
                .filter(|_| self.at_command_prompt())
                .map(|last| {
                    let short: String = last.chars().take(40).collect();
                    RichText::new(format!("Enter or right click: repeat {short}"))
                        .color(theme::weak())
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
                let typing_text = self
                    .tool
                    .as_ref()
                    .is_some_and(|t| t.want() == crate::tools::Want::Text);
                if self.command.ends_with(' ') && !typing_text {
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
        // Below the input, or above it when the command line is docked low.
        let low = below.center().y > ui.ctx().content_rect().center().y;
        let (pos, pivot) = if low {
            (
                below.left_top() + egui::vec2(-2.0, -2.0),
                egui::Align2::LEFT_BOTTOM,
            )
        } else {
            (
                below.left_bottom() + egui::vec2(-2.0, 2.0),
                egui::Align2::LEFT_TOP,
            )
        };
        egui::Area::new(egui::Id::new("command-autocomplete"))
            .order(egui::Order::Foreground)
            .pivot(pivot)
            .fixed_pos(pos)
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
                                theme::accent_bg()
                            } else {
                                theme::hover()
                            };
                            ui.painter().rect_filled(rect, 2.0, fill);
                        }
                        let p = ui.painter();
                        p.text(
                            rect.left_center() + egui::vec2(6.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(13.0),
                            theme::text(),
                        );
                        let help: String = help.chars().take(48).collect();
                        p.text(
                            rect.left_center() + egui::vec2(130.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            help,
                            egui::FontId::proportional(11.5),
                            theme::weak(),
                        );
                        if resp.clicked() {
                            run = Some(name.clone());
                        }
                    }
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("↑↓ choose · Enter / Tab / Space accept · Esc clear")
                            .small()
                            .color(theme::weak()),
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

#[cfg(test)]
mod tests {
    use super::CmdDock;

    #[test]
    fn command_area_places() {
        for d in [CmdDock::Top, CmdDock::Bottom, CmdDock::Float] {
            assert_eq!(CmdDock::parse(d.name()), Some(d));
        }
        assert_eq!(CmdDock::parse("floating"), Some(CmdDock::Float));
        assert_eq!(CmdDock::parse("left"), None);
    }
}
