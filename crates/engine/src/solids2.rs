//! More surfaces and solids as meshes (no solid kernel): Plane, SrfPt, EdgeSrf,
//! Cone, TCone, Torus, Ellipsoid, Pyramid, Tube, Pipe, ExtrudeCrvAlongCrv,
//! ExtrudeCrvTapered, Slab; mesh Weld / Unweld.

use crate::create::{finish, positive};
use crate::edit::parse_ids;
use crate::surfaces::{add_selected, closed_plane, loop_points};
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, Object};
use forma_geom::{
    cap_planar_holes, cone_mesh, coons_mesh, edge_curves_to_sides, ellipsoid_mesh, extrude_mesh,
    grid_mesh, loft_mesh, offset, pipe_mesh, planar_mesh, pyramid_mesh, tapered_extrude_mesh,
    torus_mesh, tube_mesh, Chain, Mesh, Plane, Point3, Vec3, Xform,
};

/// Adds an exact surface / solid, or the mesh when it could not be made.
fn added_exact(
    ctx: &mut Context,
    exact: Option<forma_geom::Brep>,
    mesh: impl FnOnce() -> Mesh,
    what: &str,
) -> CommandResult {
    match exact {
        Some(b) => {
            let ids = finish(ctx, vec![Geometry::Brep(b)], false)?;
            Ok(format!("added #{} {what}", ids[0].0))
        }
        None => added(ctx, mesh(), what),
    }
}

fn added(ctx: &mut Context, m: Mesh, what: &str) -> CommandResult {
    if m.triangles.is_empty() {
        return Err(CommandError::Invalid(format!("{what}: degenerate result")));
    }
    let ids = finish(ctx, vec![Geometry::Mesh(m)], false)?;
    Ok(format!("added #{} {what}", ids[0].0))
}

simple_command!(
    PlaneSrf,
    "Plane",
    &["Rectangle3D"],
    "Plane <corner> <opposite corner> [normal] — rectangular planar surface"
);
impl Command for PlaneSrf {
    impl_meta!(PlaneSrf);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first corner", ctx.last_point)?;
        let b = args.point("opposite corner", Some(a))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(a, n);
        let (u, v, _) = plane.coords(b);
        positive(u, "width", ctx)?;
        positive(v, "height", ctx)?;
        let mut pts = vec![
            plane.point_at(0.0, 0.0, 0.0),
            plane.point_at(u, 0.0, 0.0),
            plane.point_at(u, v, 0.0),
            plane.point_at(0.0, v, 0.0),
        ];
        if u * v < 0.0 {
            pts.reverse();
        }
        ctx.last_point = Some(b);
        let mut outline = pts.clone();
        outline.push(pts[0]);
        let exact =
            forma_io_3dm::exact::make_planar(&Geometry::Polyline(outline), &[], plane.z, None);
        added_exact(ctx, exact, || planar_mesh(&pts, &[], plane.z), "plane")
    }
}

simple_command!(
    SrfPt,
    "SrfPt",
    &[],
    "SrfPt <p1> <p2> <p3> [p4] — surface from 3 or 4 corner points (bilinear when not flat)"
);
impl Command for SrfPt {
    impl_meta!(SrfPt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first corner", ctx.last_point)?;
        let b = args.point("second corner", Some(a))?;
        let c = args.point("third corner", Some(b))?;
        let d = match args.peek() {
            Some(_) => Some(args.point("fourth corner", Some(c))?),
            None => None,
        };
        // Exact: a trimmed plane (3 corners) or the ruled surface between two
        // opposite sides (4 corners, bilinear).
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let exact = match d {
            None => {
                let n = (b - a).cross(c - a);
                if n.length() <= ctx.tolerance.absolute {
                    None
                } else {
                    forma_io_3dm::exact::make_planar(
                        &Geometry::Polyline(vec![a, b, c, a]),
                        &[],
                        n,
                        None,
                    )
                }
            }
            Some(d) => {
                let (s0, s1) = (
                    Geometry::Line(forma_geom::LineCurve::new(a, b)),
                    Geometry::Line(forma_geom::LineCurve::new(d, c)),
                );
                forma_io_3dm::exact::make_loft(&[&s0, &s1], true, tol)
            }
        };
        if let Some(bx) = exact.filter(|bx| bx.mesh.area() > ctx.tolerance.absolute) {
            ctx.last_point = Some(d.unwrap_or(c));
            return added_exact(ctx, Some(bx), Mesh::default, "surface");
        }
        let m = match d {
            None => {
                let n = (b - a).cross(c - a);
                positive(n.length(), "triangle area", ctx)?;
                planar_mesh(&[a, b, c], &[], n)
            }
            Some(d) => {
                // Bilinear patch a-b-c-d (a→b one side, d→c the opposite one).
                let n = 8;
                let rows: Vec<Vec<Point3>> = (0..=n)
                    .map(|j| {
                        let v = j as f64 / n as f64;
                        let (p, q) = (a + (d - a) * v, b + (c - b) * v);
                        (0..=n)
                            .map(|i| p + (q - p) * (i as f64 / n as f64))
                            .collect()
                    })
                    .collect();
                loft_mesh(&rows, false)
            }
        };
        positive(m.area(), "surface area", ctx)?;
        ctx.last_point = Some(d.unwrap_or(c));
        added(ctx, m, "surface")
    }
}

simple_command!(
    EdgeSrf,
    "EdgeSrf",
    &[],
    "EdgeSrf — surface from 2, 3 or 4 selected edge curves that meet end to end"
);
impl Command for EdgeSrf {
    impl_meta!(EdgeSrf);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let curves = crate::surfaces::selected_curves(ctx, "EdgeSrf")?;
        if !(2..=4).contains(&curves.len()) {
            return Err(CommandError::Invalid(
                "EdgeSrf: select 2, 3 or 4 open curves".into(),
            ));
        }
        let geoms: Vec<&Geometry> = curves.iter().map(|c| &c.geometry).collect();
        if let Some(b) =
            forma_io_3dm::exact::make_edge_srf(&geoms, ctx.doc.absolute_tolerance.max(1e-9))
        {
            let src = curves[0].clone();
            add_selected(ctx, vec![(Geometry::Brep(b), src)]);
            return Ok("edge surface added".into());
        }
        let pts: Vec<Vec<Point3>> = curves.iter().map(|c| c.geometry.curve_points()).collect();
        let tol = ctx.doc.absolute_tolerance.max(1e-6) * 10.0;
        let sides = edge_curves_to_sides(&pts, tol).ok_or_else(|| {
            CommandError::Invalid("EdgeSrf: the curves must meet end to end".into())
        })?;
        let nu = sides[0].len().max(sides[2].len()).clamp(2, 64);
        let nv = sides[1].len().max(sides[3].len()).clamp(2, 64);
        let m = coons_mesh(
            &sides[0],
            &sides[1],
            &sides[2],
            &sides[3],
            nu.max(8),
            nv.max(8),
        );
        if m.triangles.is_empty() {
            return Err(CommandError::Invalid("EdgeSrf: degenerate surface".into()));
        }
        let src = curves[0].clone();
        add_selected(ctx, vec![(Geometry::Mesh(m), src)]);
        Ok("edge surface added".into())
    }
}

simple_command!(
    Cone,
    "Cone",
    &[],
    "Cone <base center> <radius> <height> [normal] — cone solid (apex at height)"
);
impl Command for Cone {
    impl_meta!(Cone);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("base center", ctx.last_point)?;
        let r = positive(args.number("radius")?, "radius", ctx)?.abs();
        let h = positive(args.number("height")?, "height", ctx)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        // Apex above the base (below for a negative height).
        let base = Plane::from_normal(c, if h < 0.0 { -n } else { n });
        let exact = forma_io_3dm::exact::make_cone(&base, r, h.abs());
        added_exact(
            ctx,
            exact,
            || cone_mesh(&Plane::from_normal(c, n), r, 0.0, h, 64),
            "cone",
        )
    }
}

simple_command!(
    TCone,
    "TCone",
    &["TruncatedCone"],
    "TCone <base center> <base radius> <top radius> <height> [normal] — truncated cone solid"
);
impl Command for TCone {
    impl_meta!(TCone);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("base center", ctx.last_point)?;
        let r0 = positive(args.number("base radius")?, "base radius", ctx)?.abs();
        let r1 = args.number("top radius")?.abs();
        let h = positive(args.number("height")?, "height", ctx)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        // Exact: the profile (axis, base radius, slanted side, top radius)
        // revolved around the axis.
        let top = c + plane.z * h;
        let mut profile = vec![c, c + plane.x * r0, top + plane.x * r1];
        if r1 > ctx.tolerance.absolute {
            profile.push(top);
        }
        let exact = forma_io_3dm::exact::make_revolved(
            &Geometry::Polyline(profile),
            c,
            plane.z,
            std::f64::consts::TAU,
            ctx.doc.absolute_tolerance.max(1e-9),
        );
        added_exact(
            ctx,
            exact,
            || cone_mesh(&plane, r0, r1, h, 64),
            "truncated cone",
        )
    }
}

simple_command!(
    Torus,
    "Torus",
    &[],
    "Torus <center> <major radius> <minor radius> [normal] — torus solid"
);
impl Command for Torus {
    impl_meta!(Torus);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let r = positive(args.number("major radius")?, "major radius", ctx)?.abs();
        let s = positive(args.number("minor radius")?, "minor radius", ctx)?.abs();
        if s >= r {
            return Err(CommandError::Invalid(
                "the minor radius must be smaller than the major radius".into(),
            ));
        }
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        let exact = forma_io_3dm::exact::make_torus(&plane, r, s);
        added_exact(ctx, exact, || torus_mesh(&plane, r, s, 64), "torus")
    }
}

simple_command!(
    Ellipsoid,
    "Ellipsoid",
    &[],
    "Ellipsoid <center> <x radius> <y radius> <z radius> [normal] — ellipsoid solid"
);
impl Command for Ellipsoid {
    impl_meta!(Ellipsoid);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let rx = positive(args.number("x radius")?, "x radius", ctx)?.abs();
        let ry = positive(args.number("y radius")?, "y radius", ctx)?.abs();
        let rz = positive(args.number("z radius")?, "z radius", ctx)?.abs();
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        // A unit sphere stretched along the plane axes (exact NURBS).
        let stretch = Xform::scale_axes(&plane, rx, ry, rz);
        let exact = forma_io_3dm::exact::make_sphere(c, 1.0)
            .and_then(|s| forma_io_3dm::exact::baked(&s.transformed(&stretch)));
        added_exact(
            ctx,
            exact,
            || ellipsoid_mesh(&plane, rx, ry, rz, 48),
            "ellipsoid",
        )
    }
}

simple_command!(
    Pyramid,
    "Pyramid",
    &[],
    "Pyramid <center> <corner> <sides> <height> [normal] — pyramid on a regular polygon"
);
impl Command for Pyramid {
    impl_meta!(Pyramid);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let corner = args.point("corner", Some(c))?;
        let sides = args.number("number of sides")?;
        if sides < 3.0 || sides.fract().abs() > 1e-9 || sides > 1000.0 {
            return Err(CommandError::Invalid(
                "number of sides must be a whole number ≥ 3".into(),
            ));
        }
        let h = positive(args.number("height")?, "height", ctx)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        let start = plane.project(corner);
        positive(start.distance_to(c), "radius", ctx)?;
        let k = sides as usize;
        let base: Vec<Point3> = (0..k)
            .map(|i| {
                Xform::rotation(c, plane.z, std::f64::consts::TAU * i as f64 / k as f64)
                    .point(start)
            })
            .collect();
        let m = pyramid_mesh(&base, c + plane.z * h);
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let exact = forma_io_3dm::exact::polyhedron_brep(&m, tol, 2000);
        added_exact(ctx, exact, || m, "pyramid")
    }
}

simple_command!(
    Tube,
    "Tube",
    &[],
    "Tube <base center> <outer radius> <inner radius> <height> [normal] — hollow cylinder solid"
);
impl Command for Tube {
    impl_meta!(Tube);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("base center", ctx.last_point)?;
        let r0 = positive(args.number("outer radius")?, "outer radius", ctx)?.abs();
        let r1 = positive(args.number("inner radius")?, "inner radius", ctx)?.abs();
        if (r0 - r1).abs() <= ctx.tolerance.absolute {
            return Err(CommandError::Invalid("the two radii must differ".into()));
        }
        let h = positive(args.number("height")?, "height", ctx)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(c, n);
        // Exact: the ring between the two circles, extruded.
        let base = if h < 0.0 {
            Plane::from_normal(c + plane.z * h, n)
        } else {
            plane
        };
        let (outer, inner) = (r0.max(r1), r0.min(r1));
        let circle = |r| Geometry::Arc(forma_geom::CircleArc::circle(base, r));
        let exact = forma_io_3dm::exact::make_planar(
            &circle(outer),
            &[&circle(inner)],
            base.z,
            Some(base.z * h.abs()),
        );
        added_exact(ctx, exact, || tube_mesh(&plane, r0, r1, h, 64), "tube")
    }
}

simple_command!(
    Pipe,
    "Pipe",
    &[],
    "Pipe <radius> [open] — round pipes along the selected curves (capped unless open)"
);
impl Command for Pipe {
    impl_meta!(Pipe);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let r = positive(args.number("pipe radius")?, "radius", ctx)?.abs();
        let open = args.keyword("open");
        let curves = crate::surfaces::selected_curves(ctx, "Pipe")?;
        if curves.is_empty() {
            return Err(CommandError::Invalid("Pipe: select curves".into()));
        }
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out: Vec<(Geometry, Object)> = Vec::new();
        for c in curves {
            let mut rail = c.geometry.curve_points();
            rail.dedup_by(|a, b| a.distance_to(*b) < 1e-9);
            if rail.len() < 2 {
                continue;
            }
            // Exact: a circle swept along the rail, capped with flat discs.
            if let Some(t) = (rail[1] - rail[0]).normalized() {
                let circle = Geometry::Arc(forma_geom::CircleArc::circle(
                    Plane::from_normal(rail[0], t),
                    r,
                ));
                let closed = c.geometry.is_closed_curve();
                if let Some(mut b) = crate::surfaces::sweep1_exact(&circle, &rail, closed, tol) {
                    if !open && !c.geometry.is_closed_curve() {
                        if let Some((capped, _)) = forma_io_3dm::exact::capped(&b, tol) {
                            b = capped;
                        }
                    }
                    out.push((Geometry::Brep(b), c));
                    continue;
                }
            }
            let mut m = pipe_mesh(&rail, r, 32);
            if !open && !c.geometry.is_closed_curve() {
                m = cap_planar_holes(&m, tol).0;
            }
            if m.volume() < 0.0 {
                m = m.flipped();
            }
            out.push((Geometry::Mesh(m), c));
        }
        if out.is_empty() {
            return Err(CommandError::Invalid("Pipe: degenerate curves".into()));
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("{n} pipe(s)"))
    }
}

/// Profile points of a curve: closed loops without the repeated point.
fn profile(g: &Geometry) -> (Vec<Point3>, bool) {
    if g.is_closed_curve() {
        (loop_points(g), true)
    } else {
        (g.curve_points(), false)
    }
}

simple_command!(
    ExtrudeCrvAlongCrv,
    "ExtrudeCrvAlongCrv",
    &["ExtrudeAlongCrv"],
    "ExtrudeCrvAlongCrv #path — extrude the selected curves along the path curve (no rotation; closed planar profiles are capped)"
);
impl Command for ExtrudeCrvAlongCrv {
    impl_meta!(ExtrudeCrvAlongCrv);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("path curve #id"))?;
        let path_id = parse_ids(vec![tok])?[0];
        let path_geometry = ctx
            .doc
            .object(path_id)
            .filter(|o| o.geometry.is_curve())
            .ok_or_else(|| CommandError::Invalid(format!("#{} is not a curve", path_id.0)))?
            .geometry
            .clone();
        let path = path_geometry.curve_points();
        let curves: Vec<Object> = crate::surfaces::selected_curves(ctx, "ExtrudeCrvAlongCrv")?
            .into_iter()
            .filter(|o| o.id != path_id)
            .collect();
        if curves.is_empty() {
            return Err(CommandError::Invalid(
                "ExtrudeCrvAlongCrv: select the profile curves".into(),
            ));
        }
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        for c in curves {
            // Exact: the profile moved along the path, capped when it is a
            // closed planar curve.
            if let Some(mut s) = forma_io_3dm::exact::make_along(&c.geometry, &path_geometry, tol) {
                if c.geometry.is_closed_curve() && closed_plane(&c.geometry, tol).is_some() {
                    if let Some((capped, _)) = forma_io_3dm::exact::capped(&s, tol) {
                        s = capped;
                    }
                }
                out.push((Geometry::Brep(s), c));
                continue;
            }
            let (prof, closed) = profile(&c.geometry);
            let rows: Vec<Vec<Point3>> = path
                .iter()
                .map(|p| {
                    let d = *p - path[0];
                    prof.iter().map(|q| *q + d).collect()
                })
                .collect();
            let mut m = grid_mesh(&rows, closed);
            if closed && closed_plane(&c.geometry, tol).is_some() {
                m = cap_planar_holes(&m, tol).0;
                if m.volume() < 0.0 {
                    m = m.flipped();
                }
            }
            if !m.triangles.is_empty() {
                out.push((Geometry::Mesh(m), c));
            }
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("extruded {n} curve(s) along #{}", path_id.0))
    }
}

simple_command!(
    ExtrudeCrvTapered,
    "ExtrudeCrvTapered",
    &["ExtrudeTapered"],
    "ExtrudeCrvTapered <distance> <draft angle°> — extrude the selected closed planar curves along their normal, shrinking by the draft angle (negative angle grows)"
);
impl Command for ExtrudeCrvTapered {
    impl_meta!(ExtrudeCrvTapered);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let d = positive(args.number("extrusion distance")?, "distance", ctx)?;
        let a = args.number("draft angle")?;
        if a.abs() >= 89.0 {
            return Err(CommandError::Invalid(
                "draft angle must be below 89°".into(),
            ));
        }
        let curves = crate::surfaces::selected_curves(ctx, "ExtrudeCrvTapered")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let inset = d.abs() * a.to_radians().tan();
        let mut out = Vec::new();
        for c in curves {
            let Some((plane, pts)) = closed_plane(&c.geometry, tol) else {
                continue;
            };
            let m = tapered_extrude_mesh(&pts, plane.z, d, inset);
            if !m.triangles.is_empty() {
                out.push((Geometry::Mesh(m), c));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "ExtrudeCrvTapered: select closed planar curves".into(),
            ));
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("extruded {n} curve(s) by {d} with {a}° draft"))
    }
}

simple_command!(
    ExtrudeCrvToPoint,
    "ExtrudeCrvToPoint",
    &[],
    "ExtrudeCrvToPoint <apex> — extrude the selected curves to a point (closed planar curves give capped solids)"
);
impl Command for ExtrudeCrvToPoint {
    impl_meta!(ExtrudeCrvToPoint);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let apex = args.point("point to extrude to", ctx.last_point)?;
        let curves = crate::surfaces::selected_curves(ctx, "ExtrudeCrvToPoint")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        for c in curves {
            let closed = c.geometry.is_closed_curve();
            let mut pts = if closed {
                loop_points(&c.geometry)
            } else {
                c.geometry.curve_points()
            };
            if pts.len() < 2 {
                continue;
            }
            let plane = closed_plane(&c.geometry, tol);
            if let Some((pl, _)) = &plane {
                if pl.coords(apex).2.abs() <= tol {
                    return Err(CommandError::Invalid(
                        "ExtrudeCrvToPoint: the point lies in the curve's plane".into(),
                    ));
                }
            }
            // Exact cone faces (and the flat base of a closed planar curve).
            let base = plane.as_ref().map(|(pl, _)| pl.z).filter(|_| closed);
            if let Some(s) = forma_io_3dm::exact::make_to_point(&c.geometry, apex, base, tol) {
                out.push((Geometry::Brep(s), c));
                continue;
            }
            if let Some((pl, _)) = &plane {
                // Loop counter-clockwise seen from the apex side: sides face out.
                let n = forma_geom::newell_area(&pts);
                if n.dot(apex - pts[0]) < 0.0 {
                    pts.reverse();
                }
                if pl.coords(apex).2.abs() <= tol {
                    return Err(CommandError::Invalid(
                        "ExtrudeCrvToPoint: the point lies in the curve's plane".into(),
                    ));
                }
            }
            let mut m = Mesh::default();
            let count = if closed { pts.len() } else { pts.len() - 1 };
            for i in 0..count {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (b - a).cross(apex - a).length() <= 1e-12 {
                    continue;
                }
                let k = m.positions.len() as u32;
                m.positions.extend([a, b, apex]);
                m.triangles.push([k, k + 1, k + 2]);
            }
            if let (true, Some(_)) = (closed, &plane) {
                let d = forma_geom::newell_area(&pts);
                let mut cap = planar_mesh(&pts, &[], -d);
                cap.normals.clear();
                m.append(&cap);
            }
            if !m.triangles.is_empty() {
                out.push((Geometry::Mesh(m), c));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "ExtrudeCrvToPoint: select curves".into(),
            ));
        }
        let n = out.len();
        ctx.last_point = Some(apex);
        add_selected(ctx, out);
        Ok(format!("extruded {n} curve(s) to a point"))
    }
}

simple_command!(
    Slab,
    "Slab",
    &[],
    "Slab <thickness> <height> [center|left|right] — solids from the selected planar curves offset by the thickness (walls from axis lines; default centred)"
);
impl Command for Slab {
    impl_meta!(Slab);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let t = positive(args.number("thickness")?, "thickness", ctx)?.abs();
        let h = positive(args.number("height")?, "height", ctx)?;
        let side = if args.keyword("left") {
            1.0
        } else if args.keyword("right") {
            -1.0
        } else {
            let _ = args.keyword("center") || args.keyword("centre");
            0.0
        };
        let curves = crate::surfaces::selected_curves(ctx, "Slab")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        for c in curves {
            let Some(chain) = c.geometry.to_chain() else {
                continue;
            };
            let n = c.geometry.curve_normal().unwrap_or(Vec3::Z);
            let n = if n.dot(Vec3::Z) < 0.0 { -n } else { n };
            let plane = Plane::from_normal(chain.start(), n);
            let pts = chain.points();
            if pts.iter().any(|p| plane.coords(*p).2.abs() > tol) {
                continue; // not planar
            }
            // The two sides of the slab.
            let (d_a, d_b) = if side == 0.0 {
                (t / 2.0, -t / 2.0)
            } else {
                (0.0, side * t)
            };
            let side_chain = |d: f64| -> Option<Chain> {
                if d.abs() < 1e-12 {
                    Some(chain.clone())
                } else {
                    offset(&chain, d.abs(), d.signum(), &plane, tol).ok()
                }
            };
            let (Some(a), Some(b)) = (side_chain(d_a), side_chain(d_b)) else {
                continue;
            };
            // Exact: the outline of the slab (a ring for closed curves) extruded.
            let exact = if c.geometry.is_closed_curve() {
                let (ga, gb) = (
                    Geometry::from_chain(a.clone()),
                    Geometry::from_chain(b.clone()),
                );
                match (ga, gb) {
                    (Some(ga), Some(gb)) => {
                        let (pa, pb) = (strip_closing(a.points()), strip_closing(b.points()));
                        let (outer, inner) = if forma_geom::newell_area(&pa).length()
                            >= forma_geom::newell_area(&pb).length()
                        {
                            (ga, gb)
                        } else {
                            (gb, ga)
                        };
                        forma_io_3dm::exact::make_planar(
                            &outer,
                            &[&inner],
                            plane.z,
                            Some(plane.z * h),
                        )
                    }
                    _ => None,
                }
            } else {
                let mut segs = a.segs.clone();
                let back = b.reversed();
                segs.push(forma_geom::Seg::Line(a.end(), back.start()));
                segs.extend(back.segs.iter().copied());
                segs.push(forma_geom::Seg::Line(back.end(), a.start()));
                Geometry::from_chain(Chain::new(segs)).and_then(|outline| {
                    forma_io_3dm::exact::make_planar(&outline, &[], plane.z, Some(plane.z * h))
                })
            };
            if let Some(s) = exact {
                out.push((Geometry::Brep(s), c));
                continue;
            }
            let mesh = if c.geometry.is_closed_curve() {
                // A ring between the two closed offsets.
                let (pa, pb) = (strip_closing(a.points()), strip_closing(b.points()));
                let (outer, inner) = if forma_geom::newell_area(&pa).length()
                    >= forma_geom::newell_area(&pb).length()
                {
                    (pa, pb)
                } else {
                    (pb, pa)
                };
                let region = planar_mesh(&outer, &[inner], plane.z);
                forma_geom::extrude_open_mesh(&region, plane.z * h, tol)
            } else {
                let mut outline = a.points();
                outline.extend(b.reversed().points());
                outline.push(outline[0]);
                Some(extrude_mesh(&outline, plane.z * h, true))
            };
            if let Some(m) = mesh.filter(|m| !m.triangles.is_empty()) {
                out.push((Geometry::Mesh(m), c));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid("Slab: select planar curves".into()));
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("{n} slab(s), thickness {t}, height {h}"))
    }
}

fn strip_closing(mut p: Vec<Point3>) -> Vec<Point3> {
    if p.len() > 1 && p[0].distance_to(*p.last().expect("len")) < 1e-9 {
        p.pop();
    }
    p
}

/// Rebuild the selected meshes' vertices with `angle` (degrees) as the smoothing
/// limit.
fn reweld(ctx: &mut Context, cmd: &str, angle: f64) -> CommandResult {
    let ids = ctx.selected(cmd)?;
    let tol = ctx.doc.absolute_tolerance.max(1e-9);
    let mut t = ctx.doc.begin();
    let mut n = 0;
    for id in ids {
        if let Geometry::Mesh(m) = &t.doc().object(id).expect("selected").geometry {
            let r = m.split_by_angle(angle.clamp(0.0, 180.0).to_radians(), tol);
            t.replace(id, Geometry::Mesh(r));
            n += 1;
        }
    }
    if n == 0 {
        return Err(CommandError::Invalid(format!("{cmd}: select meshes")));
    }
    t.commit();
    Ok(format!("{n} mesh(es) rebuilt with a {angle}° angle"))
}

simple_command!(
    Weld,
    "Weld",
    &[],
    "Weld [angle°=180] — merge mesh vertices where faces meet at less than the angle (smooth shading, edges hidden)"
);
impl Command for Weld {
    impl_meta!(Weld);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.optional_number().unwrap_or(180.0);
        reweld(ctx, "Weld", a)
    }
}

simple_command!(
    Unweld,
    "Unweld",
    &[],
    "Unweld [angle°=0] — split mesh vertices where faces meet at more than the angle (flat shading, edges shown)"
);
impl Command for Unweld {
    impl_meta!(Unweld);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.optional_number().unwrap_or(0.0);
        reweld(ctx, "Unweld", a)
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::{Geometry, ObjectId};
    use std::f64::consts::PI;

    fn last_mesh(e: &Engine) -> forma_geom::Mesh {
        match e.doc().objects().last().unwrap().geometry.as_mesh() {
            Some(m) => m.clone(),
            None => panic!("no faces"),
        }
    }

    fn last_brep(e: &Engine) -> forma_geom::Brep {
        match &e.doc().objects().last().unwrap().geometry {
            Geometry::Brep(b) => b.clone(),
            g => panic!("not exact: {}", g.kind()),
        }
    }

    /// Lengths of the naked edges of a brep, sorted.
    fn edge_lengths(b: &forma_geom::Brep) -> Vec<f64> {
        let mut v: Vec<f64> = forma_io_3dm::exact::edges(b, true)
            .iter()
            .filter_map(Geometry::length)
            .collect();
        v.sort_by(f64::total_cmp);
        v
    }

    #[test]
    fn free_form_surfaces_are_exact() {
        let mut e = Engine::new();
        // EdgeSrf, four sides (one an arc): its edges are the curves.
        e.run_line("Curve 0,0,0 50,0,40 100,0,0").unwrap(); // #1
        e.run_line("Curve 100,0,0 100,50,-30 100,100,0").unwrap(); // #2
        e.run_line("Curve 100,100,0 50,100,40 0,100,0").unwrap(); // #3
        e.run_line("Arc 0,50,0 0,0,0 0,100,0").unwrap(); // #4
        let mut want: Vec<f64> = (1..=4)
            .map(|i| {
                e.doc()
                    .object(ObjectId(i))
                    .unwrap()
                    .geometry
                    .length()
                    .unwrap()
            })
            .collect();
        want.sort_by(f64::total_cmp);
        e.run_line("SelAll").unwrap();
        e.run_line("EdgeSrf").unwrap();
        let b = last_brep(&e);
        assert_eq!(b.faces, 1);
        let got = edge_lengths(&b);
        assert_eq!(got.len(), 4, "{got:?}");
        for (g, w) in got.iter().zip(&want) {
            assert!((g - w).abs() < 1e-3, "edges {got:?} vs curves {want:?}");
        }
        // Three sides.
        e.run_line("SelNone").unwrap();
        e.run_line("Line 150,0,0 250,0,0").unwrap();
        e.run_line("Curve 250,0,0 230,50,40 200,90,0").unwrap();
        e.run_line("Curve 200,90,0 160,50,30 150,0,0").unwrap();
        e.run_line("Select #6 #7 #8").unwrap();
        e.run_line("EdgeSrf").unwrap();
        assert_eq!(edge_lengths(&last_brep(&e)).len(), 3);
        // SrfPt: a twisted (bilinear) patch and a flat triangle.
        e.run_line("SrfPt 0,0,0 10,0,5 10,10,0 0,10,5").unwrap();
        let b = last_brep(&e);
        let bb = b.mesh.bounding_box().unwrap();
        assert!(
            (bb.max.z - 5.0).abs() < 1e-9 && bb.min.z.abs() < 1e-9,
            "{bb:?}"
        );
        e.run_line("SrfPt 0,0,0 10,0,0 0,10,0").unwrap();
        assert!((last_brep(&e).mesh.area() - 50.0).abs() < 1e-6);
    }

    #[test]
    fn sweeps_and_pipes_are_exact() {
        let mut e = Engine::new();
        // Pipe along a smooth curve: a closed solid around the curve.
        e.run_line("Line 0,0,0 100,0,0").unwrap(); // #1
        e.run_line("SelAll").unwrap();
        e.run_line("Pipe 5").unwrap();
        let b = last_brep(&e);
        assert!(b.solid);
        let v = b.mesh.volume();
        assert!((v - PI * 25.0 * 100.0).abs() / v < 0.005, "{v}");
        // Pipe along a polyline: mitred corners, still one closed solid of the
        // same volume (centre line × section).
        e.run_line("SelNone").unwrap();
        e.run_line("Polyline 0,50,0 100,50,0 100,150,0 0,150,30")
            .unwrap(); // #3
        e.run_line("Select #3").unwrap();
        e.run_line("Pipe 4").unwrap();
        let b = last_brep(&e);
        assert!(b.solid && b.faces >= 5, "faces {}", b.faces);
        let len = 200.0 + (100.0f64 * 100.0 + 30.0 * 30.0).sqrt();
        let v = b.mesh.volume();
        assert!((v - PI * 16.0 * len).abs() / v < 0.01, "{v}");
        // Sweep1 of a circle along a curve: the profile stays the same size.
        e.run_line("SelNone").unwrap();
        e.run_line("Curve 0,300,0 0,400,60 150,450,120 300,400,0")
            .unwrap(); // #5
        e.run_line("Circle 0,300,0 20 0,1,0").unwrap(); // #6
        e.run_line("Select #6").unwrap();
        e.run_line("Sweep1 #5").unwrap();
        let b = last_brep(&e);
        assert_eq!(b.faces, 1);
        let ends = edge_lengths(&b);
        assert_eq!(ends.len(), 2, "{ends:?}");
        for l in ends {
            assert!((l - 2.0 * PI * 20.0).abs() < 0.05, "{l}");
        }
    }

    #[test]
    fn plane_and_corner_surfaces() {
        let mut e = Engine::new();
        e.run_line("Plane 0,0 200,100").unwrap();
        assert!((last_mesh(&e).area() - 20_000.0).abs() < 1e-6);
        e.run_line("SrfPt 0,0 10,0 0,10").unwrap();
        assert!((last_mesh(&e).area() - 50.0).abs() < 1e-9);
        e.run_line("SrfPt 0,0 10,0 10,10 0,10").unwrap();
        assert!((last_mesh(&e).area() - 100.0).abs() < 1e-9);
        e.run_line("SrfPt 0,0 10,0 10,10,5 0,10").unwrap();
        assert!(last_mesh(&e).area() > 100.0);
        assert!(e.run_line("SrfPt 0,0 1,0 2,0").is_err());
    }

    #[test]
    fn edge_surface_from_four_lines() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 100,0").unwrap();
        e.run_line("Line 100,0 100,100,50").unwrap();
        e.run_line("Line 0,100 100,100,50").unwrap();
        e.run_line("Line 0,0 0,100").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("EdgeSrf").unwrap();
        let m = last_mesh(&e);
        let b = m.bounding_box().unwrap();
        assert!((b.max.z - 50.0).abs() < 1e-9 && (b.max.x - 100.0).abs() < 1e-9);
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1 #3").unwrap();
        e.run_line("EdgeSrf").unwrap();
        assert!(e.run_line("Select #2").is_ok());
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        assert!(e.run_line("EdgeSrf").is_err());
    }

    #[test]
    fn round_solids() {
        let mut e = Engine::new();
        e.run_line("Cone 0,0 10 30").unwrap();
        // Exact solids; their display meshes are within 1% of the volume.
        let kind = |e: &Engine| e.doc().objects().last().unwrap().geometry.kind();
        assert_eq!(kind(&e), "polysurface");
        let v = last_mesh(&e).volume();
        assert!((v - PI * 100.0 * 10.0).abs() / v < 1e-2, "{v}");
        e.run_line("TCone 0,0 10 5 30").unwrap();
        assert_eq!(kind(&e), "polysurface");
        assert!(last_mesh(&e).is_closed(1e-6));
        let v = last_mesh(&e).volume();
        let exact = PI * 30.0 / 3.0 * (100.0 + 50.0 + 25.0);
        assert!((v - exact).abs() / exact < 1e-2, "{v}");
        e.run_line("Torus 0,0 20 5").unwrap();
        assert!(last_mesh(&e).is_closed(1e-6));
        assert!(e.run_line("Torus 0,0 5 20").is_err());
        e.run_line("Ellipsoid 0,0 10 20 30").unwrap();
        let b = last_mesh(&e).bounding_box().unwrap();
        assert!(
            (b.max.z - 30.0).abs() < 1e-3 && (b.max.y - 20.0).abs() < 0.5,
            "{b:?}"
        );
        e.run_line("Pyramid 0,0 10,0 4 30").unwrap();
        let v = last_mesh(&e).volume();
        assert!((v - 200.0 * 30.0 / 3.0).abs() < 1e-6, "{v}");
        e.run_line("Tube 0,0 10 8 100").unwrap();
        assert_eq!(kind(&e), "polysurface");
        let v = last_mesh(&e).volume();
        assert!((v - PI * 36.0 * 100.0).abs() / v < 1e-2, "{v}");
        assert!(e.run_line("Tube 0,0 10 10 100").is_err());
        assert_eq!(e.doc().len(), 6);
    }

    #[test]
    fn extrude_to_a_point() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("ExtrudeCrvToPoint 5,5,30").unwrap();
        let Geometry::Brep(b) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(b.solid);
        let m = &b.mesh;
        assert!(m.is_closed(1e-6));
        assert!((m.volume() - 1000.0).abs() < 1e-6, "{}", m.volume());
        // Apex below: still a positive volume.
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("ExtrudeCrvToPoint 5,5,-30").unwrap();
        let Geometry::Brep(b) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(b.solid);
        let m = &b.mesh;
        assert!((m.volume() - 1000.0).abs() < 1e-6, "{}", m.volume());
        assert!(e.run_line("ExtrudeCrvToPoint 20,20,0").is_err());
        // An open curve: a fan surface.
        e.run_line("SelNone").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("ExtrudeCrvToPoint 0,0,10").unwrap();
        let Geometry::Brep(b) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(!b.solid);
        let m = &b.mesh;
        assert!((m.area() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn pipe_and_extrusions() {
        let mut e = Engine::new();
        e.run_line("Polyline 0,0 100,0 100,100").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Pipe 2").unwrap();
        let m = last_mesh(&e);
        assert!(m.is_closed(1e-6));
        let v = m.volume();
        assert!((v - PI * 4.0 * 200.0).abs() / v < 0.01, "{v}");
        e.run_line("SelNone").unwrap();
        e.run_line("Rectangle 0,0,0 10,10,0").unwrap(); // #3
        e.run_line("Line 0,0,0 30,0,40").unwrap(); // #4, length 50
        e.run_line("Select #3").unwrap();
        e.run_line("ExtrudeCrvAlongCrv #4").unwrap();
        let m = last_mesh(&e);
        assert!(m.is_closed(1e-6));
        // Prism: base area × height along the normal.
        assert!((m.volume() - 100.0 * 40.0).abs() < 1e-6, "{}", m.volume());
        e.run_line("SelNone").unwrap();
        e.run_line("Select #3").unwrap();
        e.run_line("ExtrudeCrvTapered 3 18.434948822922").unwrap(); // tan = 1/3 → inset 1
        let m = last_mesh(&e);
        assert!(
            (m.volume() - (100.0 + 64.0 + 80.0)).abs() < 1e-3,
            "{}",
            m.volume()
        );
    }

    #[test]
    fn slab_walls_from_an_axis() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Polyline 0,0 400,0 400,300").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Slab 20 270").unwrap();
        let m = last_mesh(&e);
        assert!(m.is_closed(0.01));
        // Centre line length 700 × 20 × 270 (mitred corner keeps the area).
        let v = m.volume();
        assert!((v - 700.0 * 20.0 * 270.0).abs() < 1e-3, "{v}");
        e.run_line("SelNone").unwrap();
        e.run_line("Rectangle 0,0 400,300").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("Slab 10 100 left").unwrap();
        let v = last_mesh(&e).volume();
        // Ring between the rectangle and its 10 cm offset (inside or outside).
        let inner = 380.0 * 280.0;
        let outer = 420.0 * 320.0;
        let ok = |a: f64, b: f64| ((a - b) * 100.0 - v).abs() < 1e-3;
        assert!(ok(400.0 * 300.0, inner) || ok(outer, 400.0 * 300.0), "{v}");
    }

    #[test]
    fn weld_unweld() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        // Weld works on meshes, not on polysurfaces.
        assert!(e.run_line("Weld").is_err());
        e.run_line("Mesh").unwrap();
        e.run_line("Weld").unwrap();
        assert!(last_mesh(&e).boundary_edges().is_empty());
        e.run_line("Unweld 30").unwrap();
        assert_eq!(last_mesh(&e).boundary_edges().len(), 24);
        e.run_line("SelNone").unwrap();
        assert!(e.run_line("Weld").is_err());
    }
}
