//! NURBS curve structure and analysis: InsertKnot, RemoveKnot, CrvSeam, Curvature.
//!
//! Each command works on the selected curve nearest to the picked point. Lines,
//! arcs, polylines and polycurves are read as the exact NURBS curve they are;
//! the commands that change the structure store the result as a NURBS curve.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{NurbsCurve, Plane, Point3};
use std::fmt::Write as _;

/// The selected curve closest to `p`, as a NURBS curve, with the parameter of
/// its closest point.
fn nearest(
    ctx: &Context,
    cmd: &str,
    p: Point3,
) -> Result<(ObjectId, NurbsCurve, f64), CommandError> {
    let mut best: Option<(f64, ObjectId, NurbsCurve, f64)> = None;
    for id in ctx.selected(cmd)? {
        let Some(c) = ctx.doc.object(id).and_then(|o| o.geometry.to_nurbs()) else {
            continue;
        };
        let t = c.closest_param(p);
        let d = c.point_at(t).distance_to(p);
        if best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, id, c, t));
        }
    }
    best.map(|(_, id, c, t)| (id, c, t))
        .ok_or_else(|| CommandError::Invalid(format!("{cmd}: select a curve")))
}

/// Every remaining token as a point (at least one).
fn points(
    ctx: &mut Context,
    args: &mut Args,
    what: &'static str,
) -> Result<Vec<Point3>, CommandError> {
    let mut out = vec![args.point(what, ctx.last_point)?];
    while args.peek().is_some() {
        out.push(args.point(what, out.last().copied())?);
    }
    ctx.last_point = out.last().copied();
    Ok(out)
}

/// Largest distance between two curves on the same domain, parameter by
/// parameter.
fn deviation(a: &NurbsCurve, b: &NurbsCurve) -> f64 {
    let (t0, t1) = a.domain();
    (0..=400)
        .map(|i| {
            let t = t0 + (t1 - t0) * f64::from(i) / 400.0;
            a.point_at(t).distance_to(b.point_at(t))
        })
        .fold(0.0, f64::max)
}

simple_command!(
    InsertKnot,
    "InsertKnot",
    &[],
    "InsertKnot <points on the curve…> | Midpoints — add knots (and control points) to the selected curve without changing its shape"
);
impl Command for InsertKnot {
    impl_meta!(InsertKnot);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let (id, before, params) = if args.keyword("Midpoints") {
            let ids = ctx.selected("InsertKnot")?;
            let (id, c) = ids
                .iter()
                .find_map(|id| Some((*id, ctx.doc.object(*id)?.geometry.to_nurbs()?)))
                .ok_or_else(|| CommandError::Invalid("InsertKnot: select a curve".into()))?;
            let (a, b) = c.domain();
            let mut k = vec![a];
            k.extend(c.interior_knots());
            k.push(b);
            let mids = k.windows(2).map(|w| (w[0] + w[1]) / 2.0).collect();
            (id, c, mids)
        } else {
            let pts = points(ctx, args, "point on the curve")?;
            let (id, c, t) = nearest(ctx, "InsertKnot", pts[0])?;
            let mut params = vec![t];
            params.extend(pts[1..].iter().map(|p| c.closest_param(*p)));
            (id, c, params)
        };
        let mut c = before.clone();
        for t in params {
            c = c.with_knot(t, 1);
        }
        let added = c.points.len() - before.points.len();
        if added == 0 {
            return Err(CommandError::Invalid(
                "InsertKnot: no knot can be added there (a curve end, or a corner)".into(),
            ));
        }
        let n = c.points.len();
        let mut t = ctx.doc.begin();
        t.replace(id, Geometry::Nurbs(c));
        t.commit();
        Ok(format!(
            "added {added} knot(s) to #{}: {n} control points",
            id.0
        ))
    }
}

simple_command!(
    RemoveKnot,
    "RemoveKnot",
    &[],
    "RemoveKnot <points near knots…> — remove the nearest knot of the selected curve (one control point fewer; the shape may change, by the distance reported)"
);
impl Command for RemoveKnot {
    impl_meta!(RemoveKnot);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let pts = points(ctx, args, "point near a knot")?;
        let (id, before, _) = nearest(ctx, "RemoveKnot", pts[0])?;
        let mut c = before.clone();
        for p in &pts {
            let knots = c.interior_knots();
            let k = knots
                .iter()
                .copied()
                .min_by(|a, b| {
                    c.point_at(*a)
                        .distance_to(*p)
                        .total_cmp(&c.point_at(*b).distance_to(*p))
                })
                .ok_or_else(|| {
                    CommandError::Invalid(format!(
                        "RemoveKnot: #{} has no knot left to remove",
                        id.0
                    ))
                })?;
            c = c.without_knot(k).ok_or_else(|| {
                CommandError::Invalid(format!("RemoveKnot: cannot remove that knot of #{}", id.0))
            })?;
        }
        let dev = deviation(&before, &c);
        let n = c.points.len();
        let unit = ctx.doc.units.abbreviation();
        let mut t = ctx.doc.begin();
        t.replace(id, Geometry::Nurbs(c));
        t.commit();
        Ok(format!(
            "removed {} knot(s) from #{}: {n} control points, deviation {dev:.6} {unit}",
            pts.len(),
            id.0
        ))
    }
}

/// Closed polyline (first point repeated at the end) restarted at the point of
/// it closest to `p`.
fn polyline_seam(pts: &[Point3], p: Point3) -> Option<Vec<Point3>> {
    let ring = &pts[..pts.len() - 1];
    let n = ring.len();
    let mut best: Option<(f64, usize, Point3)> = None;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let ab = b - a;
        let len2 = ab.dot(ab);
        let s = if len2 < 1e-24 {
            0.0
        } else {
            ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
        };
        let q = a + ab * s;
        let d = q.distance_to(p);
        if best.is_none_or(|x| d < x.0) {
            best = Some((d, i, q));
        }
    }
    let (_, i, q) = best?;
    let mut out = vec![q];
    for k in 1..=n {
        let v = ring[(i + k) % n];
        if v.distance_to(*out.last()?) > 1e-9 {
            out.push(v);
        }
    }
    if out.last()?.distance_to(q) > 1e-9 {
        out.push(q);
    } else {
        *out.last_mut()? = q;
    }
    Some(out)
}

simple_command!(
    CrvSeam,
    "CrvSeam",
    &["Seam"],
    "CrvSeam <point> — move the start / end point of the selected closed curve to the point of it nearest to <point>"
);
impl Command for CrvSeam {
    impl_meta!(CrvSeam);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let p = args.point("new seam point", ctx.last_point)?;
        ctx.last_point = Some(p);
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut best: Option<(f64, ObjectId)> = None;
        for id in ctx.selected("CrvSeam")? {
            let Some(g) = ctx.doc.object(id).map(|o| &o.geometry) else {
                continue;
            };
            if !g.is_closed_curve() {
                continue;
            }
            let Some(c) = g.to_nurbs() else { continue };
            let d = c.point_at(c.closest_param(p)).distance_to(p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, id));
            }
        }
        let (_, id) =
            best.ok_or_else(|| CommandError::Invalid("CrvSeam: select a closed curve".into()))?;
        let g = ctx.doc.object(id).expect("selected").geometry.clone();
        let same = || CommandError::Invalid(format!("CrvSeam: the seam of #{} is there", id.0));
        let moved = match &g {
            // A circle stays a circle: its plane turns so that it starts there.
            Geometry::Arc(a) => {
                let v = p - a.center();
                let x = (v - a.plane.z * v.dot(a.plane.z))
                    .normalized()
                    .ok_or_else(|| {
                        CommandError::Invalid("CrvSeam: pick a point off the centre".into())
                    })?;
                let mut a = *a;
                a.plane = Plane {
                    origin: a.plane.origin,
                    x,
                    y: a.plane.z.cross(x),
                    z: a.plane.z,
                };
                Geometry::Arc(a)
            }
            Geometry::Polyline(pts) => {
                let out = polyline_seam(pts, p).ok_or_else(same)?;
                if out[0].distance_to(pts[0]) <= 1e-9 {
                    return Err(same());
                }
                Geometry::Polyline(out)
            }
            g => {
                let c = g.to_nurbs().expect("closed curve");
                let t = c.closest_param(p);
                Geometry::Nurbs(c.with_seam(t, tol).ok_or_else(same)?)
            }
        };
        let start = moved.to_nurbs().expect("curve").start();
        let mut t = ctx.doc.begin();
        t.replace(id, moved);
        t.commit();
        Ok(format!(
            "seam of #{} moved to {:.6},{:.6},{:.6}",
            id.0, start.x, start.y, start.z
        ))
    }
}

simple_command!(
    Curvature,
    "Curvature",
    &[],
    "Curvature <points…> — radius of curvature of the selected curve at the points of it nearest to the given ones"
);
impl Command for Curvature {
    impl_meta!(Curvature);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let pts = points(ctx, args, "point on the curve")?;
        let unit = ctx.doc.units.abbreviation();
        let mut out = String::new();
        for p in pts {
            let (id, c, t) = nearest(ctx, "Curvature", p)?;
            let q = c.point_at(t);
            if !out.is_empty() {
                out.push('\n');
            }
            let _ = write!(out, "#{} at {:.6},{:.6},{:.6}: ", id.0, q.x, q.y, q.z);
            match c.curvature_at(t).map(|k| k.length()) {
                Some(k) if k > 1e-12 => {
                    let _ = write!(
                        out,
                        "radius of curvature = {:.6} {unit}, curvature = {k:.6} 1/{unit}",
                        1.0 / k
                    );
                }
                Some(_) => out.push_str("straight here (curvature 0)"),
                None => out.push_str("no direction here (a cusp)"),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::{NurbsCurve, Point3};

    fn nurbs(e: &Engine) -> NurbsCurve {
        match &e.doc().objects().last().unwrap().geometry {
            Geometry::Nurbs(n) => n.clone(),
            g => panic!("{}", g.kind()),
        }
    }

    fn apart(a: &NurbsCurve, b: &NurbsCurve) -> f64 {
        let (t0, t1) = a.domain();
        (0..=200)
            .map(|i| {
                let p = a.point_at(t0 + (t1 - t0) * f64::from(i) / 200.0);
                b.point_at(b.closest_param(p)).distance_to(p)
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn insert_then_remove_knot_keeps_the_curve() {
        let mut e = Engine::new();
        e.run_line("Curve 0,0 30,40 70,-20 100,20 130,0").unwrap();
        e.run_line("SelAll").unwrap();
        let c = nurbs(&e);
        let p = c.point_at(0.6);
        let line = format!("InsertKnot {},{},{}", p.x, p.y, p.z);
        let msg = e.run_line(&line).unwrap();
        assert!(msg.contains("6 control points"), "{msg}");
        let k = nurbs(&e);
        assert_eq!(k.points.len(), 6);
        assert!(apart(&c, &k) < 1e-6);
        // Removing it again: the same five points, no deviation.
        let msg = e
            .run_line(&format!("RemoveKnot {},{},{}", p.x, p.y, p.z))
            .unwrap();
        assert!(msg.contains("5 control points"), "{msg}");
        let back = nurbs(&e);
        for (a, b) in back.points.iter().zip(&c.points) {
            assert!(a.distance_to(*b) < 1e-6, "{a} {b}");
        }
        // One undo step per command.
        e.run_line("Undo").unwrap();
        assert_eq!(nurbs(&e).points.len(), 6);
        // Midpoints: one knot per span (two spans → two more points).
        e.run_line("Undo").unwrap();
        e.run_line("InsertKnot Midpoints").unwrap();
        assert_eq!(nurbs(&e).points.len(), 7);
        assert!(apart(&c, &nurbs(&e)) < 1e-6);
    }

    #[test]
    fn insert_knot_turns_a_line_into_nurbs_and_refuses_ends() {
        let mut e = Engine::new();
        e.run_line("Line 0,0,0 100,0,0").unwrap();
        e.run_line("SelAll").unwrap();
        assert!(e.run_line("InsertKnot 0,0,0").is_err());
        e.run_line("InsertKnot 25,0,0 75,3,0").unwrap();
        let n = nurbs(&e);
        assert_eq!(n.points.len(), 4);
        assert!(n.points.iter().all(|p| p.y.abs() < 1e-9));
        assert!(e.run_line("InsertKnot").is_err());
        e.run_line("SelNone").unwrap();
        assert!(e.run_line("InsertKnot 25,0,0").is_err());
    }

    #[test]
    fn remove_knot_reports_the_deviation() {
        let mut e = Engine::new();
        e.run_line("Curve 0,0 10,8 20,0 30,8 40,0 50,8 60,0 70,8")
            .unwrap();
        e.run_line("SelAll").unwrap();
        let c = nurbs(&e);
        let msg = e.run_line("RemoveKnot 35,4,0").unwrap();
        let r = nurbs(&e);
        assert_eq!(r.points.len(), c.points.len() - 1);
        assert!(r.start().distance_to(c.start()) < 1e-9);
        assert!(r.end().distance_to(c.end()) < 1e-9);
        let dev: f64 = msg
            .split("deviation ")
            .nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert!(dev > 1e-3 && dev < 8.0, "{msg}");
        // A curve without interior knots has nothing to remove.
        e.run_line("SelNone").unwrap();
        e.run_line("Curve 0,100 10,110 20,100 30,110").unwrap();
        e.run_line("SelLast").unwrap();
        assert!(e.run_line("RemoveKnot 15,105,0").is_err());
    }

    #[test]
    fn seam_moves_on_circles_polylines_and_free_form_curves() {
        let mut e = Engine::new();
        e.run_line("Circle 0,0,0 50").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("CrvSeam 0,80,0").unwrap();
        let Geometry::Arc(a) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(a.is_closed() && (a.radius - 50.0).abs() < 1e-9);
        assert!(a.start().distance_to(Point3::new(0.0, 50.0, 0.0)) < 1e-9);
        e.run_line("Delete").unwrap();

        e.run_line("Rectangle 0,0,0 100,60,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("CrvSeam 40,-5,0").unwrap();
        let g = e.doc().objects().last().unwrap().geometry.clone();
        let Geometry::Polyline(p) = &g else {
            panic!("{}", g.kind())
        };
        assert!(g.is_closed_curve());
        assert_eq!(p.len(), 6);
        assert!(p[0].distance_to(Point3::new(40.0, 0.0, 0.0)) < 1e-9);
        assert!((g.length().unwrap() - 320.0).abs() < 1e-9);
        // At a corner no vertex is added.
        e.run_line("CrvSeam 100,60,0").unwrap();
        let Geometry::Polyline(p) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert_eq!(p.len(), 6);
        assert!(e.run_line("CrvSeam 100,60,0").is_err());
        e.run_line("Delete").unwrap();

        e.run_line("Ellipse 0,0 10,0 0,4").unwrap();
        e.run_line("SelAll").unwrap();
        let before = nurbs(&e);
        e.run_line("CrvSeam 0,9,0").unwrap();
        let after = nurbs(&e);
        assert!(after.start().distance_to(Point3::new(0.0, 4.0, 0.0)) < 1e-6);
        assert!(after.is_closed());
        assert!(apart(&before, &after) < 1e-6 && apart(&after, &before) < 1e-6);

        // Open curves have no seam.
        e.run_line("Delete").unwrap();
        e.run_line("Line 0,0,0 10,0,0").unwrap();
        e.run_line("SelAll").unwrap();
        assert!(e.run_line("CrvSeam 5,0,0").is_err());
    }

    #[test]
    fn curvature_of_circles_lines_and_ellipses() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Circle 0,0,0 25").unwrap();
        e.run_line("SelAll").unwrap();
        let msg = e.run_line("Curvature 40,0,0").unwrap();
        assert!(msg.contains("radius of curvature = 25.000000 cm"), "{msg}");
        assert!(msg.contains("curvature = 0.040000 1/cm"), "{msg}");
        e.run_line("Delete").unwrap();
        e.run_line("Line 0,0,0 10,0,0").unwrap();
        e.run_line("SelAll").unwrap();
        assert!(e.run_line("Curvature 5,1,0").unwrap().contains("straight"));
        e.run_line("Delete").unwrap();
        // Ellipse a = 10, b = 4: radius b²/a at the ends of the long axis, a²/b
        // at the ends of the short one. Two points in one call.
        e.run_line("Ellipse 0,0 10,0 0,4").unwrap();
        e.run_line("SelAll").unwrap();
        let msg = e.run_line("Curvature 12,0,0 0,5,0").unwrap();
        let lines: Vec<&str> = msg.lines().collect();
        assert_eq!(lines.len(), 2, "{msg}");
        assert!(lines[0].contains("radius of curvature = 1.600000"), "{msg}");
        assert!(
            lines[1].contains("radius of curvature = 25.000000"),
            "{msg}"
        );
        e.run_line("SelNone").unwrap();
        assert!(e.run_line("Curvature 0,0,0").is_err());
    }
}
