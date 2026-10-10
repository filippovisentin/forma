//! User settings kept between runs in a small `key=value` text file:
//! `%APPDATA%\Forma\settings.txt` on Windows, `$XDG_CONFIG_HOME/forma` or
//! `~/.config/forma` elsewhere (`FORMA_CONFIG_DIR` overrides the folder).
//! A missing or damaged file just means defaults.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// How many recent files the File menu lists.
pub const RECENT_MAX: usize = 10;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    values: BTreeMap<String, String>,
}

impl Settings {
    /// Read `key=value` lines; blank lines, `#` comments and malformed lines are
    /// ignored.
    pub fn parse(text: &str) -> Settings {
        let mut values = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if !k.is_empty() {
                    values.insert(k.to_string(), v.trim().to_string());
                }
            }
        }
        Settings { values }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::from("# Forma settings (written by the app)\n");
        for (k, v) in &self.values {
            out.push_str(k);
            out.push('=');
            out.push_str(v);
            out.push('\n');
        }
        out
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: &str, value: impl ToString) {
        // Values are single lines.
        let v = value.to_string().replace(['\n', '\r'], " ");
        self.values.insert(key.to_string(), v);
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.to_ascii_lowercase().as_str() {
            "1" | "true" | "on" | "yes" => Some(true),
            "0" | "false" | "off" | "no" => Some(false),
            _ => None,
        }
    }

    /// Overwrite `b` with the stored value, if there is a valid one.
    pub fn load_bool(&self, key: &str, b: &mut bool) {
        if let Some(v) = self.get_bool(key) {
            *b = v;
        }
    }

    /// A finite number, if stored.
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.get(key)?.parse::<f32>().ok().filter(|v| v.is_finite())
    }

    /// Recent files, most recent first.
    pub fn recent(&self) -> Vec<String> {
        (0..RECENT_MAX)
            .filter_map(|i| self.get(&format!("recent.{i}")))
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect()
    }

    fn set_recent(&mut self, list: &[String]) {
        for i in 0..RECENT_MAX {
            match list.get(i) {
                Some(p) => self.set(&format!("recent.{i}"), p),
                None => self.remove(&format!("recent.{i}")),
            }
        }
    }

    /// Put a file at the top of the recent list.
    pub fn push_recent(&mut self, path: &str) {
        let mut list = self.recent();
        list.retain(|p| !same_path(p, path));
        list.insert(0, path.to_string());
        list.truncate(RECENT_MAX);
        self.set_recent(&list);
    }

    pub fn remove_recent(&mut self, path: &str) {
        let mut list = self.recent();
        list.retain(|p| !same_path(p, path));
        self.set_recent(&list);
    }

    /// The settings file of this user.
    pub fn path() -> Option<PathBuf> {
        let env = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let dir = if let Some(d) = env("FORMA_CONFIG_DIR") {
            d
        } else if cfg!(windows) {
            env("APPDATA")?.join("Forma")
        } else if let Some(x) = env("XDG_CONFIG_HOME") {
            x.join("forma")
        } else {
            env("HOME")?.join(".config").join("forma")
        };
        Some(dir.join("settings.txt"))
    }

    /// Settings of this user (defaults when the file is missing or unreadable).
    pub fn load() -> Settings {
        Settings::path()
            .and_then(|p| std::fs::read(p).ok())
            .map(|b| Settings::parse(&String::from_utf8_lossy(&b)))
            .unwrap_or_default()
    }

    /// Write the settings file (through a temporary file, so a crash never
    /// leaves half a file).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Settings::path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("txt.tmp");
        std::fs::write(&tmp, self.to_text())?;
        std::fs::rename(&tmp, &path)
    }
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut s = Settings::default();
        s.set("osnap.End", true);
        s.set("panel.side", 280.5);
        s.set("last_dir", "C:\\Users\\Filippo\\Progetti = 2026");
        let back = Settings::parse(&s.to_text());
        assert_eq!(back, s);
        assert_eq!(back.get_bool("osnap.End"), Some(true));
        assert_eq!(back.get_f32("panel.side"), Some(280.5));
        // The first '=' splits key and value.
        assert_eq!(
            back.get("last_dir"),
            Some("C:\\Users\\Filippo\\Progetti = 2026")
        );
    }

    #[test]
    fn damaged_files_give_defaults() {
        let s = Settings::parse(
            "garbage\n=nokey\n# comment\nsnap.grid=maybe\npanel.side=NaN\n\u{0}\u{1}\nok = 1 \n",
        );
        assert_eq!(s.get_bool("snap.grid"), None);
        assert_eq!(s.get_f32("panel.side"), None);
        assert_eq!(s.get_bool("ok"), Some(true));
        let mut b = false;
        s.load_bool("missing", &mut b);
        assert!(!b);
        assert_eq!(Settings::parse("").recent(), Vec::<String>::new());
    }

    #[test]
    fn values_stay_on_one_line() {
        let mut s = Settings::default();
        s.set("x", "a\nb=c");
        assert_eq!(Settings::parse(&s.to_text()).get("x"), Some("a b=c"));
    }

    #[test]
    fn recent_files_are_a_capped_mru_list() {
        let mut s = Settings::default();
        for i in 0..12 {
            s.push_recent(&format!("/m/{i}.3dm"));
        }
        s.push_recent("/m/5.3dm");
        let r = s.recent();
        assert_eq!(r.len(), RECENT_MAX);
        assert_eq!(r[0], "/m/5.3dm");
        assert_eq!(r[1], "/m/11.3dm");
        assert_eq!(r.iter().filter(|p| *p == "/m/5.3dm").count(), 1);
        s.remove_recent("/m/5.3dm");
        assert_eq!(s.recent()[0], "/m/11.3dm");
        assert_eq!(Settings::parse(&s.to_text()).recent(), s.recent());
    }
}
