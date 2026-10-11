//! What AI agents see of a model, and the line format of the live link between
//! the MCP server (`forma-cli mcp --live`) and the desktop app.
//!
//! The live link is a localhost TCP connection. Each request is one line:
//! `<token> TAB <verb> TAB <escaped text>`, each reply one line:
//! `ok|error TAB <escaped text>`. Text is escaped so that it fits on one line
//! ([`escape`] / [`unescape`]). The token is a random secret the app writes to a
//! file only the user can read; requests without it are refused.

use crate::Engine;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Verbs of the live link.
pub const RUN: &str = "run";
pub const INSPECT: &str = "inspect";
/// The app writes a copy of its model to a temporary file and answers its path.
pub const SNAPSHOT: &str = "snapshot";

/// A plain-text summary of the document for an agent: units, layers, the
/// objects (up to `limit`, with id, kind, layer, name and bounding box) and the
/// selection.
pub fn inspect(engine: &Engine, limit: usize) -> String {
    let doc = engine.doc();
    let mut s = String::new();
    let units = doc.units.abbreviation();
    let _ = writeln!(
        s,
        "file: {}",
        doc.path.as_deref().unwrap_or("(not saved yet)")
    );
    let _ = writeln!(
        s,
        "units: {units}, absolute tolerance {}",
        doc.absolute_tolerance
    );
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for o in doc.objects() {
        *kinds.entry(o.geometry.kind()).or_default() += 1;
    }
    let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{n} {k}")).collect();
    let _ = writeln!(
        s,
        "objects: {}{}",
        doc.len(),
        if kinds.is_empty() {
            String::new()
        } else {
            format!(" ({})", kinds.join(", "))
        }
    );
    let current = doc.layer(doc.current_layer).name.clone();
    let _ = writeln!(s, "layers (current: {current}):");
    for (i, l) in doc.layers.iter().enumerate() {
        let n = doc.objects().filter(|o| o.layer.0 == i).count();
        let _ = writeln!(
            s,
            "  {} — colour {},{},{}{}{}, {n} object(s)",
            l.name,
            l.color[0],
            l.color[1],
            l.color[2],
            if l.visible { "" } else { ", hidden" },
            if l.locked { ", locked" } else { "" }
        );
    }
    let f = |v: f64| {
        let r = (v * 1000.0).round() / 1000.0;
        if r == 0.0 {
            "0".to_string()
        } else {
            format!("{r}")
        }
    };
    let total = doc.len();
    if total > 0 {
        let _ = writeln!(s, "objects (id kind layer [name] min .. max):");
    }
    for o in doc.objects().take(limit) {
        let b = o.geometry.bounding_box();
        let mut flags = String::new();
        if o.hidden {
            flags.push_str(" hidden");
        }
        if o.locked {
            flags.push_str(" locked");
        }
        if let Some(g) = o.group {
            let _ = write!(flags, " group {g}");
        }
        let _ = writeln!(
            s,
            "  #{} {} {}{} {},{},{} .. {},{},{}{flags}",
            o.id.0,
            o.geometry.kind(),
            doc.layer(o.layer).name,
            o.name
                .as_deref()
                .map_or(String::new(), |n| format!(" \"{n}\"")),
            f(b.min.x),
            f(b.min.y),
            f(b.min.z),
            f(b.max.x),
            f(b.max.y),
            f(b.max.z),
        );
    }
    if total > limit {
        let _ = writeln!(s, "  … {} more", total - limit);
    }
    let sel: Vec<String> = engine
        .ctx
        .selection
        .iter()
        .map(|id| format!("#{}", id.0))
        .collect();
    let _ = writeln!(
        s,
        "selection: {}",
        if sel.is_empty() {
            "none".to_string()
        } else {
            sel.join(" ")
        }
    );
    s
}

/// The per-user settings folder: `FORMA_CONFIG_DIR`, else `%APPDATA%\Forma` on
/// Windows, `$XDG_CONFIG_HOME/forma` or `~/.config/forma` elsewhere (the same
/// folder as the app's settings).
pub fn config_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = env("FORMA_CONFIG_DIR") {
        Some(d)
    } else if cfg!(windows) {
        Some(env("APPDATA")?.join("Forma"))
    } else if let Some(x) = env("XDG_CONFIG_HOME") {
        Some(x.join("forma"))
    } else {
        Some(env("HOME")?.join(".config").join("forma"))
    }
}

/// Where the app writes the port and token of the live link while it is on.
pub fn link_file() -> Option<std::path::PathBuf> {
    config_dir().map(|d| d.join("agent.txt"))
}

/// A random 128-bit secret as 32 hex digits (from the OS-seeded hasher keys).
pub fn new_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let half = |salt: u64| {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u32(std::process::id());
        h.write_u64(salt);
        h.finish()
    };
    format!("{:016x}{:016x}", half(1), half(2))
}

/// Make text fit on one line: `\` → `\\`, newline → `\n`, tab → `\t`.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Inverse of [`escape`].
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// A request line (without the trailing newline).
pub fn request_line(token: &str, verb: &str, text: &str) -> String {
    format!("{token}\t{verb}\t{}", escape(text))
}

/// Split a request line into token, verb and text.
pub fn parse_request(line: &str) -> Option<(&str, &str, String)> {
    let mut parts = line.trim_end_matches(['\r', '\n']).splitn(3, '\t');
    let token = parts.next()?;
    let verb = parts.next()?;
    let text = unescape(parts.next().unwrap_or(""));
    Some((token, verb, text))
}

/// A reply line (without the trailing newline).
pub fn reply_line(ok: bool, text: &str) -> String {
    format!("{}\t{}", if ok { "ok" } else { "error" }, escape(text))
}

/// Split a reply line into success and text.
pub fn parse_reply(line: &str) -> Option<(bool, String)> {
    let (status, text) = line.trim_end_matches(['\r', '\n']).split_once('\t')?;
    match status {
        "ok" => Some((true, unescape(text))),
        "error" => Some((false, unescape(text))),
        _ => None,
    }
}

/// Compare two secrets without stopping at the first difference.
pub fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Run a script for an agent: every line, stopping at the first error, with the
/// output of each command. Returns `(all succeeded, transcript)`.
pub fn run_script(engine: &mut Engine, script: &str) -> (bool, String) {
    let mut out = String::new();
    for line in script.split(['\n', ';']) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let _ = writeln!(out, "> {line}");
        match engine.run_line(line) {
            Ok(msg) => {
                if !msg.is_empty() {
                    let _ = writeln!(out, "{msg}");
                }
            }
            Err(e) => {
                let _ = writeln!(out, "error: {e}");
                let _ = writeln!(out, "(stopped; the lines after this one did not run)");
                return (false, out);
            }
        }
    }
    (true, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_round_trips() {
        for s in [
            "",
            "a",
            "Line 0,0 1,0\nBox 0,0 1,1 1",
            "C:\\dir\\a b.3dm",
            "x\ty\\n",
        ] {
            assert_eq!(unescape(&escape(s)), s);
            assert!(!escape(s).contains('\n'));
        }
        let line = request_line("secret", RUN, "Line 0,0 1,0\nSelAll");
        let (t, v, text) = parse_request(&line).unwrap();
        assert_eq!(
            (t, v, text.as_str()),
            ("secret", RUN, "Line 0,0 1,0\nSelAll")
        );
        assert_eq!(
            parse_reply(&reply_line(false, "no\nway")),
            Some((false, "no\nway".into()))
        );
        assert_eq!(parse_reply("garbage"), None);
    }

    #[test]
    fn tokens_are_random_hex() {
        let a = new_token();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, new_token());
    }

    #[test]
    fn tokens_compare_exactly() {
        assert!(same_token("abc123", "abc123"));
        assert!(!same_token("abc123", "abc124"));
        assert!(!same_token("abc", "abc123"));
    }

    #[test]
    fn scripts_report_each_command_and_stop_at_errors() {
        let mut e = Engine::new();
        let (ok, out) = run_script(
            &mut e,
            "New cm\nBox 0,0,0 10,10,0 10\nFrobnicate\nSphere 0,0,0 1",
        );
        assert!(!ok);
        assert!(out.contains("> Box 0,0,0 10,10,0 10"), "{out}");
        assert!(out.contains("error: unknown command"), "{out}");
        assert_eq!(e.doc().len(), 1, "the sphere after the error did not run");
        let summary = inspect(&e, 50);
        assert!(summary.contains("units: cm"), "{summary}");
        assert!(summary.contains("#1 polysurface Default"), "{summary}");
        assert!(summary.contains("10,10,10"), "{summary}");
        assert!(summary.contains("selection: none"), "{summary}");
    }
}
