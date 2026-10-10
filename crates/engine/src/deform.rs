//! More transforms: Shear, Rotate3D, ScaleNU, SetPt, ArrayCrv, Twist, Taper.

use crate::edit::{parse_ids, transform_selection};
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::Geometry;
use forma_geom::{densify, Deform, Plane, Vec3, Xform};

simple_command!(
    Shear,
    "Shear",
    &[],
    "Shear <origin> <reference point> <angle°> [normal] — shear the selection parallel to origin→reference in the construction plane"
);
impl Command for Shear {
    impl_meta!(Shear);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let o = args.point("origin point", ctx.last_point)?;
        let r = args.point("reference point", Some(o))?;
        let a = args.number("shear angle")?;
        if a.abs() >= 89.0 {
            return Err(CommandError::Invalid(
                "shear angle must be below 89°".into(),
            ));
        }
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let n = n.normalized().unwrap_or(Vec3::Z);
        let dir = forma_geom::in_plane(r - o, n)
            .ok_or_else(|| CommandError::Invalid("reference point is the origin".into()))?;
        let across = n.cross(dir);
        transform_selection(
            ctx,
            "Shear",
            &Xform::shear(o, dir, across, a.to_radians().tan()),
            false,
        )
    }
}

simple_command!(
    Rotate3D,
    "Rotate3D",
    &["Ro3D"],
    "Rotate3D <axis start> <axis end> <angle°> [copy] — rotate the selection around an axis through two points"
);
impl Command for Rotate3D {
    impl_meta!(Rotate3D);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of rotation axis", ctx.last_point)?;
        let b = args.point("end of rotation axis", Some(a))?;
        let ang = args.number("angle")?;
        let copy = args.keyword("copy");
        let axis = (b - a)
            .normalized()
            .ok_or_else(|| CommandError::Invalid("axis points coincide".into()))?;
        transform_selection(
            ctx,
            "Rotate3D",
            &Xform::rotation(a, axis, ang.to_radians()),
            copy,
        )
    }
}

simple_command!(
    ScaleNU,
    "ScaleNU",
    &[],
    "ScaleNU <origin> <x factor> <y factor> <z factor> — non-uniform scale along the world axes"
);
impl Command for ScaleNU {
    impl_meta!(ScaleNU);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let o = args.point("origin point", ctx.last_point)?;
        let mut f = [0.0; 3];
        for (i, what) in ["x scale factor", "y scale factor", "z scale factor"]
            .into_iter()
            .enumerate()
        {
            f[i] = args.number(what)?;
            if f[i].abs() < 1e-9 {
                return Err(CommandError::Invalid(
                    "scale factors must not be zero".into(),
                ));
            }
        }
        transform_selection(
            ctx,
            "ScaleNU",
            &Xform::scale_axes(&Plane::TOP.moved_to(o), f[0], f[1], f[2]),
            false,
        )
    }
}

simple_command!(
    SetPt,
    "SetPt",
    &[],
    "SetPt <point> [x] [y] [z] — set the chosen coordinates of every point of the selection to those of <point> (default x y z)"
);
impl Command for SetPt {
    impl_meta!(SetPt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let p = args.point("location", ctx.last_point)?;
        let (mut x, mut y, mut z) = (false, false, false);
        loop {
            if args.keyword("x") {
                x = true;
            } else if args.keyword("y") {
                y = true;
            } else if args.keyword("z") {
                z = true;
            } else {
                break;
            }
        }
        if !(x || y || z) {
            (x, y, z) = (true, true, true);
        }
        let k = |set: bool| if set { 0.0 } else { 1.0 };
        transform_selection(
            ctx,
            "SetPt",
            &Xform::scale_axes(&Plane::TOP.moved_to(p), k(x), k(y), k(z)),
            false,
        )
    }
}

simple_command!(
    ArrayCrv,
    "ArrayCrv",
    &[],
    "ArrayCrv #path <count> [norotate] — copies of the selection spaced evenly along the path curve, turned with its tangent"
);
impl Command for ArrayCrv {
    impl_meta!(ArrayCrv);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("path curve #id"))?;
        let path_id = parse_ids(vec![tok])?[0];
        let n = args.number("number of items")?;
        if n < 2.0 || n.fract().abs() > 1e-9 || n > 10_000.0 {
            return Err(CommandError::Invalid(
                "number of items must be a whole number ≥ 2".into(),
            ));
        }
        let rotate = !args.keyword("norotate");
        let path = ctx
            .doc
            .object(path_id)
            .filter(|o| o.geometry.is_curve())
            .ok_or_else(|| CommandError::Invalid(format!("#{} is not a curve", path_id.0)))?
            .geometry
            .clone();
        let ids: Vec<_> = ctx
            .selected("ArrayCrv")?
            .into_iter()
            .filter(|id| *id != path_id)
            .collect();
        if ids.is_empty() {
            return Err(CommandError::Invalid(
                "ArrayCrv: select the objects to array".into(),
            ));
        }
        let total = path.length().unwrap_or(0.0);
        if total <= ctx.tolerance.absolute {
            return Err(CommandError::Invalid("ArrayCrv: path is too short".into()));
        }
        let n = n as usize;
        let closed = path.is_closed_curve();
        let lengths = forma_geom::division_lengths(total, if closed { n } else { n - 1 }, closed);
        // Points and tangents (from a small step along the curve).
        let h = total * 1e-4;
        let probe: Vec<f64> = lengths
            .iter()
            .flat_map(|s| [(s - h).max(0.0), (s + h).min(total)])
            .collect();
        let at = |ls: &[f64]| -> Vec<forma_geom::Point3> {
            match &path {
                Geometry::Nurbs(c) => c.points_at_lengths(ls),
                g => g
                    .to_chain()
                    .map(|c| forma_geom::chain_points_at_lengths(&c, ls))
                    .unwrap_or_default(),
            }
        };
        let pts = at(&lengths);
        let pr = at(&probe);
        let tangent = |i: usize| (pr[2 * i + 1] - pr[2 * i]).normalized().unwrap_or(Vec3::X);
        let (p0, t0) = (pts[0], tangent(0));
        let xs: Vec<Xform> = (1..pts.len())
            .map(|i| {
                let mv = Xform::translation(pts[i] - p0);
                if rotate {
                    let r = Xform::rotation_between(t0, tangent(i));
                    // Rotate about the path point after moving there.
                    let about = Xform::translation(-(pts[i].to_vec()))
                        .then(&r)
                        .then(&Xform::translation(pts[i].to_vec()));
                    mv.then(&about)
                } else {
                    mv
                }
            })
            .collect();
        let mut t = ctx.doc.begin();
        let mut count = 0;
        for id in &ids {
            let obj = t.doc().object(*id).expect("selected").clone();
            for x in &xs {
                t.add_like(obj.geometry.transformed(x), &obj);
                count += 1;
            }
        }
        t.commit();
        Ok(format!("{count} copies along #{}", path_id.0))
    }
}

/// Apply a deformation to the selection: meshes are subdivided first so faces
/// bend, curves become polylines (NURBS keep their type: control points move).
fn deform(ctx: &mut Context, cmd: &str, d: &Deform, axis_len: f64) -> CommandResult {
    let ids = ctx.selected(cmd)?;
    let tol = ctx.doc.absolute_tolerance.max(1e-9);
    let step = axis_len / 32.0;
    let mut t = ctx.doc.begin();
    let mut n = 0;
    for id in &ids {
        let g = t.doc().object(*id).expect("selected").geometry.clone();
        let out = match &g {
            Geometry::Mesh(m) => {
                let longest = m
                    .triangles
                    .iter()
                    .flat_map(|t| {
                        let [a, b, c] = t.map(|i| m.positions[i as usize]);
                        [a.distance_to(b), b.distance_to(c), c.distance_to(a)]
                    })
                    .fold(0.0, f64::max);
                // Same number of cuts on every edge (no cracks), within a budget.
                let budget = (200_000.0 / m.triangles.len().max(1) as f64).sqrt().floor();
                let k = (longest / step).ceil().min(32.0).min(budget).max(1.0) as usize;
                let fine = d.mesh(&m.subdivided(k));
                Geometry::Mesh(fine.split_by_angle(30f64.to_radians(), tol))
            }
            Geometry::Point(p) => Geometry::Point(d.point(*p)),
            Geometry::Nurbs(c) => Geometry::Nurbs(forma_geom::NurbsCurve {
                points: c.points.iter().map(|p| d.point(*p)).collect(),
                ..c.clone()
            }),
            g if g.is_curve() => Geometry::Polyline(
                densify(&g.curve_points(), step)
                    .into_iter()
                    .map(|p| d.point(p))
                    .collect(),
            ),
            _ => continue,
        };
        t.replace(*id, out);
        n += 1;
    }
    if n == 0 {
        return Err(CommandError::Invalid(format!("{cmd}: nothing to deform")));
    }
    t.commit();
    Ok(format!("deformed {n} object(s)"))
}

simple_command!(
    Twist,
    "Twist",
    &[],
    "Twist <axis start> <axis end> <angle°> — twist the selection around the axis (0° at the start, full angle at the end)"
);
impl Command for Twist {
    impl_meta!(Twist);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of twist axis", ctx.last_point)?;
        let b = args.point("end of twist axis", Some(a))?;
        let ang = args.number("twist angle")?;
        let len = crate::create::positive(a.distance_to(b), "axis length", ctx)?;
        deform(
            ctx,
            "Twist",
            &Deform::Twist {
                start: a,
                end: b,
                angle: ang.to_radians(),
            },
            len,
        )
    }
}

simple_command!(
    Taper,
    "Taper",
    &[],
    "Taper <axis start> <axis end> <start factor> <end factor> — scale the distance from the axis gradually along it"
);
impl Command for Taper {
    impl_meta!(Taper);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of taper axis", ctx.last_point)?;
        let b = args.point("end of taper axis", Some(a))?;
        let s0 = args.number("start factor")?;
        let s1 = args.number("end factor")?;
        let len = crate::create::positive(a.distance_to(b), "axis length", ctx)?;
        deform(
            ctx,
            "Taper",
            &Deform::Taper {
                start: a,
                end: b,
                s0,
                s1,
            },
            len,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    const TOL: f64 = 1e-6;

    #[test]
    fn shear_rotate3d_scalenu_setpt() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Shear 0,0 10,0 45").unwrap();
        let Geometry::Polyline(p) = &e.doc().objects().next().unwrap().geometry else {
            panic!()
        };
        assert!(
            p[2].distance_to(Point3::new(20.0, 10.0, 0.0)) < TOL,
            "{}",
            p[2]
        );
        e.run_line("Undo").unwrap();
        e.run_line("Rotate3D 0,0,0 10,0,0 90").unwrap();
        let b = e.doc().objects().next().unwrap().geometry.bounding_box();
        assert!((b.max.z - 10.0).abs() < TOL && b.max.y.abs() < TOL, "{b:?}");
        e.run_line("Undo").unwrap();
        e.run_line("ScaleNU 0,0,0 2 3 1").unwrap();
        let b = e.doc().objects().next().unwrap().geometry.bounding_box();
        assert!((b.max.x - 20.0).abs() < TOL && (b.max.y - 30.0).abs() < TOL);
        e.run_line("Box 0,0,5 10,10 10").unwrap();
        e.run_line("SelNone").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("SetPt 0,0,0 z").unwrap();
        let b = e.doc().objects().last().unwrap().geometry.bounding_box();
        assert!(b.max.z.abs() < TOL && (b.max.x - 10.0).abs() < TOL);
        assert!(e.run_line("ScaleNU 0,0 1 0 1").is_err());
    }

    #[test]
    fn array_along_a_curve() {
        let mut e = Engine::new();
        e.run_line("Circle 0,0 100").unwrap(); // path #1
        e.run_line("Line 100,-5 100,5").unwrap(); // #2, tangent to the circle at its start
        e.run_line("Select #2").unwrap();
        e.run_line("ArrayCrv #1 4").unwrap();
        assert_eq!(e.doc().len(), 5);
        // The copy at a quarter turn is horizontal at (0,100).
        let q = e
            .doc()
            .objects()
            .find(|o| {
                o.geometry
                    .bounding_box()
                    .center()
                    .distance_to(Point3::new(0.0, 100.0, 0.0))
                    < 1e-3
            })
            .expect("copy at the top");
        let b = q.geometry.bounding_box();
        assert!((b.max.x - b.min.x - 10.0).abs() < 1e-3, "{b:?}");
        e.run_line("Line 0,0 300,0").unwrap(); // #6
        e.run_line("SelNone").unwrap();
        e.run_line("Select #2").unwrap();
        e.run_line("ArrayCrv #6 3 norotate").unwrap();
        assert_eq!(e.doc().len(), 8);
        assert!(e.run_line("ArrayCrv #6 1").is_err());
    }

    #[test]
    fn twist_and_taper_a_box() {
        let mut e = Engine::new();
        e.run_line("Box -5,-5 5,5 100").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Twist 0,0,0 0,0,100 90").unwrap();
        let Geometry::Mesh(m) = &e.doc().objects().next().unwrap().geometry else {
            panic!()
        };
        assert!(m.is_closed(1e-6));
        assert!((m.volume() - 10_000.0).abs() < 50.0, "{}", m.volume());
        // The top corner (5,5,100) turned to (-5,5,100).
        assert!(m
            .positions
            .iter()
            .any(|p| p.distance_to(Point3::new(-5.0, 5.0, 100.0)) < 1e-6));
        e.run_line("Undo").unwrap();
        e.run_line("Taper 0,0,0 0,0,100 1 0.5").unwrap();
        let b = e.doc().objects().next().unwrap().geometry.bounding_box();
        assert!((b.max.x - 5.0).abs() < 1e-6);
        let Geometry::Mesh(m) = &e.doc().objects().next().unwrap().geometry else {
            panic!()
        };
        let top = m.positions.iter().filter(|p| (p.z - 100.0).abs() < 1e-9);
        assert!(top.into_iter().all(|p| p.x.abs() <= 2.5 + 1e-9));
    }
}
