//! Command-line autocomplete (Rhino: a list of matching command names while
//! typing) and command history recall.

/// A command name offered by the autocomplete list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub help: String,
}

/// Commands that only exist in the UI (views, display modes, toggles).
pub const UI_COMMANDS: [(&str, &str); 20] = [
    ("ZE", "Zoom extents in the active view"),
    ("ZEA", "Zoom extents in all views"),
    ("ZS", "Zoom to the selected objects"),
    ("ZoomExtents", "Zoom extents in all views"),
    ("ZoomSelected", "Zoom to the selected objects"),
    ("Top", "Set the active view to Top"),
    ("Front", "Set the active view to Front"),
    ("Right", "Set the active view to Right"),
    ("Perspective", "Set the active view to Perspective"),
    ("Wireframe", "Display mode of the active view"),
    ("Shaded", "Display mode of the active view"),
    ("Ghosted", "Display mode of the active view"),
    ("XRay", "Display mode of the active view"),
    ("MaxViewport", "Maximize / restore the active view"),
    ("Help", "Commands, keys and mouse"),
    ("Ortho", "Toggle Ortho (F8)"),
    ("GridSnap", "Toggle grid snap (F9)"),
    ("Planar", "Toggle Planar mode"),
    ("Osnap", "Toggle object snaps"),
    ("Gumball", "Toggle the gumball"),
];

/// Sorted, de-duplicated command names with a one-line description.
#[derive(Debug, Default)]
pub struct Completer {
    entries: Vec<Entry>,
}

impl Completer {
    pub fn new(items: impl IntoIterator<Item = (String, String)>) -> Completer {
        let mut entries: Vec<Entry> = Vec::new();
        for (name, help) in items {
            if name.is_empty() || name.contains(char::is_whitespace) {
                continue;
            }
            match entries
                .iter_mut()
                .find(|e| e.name.eq_ignore_ascii_case(&name))
            {
                // Keep the first description, but fill in a missing one.
                Some(e) if e.help.is_empty() => e.help = help,
                Some(_) => {}
                None => entries.push(Entry { name, help }),
            }
        }
        entries.sort_by_key(|e| e.name.to_ascii_lowercase());
        Completer { entries }
    }

    /// Commands matching what was typed: names starting with it first, then
    /// names containing it; alphabetical within each group, at most `max`.
    pub fn matches(&self, typed: &str, max: usize) -> Vec<&Entry> {
        let t = typed.trim().to_ascii_lowercase();
        if t.is_empty() || t.contains(char::is_whitespace) {
            return Vec::new();
        }
        let mut found: Vec<(u8, &Entry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let n = e.name.to_ascii_lowercase();
                if n == t {
                    Some((0, e))
                } else if n.starts_with(&t) {
                    Some((1, e))
                } else if n.contains(&t) {
                    Some((2, e))
                } else {
                    None
                }
            })
            .collect();
        found.sort_by_key(|(g, _)| *g); // stable: alphabetical inside a group
        found.into_iter().take(max).map(|(_, e)| e).collect()
    }

    /// Exact (case-insensitive) name of a listed command.
    pub fn exact(&self, typed: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(typed.trim()))
    }

    /// What Enter / Space should run for a typed word: the word itself when it
    /// is a known command or alias (`is_known`), otherwise the first command
    /// whose name starts with it (Rhino completes `Polyl` to `Polyline`).
    pub fn resolve(&self, typed: &str, is_known: impl Fn(&str) -> bool) -> Option<String> {
        let t = typed.trim();
        if t.is_empty() || t.contains(char::is_whitespace) || is_known(t) {
            return None;
        }
        let lower = t.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| e.name.to_ascii_lowercase().starts_with(&lower))
            .map(|e| e.name.clone())
    }
}

/// Recently run commands, recalled with Up / Down on an empty command line.
#[derive(Debug, Default)]
pub struct History {
    items: Vec<String>,
    /// Position while browsing (index into `items`).
    pos: Option<usize>,
}

const HISTORY_MAX: usize = 50;

impl History {
    pub fn push(&mut self, cmd: &str) {
        let cmd = cmd.trim();
        self.pos = None;
        if cmd.is_empty() {
            return;
        }
        self.items.retain(|c| !c.eq_ignore_ascii_case(cmd));
        self.items.push(cmd.to_string());
        if self.items.len() > HISTORY_MAX {
            self.items.remove(0);
        }
    }

    pub fn is_browsing(&self) -> bool {
        self.pos.is_some()
    }

    /// Stop browsing (the user typed something else).
    pub fn reset(&mut self) {
        self.pos = None;
    }

    /// Older entry (Up).
    pub fn up(&mut self) -> Option<&str> {
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        let p = match self.pos {
            Some(p) => p.saturating_sub(1),
            None => n - 1,
        };
        self.pos = Some(p);
        Some(&self.items[p])
    }

    /// Newer entry (Down); `None` past the newest (back to an empty line).
    pub fn down(&mut self) -> Option<&str> {
        let p = self.pos? + 1;
        if p >= self.items.len() {
            self.pos = None;
            return None;
        }
        self.pos = Some(p);
        Some(&self.items[p])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completer() -> Completer {
        Completer::new(
            [
                "Line",
                "Polyline",
                "PlanarSrf",
                "Polygon",
                "Layer",
                "SelLast",
                "line",
            ]
            .iter()
            .map(|n| (n.to_string(), format!("{n} help"))),
        )
    }

    #[test]
    fn prefix_matches_come_first() {
        let c = completer();
        let names: Vec<&str> = c.matches("l", 10).iter().map(|e| e.name.as_str()).collect();
        // Prefix matches alphabetically, then names containing "l".
        assert_eq!(
            names,
            vec![
                "Layer",
                "Line",
                "PlanarSrf",
                "Polygon",
                "Polyline",
                "SelLast"
            ]
        );
        let names: Vec<&str> = c
            .matches("POLY", 10)
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, vec!["Polygon", "Polyline"]);
        assert_eq!(c.matches("line", 10)[0].name, "Line"); // exact first
        assert!(c.matches("", 10).is_empty());
        assert!(c.matches("line 0,0", 10).is_empty());
        assert_eq!(c.matches("l", 2).len(), 2);
    }

    #[test]
    fn duplicates_are_merged() {
        let c = completer();
        assert_eq!(c.matches("line", 10).len(), 2); // Line + Polyline, no "line"
        assert!(c.exact("LINE").is_some());
    }

    #[test]
    fn enter_completes_unknown_prefixes_only() {
        let c = completer();
        let known = |w: &str| ["l", "pl", "line"].contains(&w.to_ascii_lowercase().as_str());
        assert_eq!(c.resolve("polyl", known), Some("Polyline".to_string()));
        assert_eq!(c.resolve("L", known), None); // an alias runs as typed
        assert_eq!(c.resolve("zzz", known), None);
        assert_eq!(c.resolve("Line 0,0,0", known), None);
    }

    #[test]
    fn history_browses_back_and_forth() {
        let mut h = History::default();
        assert_eq!(h.up(), None);
        h.push("Line");
        h.push("Circle");
        h.push("line"); // moves to the end, no duplicate
        assert_eq!(h.up(), Some("line"));
        assert_eq!(h.up(), Some("Circle"));
        assert_eq!(h.up(), Some("Circle")); // stays at the oldest
        assert_eq!(h.down(), Some("line"));
        assert_eq!(h.down(), None);
        assert!(!h.is_browsing());
    }
}
