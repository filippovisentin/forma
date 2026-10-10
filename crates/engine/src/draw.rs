//! More curve creation: circles and arcs through points, rectangle variants,
//! slots, helices and spirals.

use crate::create::{finish, positive};
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::Geometry;
use forma_geom::{
    arc_3pt, circle_3pt, spiral_points, CircleArc, NurbsCurve, Plane, Point3, Seg, Vec3,
};

fn added(ctx: &mut Context, g: Geometry, what: &str) -> CommandResult {
    let ids = finish(ctx, vec![g], false)?;
    Ok(format!("added #{} {what}", ids[0].0))
}

simple_command!(
    Circle3Pt,
    "Circle3Pt",
    &["Circle3Point"],
    "Circle3Pt <p1> <p2> <p3> — circle through three points"
);
impl Command for Circle3Pt {
    impl_meta!(Circle3Pt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first point", ctx.last_point)?;
        let b = args.point("second point", Some(a))?;
        let c = args.point("third point", Some(b))?;
        let circle = circle_3pt(a, b, c)
            .ok_or_else(|| CommandError::Invalid("the points are collinear".into()))?;
        positive(circle.radius, "radius", ctx)?;
        ctx.last_point = Some(c);
        let r = circle.radius;
        added(ctx, Geometry::Arc(circle), &format!("circle r {r:.4}"))
    }
}

simple_command!(
    Circle2Pt,
    "Circle2Pt",
    &["CircleDiameter", "CircleD"],
    "Circle2Pt <p1> <p2> [normal] — circle with p1–p2 as its diameter"
);
impl Command for Circle2Pt {
    impl_meta!(Circle2Pt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of diameter", ctx.last_point)?;
        let b = args.point("end of diameter", Some(a))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let r = positive(a.distance_to(b) / 2.0, "radius", ctx)?;
        let c = a.mid(b);
        let x = (a - c)
            .normalized()
            .ok_or_else(|| CommandError::Invalid("points coincide".into()))?;
        let z = (n - x * n.dot(x))
            .normalized()
            .unwrap_or_else(|| Plane::from_normal(c, x).x);
        let plane = Plane {
            origin: c,
            x,
            y: z.cross(x),
            z,
        };
        ctx.last_point = Some(b);
        added(
            ctx,
            Geometry::Arc(CircleArc::circle(plane, r)),
            &format!("circle r {r:.4}"),
        )
    }
}

simple_command!(
    Arc3Pt,
    "Arc3Pt",
    &["ArcSEP"],
    "Arc3Pt <start> <end> <point on arc> — arc from start to end through a point"
);
impl Command for Arc3Pt {
    impl_meta!(Arc3Pt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let s = args.point("start of arc", ctx.last_point)?;
        let e = args.point("end of arc", Some(s))?;
        let m = args.point("point on arc", Some(e))?;
        let arc = arc_3pt(s, e, m)
            .ok_or_else(|| CommandError::Invalid("the points are collinear".into()))?;
        positive(arc.radius, "radius", ctx)?;
        ctx.last_point = Some(e);
        added(ctx, Geometry::Arc(arc), "arc")
    }
}

/// Closed polyline through four corners.
fn quad(c: [Point3; 4]) -> Geometry {
    Geometry::Polyline(vec![c[0], c[1], c[2], c[3], c[0]])
}

simple_command!(
    Rectangle3Pt,
    "Rectangle3Pt",
    &["Rec3Pt"],
    "Rectangle3Pt <p1> <p2> <width point> — rectangle with edge p1–p2 and width towards the third point"
);
impl Command for Rectangle3Pt {
    impl_meta!(Rectangle3Pt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of edge", ctx.last_point)?;
        let b = args.point("end of edge", Some(a))?;
        let c = args.point("width", Some(b))?;
        let x = b - a;
        let len = positive(x.length(), "edge length", ctx)?;
        let x = x * (1.0 / len);
        let d = c - b;
        let off = d - x * d.dot(x);
        positive(off.length(), "width", ctx)?;
        ctx.last_point = Some(c);
        let (w, l) = (off.length(), len);
        added(
            ctx,
            quad([a, b, b + off, a + off]),
            &format!("rectangle {l:.4} x {w:.4}"),
        )
    }
}

simple_command!(
    RectangleCenter,
    "RectangleCenter",
    &["RecCenter"],
    "RectangleCenter <center> <corner> [normal] — rectangle from its centre and one corner"
);
impl Command for RectangleCenter {
    impl_meta!(RectangleCenter);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let k = args.point("corner", Some(c))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        let (u, v, _) = plane.coords(k);
        positive(u, "width", ctx)?;
        positive(v, "height", ctx)?;
        let (u, v) = (u.abs(), v.abs());
        ctx.last_point = Some(k);
        added(
            ctx,
            quad([
                plane.point_at(-u, -v, 0.0),
                plane.point_at(u, -v, 0.0),
                plane.point_at(u, v, 0.0),
                plane.point_at(-u, v, 0.0),
            ]),
            &format!("rectangle {:.4} x {:.4}", 2.0 * u, 2.0 * v),
        )
    }
}

simple_command!(
    RoundedRectangle,
    "RoundedRectangle",
    &["RecRounded"],
    "RoundedRectangle <corner> <opposite corner> <radius> [normal] — rectangle with rounded corners"
);
impl Command for RoundedRectangle {
    impl_meta!(RoundedRectangle);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first corner", ctx.last_point)?;
        let b = args.point("opposite corner", Some(a))?;
        let r = positive(args.number("corner radius")?, "radius", ctx)?.abs();
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(a, n);
        let (u, v, _) = plane.coords(b);
        positive(u, "width", ctx)?;
        positive(v, "height", ctx)?;
        if r * 2.0 > u.abs().min(v.abs()) + ctx.tolerance.absolute {
            return Err(CommandError::Invalid(
                "radius is larger than half the shorter side".into(),
            ));
        }
        let (u0, u1) = (u.min(0.0), u.max(0.0));
        let (v0, v1) = (v.min(0.0), v.max(0.0));
        let q = |a: f64, b: f64| plane.point_at(a, b, 0.0);
        let corner = |c: (f64, f64), from: (f64, f64), to: (f64, f64)| {
            CircleArc::from_center_start_end(q(c.0, c.1), q(from.0, from.1), q(to.0, to.1), plane.z)
                .map(Seg::Arc)
                .ok_or_else(|| CommandError::Invalid("degenerate corner".into()))
        };
        let mut segs = Vec::new();
        let tol = ctx.tolerance.absolute;
        let line = |segs: &mut Vec<Seg>, a: (f64, f64), b: (f64, f64)| {
            if q(a.0, a.1).distance_to(q(b.0, b.1)) > tol {
                segs.push(Seg::Line(q(a.0, a.1), q(b.0, b.1)));
            }
        };
        line(&mut segs, (u0 + r, v0), (u1 - r, v0));
        segs.push(corner((u1 - r, v0 + r), (u1 - r, v0), (u1, v0 + r))?);
        line(&mut segs, (u1, v0 + r), (u1, v1 - r));
        segs.push(corner((u1 - r, v1 - r), (u1, v1 - r), (u1 - r, v1))?);
        line(&mut segs, (u1 - r, v1), (u0 + r, v1));
        segs.push(corner((u0 + r, v1 - r), (u0 + r, v1), (u0, v1 - r))?);
        line(&mut segs, (u0, v1 - r), (u0, v0 + r));
        segs.push(corner((u0 + r, v0 + r), (u0, v0 + r), (u0 + r, v0))?);
        ctx.last_point = Some(b);
        let g = Geometry::PolyCurve(segs);
        added(ctx, g, "rounded rectangle")
    }
}

simple_command!(
    Slot,
    "Slot",
    &[],
    "Slot <center1> <center2> <width> [normal] — closed slot (two half circles joined by lines)"
);
impl Command for Slot {
    impl_meta!(Slot);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first centre", ctx.last_point)?;
        let b = args.point("second centre", Some(a))?;
        let w = positive(args.number("slot width")?, "width", ctx)?.abs();
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let x = b - a;
        positive(x.length(), "slot length", ctx)?;
        let x = (x - n * x.dot(n.normalized().unwrap_or(Vec3::Z)))
            .normalized()
            .ok_or_else(|| CommandError::Invalid("slot axis is parallel to the normal".into()))?;
        let z = n.normalized().unwrap_or(Vec3::Z);
        let y = z.cross(x);
        let r = w / 2.0;
        let arc = |c: Point3, from: Point3, to: Point3| {
            CircleArc::from_center_start_end(c, from, to, z).map(Seg::Arc)
        };
        let (p1, p2) = (a - y * r, b - y * r);
        let (p3, p4) = (b + y * r, a + y * r);
        let segs = vec![
            Seg::Line(p1, p2),
            arc(b, p2, p3).ok_or_else(|| CommandError::Invalid("degenerate slot".into()))?,
            Seg::Line(p3, p4),
            arc(a, p4, p1).ok_or_else(|| CommandError::Invalid("degenerate slot".into()))?,
        ];
        ctx.last_point = Some(b);
        added(ctx, Geometry::PolyCurve(segs), "slot")
    }
}

/// Smooth curve through spiral samples (a polyline for very long ones).
fn spiral_curve(points: Vec<Point3>) -> Geometry {
    if points.len() <= 400 {
        if let Some(c) = NurbsCurve::interpolate(&points, 3) {
            return Geometry::Nurbs(c);
        }
    }
    Geometry::Polyline(points)
}

fn turns(args: &mut Args) -> Result<f64, CommandError> {
    let t = args.number("number of turns")?;
    if t.abs() < 1e-9 || t.abs() > 1000.0 {
        return Err(CommandError::Invalid(
            "turns must be between 0 and 1000".into(),
        ));
    }
    Ok(t)
}

simple_command!(
    Helix,
    "Helix",
    &[],
    "Helix <axis start> <axis end> <radius> <turns> — helix around the axis (negative turns: clockwise)"
);
impl Command for Helix {
    impl_meta!(Helix);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of axis", ctx.last_point)?;
        let b = args.point("end of axis", Some(a))?;
        let r = positive(args.number("radius")?, "radius", ctx)?.abs();
        let t = turns(args)?;
        let h = positive(a.distance_to(b), "axis length", ctx)?;
        let plane = Plane::from_normal(a, b - a);
        let pts = spiral_points(&plane, r, r, t, h, 16);
        ctx.last_point = Some(b);
        added(ctx, spiral_curve(pts), &format!("helix, {t} turns"))
    }
}

simple_command!(
    Spiral,
    "Spiral",
    &[],
    "Spiral <center> <start radius> <end radius> <turns> [height] [normal] — flat (or rising) spiral"
);
impl Command for Spiral {
    impl_meta!(Spiral);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let r0 = args.number("start radius")?.abs();
        let r1 = args.number("end radius")?.abs();
        positive(r0.max(r1), "radius", ctx)?;
        let t = turns(args)?;
        let h = args.optional_number().unwrap_or(0.0);
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let pts = spiral_points(&Plane::from_normal(c, n), r0, r1, t, h, 16);
        ctx.last_point = Some(c);
        added(ctx, spiral_curve(pts), &format!("spiral, {t} turns"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    const TOL: f64 = 1e-6;

    fn last(e: &Engine) -> Geometry {
        e.doc().objects().last().unwrap().geometry.clone()
    }

    #[test]
    fn circles_through_points() {
        let mut e = Engine::new();
        e.run_line("Circle3Pt 10,0 0,10 -10,0").unwrap();
        let Geometry::Arc(c) = last(&e) else { panic!() };
        assert!((c.radius - 10.0).abs() < TOL && c.center().distance_to(Point3::ORIGIN) < TOL);
        assert!(e.run_line("Circle3Pt 0,0 1,0 2,0").is_err());
        e.run_line("Circle2Pt 0,0 0,0,40 1,0,0").unwrap();
        let Geometry::Arc(c) = last(&e) else { panic!() };
        assert!((c.radius - 20.0).abs() < TOL);
        assert!(c.center().distance_to(Point3::new(0.0, 0.0, 20.0)) < TOL);
        assert!(c.plane.z.x.abs() > 0.999); // in the YZ plane
        e.run_line("circled 0,0 30,0").unwrap();
        assert_eq!(e.doc().len(), 3);
    }

    #[test]
    fn arc_through_a_point() {
        let mut e = Engine::new();
        e.run_line("Arc3Pt 0,0 100,0 50,20").unwrap();
        let Geometry::Arc(a) = last(&e) else { panic!() };
        assert!(a.start().distance_to(Point3::ORIGIN) < TOL);
        assert!(a.end().distance_to(Point3::new(100.0, 0.0, 0.0)) < TOL);
        assert!(a.mid().distance_to(Point3::new(50.0, 20.0, 0.0)) < TOL);
    }

    #[test]
    fn rectangle_variants() {
        let mut e = Engine::new();
        e.run_line("Rectangle3Pt 0,0 100,100 0,200").unwrap();
        let g = last(&e);
        assert!(g.is_closed_curve());
        let len = g.length().unwrap();
        let (l, w) = (100.0 * 2f64.sqrt(), 100.0 * 2f64.sqrt());
        assert!((len - 2.0 * (l + w)).abs() < TOL, "{len}");
        e.run_line("RectangleCenter 0,0 30,20").unwrap();
        let b = last(&e).bounding_box();
        assert!((b.min.x + 30.0).abs() < TOL && (b.max.y - 20.0).abs() < TOL);
        e.run_line("RoundedRectangle 0,0 100,60 10").unwrap();
        let g = last(&e);
        assert!(g.is_closed_curve());
        let Geometry::PolyCurve(s) = &g else {
            panic!("{g:?}")
        };
        assert_eq!(s.len(), 8);
        let pi = std::f64::consts::PI;
        let expect = 2.0 * (80.0 + 40.0) + 2.0 * pi * 10.0;
        assert!((g.length().unwrap() - expect).abs() < 1e-6);
        assert!(e.run_line("RoundedRectangle 0,0 100,60 31").is_err());
    }

    #[test]
    fn slot_helix_spiral() {
        let mut e = Engine::new();
        e.run_line("Slot 0,0 100,0 20").unwrap();
        let g = last(&e);
        assert!(g.is_closed_curve());
        let expect = 200.0 + std::f64::consts::PI * 20.0;
        assert!((g.length().unwrap() - expect).abs() < 1e-6);
        e.run_line("Helix 0,0,0 0,0,100 10 4").unwrap();
        let g = last(&e);
        let b = g.bounding_box();
        assert!(
            (b.max.z - 100.0).abs() < 1e-6 && (b.max.x - 10.0).abs() < 0.1,
            "{b:?}"
        );
        e.run_line("Spiral 0,0 5 50 3").unwrap();
        let b = last(&e).bounding_box();
        assert!(b.max.z.abs() < 1e-9 && b.max.x > 40.0);
        assert!(e.run_line("Helix 0,0 0,0,10 5 0").is_err());
    }
}
