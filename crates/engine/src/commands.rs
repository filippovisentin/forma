//! Built-in commands. One struct per command; keep each small and tested.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::Geometry;
use forma_geom::LineCurve;

pub fn builtin() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(Line),
        Box::new(Polyline),
        Box::new(crate::create::Rectangle),
        Box::new(crate::create::Circle),
        Box::new(crate::create::Arc),
        Box::new(crate::create::BoxCmd),
        Box::new(crate::create::Cylinder),
        Box::new(crate::create::Sphere),
        Box::new(crate::create::Extrude),
        Box::new(crate::edit::Move),
        Box::new(crate::edit::Copy),
        Box::new(crate::edit::Rotate),
        Box::new(crate::edit::Scale),
        Box::new(crate::edit::Mirror),
        Box::new(crate::edit::Delete),
        Box::new(crate::solid::MoveFace),
        Box::new(crate::solid::PushPull),
        Box::new(crate::kernel::BooleanUnion),
        Box::new(crate::kernel::BooleanDifference),
        Box::new(crate::kernel::BooleanIntersection),
        Box::new(crate::kernel::BooleanSplit),
        Box::new(crate::kernel::FilletEdge),
        Box::new(crate::kernel::ChamferEdge),
        Box::new(crate::kernel::Shell),
        Box::new(crate::kernel::OffsetSrf),
        Box::new(crate::curves::Offset),
        Box::new(crate::curves::Trim),
        Box::new(crate::curves::Extend),
        Box::new(crate::curves::Fillet),
        Box::new(crate::curves::FilletCorners),
        Box::new(crate::curves::Join),
        Box::new(crate::curves::Explode),
        Box::new(crate::attrs::Array),
        Box::new(crate::attrs::ArrayLinear),
        Box::new(crate::attrs::ArrayPolar),
        Box::new(crate::attrs::ObjectColor),
        Box::new(crate::attrs::LayerColor),
        Box::new(crate::attrs::LayerVisible),
        Box::new(crate::attrs::LayerLock),
        Box::new(crate::edit::SelAll),
        Box::new(crate::edit::SelNone),
        Box::new(crate::edit::Select),
        Box::new(crate::edit::LayerCmd),
        Box::new(crate::edit::ChangeLayer),
        Box::new(crate::edit::New),
        Box::new(crate::edit::Save),
        Box::new(crate::create::Point),
        Box::new(crate::create::Points),
        Box::new(crate::create::Curve),
        Box::new(crate::create::InterpCrv),
        Box::new(crate::create::Ellipse),
        Box::new(crate::create::Polygon),
        Box::new(crate::curves::Split),
        Box::new(crate::curves::Chamfer),
        Box::new(crate::curves::Intersect),
        Box::new(crate::transform::Scale1D),
        Box::new(crate::transform::Scale2D),
        Box::new(crate::transform::Orient),
        Box::new(crate::transform::Align),
        Box::new(crate::transform::Flip),
        Box::new(crate::transform::MatchProperties),
        Box::new(crate::transform::BoundingBoxCmd),
        Box::new(crate::transform::ProjectToCPlane),
        Box::new(crate::select::Hide),
        Box::new(crate::select::Show),
        Box::new(crate::select::Isolate),
        Box::new(crate::select::Lock),
        Box::new(crate::select::Unlock),
        Box::new(crate::select::Group),
        Box::new(crate::select::Ungroup),
        Box::new(crate::select::SelGroup),
        Box::new(crate::select::SelLast),
        Box::new(crate::workflow::SelPrev),
        Box::new(crate::workflow::ShowSelected),
        Box::new(crate::workflow::UnlockSelected),
        Box::new(crate::workflow::Lines),
        Box::new(crate::workflow::Stretch),
        Box::new(crate::workflow::ClosestPt),
        Box::new(crate::workflow::DupFaceBorder),
        Box::new(crate::workflow::UnifyMeshNormals),
        Box::new(crate::workflow::Block),
        Box::new(crate::workflow::Insert),
        Box::new(crate::workflow::MoveGrips),
        Box::new(crate::workflow::EditText),
        Box::new(crate::workflow::SelBoundary),
        Box::new(crate::workflow::Make2D),
        Box::new(crate::edit::IncrementalSave),
        Box::new(crate::select::SelCrv),
        Box::new(crate::select::SelMesh),
        Box::new(crate::select::SelPt),
        Box::new(crate::select::Invert),
        Box::new(crate::select::HideSwap),
        Box::new(crate::select::LockSwap),
        Box::new(crate::select::SelVisible),
        Box::new(crate::surfaces::PlanarSrf),
        Box::new(crate::surfaces::ExtrudeSrf),
        Box::new(crate::surfaces::Cap),
        Box::new(crate::surfaces::Loft),
        Box::new(crate::surfaces::Revolve),
        Box::new(crate::surfaces::Sweep1),
        Box::new(crate::analysis::Distance),
        Box::new(crate::analysis::Length),
        Box::new(crate::analysis::Area),
        Box::new(crate::analysis::Volume),
        Box::new(crate::analysis::What),
        Box::new(crate::files::CopyToClipboard),
        Box::new(crate::files::Cut),
        Box::new(crate::files::Paste),
        Box::new(crate::files::Import),
        Box::new(crate::files::Export),
        Box::new(Undo),
        Box::new(Redo),
        Box::new(Open),
        // Curves
        Box::new(crate::draw::Circle3Pt),
        Box::new(crate::draw::Circle2Pt),
        Box::new(crate::draw::Arc3Pt),
        Box::new(crate::draw::Rectangle3Pt),
        Box::new(crate::draw::RectangleCenter),
        Box::new(crate::draw::RoundedRectangle),
        Box::new(crate::draw::Slot),
        Box::new(crate::draw::Helix),
        Box::new(crate::draw::Spiral),
        Box::new(crate::crvtools::Divide),
        Box::new(crate::crvtools::DivideByLength),
        Box::new(crate::crvtools::Contour),
        Box::new(crate::crvtools::Section),
        Box::new(crate::crvtools::DupBorder),
        Box::new(crate::crvtools::DupEdge),
        Box::new(crate::crvtools::CurveBoolean),
        Box::new(crate::crvtools::Convert),
        Box::new(crate::crvtools::Rebuild),
        Box::new(crate::crvtools::CloseCrv),
        Box::new(crate::crvtools::ExtractPt),
        Box::new(crate::crvtools::Project),
        Box::new(crate::crvtools::Pull),
        // Surfaces, solids, meshes
        Box::new(crate::solids2::PlaneSrf),
        Box::new(crate::solids2::SrfPt),
        Box::new(crate::solids2::EdgeSrf),
        Box::new(crate::solids2::Cone),
        Box::new(crate::solids2::TCone),
        Box::new(crate::solids2::Torus),
        Box::new(crate::solids2::Ellipsoid),
        Box::new(crate::solids2::Pyramid),
        Box::new(crate::solids2::Tube),
        Box::new(crate::solids2::Pipe),
        Box::new(crate::solids2::ExtrudeCrvAlongCrv),
        Box::new(crate::solids2::ExtrudeCrvTapered),
        Box::new(crate::solids2::Slab),
        Box::new(crate::solids2::ExtrudeCrvToPoint),
        Box::new(crate::solids2::Weld),
        Box::new(crate::solids2::Unweld),
        // Transforms
        Box::new(crate::deform::Shear),
        Box::new(crate::deform::Rotate3D),
        Box::new(crate::deform::ScaleNU),
        Box::new(crate::deform::SetPt),
        Box::new(crate::deform::ArrayCrv),
        Box::new(crate::deform::Twist),
        Box::new(crate::deform::Taper),
        Box::new(crate::deform::Bend),
        Box::new(crate::deform::Mirror3Pt),
        Box::new(crate::deform::BoxEdit),
        Box::new(crate::deform::Distribute),
        Box::new(crate::deform::Orient3Pt),
        // Attributes, selection, layers
        Box::new(crate::layers::SetObjectName),
        Box::new(crate::layers::SelName),
        Box::new(crate::layers::SelLayer),
        Box::new(crate::layers::SelColor),
        Box::new(crate::layers::SelDup),
        Box::new(crate::layers::SelOpenCrv),
        Box::new(crate::layers::SelClosedCrv),
        Box::new(crate::layers::SelPolyline),
        Box::new(crate::layers::SelLine),
        Box::new(crate::layers::SelText),
        Box::new(crate::layers::SelDim),
        Box::new(crate::layers::SelSmall),
        Box::new(crate::layers::SelClosedMesh),
        Box::new(crate::layers::SelOpenMesh),
        Box::new(crate::layers::Purge),
        Box::new(crate::layers::RenameLayer),
        Box::new(crate::layers::DeleteLayer),
        Box::new(crate::layers::OneLayerOn),
        Box::new(crate::layers::AllLayersOn),
        Box::new(crate::layers::OneLayerOff),
        Box::new(crate::layers::ChangeToCurrentLayer),
        Box::new(crate::layers::CopyObjectsToLayer),
        // Analysis
        Box::new(crate::measure::Angle),
        Box::new(crate::measure::Radius),
        Box::new(crate::measure::EvaluatePt),
        Box::new(crate::measure::AreaCentroid),
        Box::new(crate::measure::VolumeCentroid),
        // Annotation
        Box::new(crate::annotate::TextCmd),
        Box::new(crate::annotate::TextDot),
        Box::new(crate::annotate::Dim),
        Box::new(crate::annotate::DimAligned),
        Box::new(crate::annotate::DimRadius),
        Box::new(crate::annotate::DimDiameter),
        Box::new(crate::annotate::DimAngle),
        Box::new(crate::annotate::DimStyle),
        Box::new(crate::annotate::Leader),
        Box::new(crate::annotate::Hatch),
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
        pts.dedup_by(|a, b| a.distance_to(*b) <= ctx.tolerance.absolute);
        if pts.len() < 2 || (close && pts.len() < 4) {
            return Err(CommandError::Invalid("polyline is degenerate".into()));
        }
        let mut t = ctx.doc.begin();
        let id = t.add(Geometry::Polyline(pts.clone()));
        t.commit();
        ctx.last_point = pts.last().copied();
        Ok(format!(
            "added #{} polyline, {} segments",
            id.0,
            pts.len() - 1
        ))
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
    fn closed_polyline_is_one_object() {
        let mut e = Engine::new();
        e.run_line("Polyline 0,0 600,0 600,400 0,400 c").unwrap();
        assert_eq!(e.doc().len(), 1);
        assert!(e.doc().objects().next().unwrap().geometry.is_closed_curve());
        e.run_line("Undo").unwrap();
        assert!(e.doc().is_empty());
        e.run_line("Redo").unwrap();
        assert_eq!(e.doc().len(), 1);
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
        // 477 model objects; the 690 objects of its unused block definitions are
        // not shown (Rhino does not show them either).
        assert!(e.doc().len() > 450 && e.doc().len() < 1100);
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

    #[test]
    fn names_and_aliases_are_unique() {
        let mut seen = std::collections::HashMap::new();
        for c in super::builtin() {
            for n in std::iter::once(c.name()).chain(c.aliases().iter().copied()) {
                if let Some(other) = seen.insert(n.to_lowercase(), c.name()) {
                    panic!("{n} is used by {other} and {}", c.name());
                }
            }
        }
    }
}
