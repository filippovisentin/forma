//! Forma command engine: the single entry point for every action.
//!
//! UI, MCP and CLI all end up calling [`Engine::run_line`] (text, Rhino-style
//! command line) or [`Engine::execute`] (command name + argument tokens).

mod args;
mod commands;

pub use args::{parse_point, Args};

use forma_doc::Document;
use forma_geom::{Point3, Tolerance};
use std::collections::HashMap;
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

    /// Run a command by name with pre-split argument tokens.
    pub fn execute(&mut self, name: &str, tokens: &[&str]) -> CommandResult {
        let idx = *self
            .lookup
            .get(&name.to_lowercase())
            .ok_or_else(|| CommandError::UnknownCommand(name.to_string()))?;
        let mut args = Args::new(tokens);
        let out = self.commands[idx].run(&mut self.ctx, &mut args)?;
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
        assert_eq!(e.doc().len(), 5);
    }

    #[test]
    fn unused_input_is_an_error_and_changes_nothing() {
        let mut e = Engine::new();
        assert!(e.run_line("Line 0,0 1,0 2,0").is_err());
        assert!(e.doc().is_empty());
    }
}
