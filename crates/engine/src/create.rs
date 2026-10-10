//! Creation commands: curves and solids. All coordinates are world coordinates;
//! an optional trailing vector is the plane normal (default world Z), so the UI
//! can draw on the Top, Front and Right construction planes.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{
    box_mesh, cylinder_mesh, extrude_mesh, sphere_mesh, CircleArc, NurbsCurve, Plane, Point3, Vec3,
};

pub(crate) fn finish(
    ctx: &mut Context,
    geometry: Vec<Geometry>,
    select: bool,
) -> Result<Vec<ObjectId>, CommandError> {
    let mut t = ctx.doc.begin();
    let ids: Vec<ObjectId> = geometry.into_iter().map(|g| t.add(g)).collect();
    t.commit();
    if select {
        ctx.selection = ids.iter().copied().collect();
    }
    Ok(ids)
}

pub(crate) fn positive(v: f64, what: &str, ctx: &Context) -> Result<f64, CommandError> {
    if v.abs() <= ctx.tolerance.absolute {
        Err(CommandError::Invalid(format!("{what} is too small")))
    } else {
        Ok(v)
    }
}

/// `Rectangle <corner> <opposite> [normal]`
pub struct Rectangle;

impl Command for Rectangle {
    fn name(&self) -> &'static str {
        "Rectangle"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["Rec"]
    }
    fn help(&self) -> &'static str {
        "Rectangle <corner> <opposite corner> [normal] — closed rectangle"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first corner", ctx.last_point)?;
        let b = args.point("opposite corner", Some(a))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(a, n);
        let (u, v, _) = plane.coords(b);
        positive(u, "width", ctx)?;
        positive(v, "height", ctx)?;
        let pts = vec![
            plane.point_at(0.0, 0.0, 0.0),
            plane.point_at(u, 0.0, 0.0),
            plane.point_at(u, v, 0.0),
            plane.point_at(0.0, v, 0.0),
            plane.point_at(0.0, 0.0, 0.0),
        ];
        ctx.last_point = Some(b);
        let ids = finish(ctx, vec![Geometry::Polyline(pts)], false)?;
        Ok(format!(
            "added #{} rectangle {:.4} x {:.4}",
            ids[0].0,
            u.abs(),
            v.abs()
        ))
    }
}

/// `Circle <center> <radius> [normal]`
pub struct Circle;

impl Command for Circle {
    fn name(&self) -> &'static str {
        "Circle"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["C"]
    }
    fn help(&self) -> &'static str {
        "Circle <center> <radius> [normal] — circle"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let r = positive(args.number("radius")?, "radius", ctx)?.abs();
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        ctx.last_point = Some(c);
        let ids = finish(
            ctx,
            vec![Geometry::Arc(CircleArc::circle(
                Plane::from_normal(c, n),
                r,
            ))],
            false,
        )?;
        Ok(format!("added #{} circle r {r}", ids[0].0))
    }
}

/// `Arc <center> <start> <end> [normal]` — counter-clockwise around the normal.
pub struct Arc;

impl Command for Arc {
    fn name(&self) -> &'static str {
        "Arc"
    }
    fn help(&self) -> &'static str {
        "Arc <center> <start> <end> [normal] — arc counter-clockwise from start to end"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let s = args.point("start point", Some(c))?;
        let e = args.point("end point", Some(s))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let arc = CircleArc::from_center_start_end(c, s, e, n)
            .ok_or_else(|| CommandError::Invalid("degenerate arc".into()))?;
        positive(arc.radius, "radius", ctx)?;
        ctx.last_point = Some(arc.end());
        let ids = finish(ctx, vec![Geometry::Arc(arc)], false)?;
        Ok(format!("added #{} arc", ids[0].0))
    }
}

/// `Box <corner> <opposite corner> <height> [normal]`
pub struct BoxCmd;

impl Command for BoxCmd {
    fn name(&self) -> &'static str {
        "Box"
    }
    fn help(&self) -> &'static str {
        "Box <corner> <opposite corner> <height> [normal] — box solid"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first corner", ctx.last_point)?;
        let b = args.point("opposite corner", Some(a))?;
        let h = positive(args.number("height")?, "height", ctx)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(a, n);
        let (u, v, _) = plane.coords(b);
        positive(u, "width", ctx)?;
        positive(v, "depth", ctx)?;
        let ids = finish(ctx, vec![Geometry::Mesh(box_mesh(&plane, u, v, h))], false)?;
        Ok(format!(
            "added #{} box {:.4} x {:.4} x {:.4}",
            ids[0].0,
            u.abs(),
            v.abs(),
            h.abs()
        ))
    }
}

/// `Cylinder <center> <radius> <height> [normal]`
pub struct Cylinder;

impl Command for Cylinder {
    fn name(&self) -> &'static str {
        "Cylinder"
    }
    fn help(&self) -> &'static str {
        "Cylinder <base center> <radius> <height> [normal] — cylinder solid"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("base center", ctx.last_point)?;
        let r = positive(args.number("radius")?, "radius", ctx)?.abs();
        let h = positive(args.number("height")?, "height", ctx)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let mesh = cylinder_mesh(&Plane::from_normal(c, n), r, h, 64);
        let ids = finish(ctx, vec![Geometry::Mesh(mesh)], false)?;
        Ok(format!("added #{} cylinder r {r} h {}", ids[0].0, h.abs()))
    }
}

/// `Sphere <center> <radius>`
pub struct Sphere;

impl Command for Sphere {
    fn name(&self) -> &'static str {
        "Sphere"
    }
    fn help(&self) -> &'static str {
        "Sphere <center> <radius> — sphere solid"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let r = positive(args.number("radius")?, "radius", ctx)?.abs();
        let ids = finish(ctx, vec![Geometry::Mesh(sphere_mesh(c, r, 48))], false)?;
        Ok(format!("added #{} sphere r {r}", ids[0].0))
    }
}

/// `Extrude <distance> [direction]` — extrudes the selected curves. Closed curves
/// become capped solids. Without a direction, each curve goes along its own plane
/// normal (world Z for straight lines).
pub struct Extrude;

impl Command for Extrude {
    fn name(&self) -> &'static str {
        "Extrude"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ExtrudeCrv", "Ext"]
    }
    fn help(&self) -> &'static str {
        "Extrude <distance> [direction] — extrude selected curves (closed → solid)"
    }
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let d = positive(args.number("distance")?, "distance", ctx)?;
        let dir = args.optional_vector();
        let ids = ctx.selected("Extrude")?;
        let mut out = Vec::new();
        for id in &ids {
            let g = &ctx.doc.object(*id).expect("selected exists").geometry;
            if !g.is_curve() {
                continue;
            }
            let along = dir
                .or_else(|| g.curve_normal())
                .unwrap_or(Vec3::Z)
                .normalized()
                .unwrap_or(Vec3::Z);
            let pts = g.curve_points();
            out.push(Geometry::Mesh(extrude_mesh(
                &pts,
                along * d,
                g.is_closed_curve(),
            )));
        }
        if out.is_empty() {
            return Err(CommandError::Invalid("Extrude: select curves first".into()));
        }
        let n = out.len();
        finish(ctx, out, true)?;
        Ok(format!("extruded {n} curve(s) by {d}"))
    }
}

/// All remaining tokens as points (each relative to the previous one for `@`).
pub(crate) fn point_list(
    ctx: &Context,
    args: &mut Args,
    first: &'static str,
) -> Result<Vec<Point3>, CommandError> {
    let mut pts = vec![args.point(first, ctx.last_point)?];
    while args.peek().is_some() {
        let last = *pts.last().expect("non-empty");
        pts.push(args.point("next point", Some(last))?);
    }
    Ok(pts)
}

simple_command!(
    Point,
    "Point",
    &["Pt"],
    "Point <point> — add a point object"
);
impl Command for Point {
    impl_meta!(Point);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let p = args.point("point location", ctx.last_point)?;
        ctx.last_point = Some(p);
        let ids = finish(ctx, vec![Geometry::Point(p)], false)?;
        Ok(format!("added #{} point", ids[0].0))
    }
}

simple_command!(
    Points,
    "Points",
    &[],
    "Points <p1> <p2> … — add several point objects"
);
impl Command for Points {
    impl_meta!(Points);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let pts = point_list(ctx, args, "point location")?;
        ctx.last_point = pts.last().copied();
        let n = pts.len();
        finish(ctx, pts.into_iter().map(Geometry::Point).collect(), false)?;
        Ok(format!("added {n} points"))
    }
}

simple_command!(
    Curve,
    "Curve",
    &["Crv"],
    "Curve <p1> <p2> … — degree-3 curve from control points (lower degree for fewer points)"
);
impl Command for Curve {
    impl_meta!(Curve);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut pts = point_list(ctx, args, "start of curve")?;
        pts.dedup_by(|a, b| a.distance_to(*b) <= ctx.tolerance.absolute);
        let c = NurbsCurve::clamped_uniform(&pts, 3)
            .ok_or(CommandError::MissingInput("at least two distinct points"))?;
        ctx.last_point = pts.last().copied();
        let ids = finish(ctx, vec![Geometry::Nurbs(c)], false)?;
        Ok(format!("added #{} curve", ids[0].0))
    }
}

simple_command!(
    InterpCrv,
    "InterpCrv",
    &["Interp"],
    "InterpCrv <p1> <p2> … — degree-3 curve through the points"
);
impl Command for InterpCrv {
    impl_meta!(InterpCrv);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut pts = point_list(ctx, args, "start of curve")?;
        pts.dedup_by(|a, b| a.distance_to(*b) <= ctx.tolerance.absolute);
        if pts.len() < 2 {
            return Err(CommandError::MissingInput("at least two distinct points"));
        }
        let c = NurbsCurve::interpolate(&pts, 3)
            .ok_or_else(|| CommandError::Invalid("cannot interpolate these points".into()))?;
        ctx.last_point = pts.last().copied();
        let ids = finish(ctx, vec![Geometry::Nurbs(c)], false)?;
        Ok(format!("added #{} curve", ids[0].0))
    }
}

simple_command!(
    Ellipse,
    "Ellipse",
    &["El"],
    "Ellipse <center> <end of first axis> <end of second axis | radius> [normal] — exact NURBS ellipse"
);
impl Command for Ellipse {
    impl_meta!(Ellipse);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let a = args.point("end of first axis", Some(c))?;
        let second = args
            .next_token()
            .ok_or(CommandError::MissingInput("end of second axis"))?;
        let x = a - c;
        let ra = positive(x.length(), "first axis", ctx)?;
        let x = x * (1.0 / ra);
        let (n, rb) = match second.parse::<f64>() {
            Ok(r) => {
                let n = args.optional_vector().unwrap_or(Vec3::Z);
                (n, r.abs())
            }
            Err(_) => {
                let b = crate::parse_point(second, Some(c))?;
                let n = args
                    .optional_vector()
                    .or_else(|| x.cross(b - c).normalized())
                    .unwrap_or(Vec3::Z);
                let y = n.cross(x).normalized().unwrap_or(Vec3::Y);
                (n, (b - c).dot(y).abs())
            }
        };
        positive(rb, "second axis", ctx)?;
        let z = (n - x * n.dot(x))
            .normalized()
            .ok_or_else(|| CommandError::Invalid("first axis is parallel to the normal".into()))?;
        let plane = Plane {
            origin: c,
            x,
            y: z.cross(x),
            z,
        };
        let e = NurbsCurve::ellipse(&plane, ra, rb);
        ctx.last_point = Some(c);
        let ids = finish(ctx, vec![Geometry::Nurbs(e)], false)?;
        Ok(format!("added #{} ellipse {ra} x {rb}", ids[0].0))
    }
}

simple_command!(
    Polygon,
    "Polygon",
    &["Pol"],
    "Polygon <center> <corner> [sides=5] [normal] — regular polygon through the corner"
);
impl Command for Polygon {
    impl_meta!(Polygon);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let corner = args.point("corner", Some(c))?;
        let sides = args.optional_number().unwrap_or(5.0);
        if sides < 3.0 || sides.fract().abs() > 1e-9 || sides > 1000.0 {
            return Err(CommandError::Invalid(
                "number of sides must be a whole number ≥ 3".into(),
            ));
        }
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        let start = plane.project(corner);
        positive(start.distance_to(c), "radius", ctx)?;
        let sides = sides as usize;
        let mut pts: Vec<Point3> = (0..sides)
            .map(|i| {
                let x = forma_geom::Xform::rotation(
                    c,
                    plane.z,
                    std::f64::consts::TAU * i as f64 / sides as f64,
                );
                x.point(start)
            })
            .collect();
        pts.push(pts[0]);
        ctx.last_point = Some(c);
        let ids = finish(ctx, vec![Geometry::Polyline(pts)], false)?;
        Ok(format!("added #{} polygon, {sides} sides", ids[0].0))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    fn last(e: &Engine) -> Geometry {
        e.doc().objects().last().unwrap().geometry.clone()
    }

    #[test]
    fn points_and_point() {
        let mut e = Engine::new();
        e.run_line("Point 1,2,3").unwrap();
        e.run_line("Points 0,0 @10,0 @0,10").unwrap();
        assert_eq!(e.doc().len(), 4);
        assert!(e.doc().dump().contains("#1 point [Default] 1,2,3"));
        assert_eq!(last(&e), Geometry::Point(Point3::new(10.0, 10.0, 0.0)));
    }

    #[test]
    fn control_point_and_interpolated_curves() {
        let mut e = Engine::new();
        e.run_line("Curve 0,0 10,10 20,0 30,10").unwrap();
        let Geometry::Nurbs(c) = last(&e) else {
            panic!()
        };
        assert_eq!(c.degree, 3);
        assert!(c.end().distance_to(Point3::new(30.0, 10.0, 0.0)) < 1e-9);
        e.run_line("Crv 0,0 10,10").unwrap();
        let Geometry::Nurbs(c) = last(&e) else {
            panic!()
        };
        assert_eq!(c.degree, 1);
        e.run_line("InterpCrv 0,0 10,10 20,0 30,10 40,0").unwrap();
        let Geometry::Nurbs(c) = last(&e) else {
            panic!()
        };
        let pts = c.sample(200);
        for q in [Point3::new(10.0, 10.0, 0.0), Point3::new(20.0, 0.0, 0.0)] {
            assert!(pts.iter().any(|p| p.distance_to(q) < 0.5), "{q}");
        }
        assert!(e.run_line("InterpCrv 0,0").is_err());
        assert_eq!(e.doc().len(), 3);
    }

    #[test]
    fn ellipse_by_point_and_radius() {
        let mut e = Engine::new();
        e.run_line("Ellipse 0,0 100,0 0,40").unwrap();
        let g = last(&e);
        assert!(g.is_closed_curve());
        let b = g.bounding_box();
        assert!(
            (b.max.x - 100.0).abs() < 1e-6 && (b.max.y - 40.0).abs() < 1e-6,
            "{b:?}"
        );
        e.run_line("Ellipse 0,0,0 0,0,50 20 1,0,0").unwrap();
        let b = last(&e).bounding_box();
        assert!(
            (b.max.z - 50.0).abs() < 1e-6 && (b.max.y - 20.0).abs() < 1e-6,
            "{b:?}"
        );
        assert!(b.max.x.abs() < 1e-9);
        assert!(e.run_line("Ellipse 0,0 0,0 10").is_err());
    }

    #[test]
    fn polygons() {
        let mut e = Engine::new();
        e.run_line("Polygon 0,0 10,0").unwrap();
        let Geometry::Polyline(p) = last(&e) else {
            panic!()
        };
        assert_eq!(p.len(), 6);
        assert!(p
            .iter()
            .all(|q| (q.distance_to(Point3::ORIGIN) - 10.0).abs() < 1e-9));
        e.run_line("Polygon 0,0 10,0 6").unwrap();
        let Geometry::Polyline(p) = last(&e) else {
            panic!()
        };
        assert_eq!(p.len(), 7);
        assert!(p[1].distance_to(Point3::new(5.0, 75f64.sqrt(), 0.0)) < 1e-9);
        assert!(e.run_line("Polygon 0,0 10,0 2").is_err());
    }

    #[test]
    fn rectangle_circle_arc() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 600,400").unwrap();
        e.run_line("Circle 100,100 50").unwrap();
        e.run_line("Arc 0,0 10,0 0,10").unwrap();
        let d = e.doc().dump();
        assert!(d.contains("polyline"), "{d}");
        assert!(d.contains("circle [Default] center 100,100,0 r 50"), "{d}");
        assert!(d.contains("arc"), "{d}");
    }

    #[test]
    fn rectangle_on_front_plane() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0,0 100,0,50 0,-1,0").unwrap();
        let o = e.doc().objects().next().unwrap();
        let b = o.geometry.bounding_box();
        assert!(
            (b.max.x - 100.0).abs() < 1e-9 && (b.max.z - 50.0).abs() < 1e-9 && b.max.y.abs() < 1e-9
        );
    }

    #[test]
    fn solids() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 600,400 18").unwrap();
        e.run_line("Cylinder 0,0 20 100").unwrap();
        e.run_line("Sphere 0,0,500 100").unwrap();
        assert_eq!(e.doc().len(), 3);
        assert!(e.run_line("Box 0,0 0,400 18").is_err());
    }

    #[test]
    fn extrude_selected_rectangle_into_solid() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 600,400").unwrap();
        assert!(e.run_line("Extrude 720").is_err()); // nothing selected
        e.run_line("SelAll").unwrap();
        e.run_line("Extrude 720").unwrap();
        assert_eq!(e.doc().len(), 2);
        let solid = e
            .doc()
            .objects()
            .find(|o| matches!(o.geometry, forma_doc::Geometry::Mesh(_)))
            .unwrap();
        let b = solid.geometry.bounding_box();
        assert!((b.max.z - 720.0).abs() < 1e-9);
        assert_eq!(e.ctx.selection.len(), 1); // the new solid is selected
    }
}
