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
        added(ctx, planar_mesh(&pts, &[], plane.z), "plane")
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
        added(
            ctx,
            cone_mesh(&Plane::from_normal(c, n), r, 0.0, h, 64),
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
        added(
            ctx,
            cone_mesh(&Plane::from_normal(c, n), r0, r1, h, 64),
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
        added(
            ctx,
            torus_mesh(&Plane::from_normal(c, n), r, s, 64),
            "torus",
        )
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
        added(
            ctx,
            ellipsoid_mesh(&Plane::from_normal(c, n), rx, ry, rz, 48),
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
        added(ctx, pyramid_mesh(&base, c + plane.z * h), "pyramid")
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
        added(
            ctx,
            tube_mesh(&Plane::from_normal(c, n), r0, r1, h, 64),
            "tube",
        )
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
        let path = ctx
            .doc
            .object(path_id)
            .filter(|o| o.geometry.is_curve())
            .ok_or_else(|| CommandError::Invalid(format!("#{} is not a curve", path_id.0)))?
            .geometry
            .curve_points();
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
    use forma_doc::Geometry;
    use std::f64::consts::PI;

    fn last_mesh(e: &Engine) -> forma_geom::Mesh {
        match &e.doc().objects().last().unwrap().geometry {
            Geometry::Mesh(m) => m.clone(),
            g => panic!("{g:?}"),
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
        let v = last_mesh(&e).volume();
        assert!((v - PI * 100.0 * 10.0).abs() / v < 5e-3, "{v}");
        e.run_line("TCone 0,0 10 5 30").unwrap();
        assert!(last_mesh(&e).is_closed(1e-6));
        e.run_line("Torus 0,0 20 5").unwrap();
        assert!(last_mesh(&e).is_closed(1e-6));
        assert!(e.run_line("Torus 0,0 5 20").is_err());
        e.run_line("Ellipsoid 0,0 10 20 30").unwrap();
        let b = last_mesh(&e).bounding_box().unwrap();
        assert!((b.max.z - 30.0).abs() < 1e-6 && (b.max.y - 20.0).abs() < 1e-3);
        e.run_line("Pyramid 0,0 10,0 4 30").unwrap();
        let v = last_mesh(&e).volume();
        assert!((v - 200.0 * 30.0 / 3.0).abs() < 1e-6, "{v}");
        e.run_line("Tube 0,0 10 8 100").unwrap();
        let v = last_mesh(&e).volume();
        assert!((v - PI * 36.0 * 100.0).abs() / v < 5e-3, "{v}");
        assert!(e.run_line("Tube 0,0 10 10 100").is_err());
        assert_eq!(e.doc().len(), 6);
    }

    #[test]
    fn extrude_to_a_point() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("ExtrudeCrvToPoint 5,5,30").unwrap();
        let Geometry::Mesh(m) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(m.is_closed(1e-6));
        assert!((m.volume() - 1000.0).abs() < 1e-6, "{}", m.volume());
        // Apex below: still a positive volume.
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("ExtrudeCrvToPoint 5,5,-30").unwrap();
        let Geometry::Mesh(m) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!((m.volume() - 1000.0).abs() < 1e-6, "{}", m.volume());
        assert!(e.run_line("ExtrudeCrvToPoint 20,20,0").is_err());
        // An open curve: a fan surface.
        e.run_line("SelNone").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("ExtrudeCrvToPoint 0,0,10").unwrap();
        let Geometry::Mesh(m) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
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
        e.run_line("Weld").unwrap();
        assert!(last_mesh(&e).boundary_edges().is_empty());
        e.run_line("Unweld 30").unwrap();
        assert_eq!(last_mesh(&e).boundary_edges().len(), 24);
        e.run_line("SelNone").unwrap();
        assert!(e.run_line("Weld").is_err());
    }
}
