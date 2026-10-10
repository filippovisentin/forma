//! Forma command engine: the single entry point for every action.
//!
//! UI, MCP and CLI all end up calling [`Engine::run_line`] (text, Rhino-style
//! command line) or [`Engine::execute`] (command name + argument tokens).

macro_rules! simple_command {
    ($ty:ident, $name:literal, $aliases:expr, $help:literal) => {
        pub struct $ty;
        impl $ty {
            const NAME: &'static str = $name;
            const ALIASES: &'static [&'static str] = $aliases;
            const HELP: &'static str = $help;
        }
    };
}

macro_rules! impl_meta {
    ($ty:ident) => {
        fn name(&self) -> &'static str {
            $ty::NAME
        }
        fn aliases(&self) -> &'static [&'static str] {
            $ty::ALIASES
        }
        fn help(&self) -> &'static str {
            $ty::HELP
        }
    };
}

mod analysis;
mod annotate;
mod args;
mod attrs;
mod commands;
mod create;
mod crvtools;
mod curves;
mod deform;
mod draw;
mod edit;
mod files;
mod import;
mod kernel;
mod layers;
mod measure;
mod select;
mod solid;
mod solids2;
mod surfaces;
mod transform;

pub use args::{parse_point, Args};

use forma_doc::{Document, Geometry, ObjectId};
use forma_geom::{Point3, Tolerance};
use std::collections::{BTreeSet, HashMap};
use std::fmt;

/// Error returned by a command. Commands must leave the document unchanged on error
/// (open a transaction and let it drop instead of committing).
#[derive(Debug, Clone, PartialEq)]
pub enum CommandError {
    UnknownCommand(String),
    MissingInput(&'static str),
    BadInput(String),
    Invalid(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::UnknownCommand(c) => write!(f, "unknown command: {c}"),
            CommandError::MissingInput(what) => write!(f, "missing input: {what}"),
            CommandError::BadInput(s) => write!(f, "cannot parse input: {s}"),
            CommandError::Invalid(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for CommandError {}

pub type CommandResult = Result<String, CommandError>;

/// Session state shared by commands.
pub struct Context {
    pub doc: Document,
    pub tolerance: Tolerance,
    /// Last point entered, used for `@` relative coordinates.
    pub last_point: Option<Point3>,
    /// Selected objects; editing commands (Move, Rotate, Extrude…) act on these.
    pub selection: BTreeSet<ObjectId>,
    /// Objects added by the last command that added any (for SelLast).
    pub last_created: Vec<ObjectId>,
    /// Internal clipboard filled by CopyToClipboard / Cut, used by Paste.
    pub clipboard: Vec<ClipboardItem>,
}

/// One object on the internal clipboard: geometry plus the attributes that survive
/// a paste into another document (layer by name, colour).
#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardItem {
    pub geometry: Geometry,
    pub layer: String,
    pub layer_color: [u8; 3],
    pub color: Option<[u8; 3]>,
}

impl Context {
    /// Selected object ids, or an error naming the command that needs them.
    pub fn selected(&self, command: &str) -> Result<Vec<ObjectId>, CommandError> {
        if self.selection.is_empty() {
            Err(CommandError::Invalid(format!(
                "{command}: select objects first"
            )))
        } else {
            Ok(self.selection.iter().copied().collect())
        }
    }
}

/// A command, e.g. `Line`. Implementations live in `commands/`.
pub trait Command: Send + Sync {
    /// Canonical name (Rhino spelling where one exists).
    fn name(&self) -> &'static str;
    /// Short aliases, e.g. `["L"]`.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
    /// One-line help shown by `Help`.
    fn help(&self) -> &'static str;
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult;
}

/// Command registry plus session.
pub struct Engine {
    pub ctx: Context,
    commands: Vec<Box<dyn Command>>,
    lookup: HashMap<String, usize>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        let mut engine = Engine {
            ctx: Context {
                doc: Document::new(),
                tolerance: Tolerance::default(),
                last_point: None,
                selection: BTreeSet::new(),
                last_created: Vec::new(),
                clipboard: Vec::new(),
            },
            commands: Vec::new(),
            lookup: HashMap::new(),
        };
        for c in commands::builtin() {
            engine.register(c);
        }
        engine
    }

    pub fn register(&mut self, command: Box<dyn Command>) {
        let idx = self.commands.len();
        self.lookup.insert(command.name().to_lowercase(), idx);
        for a in command.aliases() {
            self.lookup.insert(a.to_lowercase(), idx);
        }
        self.commands.push(command);
    }

    /// Names and help of all registered commands, sorted by name.
    pub fn command_list(&self) -> Vec<(&'static str, &'static str)> {
        let mut v: Vec<_> = self.commands.iter().map(|c| (c.name(), c.help())).collect();
        v.sort_by_key(|(n, _)| *n);
        v
    }

    /// Canonical name of a registered command or alias (case-insensitive).
    pub fn resolve(&self, name: &str) -> Option<&'static str> {
        let idx = *self.lookup.get(&name.to_lowercase())?;
        Some(self.commands[idx].name())
    }

    /// For a bare command name, what its first required argument is (from its
    /// help text: `Name <arg> …`), or `None` when it can run without arguments.
    /// Lets a UI wait for arguments instead of failing.
    pub fn missing_input(&self, name: &str) -> Option<&'static str> {
        let idx = *self.lookup.get(&name.to_lowercase())?;
        let help: &'static str = self.commands[idx].help();
        let rest = help.split_once(' ').map_or("", |(_, r)| r).trim_start();
        if let Some(inner) = rest.strip_prefix('<') {
            return inner.split_once('>').map(|(a, _)| a);
        }
        rest.starts_with('#').then_some("object ids")
    }

    /// For a bare command name whose arguments are all optional (`Name [arg] …`),
    /// the argument part of its help, so a UI can offer them before running it.
    /// Commands whose optional arguments are object ids (they act on the
    /// selection) give `None`.
    pub fn optional_args(&self, name: &str) -> Option<&'static str> {
        let idx = *self.lookup.get(&name.to_lowercase())?;
        let help: &'static str = self.commands[idx].help();
        let rest = help.split_once(' ').map_or("", |(_, r)| r).trim_start();
        let args = rest.split(" — ").next().unwrap_or("").trim();
        (args.starts_with('[') && !args.starts_with("[#")).then_some(args)
    }

    /// Run a command by name with pre-split argument tokens.
    pub fn execute(&mut self, name: &str, tokens: &[&str]) -> CommandResult {
        let idx = *self
            .lookup
            .get(&name.to_lowercase())
            .ok_or_else(|| CommandError::UnknownCommand(name.to_string()))?;
        let mut args = Args::new(tokens);
        let before: BTreeSet<ObjectId> = self.ctx.doc.objects().map(|o| o.id).collect();
        let result = self.commands[idx].run(&mut self.ctx, &mut args);
        // Drop selected ids that no longer exist (deleted, undone…) or that cannot
        // be selected any more (hidden, locked).
        let doc = &self.ctx.doc;
        self.ctx
            .selection
            .retain(|id| doc.object(*id).is_some_and(|o| doc.is_selectable(o)));
        if result.is_ok() {
            let added: Vec<ObjectId> = doc
                .objects()
                .map(|o| o.id)
                .filter(|id| !before.contains(id))
                .collect();
            if !added.is_empty() {
                self.ctx.last_created = added;
            }
        }
        let out = result?;
        if let Some(extra) = args.remaining() {
            return Err(CommandError::BadInput(format!("unused input: {extra}")));
        }
        Ok(out)
    }

    /// Run one command-line string, e.g. `"Line 0,0,0 @100,0"`.
    pub fn run_line(&mut self, line: &str) -> CommandResult {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            return Ok(String::new());
        };
        let tokens: Vec<&str> = parts.collect();
        self.execute(name, &tokens)
    }

    /// Run a script: one command per line, or `;`-separated. Lines starting with
    /// `#` are comments. Stops at the first error.
    pub fn run_script(&mut self, script: &str) -> Result<Vec<String>, (usize, CommandError)> {
        let mut out = Vec::new();
        for (i, line) in script.split(['\n', ';']).enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            out.push(self.run_line(line).map_err(|e| (i + 1, e))?);
        }
        Ok(out)
    }

    pub fn doc(&self) -> &Document {
        &self.ctx.doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_input_from_help() {
        let e = Engine::new();
        assert_eq!(e.missing_input("Layer"), Some("name"));
        assert_eq!(e.missing_input("select"), Some("object ids"));
        assert_eq!(e.missing_input("SelAll"), None);
        assert_eq!(e.missing_input("Delete"), None);
        assert_eq!(e.optional_args("New"), Some("[cm|mm|m]"));
        assert_eq!(e.optional_args("Delete"), None);
        assert_eq!(e.optional_args("SelAll"), None);
    }

    #[test]
    fn resolve_names_and_aliases() {
        let e = Engine::new();
        assert_eq!(e.resolve("line"), Some("Line"));
        assert_eq!(e.resolve("SELALL"), Some("SelAll"));
        assert_eq!(e.resolve("nosuchcommand"), None);
    }

    #[test]
    fn unknown_command() {
        let mut e = Engine::new();
        assert!(matches!(
            e.run_line("Frobnicate"),
            Err(CommandError::UnknownCommand(_))
        ));
    }

    #[test]
    fn script_with_comments_and_semicolons() {
        let mut e = Engine::new();
        e.run_script("# a square\nPolyline 0,0 100,0 100,100 0,100 c; Line 0,0 @0,0,50")
            .unwrap();
        assert_eq!(e.doc().len(), 2);
    }

    #[test]
    fn unused_input_is_an_error_and_changes_nothing() {
        let mut e = Engine::new();
        assert!(e.run_line("Line 0,0 1,0 2,0").is_err());
        assert!(e.doc().is_empty());
    }
}
