//! Conic, MatchCrv and CrvDeviation: conic arcs, matching the end of a curve to
//! another curve, and how far apart two curves are.

use crate::workflow::object_arg;
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{NurbsCurve, Point3};

simple_command!(
    Conic,
    "Conic",
    &[],
    "Conic <start> <end> <apex> [rho=0.5] — conic arc tangent at its ends to the lines to the apex; rho in (0, 1): below 0.5 an ellipse, 0.5 a parabola, above a hyperbola"
);
impl Command for Conic {
    impl_meta!(Conic);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let s = args.point("start of conic", ctx.last_point)?;
        let e = args.point("end of conic", Some(s))?;
        let a = args.point("apex", Some(e))?;
        let rho = args.optional_number().unwrap_or(0.5);
        if !(rho > 0.0 && rho < 1.0) {
            return Err(CommandError::Invalid("rho must be between 0 and 1".into()));
        }
        let c = NurbsCurve::conic(s, a, e, rho)
            .ok_or_else(|| CommandError::Invalid("the points are in a line".into()))?;
        let mut t = ctx.doc.begin();
        let id = t.add(Geometry::Nurbs(c));
        t.commit();
        ctx.last_point = Some(e);
        let kind = if (rho - 0.5).abs() < 1e-9 {
            "parabola"
        } else if rho < 0.5 {
            "ellipse"
        } else {
            "hyperbola"
        };
        Ok(format!("added #{} conic ({kind}, rho {rho})", id.0))
    }
}

fn nurbs_of(ctx: &Context, id: ObjectId) -> Result<NurbsCurve, CommandError> {
    ctx.doc
        .object(id)
        .and_then(|o| o.geometry.to_nurbs())
        .ok_or_else(|| CommandError::Invalid(format!("#{} is not a curve", id.0)))
}

simple_command!(
    MatchCrv,
    "MatchCrv",
    &["Match"],
    "MatchCrv #curve <point near the end to change> #target <point near the end to match> [position|tangency|curvature] — move the end of a curve onto the end of another and make it go on smoothly from it (default tangency)"
);
impl Command for MatchCrv {
    impl_meta!(MatchCrv);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = object_arg(ctx, args, "curve to change")?;
        let pa = args.point("point near the end to change", ctx.last_point)?;
        let b = object_arg(ctx, args, "curve to match")?;
        let pb = args.point("point near the end to match", Some(pa))?;
        let level = match args.next_token().map(str::to_lowercase).as_deref() {
            Some("position" | "g0") => 0,
            None | Some("tangency" | "g1" | "tangent") => 1,
            Some("curvature" | "g2") => 2,
            Some(o) => return Err(CommandError::BadInput(o.to_string())),
        };
        if a == b {
            return Err(CommandError::Invalid(
                "MatchCrv: pick two different curves".into(),
            ));
        }
        let (ca, cb) = (nurbs_of(ctx, a)?, nurbs_of(ctx, b)?);
        let near_end =
            |c: &NurbsCurve, p: Point3| c.end().distance_to(p) < c.start().distance_to(p);
        // The target end: where it is, the direction in which the target would
        // go on past it, and its curvature there.
        let (t0, t1) = cb.domain();
        let (tb, sign) = if near_end(&cb, pb) {
            (t1, 1.0)
        } else {
            (t0, -1.0)
        };
        let (at, d1, _) = cb.derivatives_at(tb);
        let leave = (level >= 1).then(|| (d1 * sign).normalized()).flatten();
        if level >= 1 && leave.is_none() {
            return Err(CommandError::Invalid(format!(
                "MatchCrv: #{} has no direction at that end",
                b.0
            )));
        }
        let k = if level == 2 {
            cb.curvature_at(tb)
        } else {
            None
        };
        let matched = if near_end(&ca, pa) {
            ca.matched_end(at, leave, k)
        } else {
            ca.matched_start(at, leave, k)
        }
        .ok_or_else(|| {
            CommandError::Invalid(format!(
                "MatchCrv: #{} has too few control points for that (Rebuild it, or add knots with InsertKnot)",
                a.0
            ))
        })?;
        let mut t = ctx.doc.begin();
        t.replace(a, Geometry::Nurbs(matched));
        t.commit();
        ctx.last_point = Some(at);
        Ok(format!(
            "matched #{} to #{} ({})",
            a.0,
            b.0,
            ["position", "tangency", "curvature"][level]
        ))
    }
}

simple_command!(
    CrvDeviation,
    "CrvDeviation",
    &[],
    "CrvDeviation — largest and smallest distance between the two selected curves, where they run alongside each other"
);
impl Command for CrvDeviation {
    impl_meta!(CrvDeviation);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let curves: Vec<NurbsCurve> = ctx
            .selected("CrvDeviation")?
            .into_iter()
            .filter_map(|id| ctx.doc.object(id)?.geometry.to_nurbs())
            .collect();
        let [a, b] = curves.as_slice() else {
            return Err(CommandError::Invalid(
                "CrvDeviation: select two curves".into(),
            ));
        };
        let unit = ctx.doc.units.abbreviation();
        let Some(d) = a.deviation(b) else {
            return Ok("the curves do not run alongside each other".into());
        };
        let p = |q: Point3| format!("{:.6},{:.6},{:.6}", q.x, q.y, q.z);
        Ok(format!(
            "maximum deviation = {:.6} {unit} between {} and {}\nminimum deviation = {:.6} {unit} between {} and {}",
            d.max,
            p(d.max_a),
            p(d.max_b),
            d.min,
            p(d.min_a),
            p(d.min_b)
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::{NurbsCurve, Point3};

    fn nurbs(e: &Engine, i: usize) -> NurbsCurve {
        match &e.doc().objects().nth(i).unwrap().geometry {
            Geometry::Nurbs(n) => n.clone(),
            g => panic!("{}", g.kind()),
        }
    }

    #[test]
    fn conic_is_a_rational_arc_through_its_shoulder() {
        let mut e = Engine::new();
        let msg = e.run_line("Conic 0,0,0 20,0,0 10,20,0").unwrap();
        assert!(msg.contains("parabola"), "{msg}");
        let c = nurbs(&e, 0);
        assert_eq!((c.degree, c.points.len()), (2, 3));
        assert!(c.point_at(0.5).distance_to(Point3::new(10.0, 10.0, 0.0)) < 1e-9);
        let msg = e.run_line("Conic 0,0,0 20,0,0 10,20,0 0.8").unwrap();
        assert!(msg.contains("hyperbola"), "{msg}");
        assert!(
            nurbs(&e, 1)
                .point_at(0.5)
                .distance_to(Point3::new(10.0, 16.0, 0.0))
                < 1e-9
        );
        assert!(e.run_line("Conic 0,0,0 20,0,0 10,20,0 1").is_err());
        assert!(e.run_line("Conic 0,0,0 20,0,0 10,0,0").is_err());
        assert!(e.run_line("Conic 0,0,0 20,0,0").is_err());
        e.run_line("Undo").unwrap();
        assert_eq!(e.doc().objects().count(), 1);
    }

    #[test]
    fn match_crv_moves_the_picked_end_like_rhino() {
        let mut e = Engine::new();
        e.run_line("Curve 0,0 10,10 20,5 30,12 40,0 50,6").unwrap();
        e.run_line("Curve -40,-10 -30,10 -15,5 -5,-3").unwrap();
        let (a, b) = {
            let mut ids = e.doc().objects().map(|o| o.id.0);
            (ids.next().unwrap(), ids.next().unwrap())
        };
        // Start of the first onto the start of the second, with curvature:
        // the control points Rhino 8.36 gives.
        e.run_line(&format!("MatchCrv #{a} 1,1,0 #{b} -39,-9,0 curvature"))
            .unwrap();
        let c = nurbs(&e, 0);
        let want = [
            (-40.0, -10.0),
            (-46.324_555_32, -22.649_110_641),
            (-36.705_694_15, -31.411_388_301),
            (30.0, 12.0),
            (40.0, 0.0),
            (50.0, 6.0),
        ];
        for (p, w) in c.points.iter().zip(want) {
            assert!(p.distance_to(Point3::new(w.0, w.1, 0.0)) < 1e-6, "{p}");
        }
        // The target is untouched.
        assert!(
            nurbs(&e, 1)
                .start()
                .distance_to(Point3::new(-40.0, -10.0, 0.0))
                < 1e-12
        );
        e.run_line("Undo").unwrap();
        // The end of the first onto the end of the second, tangency (default).
        e.run_line(&format!("MatchCrv #{a} 49,6,0 #{b} -5,-2,0"))
            .unwrap();
        let c = nurbs(&e, 0);
        let t = nurbs(&e, 1);
        assert!(c.end().distance_to(t.end()) < 1e-12);
        assert!(c.start().distance_to(Point3::ORIGIN) < 1e-12);
        // Arriving against the target's direction: one smooth curve through.
        let (tc, tt) = (c.tangent_at(c.domain().1), t.tangent_at(t.domain().1));
        assert!(tc.dot(tt) < -1.0 + 1e-6, "{tc:?} {tt:?}");
        e.run_line("Undo").unwrap();
        e.run_line(&format!("MatchCrv #{a} 1,1,0 #{b} -5,-2,0 position"))
            .unwrap();
        let c = nurbs(&e, 0);
        assert!(c.start().distance_to(Point3::new(-5.0, -3.0, 0.0)) < 1e-12);
        assert!(c.points[1].distance_to(Point3::new(10.0, 10.0, 0.0)) < 1e-12);

        assert!(e
            .run_line(&format!("MatchCrv #{a} 1,1,0 #{a} 2,2,0"))
            .is_err());
        assert!(e
            .run_line(&format!("MatchCrv #{a} 1,1,0 #{b} 2,2,0 wavy"))
            .is_err());
        // A line has no control point to spare for tangency.
        e.run_line("Line 100,0,0 110,0,0").unwrap();
        let l = e.doc().objects().last().unwrap().id.0;
        assert!(e
            .run_line(&format!("MatchCrv #{l} 100,0,0 #{b} -5,-3,0"))
            .is_err());
        e.run_line(&format!("MatchCrv #{l} 100,0,0 #{b} -5,-3,0 position"))
            .unwrap();
    }

    #[test]
    fn crv_deviation_between_a_circle_and_a_bigger_one() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Circle 0,0,0 10").unwrap();
        e.run_line("Circle 1,0,0 12").unwrap();
        e.run_line("SelAll").unwrap();
        let msg = e.run_line("CrvDeviation").unwrap();
        // Concentric but for 1 cm: between 12 − 10 − 1 and 12 − 10 + 1.
        assert!(msg.contains("maximum deviation = 3.0000"), "{msg}");
        assert!(msg.contains("minimum deviation = 1.0000"), "{msg}");
        assert!(msg.contains(" cm "), "{msg}");
        e.run_line("SelNone").unwrap();
        assert!(e.run_line("CrvDeviation").is_err());
        e.run_line("Line 0,0,0 5,0,0").unwrap();
        e.run_line("SelLast").unwrap();
        assert!(e.run_line("CrvDeviation").is_err());
    }
}
