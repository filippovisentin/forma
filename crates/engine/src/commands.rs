//! Built-in commands. One struct per command; keep each small and tested.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::LineCurve;

pub fn builtin() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(Line),
        Box::new(Polyline),
        Box::new(Delete),
        Box::new(Undo),
        Box::new(Redo),
        Box::new(Open),
    ]
}

/// `Open <path.3dm>` — replace the document with a Rhino file (display geometry).
pub struct Open;

impl Command for Open {
    fn name(&self) -> &'static str {
        "Open"
    }
    fn help(&self) -> &'static str {
        "Open <file.3dm> — open a Rhino file (replaces the current document)"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut parts = Vec::new();
        while let Some(t) = args.next_token() {
            parts.push(t);
        }
        let path = parts.join(" ");
        let path = path.trim().trim_matches('"');
        if path.is_empty() {
            return Err(CommandError::MissingInput("file path"));
        }
        let doc = crate::import::open_3dm(path)?;
        let msg = format!(
            "opened {} — {} objects, {} layers, units {}",
            path,
            doc.len(),
            doc.layers.len(),
            doc.units.abbreviation()
        );
        ctx.doc = doc;
        ctx.last_point = None;
        Ok(msg)
    }
}

/// `Line <start> <end>`
pub struct Line;

impl Command for Line {
    fn name(&self) -> &'static str {
        "Line"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["L"]
    }
    fn help(&self) -> &'static str {
        "Line <start> <end> — draw a line segment"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start point", ctx.last_point)?;
        let b = args.point("end point", Some(a))?;
        let line = LineCurve::new(a, b);
        if line.is_degenerate(ctx.tolerance) {
            return Err(CommandError::Invalid(
                "line is shorter than tolerance".into(),
            ));
        }
        // Validate before mutating: the engine rejects leftover input afterwards,
        // but the document must stay untouched on error.
        if let Some(extra) = args.remaining() {
            return Err(CommandError::BadInput(format!("unused input: {extra}")));
        }
        let mut t = ctx.doc.begin();
        let id = t.add(Geometry::Line(line));
        t.commit();
        ctx.last_point = Some(b);
        Ok(format!("added #{}", id.0))
    }
}

/// `Polyline <p1> <p2> ... [c]` — consecutive segments; `c` closes back to p1.
/// Stored as individual lines until a polyline curve type lands in M1.
pub struct Polyline;

impl Command for Polyline {
    fn name(&self) -> &'static str {
        "Polyline"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["PL"]
    }
    fn help(&self) -> &'static str {
        "Polyline <p1> <p2> ... [c] — connected segments, c closes"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut pts = vec![args.point("first point", ctx.last_point)?];
        let mut close = false;
        while args.peek().is_some() {
            if args.keyword("c") || args.keyword("close") {
                close = true;
                break;
            }
            let last = *pts.last().expect("non-empty");
            pts.push(args.point("next point", Some(last))?);
        }
        if let Some(extra) = args.remaining() {
            return Err(CommandError::BadInput(format!("unused input: {extra}")));
        }
        if close {
            pts.push(pts[0]);
        }
        if pts.len() < 2 {
            return Err(CommandError::MissingInput("second point"));
        }
        let mut t = ctx.doc.begin();
        for w in pts.windows(2) {
            let seg = LineCurve::new(w[0], w[1]);
            if seg.is_degenerate(ctx.tolerance) {
                return Err(CommandError::Invalid(
                    "segment shorter than tolerance".into(),
                ));
            }
            t.add(Geometry::Line(seg));
        }
        t.commit();
        ctx.last_point = pts.last().copied();
        Ok(format!("added {} segments", pts.len() - 1))
    }
}

/// `Delete #id ...`
pub struct Delete;

impl Command for Delete {
    fn name(&self) -> &'static str {
        "Delete"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["Del"]
    }
    fn help(&self) -> &'static str {
        "Delete #id ... — remove objects by id"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut ids = Vec::new();
        while let Some(tok) = args.next_token() {
            let n: u64 = tok
                .trim_start_matches('#')
                .parse()
                .map_err(|_| CommandError::BadInput(tok.to_string()))?;
            ids.push(ObjectId(n));
        }
        if ids.is_empty() {
            return Err(CommandError::MissingInput("object ids"));
        }
        let mut t = ctx.doc.begin();
        for id in &ids {
            if !t.remove(*id) {
                return Err(CommandError::Invalid(format!("no object #{}", id.0)));
            }
        }
        t.commit();
        Ok(format!("deleted {}", ids.len()))
    }
}

pub struct Undo;

impl Command for Undo {
    fn name(&self) -> &'static str {
        "Undo"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["U"]
    }
    fn help(&self) -> &'static str {
        "Undo — revert the last command"
    }
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        if ctx.doc.undo() {
            Ok("undone".into())
        } else {
            Err(CommandError::Invalid("nothing to undo".into()))
        }
    }
}

pub struct Redo;

impl Command for Redo {
    fn name(&self) -> &'static str {
        "Redo"
    }
    fn help(&self) -> &'static str {
        "Redo — re-apply the last undone command"
    }
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        if ctx.doc.redo() {
            Ok("redone".into())
        } else {
            Err(CommandError::Invalid("nothing to redo".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn line_with_relative_end() {
        let mut e = Engine::new();
        e.run_line("Line 0,0,0 @100,0").unwrap();
        assert_eq!(e.doc().dump(), "#1 line [Default] 0,0,0 -> 100,0,0\n");
    }

    #[test]
    fn line_alias_and_case() {
        let mut e = Engine::new();
        e.run_line("l 0,0 10,0").unwrap();
        e.run_line("LINE 0,0 0,10").unwrap();
        assert_eq!(e.doc().len(), 2);
    }

    #[test]
    fn degenerate_line_rejected() {
        let mut e = Engine::new();
        assert!(e.run_line("Line 0,0 0,0").is_err());
        assert!(e.doc().is_empty());
    }

    #[test]
    fn closed_polyline_is_one_undo_step() {
        let mut e = Engine::new();
        e.run_line("Polyline 0,0 600,0 600,400 0,400 c").unwrap();
        assert_eq!(e.doc().len(), 4);
        e.run_line("Undo").unwrap();
        assert!(e.doc().is_empty());
        e.run_line("Redo").unwrap();
        assert_eq!(e.doc().len(), 4);
    }

    #[test]
    fn open_needs_a_path_and_a_real_file() {
        let mut e = Engine::new();
        assert!(e.run_line("Open").is_err());
        assert!(e.run_line("Open /not/there.3dm").is_err());
    }

    #[test]
    fn open_real_file_if_present() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/data/private/binario.3dm"
        );
        if !std::path::Path::new(p).exists() {
            return;
        }
        let mut e = Engine::new();
        e.run_line(&format!("Open {p}")).unwrap();
        assert_eq!(e.doc().units.abbreviation(), "cm");
        assert_eq!(e.doc().layers.len(), 9);
        assert!(e.doc().len() > 1100);
        assert!(!e.doc().can_undo());
    }

    #[test]
    fn delete_by_id() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 1,0").unwrap();
        e.run_line("Delete #1").unwrap();
        assert!(e.doc().is_empty());
        assert!(e.run_line("Delete #99").is_err());
    }
}
