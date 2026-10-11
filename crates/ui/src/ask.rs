//! `Ask`: describe a command in words, a local language model writes it as a
//! Forma Python script (ADR 0008). The code is shown before anything runs: the
//! user runs it, asks the model to fix it, or saves it in the scripts folder,
//! where it becomes a command like any other.
//!
//! The model is reached by URL (Ask window ▸ Model): Ollama, LM Studio or any
//! OpenAI-style server. Assistants on another account or machine connect the
//! other way round, through Tools ▸ Agent Connection (`forma-cli mcp --live`).

use crate::{FormaApp, LogKind};
use eframe::egui::{self, RichText};
use forma_script::ai::{self, AiConfig, Message};
use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

#[derive(Default)]
pub(crate) struct AskWindow {
    pub(crate) open: bool,
    /// What the user asks for (the next message).
    request: String,
    /// The request the current code answers (for the saved file's header).
    asked: String,
    /// The proposed script (editable).
    code: String,
    /// The model's words around the code.
    note: String,
    /// The conversation so far (system prompt first).
    conversation: Vec<Message>,
    job: Option<(Receiver<Result<String, String>>, Instant)>,
    /// Why the last request or run failed.
    error: Option<String>,
    /// The last run's traceback, for "Ask to fix".
    run_error: Option<String>,
    save_name: String,
    show_model: bool,
    /// Put the keyboard focus on the request box (just opened).
    focus: bool,
}

/// The model settings of this user.
pub(crate) fn config(s: &crate::settings::Settings) -> AiConfig {
    let d = AiConfig::default();
    AiConfig {
        url: s
            .get("ai.url")
            .filter(|v| !v.is_empty())
            .unwrap_or(&d.url)
            .to_string(),
        model: s
            .get("ai.model")
            .filter(|v| !v.is_empty())
            .unwrap_or(&d.model)
            .to_string(),
        api_key: s.get("ai.key").filter(|v| !v.is_empty()).map(String::from),
        ..d
    }
}

/// A file name for a request: its first words, in snake case.
fn name_for(request: &str) -> String {
    let words: Vec<String> = request
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .take(3)
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        "my_command".into()
    } else {
        words.join("_")
    }
}

impl FormaApp {
    /// `Ask [request]`: open the window; with a request, send it.
    pub(crate) fn ask_command(&mut self, line: &str) {
        self.ask.open = true;
        self.ask.focus = true;
        let req = line
            .split_once(char::is_whitespace)
            .map(|(_, r)| r.trim().to_string())
            .unwrap_or_default();
        if !req.is_empty() {
            self.ask.request = req;
            self.ask_send();
        }
    }

    fn ask_send(&mut self) {
        let request = self.ask.request.trim().to_string();
        if request.is_empty() || self.ask.job.is_some() {
            return;
        }
        if self.ask.conversation.is_empty() {
            let commands = self.engine.command_list();
            let units = self.engine.doc().units.abbreviation();
            self.ask
                .conversation
                .push(Message::new("system", ai::system_prompt(&commands, units)));
            self.ask.asked = request.clone();
        }
        self.ask
            .conversation
            .push(Message::new("user", request.clone()));
        self.log(LogKind::Command, format!("Command: Ask {request}"));
        self.ask_start();
        self.ask.request.clear();
    }

    /// Send the conversation in the background.
    fn ask_start(&mut self) {
        let cfg = config(&self.settings);
        let msgs = self.ask.conversation.clone();
        let ctx = self.egui_ctx.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(ai::chat(&cfg, &msgs));
            ctx.request_repaint();
        });
        self.ask.job = Some((rx, Instant::now()));
        self.ask.error = None;
    }

    /// Called every frame: pick up the model's reply.
    pub(crate) fn ask_tick(&mut self) {
        let Some((rx, _)) = &self.ask.job else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.ask.job = None;
        match result {
            Ok(reply) => {
                self.ask
                    .conversation
                    .push(Message::new("assistant", reply.clone()));
                match ai::extract_code(&reply) {
                    Some(code) => {
                        self.ask.note = reply
                            .split("```")
                            .enumerate()
                            .filter(|(i, _)| i % 2 == 0)
                            .map(|(_, t)| t.trim())
                            .filter(|t| !t.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n");
                        self.ask.code = code;
                        self.ask.run_error = None;
                        if self.ask.save_name.is_empty() {
                            self.ask.save_name = name_for(&self.ask.asked);
                        }
                        self.log(LogKind::Normal, "Ask: script ready, check it and press Run");
                    }
                    None => {
                        self.ask.note = reply;
                        self.ask.error = Some("the reply has no Python code".into());
                    }
                }
            }
            Err(e) => {
                self.log(LogKind::Error, format!("Ask: {e}"));
                self.ask.error = Some(e);
                // The request can be sent again.
                if let Some(m) = self.ask.conversation.pop() {
                    if self.ask.request.is_empty() {
                        self.ask.request = m.content;
                    }
                }
                if self.ask.conversation.len() == 1 {
                    self.ask.conversation.clear();
                }
            }
        }
    }

    fn ask_run(&mut self) {
        let code = self.ask.code.clone();
        self.ask.run_error = self.run_python_source(&code, "<ask>").err();
    }

    fn ask_fix(&mut self) {
        let Some(err) = self.ask.run_error.take() else {
            return;
        };
        let msg = format!(
            "Running the script gave this error:\n{err}\n\nThe script was:\n```python\n{}\n```\nFix it and answer with the whole corrected script.",
            self.ask.code
        );
        self.ask.conversation.push(Message::new("user", msg));
        self.log(LogKind::Command, "Command: Ask (fix the script)");
        self.ask_start();
    }

    /// Write the script to the scripts folder: it becomes a command.
    fn ask_save(&mut self) {
        let Some(dir) = crate::scripting::scripts_dir() else {
            return;
        };
        let stem: String = self
            .ask
            .save_name
            .trim()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let stem = stem.trim_matches('_').to_string();
        if stem.is_empty() {
            self.ask.error = Some("give the command a name".into());
            return;
        }
        let file = dir.join(format!("{stem}.py"));
        let text = format!(
            "# {} — written by Ask from: {}\n{}\n",
            forma_script::command_name(&stem),
            self.ask.asked.replace('\n', " "),
            self.ask.code.trim_end()
        );
        let result = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, text));
        match result {
            Ok(()) => {
                self.completer = crate::app::completer(&self.engine);
                let cmd = forma_script::command_name(&stem);
                self.log(
                    LogKind::Normal,
                    format!("saved {}: type {cmd} to run it", file.display()),
                );
                self.ask.error = None;
            }
            Err(e) => self.ask.error = Some(format!("{}: {e}", file.display())),
        }
    }

    fn ask_reset(&mut self) {
        let open = self.ask.open;
        self.ask = AskWindow {
            open,
            ..AskWindow::default()
        };
    }

    pub(crate) fn ui_ask(&mut self, ctx: &egui::Context) {
        self.ask_tick();
        if !self.ask.open {
            return;
        }
        let mut open = true;
        let mut send = false;
        let mut run = false;
        let mut fix = false;
        let mut save = false;
        let mut reset = false;
        let mut cfg = config(&self.settings);
        let mut cfg_changed = false;
        egui::Window::new(RichText::new("Ask AI — write a command").strong())
            .id(egui::Id::new("ask-window"))
            .open(&mut open)
            .default_width(560.0)
            .default_pos(egui::pos2(220.0, 140.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                    crate::icons::paint(
                        ui.painter(),
                        r,
                        crate::icons::Icon::Named("Ask"),
                        crate::theme::text(),
                    );
                    ui.label(
                        "Describe what the command should do. A local model writes it as a \
                         Python script; nothing runs until you press Run.",
                    );
                });
                ui.add_space(4.0);
                let busy = self.ask.job.is_some();
                let hint = if self.ask.conversation.is_empty() {
                    "e.g. a staircase of 12 steps, 30 cm deep and 17 cm high, on layer Stairs"
                } else {
                    "Ask for a change to this script, or press New to start over"
                };
                // Enter sends (taken before the text box turns it into a new line).
                let req_id = egui::Id::new("ask-request");
                let enter = ui.memory(|m| m.has_focus(req_id))
                    && (ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                        || ui.input_mut(|i| {
                            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                        }));
                let te = ui.add_enabled(
                    !busy,
                    egui::TextEdit::multiline(&mut self.ask.request)
                        .id(req_id)
                        .hint_text(hint)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if std::mem::take(&mut self.ask.focus) {
                    te.request_focus();
                }
                ui.horizontal(|ui| {
                    let ask = egui::Button::new(
                        RichText::new("  Ask  ").strong().color(egui::Color32::WHITE),
                    )
                    .fill(crate::icons::VIOLET);
                    if ui
                        .add_enabled(!busy && !self.ask.request.trim().is_empty(), ask)
                        .on_hover_text("Send (Enter)")
                        .clicked()
                        || enter
                    {
                        send = true;
                    }
                    if ui.add_enabled(!busy, egui::Button::new("New")).clicked() {
                        reset = true;
                    }
                    if let Some((_, t)) = &self.ask.job {
                        ui.add(egui::Spinner::new());
                        ui.label(format!(
                            "{} is writing… {} s",
                            cfg.model,
                            t.elapsed().as_secs()
                        ));
                        ctx.request_repaint_after(std::time::Duration::from_millis(500));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.toggle_value(&mut self.ask.show_model, "Model ▾");
                    });
                });
                if self.ask.show_model {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        egui::Grid::new("ask-model").num_columns(2).show(ui, |ui| {
                            ui.label("Server URL");
                            cfg_changed |= ui
                                .add(egui::TextEdit::singleline(&mut cfg.url).desired_width(300.0))
                                .on_hover_text(
                                    "Ollama: http://localhost:11434 · LM Studio: http://localhost:1234/v1 · \
                                     any OpenAI-style server: http://host:port/v1",
                                )
                                .changed();
                            ui.end_row();
                            ui.label("Model");
                            cfg_changed |= ui
                                .add(egui::TextEdit::singleline(&mut cfg.model).desired_width(300.0))
                                .on_hover_text("e.g. qwen2.5-coder:7b (ollama pull qwen2.5-coder:7b)")
                                .changed();
                            ui.end_row();
                            ui.label("API key");
                            let mut key = cfg.api_key.clone().unwrap_or_default();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut key)
                                        .password(true)
                                        .hint_text("only if the server asks for one")
                                        .desired_width(300.0),
                                )
                                .changed()
                            {
                                cfg.api_key = Some(key);
                                cfg_changed = true;
                            }
                            ui.end_row();
                        });
                        ui.label(
                            RichText::new(
                                "Claude or another assistant on a different account: turn on \
                                 Tools ▸ Agent Connection and connect it with `forma-cli mcp --live`.",
                            )
                            .small()
                            .color(crate::theme::weak()),
                        );
                    });
                }
                if let Some(e) = &self.ask.error {
                    ui.colored_label(egui::Color32::from_rgb(220, 70, 60), e);
                }
                if !self.ask.note.is_empty() {
                    ui.label(RichText::new(&self.ask.note).italics());
                }
                if !self.ask.code.is_empty() {
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.ask.code)
                                    .code_editor()
                                    .desired_rows(12)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let run_b = egui::Button::new(
                            RichText::new("▶ Run").strong().color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(40, 150, 60));
                        if ui
                            .add_enabled(!busy, run_b)
                            .on_hover_text("Run the script on the model (one Undo reverts it)")
                            .clicked()
                        {
                            run = true;
                        }
                        if ui
                            .add_enabled(!busy && self.ask.run_error.is_some(), egui::Button::new("Ask to fix"))
                            .on_hover_text("Send the error to the model")
                            .clicked()
                        {
                            fix = true;
                        }
                        ui.separator();
                        ui.label("Save as command");
                        ui.add(egui::TextEdit::singleline(&mut self.ask.save_name).desired_width(140.0));
                        if ui
                            .button("Save")
                            .on_hover_text("Write it to the scripts folder: it becomes a command")
                            .clicked()
                        {
                            save = true;
                        }
                    });
                    if !self.ask.save_name.trim().is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "command: {}",
                                forma_script::command_name(self.ask.save_name.trim())
                            ))
                            .small()
                            .color(crate::theme::weak()),
                        );
                    }
                }
            });
        if cfg_changed {
            self.settings.set("ai.url", cfg.url.trim());
            self.settings.set("ai.model", cfg.model.trim());
            self.settings
                .set("ai.key", cfg.api_key.as_deref().unwrap_or("").trim());
        }
        if reset {
            self.ask_reset();
        }
        if send {
            self.ask_send();
        }
        if run {
            self.ask_run();
        }
        if fix {
            self.ask_fix();
        }
        if save {
            self.ask_save();
        }
        if !open {
            self.ask.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_requests() {
        assert_eq!(name_for("a staircase of 12 steps"), "staircase_steps");
        assert_eq!(name_for("Pannelli muro 60x120"), "pannelli_muro_60x120");
        assert_eq!(name_for("?!"), "my_command");
    }

    #[test]
    fn model_settings() {
        let mut s = crate::settings::Settings::default();
        assert_eq!(config(&s).url, ai::DEFAULT_URL);
        s.set("ai.url", "http://localhost:1234/v1");
        s.set("ai.model", "llama3");
        s.set("ai.key", "");
        let c = config(&s);
        assert_eq!(
            (c.url.as_str(), c.model.as_str(), c.api_key),
            ("http://localhost:1234/v1", "llama3", None)
        );
    }
}
