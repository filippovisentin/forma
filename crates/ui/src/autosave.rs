//! Autosave and crash recovery.
//!
//! While the model has unsaved changes, a copy is written every
//! [`INTERVAL`] to `autosave.3dm` in the settings folder (on a background thread,
//! without touching the open document or its undo history). A successful Save,
//! Open or New and a normal exit delete it, so a copy found at start-up means the
//! last session ended without closing: the app offers to recover it.
//!
//! Panics are appended to `crash.log` in the same folder (the release app has no
//! console to print them to).

use crate::settings::Settings;
use crate::{FormaApp, LogKind};
use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often unsaved changes are copied to the autosave file.
pub(crate) const INTERVAL: Duration = Duration::from_secs(120);

#[derive(Default)]
pub(crate) struct Autosave {
    /// When the last copy was started (or the document was last saved / opened).
    last: Option<Instant>,
    /// Document version of the last copy.
    version: Option<u64>,
    /// The copy being written.
    job: Option<JoinHandle<Result<(), String>>>,
    /// When the last copy was written successfully (for the Graphite title bar).
    written: Option<Instant>,
    /// A copy left by an earlier session, waiting for "Recover?".
    pub(crate) offer: Option<Offer>,
}

#[derive(Clone)]
pub(crate) struct Offer {
    pub(crate) file: PathBuf,
    /// Where the autosaved model was opened from or saved to, if anywhere.
    pub(crate) source: Option<String>,
    /// How long ago the copy was written.
    pub(crate) age: Option<Duration>,
}

/// The settings folder (`FORMA_CONFIG_DIR`, `%APPDATA%\Forma`, `~/.config/forma`).
pub(crate) fn dir() -> Option<PathBuf> {
    Settings::path()?.parent().map(Path::to_path_buf)
}

fn model_file() -> Option<PathBuf> {
    dir().map(|d| d.join("autosave.3dm"))
}

/// Holds the source path of the autosaved model (one line, may be empty).
fn source_file() -> Option<PathBuf> {
    dir().map(|d| d.join("autosave.txt"))
}

/// A copy left by an earlier session, if any.
pub(crate) fn find_offer() -> Option<Offer> {
    let file = model_file().filter(|f| f.is_file())?;
    let source = source_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let age = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok());
    Some(Offer { file, source, age })
}

/// Write `doc` and its source path to the autosave files.
fn write(doc: &forma_doc::Document) -> Result<(), String> {
    let (Some(model), Some(source)) = (model_file(), source_file()) else {
        return Ok(()); // no settings folder: nowhere to autosave
    };
    if let Some(d) = model.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    forma_engine::write_copy(doc, &model).map_err(|e| e.to_string())?;
    std::fs::write(&source, doc.path.as_deref().unwrap_or(""))
        .map_err(|e| format!("{}: {e}", source.display()))
}

/// Delete the autosave files (missing files are fine).
fn remove() {
    for f in [model_file(), source_file()].into_iter().flatten() {
        let _ = std::fs::remove_file(f);
    }
}

/// Append panics to `crash.log` in the settings folder, then run the default hook.
pub(crate) fn install_crash_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(d) = dir() {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let text = format!(
                "--- Forma {} panicked (unix time {secs})\n{info}\n{}\n",
                env!("CARGO_PKG_VERSION"),
                std::backtrace::Backtrace::force_capture()
            );
            let _ = std::fs::create_dir_all(&d);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(d.join("crash.log"))
            {
                use std::io::Write;
                let _ = f.write_all(text.as_bytes());
            }
        }
        default(info);
    }));
}

impl Autosave {
    /// State at start-up, with the copy of an earlier session if one was left.
    pub(crate) fn at_startup() -> Autosave {
        Autosave {
            offer: find_offer(),
            ..Autosave::default()
        }
    }
}

impl FormaApp {
    /// Called every frame: report a finished copy, start a new one when due.
    pub(crate) fn autosave_tick(&mut self) {
        if self
            .autosave
            .job
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            let job = self.autosave.job.take().expect("checked");
            match job.join() {
                Ok(Ok(())) => self.autosave.written = Some(Instant::now()),
                Ok(Err(e)) => self.log(LogKind::Error, format!("autosave failed: {e}")),
                Err(_) => self.log(LogKind::Error, "autosave failed"),
            }
        }
        let version = self.engine.doc().version();
        let due = self.autosave.last.is_none_or(|t| t.elapsed() >= INTERVAL);
        if self.autosave.job.is_some()
            || self.autosave.offer.is_some()
            || !self.is_modified()
            || self.autosave.version == Some(version)
            || !due
        {
            return;
        }
        let doc = self.engine.doc().snapshot();
        self.autosave.last = Some(Instant::now());
        self.autosave.version = Some(version);
        self.autosave.job = Some(std::thread::spawn(move || write(&doc)));
    }

    /// The document was saved, opened or replaced, or the app closes normally:
    /// the copy is no longer needed.
    pub(crate) fn autosave_discard(&mut self) {
        if let Some(job) = self.autosave.job.take() {
            let _ = job.join(); // never delete a file still being written
        }
        self.autosave.last = Some(Instant::now());
        self.autosave.version = None;
        if self.autosave.offer.is_none() {
            remove();
        }
    }

    /// Short save state for the title bar: "Saved", "Autosaved 3 min ago",
    /// "Not saved yet".
    pub(crate) fn save_state(&self) -> String {
        if !self.is_modified() {
            return "Saved".into();
        }
        match self.autosave.written {
            Some(t) if self.autosave.version.is_some() => match t.elapsed().as_secs() / 60 {
                0 => "Autosaved just now".into(),
                m => format!("Autosaved {m} min ago"),
            },
            _ => "Not saved yet".into(),
        }
    }

    /// "Recover the autosaved model?" shown at start-up after an unclean exit.
    pub(crate) fn ui_recover(&mut self, ctx: &egui::Context) {
        let Some(offer) = self.autosave.offer.clone() else {
            return;
        };
        let name = offer
            .source
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .map_or("an untitled model".to_string(), |n| {
                n.to_string_lossy().into_owned()
            });
        let when = offer.age.map_or(String::new(), |a| match a.as_secs() / 60 {
            0 => " less than a minute ago".into(),
            1 => " 1 minute ago".into(),
            m if m < 120 => format!(" {m} minutes ago"),
            m => format!(" {} hours ago", m / 60),
        });
        let mut answer = None;
        egui::Modal::new(egui::Id::new("recover-autosave")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new("Forma").strong());
            ui.add_space(6.0);
            ui.label("Forma did not close normally last time.");
            ui.label(format!("Recover {name} (autosaved{when})?"));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let w = egui::vec2(88.0, 24.0);
                if ui
                    .add(egui::Button::new(RichText::new("Recover").strong()).min_size(w))
                    .clicked()
                {
                    answer = Some(true);
                }
                if ui.add(egui::Button::new("Discard").min_size(w)).clicked() {
                    answer = Some(false);
                }
            });
        });
        let Some(recover) = answer else { return };
        self.autosave.offer = None;
        if !recover {
            remove();
            self.log(LogKind::Normal, "autosaved model discarded");
            return;
        }
        let line = format!("Open {}", offer.file.display());
        match self.engine.run_line(&line) {
            Ok(_) => {
                // The model is back as it was: unsaved, under its own name, and
                // a bare Save asks where to write (never over the autosave).
                self.engine.ctx.doc.path = offer.source.clone();
                self.new_model = None; // the recovered model replaces "New model"
                self.reset_document_view();
                self.saved_by_forma = false;
                self.saved_version = u64::MAX;
                self.saved_at = Instant::now();
                self.autosave.version = None;
                self.log(
                    LogKind::Normal,
                    format!("recovered {name}: save it to keep the changes"),
                );
            }
            Err(e) => self.log(LogKind::Error, format!("could not recover: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_round_trip_and_removal() {
        let dir = std::env::temp_dir().join(format!("forma-autosave-{}", std::process::id()));
        // Settings::path reads FORMA_CONFIG_DIR; this is the only test that sets it.
        std::env::set_var("FORMA_CONFIG_DIR", &dir);
        let mut e = forma_engine::Engine::new();
        e.run_line("Line 0,0,0 10,0,0").unwrap();
        let mut doc = e.doc().snapshot();
        doc.path = Some("C:\\progetti\\stand.3dm".into());
        write(&doc).unwrap();
        let offer = find_offer().expect("a copy");
        assert_eq!(offer.source.as_deref(), Some("C:\\progetti\\stand.3dm"));
        let mut back = forma_engine::Engine::new();
        back.run_line(&format!("Open {}", offer.file.display()))
            .unwrap();
        assert_eq!(back.doc().len(), 1);
        remove();
        assert!(find_offer().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
