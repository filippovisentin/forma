//! Open / Save dialogs, recent files and the "Save changes?" guard before New,
//! Open and closing the window.

use crate::{theme, FormaApp, Guarded, LogKind};
use eframe::egui::{self, Key, RichText};
use std::path::Path;

impl FormaApp {
    /// Ask "Save changes?" before `g` when the document has unsaved changes.
    /// Returns `true` when the action has to wait for the answer.
    pub(crate) fn guard(&mut self, g: Guarded) -> bool {
        if self.guard_pass || !self.is_modified() {
            return false;
        }
        self.confirm = Some(g);
        true
    }

    /// Carry out an action after the guard.
    fn proceed(&mut self, ctx: &egui::Context, g: Guarded) {
        match g {
            Guarded::Line(line) => {
                self.guard_pass = true;
                self.run_engine(&line);
                self.guard_pass = false;
            }
            Guarded::OpenDialog => self.open_dialog_now(),
            Guarded::Exit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// The window is asked to close (title bar ×, Alt+F4, File ▸ Exit).
    pub(crate) fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if !self.allow_close && self.is_modified() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Guarded::Exit);
            return;
        }
        self.save_settings(true);
    }

    /// The "Save changes?" dialog (Yes / No / Cancel, like Rhino).
    pub(crate) fn ui_confirm(&mut self, ctx: &egui::Context) {
        let Some(g) = self.confirm.clone() else {
            return;
        };
        let name = self
            .engine
            .doc()
            .path
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .map_or("Untitled".to_string(), |n| n.to_string_lossy().into_owned());
        let mut answer: Option<Option<bool>> = None; // Some(Some(save)) / Some(None) = cancel
        let modal = egui::Modal::new(egui::Id::new("save-changes")).show(ctx, |ui| {
            ui.set_width(330.0);
            ui.label(RichText::new("Forma").strong());
            ui.add_space(6.0);
            ui.label(format!("Save changes to {name}?"));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let w = egui::vec2(88.0, 24.0);
                if ui
                    .add(egui::Button::new(RichText::new("Yes").strong()).min_size(w))
                    .clicked()
                {
                    answer = Some(Some(true));
                }
                if ui.add(egui::Button::new("No").min_size(w)).clicked() {
                    answer = Some(Some(false));
                }
                if ui.add(egui::Button::new("Cancel").min_size(w)).clicked() {
                    answer = Some(None);
                }
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new("Enter = Yes · Esc = Cancel")
                    .small()
                    .color(theme::WEAK),
            );
        });
        if answer.is_none() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)) {
                answer = Some(Some(true));
            } else if modal.should_close() {
                answer = Some(None);
            }
        }
        let Some(answer) = answer else { return };
        self.confirm = None;
        match answer {
            Some(true) => {
                if self.save(false) {
                    self.proceed(ctx, g);
                }
            }
            Some(false) => self.proceed(ctx, g),
            None => self.log(LogKind::Normal, "cancelled"),
        }
    }

    /// File ▸ Open (asks to save changes first).
    pub(crate) fn open_dialog(&mut self) {
        if !self.guard(Guarded::OpenDialog) {
            self.open_dialog_now();
        }
    }

    fn open_dialog_now(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Rhino 3D model", &["3dm"]);
        if let Some(dir) = self.start_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(p) = dialog.pick_file() {
            self.guard_pass = true;
            self.run_engine(&format!("Open {}", p.display()));
            self.guard_pass = false;
        }
    }

    /// Folder for file dialogs: the current file's, else the last one used.
    fn start_dir(&self) -> Option<String> {
        self.engine
            .doc()
            .path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|d| d.display().to_string())
            .or_else(|| self.settings.get("last_dir").map(String::from))
            .filter(|d| !d.is_empty() && Path::new(d).is_dir())
    }

    /// A file was opened or saved: recent files and last folder.
    pub(crate) fn remember_file(&mut self, path: &str) {
        self.settings.push_recent(path);
        if let Some(dir) = Path::new(path).parent() {
            self.settings.set("last_dir", dir.display());
        }
        self.save_settings(true);
    }

    /// Save (or Save As). Returns `true` when the document was written.
    pub(crate) fn save(&mut self, save_as: bool) -> bool {
        let path = self.engine.doc().path.clone();
        let before = self.saved_at;
        if !save_as && self.saved_by_forma && path.is_some() {
            self.run_engine("Save");
            return self.saved_at != before;
        }
        let suggested = path
            .as_deref()
            .and_then(|p| Path::new(p).file_stem())
            .map(|s| format!("{}-forma.3dm", s.to_string_lossy()))
            .unwrap_or_else(|| "senza-titolo.3dm".into());
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Rhino 3D model", &["3dm"])
            .set_file_name(suggested);
        if let Some(dir) = self.start_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(p) = dialog.save_file() else {
            return false;
        };
        if path.is_some() && !self.saved_by_forma {
            self.log(
                LogKind::Normal,
                "note: surfaces imported from Rhino are saved as meshes in this version",
            );
        }
        self.run_engine(&format!("Save {}", p.display()));
        self.saved_at != before
    }
}
