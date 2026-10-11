//! Python scripts in the app (ADR 0008): `RunPythonScript [file.py]`, and every
//! `.py` file in the `scripts` folder of the settings folder becomes a command
//! named after it (`pannelli_muro.py` → `PannelliMuro`).
//!
//! Scripts call commands through [`forma_script`]; here they run like typed
//! commands (history, view words), and a whole script is one undo step.

use crate::agent_link::VIEW_WORDS;
use crate::{FormaApp, LogKind};
use forma_script::{Host, ObjectInfo};
use std::path::{Path, PathBuf};

/// `<settings folder>/scripts`.
pub(crate) fn scripts_dir() -> Option<PathBuf> {
    forma_engine::agent::config_dir().map(|d| d.join("scripts"))
}

/// Scripts in the scripts folder: command name and file, sorted by name.
pub(crate) fn script_commands() -> Vec<(String, PathBuf)> {
    let Some(dir) = scripts_dir() else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(String, PathBuf)> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("py")))
        .filter_map(|p| {
            let stem = p.file_stem()?.to_str()?;
            let name = forma_script::command_name(stem);
            (!name.is_empty()).then_some((name, p))
        })
        .collect();
    v.sort_by_key(|a| a.0.to_lowercase());
    v
}

/// The script file of a typed command word, if any.
pub(crate) fn script_for(word: &str) -> Option<PathBuf> {
    script_commands()
        .into_iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(word))
        .map(|(_, p)| p)
}

struct AppHost<'a> {
    app: &'a mut FormaApp,
}

impl Host for AppHost<'_> {
    fn run(&mut self, line: &str) -> Result<String, String> {
        let (ok, text) = self.app.run_captured(line);
        let text = text.trim_end().to_string();
        if self.app.confirm.is_some() {
            return Err("the app asks the user to save changes first".into());
        }
        if ok {
            Ok(text)
        } else {
            Err(text)
        }
    }
    fn objects(&mut self) -> Vec<ObjectInfo> {
        forma_script::objects_of(&self.app.engine)
    }
    fn selection(&mut self) -> Vec<u64> {
        self.app
            .engine
            .ctx
            .selection
            .iter()
            .map(|id| id.0)
            .collect()
    }
    fn units(&mut self) -> String {
        self.app.engine.doc().units.abbreviation().to_string()
    }
    fn output(&mut self, text: &str) {
        for line in text.trim_end_matches('\n').lines() {
            self.app.log(LogKind::Normal, line.to_string());
        }
    }
}

impl FormaApp {
    /// Run one line like a typed command (view words included) and return whether
    /// it worked and what it printed (the echo of the line left out).
    pub(crate) fn run_captured(&mut self, line: &str) -> (bool, String) {
        let before = self.log_count;
        let first = line.split_whitespace().next().unwrap_or("").to_lowercase();
        if VIEW_WORDS.contains(&first.as_str()) {
            self.submit(line); // views are the window's, not the engine's
        } else {
            self.run_engine(line);
        }
        let new = (self.log_count - before).min(self.log.len());
        let mut out = String::new();
        let mut ok = true;
        for (kind, msg) in &self.log[self.log.len() - new..] {
            if *kind == LogKind::Command {
                continue;
            }
            ok &= *kind != LogKind::Error;
            out.push_str(msg);
            out.push('\n');
        }
        if self.tool.is_some() {
            // Missing input started an interactive tool: not for a script.
            self.tool = None;
            out.push_str("some input of this command is missing\n");
            ok = false;
        }
        (ok, out)
    }

    /// `RunPythonScript [file.py]` (a file dialog without a path).
    pub(crate) fn run_python_command(&mut self, line: &str) {
        let arg = line
            .split_once(char::is_whitespace)
            .map(|(_, r)| r.trim().trim_matches('"').to_string())
            .filter(|r| !r.is_empty());
        let path = match arg {
            Some(p) => PathBuf::from(p),
            None => {
                let mut d = rfd::FileDialog::new().add_filter("Python script", &["py"]);
                if let Some(dir) = scripts_dir().filter(|d| d.is_dir()) {
                    d = d.set_directory(dir);
                }
                match d.pick_file() {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        self.run_python_file(&path);
    }

    pub(crate) fn run_python_file(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(src) => {
                let name = path.display().to_string();
                let _ = self.run_python_source(&src, &name);
            }
            Err(e) => self.log(LogKind::Error, format!("{}: {e}", path.display())),
        }
    }

    /// Run Python source; everything it changes is one undo step. The error is
    /// the traceback (also logged).
    pub(crate) fn run_python_source(&mut self, src: &str, name: &str) -> Result<(), String> {
        self.log(LogKind::Command, format!("Command: RunPythonScript {name}"));
        self.tool = None;
        self.pending = None;
        let was_empty = self.engine.doc().is_empty();
        let depth = self.engine.doc().undo_depth();
        let result = forma_script::run_python(&mut AppHost { app: self }, src, name);
        self.engine.ctx.doc.group_undo_since(depth);
        match &result {
            Ok(()) => self.log(LogKind::Normal, "script finished"),
            Err(e) => {
                for line in e.lines() {
                    self.log(LogKind::Error, line.to_string());
                }
            }
        }
        if was_empty && !self.engine.doc().is_empty() {
            self.pending_fit = true;
        }
        result
    }

    /// Tools ▸ Scripts ▸ Open Scripts Folder.
    pub(crate) fn open_scripts_folder(&mut self) {
        let Some(dir) = scripts_dir() else { return };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.log(LogKind::Error, format!("{}: {e}", dir.display()));
            return;
        }
        let opener = if cfg!(windows) {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(opener).arg(&dir).spawn() {
            Ok(_) => self.log(
                LogKind::Normal,
                format!(
                    "scripts folder: {} (each .py file there is a command)",
                    dir.display()
                ),
            ),
            Err(e) => self.log(LogKind::Error, format!("{}: {e}", dir.display())),
        }
    }
}
