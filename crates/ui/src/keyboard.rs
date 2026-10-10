//! Keyboard: shortcuts, typing into the command line from anywhere (Rhino),
//! autocomplete keys and history recall.

use crate::{Act, FormaApp, SidePanel, CMD_ID};
use eframe::egui::{self, Key, Modifiers};

impl FormaApp {
    pub(crate) fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let cmd_id = egui::Id::new(CMD_ID);
        let focused = ctx.memory(|m| m.focused());
        let cmd_focused = focused == Some(cmd_id);
        let other_focused = focused.is_some() && !cmd_focused;

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.cancel();
        }
        if !other_focused {
            self.command_line_keys(ctx);
        }
        // Enter in the command line submits and keeps the focus there.
        if cmd_focused && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            let text = self.take_command();
            self.submit(&text);
            self.focus_command = true;
        }
        if self.command.is_empty() && !other_focused {
            self.shortcuts(ctx);
        }
        let (save_as, save, open, new) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::N),
            )
        });
        if save_as {
            self.save(true);
        } else if save {
            self.save(false);
        }
        if open {
            self.open_dialog();
        }
        if new {
            self.run_engine("New");
        }
        let (f10, f11) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F10),
                i.consume_key(Modifiers::NONE, Key::F11),
            )
        });
        if f10 {
            self.points_on();
        }
        if f11 {
            self.points_off();
        }
        let (f3, f7, f8, f9) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F3),
                i.consume_key(Modifiers::NONE, Key::F7),
                i.consume_key(Modifiers::NONE, Key::F8),
                i.consume_key(Modifiers::NONE, Key::F9),
            )
        });
        if f3 {
            self.side_panel = SidePanel::Properties;
        }
        if f8 {
            self.snap.ortho = !self.snap.ortho;
        }
        if f9 {
            self.snap.grid = !self.snap.grid;
        }
        if f7 {
            if let Some(r) = self.renderer.as_mut() {
                r.show_grid = !r.show_grid;
            }
            self.dirty_all();
        }

        // Keystrokes typed while nothing has focus go to the command line (Rhino).
        if focused.is_none() {
            let mut typed = String::new();
            let mut enter = false;
            ctx.input_mut(|i| {
                i.events.retain(|e| match e {
                    egui::Event::Text(t) => {
                        typed.push_str(t);
                        false
                    }
                    egui::Event::Key {
                        key: Key::Enter,
                        pressed: true,
                        ..
                    } => {
                        enter = true;
                        false
                    }
                    _ => true,
                });
            });
            for ch in typed.chars() {
                if ch == ' ' {
                    let text = self.take_command();
                    self.submit_space(&text);
                } else {
                    self.command.push(ch);
                    self.history.reset();
                    self.ac_choice = None;
                    self.focus_command = true;
                }
            }
            if enter {
                let text = self.take_command();
                self.submit(&text);
            }
        }
    }

    /// The command line text, replaced by the autocomplete row chosen with the
    /// arrow keys, if any; the line is left empty.
    pub(crate) fn take_command(&mut self) -> String {
        let chosen = self
            .ac_choice
            .and_then(|i| self.completions().get(i).map(|e| e.name.clone()));
        self.ac_choice = None;
        let text = std::mem::take(&mut self.command);
        chosen.unwrap_or(text)
    }

    /// Up / Down / Tab: choose in the autocomplete list, or recall recent
    /// commands on an empty line.
    fn command_line_keys(&mut self, ctx: &egui::Context) {
        let (up, down, tab) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Tab),
            )
        });
        if !(up || down || tab) {
            return;
        }
        let n = self.completions().len();
        if n > 0 && !self.history.is_browsing() {
            if up {
                self.ac_choice = Some(self.ac_choice.map_or(n - 1, |c| (c + n - 1) % n));
            }
            if down {
                self.ac_choice = Some(self.ac_choice.map_or(0, |c| (c + 1) % n));
            }
            if tab {
                let i = self.ac_choice.unwrap_or(0);
                if let Some(e) = self.completions().get(i) {
                    self.command = e.name.clone();
                }
                self.ac_choice = None;
                self.focus_command = true;
            }
            return;
        }
        let recall =
            self.at_command_prompt() && (self.command.is_empty() || self.history.is_browsing());
        if recall && (up || down) {
            let item = if up {
                self.history.up()
            } else {
                self.history.down()
            };
            self.command = item.unwrap_or_default().to_string();
            self.focus_command = true;
        }
    }

    /// Shortcuts that work while the command line is empty.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let mut actions: Vec<&str> = Vec::new();
        ctx.input_mut(|i| {
            let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
            let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
            for (m, k, a) in [
                (Modifiers::COMMAND, Key::Z, "Undo"),
                (Modifiers::COMMAND, Key::Y, "Redo"),
                (Modifiers::COMMAND, Key::A, "SelAll"),
                (Modifiers::NONE, Key::Delete, "Delete"),
                (ctrl_alt, Key::H, "Show"),
                (ctrl_alt, Key::L, "Unlock"),
                (ctrl_shift, Key::G, "Ungroup"),
                (Modifiers::COMMAND, Key::H, "Hide"),
                (Modifiers::COMMAND, Key::L, "Lock"),
                (Modifiers::COMMAND, Key::G, "Group"),
                (Modifiers::COMMAND, Key::J, "Join"),
                (Modifiers::NONE, Key::F1, "Help"),
            ] {
                if i.consume_key(m, k) {
                    actions.push(a);
                }
            }
            // Copy / cut / paste arrive as clipboard events, not keys.
            i.events.retain(|e| match e {
                egui::Event::Copy => {
                    actions.push("CopyToClipboard");
                    false
                }
                egui::Event::Cut => {
                    actions.push("Cut");
                    false
                }
                egui::Event::Paste(_) => {
                    actions.push("Paste");
                    false
                }
                _ => true,
            });
        });
        for a in actions {
            match a {
                "Undo" | "Redo" | "SelAll" | "Show" | "Unlock" | "Paste" => self.run_engine(a),
                "Delete" if self.has_selection() && self.tool.is_none() => self.run_engine(a),
                "Help" => self.act(ctx, Act::Help),
                "Hide" | "Lock" | "Group" | "Ungroup" | "Join" | "CopyToClipboard" | "Cut"
                    if self.tool.is_none() =>
                {
                    self.act(ctx, Act::Clip(a));
                }
                _ => {}
            }
        }
    }
}
