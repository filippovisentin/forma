//! Creation commands: curves and solids. All coordinates are world coordinates;
//! an optional trailing vector is the plane normal (default world Z), so the UI
//! can draw on the Top, Front and Right construction planes.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{box_mesh, cylinder_mesh, extrude_mesh, sphere_mesh, CircleArc, Plane, Vec3};

fn finish(
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

fn positive(v: f64, what: &str, ctx: &Context) -> Result<f64, CommandError> {
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

#[cfg(test)]
mod tests {
    use crate::Engine;

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
