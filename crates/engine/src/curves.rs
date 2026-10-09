//! Curve tools: Offset, Trim, Extend, Fillet, FilletCorners, Join, Explode.
//!
//! Picks are world points near the curve to act on; an optional trailing vector
//! is the construction-plane normal used for curves that do not define a plane
//! themselves (single lines).

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{self as geom, Chain, ExtendTo, Plane, Point3, Seg, Vec3};

fn bad(e: impl std::fmt::Display) -> CommandError {
    CommandError::Invalid(e.to_string())
}

/// Working plane of a curve: its own plane if it has one, else the given normal.
fn plane_of(g: &Geometry, through: Point3, normal: Option<Vec3>) -> Plane {
    let n = g.curve_normal().or(normal).unwrap_or(Vec3::Z);
    // Keep the normal pointing like the construction plane's, so "left" is stable.
    let n = match normal {
        Some(c) if n.dot(c) < 0.0 => -n,
        _ => n,
    };
    Plane::from_normal(through, n)
}

/// Visible, unlocked curves as chains.
fn curves(ctx: &Context) -> Vec<(ObjectId, Chain)> {
    ctx.doc
        .objects()
        .filter(|o| {
            let l = ctx.doc.layer(o.layer);
            l.visible && !l.locked
        })
        .filter_map(|o| o.geometry.to_chain().map(|c| (o.id, c)))
        .collect()
}

/// The curve nearest to `pick`.
fn nearest(ctx: &Context, pick: Point3) -> Result<(ObjectId, Chain), CommandError> {
    curves(ctx)
        .into_iter()
        .map(|(id, c)| {
            let d = c.closest(pick).1;
            (d, id, c)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id, c)| (id, c))
        .ok_or_else(|| CommandError::Invalid("no curve to pick".into()))
}

/// Selected curves as chains (ignoring meshes).
fn selected_chains(ctx: &Context, cmd: &str) -> Result<Vec<(ObjectId, Chain)>, CommandError> {
    let ids = ctx.selected(cmd)?;
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            ctx.doc
                .object(id)
                .and_then(|o| o.geometry.to_chain())
                .map(|c| (id, c))
        })
        .collect())
}

fn geometry(c: Chain) -> Result<Geometry, CommandError> {
    Geometry::from_chain(c).ok_or_else(|| CommandError::Invalid("result is empty".into()))
}

simple_command!(
    Offset,
    "Offset",
    &["O"],
    "Offset <distance> <side point> [normal] — offset the selected planar curves"
);
impl Command for Offset {
    impl_meta!(Offset);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let d = args.number("offset distance")?;
        if d <= ctx.tolerance.absolute {
            return Err(CommandError::Invalid(
                "offset distance must be positive".into(),
            ));
        }
        let side_pt = args.point("side to offset", ctx.last_point)?;
        let n = args.optional_vector();
        let chains = selected_chains(ctx, "Offset")?;
        if chains.is_empty() {
            return Err(CommandError::Invalid("select curves to offset".into()));
        }
        let tol = ctx.tolerance.absolute;
        let mut out = Vec::new();
        for (id, c) in chains {
            let obj = ctx.doc.object(id).expect("selected").clone();
            let plane = plane_of(&obj.geometry, c.start(), n);
            let side = geom::side_of(&c, side_pt, &plane);
            let o = geom::offset(&c, d, side, &plane, tol).map_err(bad)?;
            out.push((obj, geometry(o.simplified(tol))?));
        }
        let mut t = ctx.doc.begin();
        let ids: Vec<ObjectId> = out.iter().map(|(o, g)| t.add_like(g.clone(), o)).collect();
        t.commit();
        ctx.selection = ids.iter().copied().collect();
        Ok(format!("{} offset curve(s)", ids.len()))
    }
}

simple_command!(
    Trim,
    "Trim",
    &["Tr"],
    "Trim <pick> [normal] — cut away the part of the curve at <pick> between the selected cutting curves"
);
impl Command for Trim {
    impl_meta!(Trim);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let pick = args.point("part to trim", None)?;
        let n = args.optional_vector();
        let cutter_ids = ctx.selected("Trim")?;
        let (target, chain) = nearest(ctx, pick)?;
        let cutters: Vec<Chain> = cutter_ids
            .iter()
            .filter(|id| **id != target)
            .filter_map(|id| ctx.doc.object(*id).and_then(|o| o.geometry.to_chain()))
            .collect();
        if cutters.is_empty() {
            return Err(CommandError::Invalid("select cutting curves first".into()));
        }
        let obj = ctx.doc.object(target).expect("exists").clone();
        let plane = plane_of(&obj.geometry, chain.start(), n);
        let tol = ctx.tolerance.absolute;
        let pieces = geom::trim(&chain, &cutters, pick, &plane, tol).map_err(bad)?;
        let mut t = ctx.doc.begin();
        let mut pieces = pieces.into_iter();
        match pieces.next() {
            Some(first) => {
                t.replace(target, geometry(first)?);
                for p in pieces {
                    t.add_like(geometry(p)?, &obj);
                }
            }
            None => {
                t.remove(target);
            }
        }
        t.commit();
        Ok(format!("trimmed #{}", target.0))
    }
}

simple_command!(
    Extend,
    "Extend",
    &["Ex"],
    "Extend <pick> [length] [normal] — extend the curve end near <pick> to the selected boundaries (or by a length)"
);
impl Command for Extend {
    impl_meta!(Extend);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let pick = args.point("end to extend", None)?;
        let length = args.optional_number();
        let n = args.optional_vector();
        let (target, chain) = nearest(ctx, pick)?;
        let obj = ctx.doc.object(target).expect("exists").clone();
        let plane = plane_of(&obj.geometry, chain.start(), n);
        let tol = ctx.tolerance.absolute;
        let bounds: Vec<Chain>;
        let to = match length {
            Some(l) => ExtendTo::Length(l),
            None => {
                bounds = ctx
                    .selected("Extend")?
                    .into_iter()
                    .filter(|id| *id != target)
                    .filter_map(|id| ctx.doc.object(id).and_then(|o| o.geometry.to_chain()))
                    .collect();
                if bounds.is_empty() {
                    return Err(CommandError::Invalid("select boundary curves first".into()));
                }
                ExtendTo::Boundaries(&bounds)
            }
        };
        let e = geom::extend(&chain, to, pick, &plane, tol).map_err(bad)?;
        let mut t = ctx.doc.begin();
        t.replace(target, geometry(e.simplified(tol))?);
        t.commit();
        Ok(format!("extended #{}", target.0))
    }
}

simple_command!(
    Fillet,
    "Fillet",
    &["F"],
    "Fillet <radius> <pick line 1> <pick line 2> [normal] — round the corner between two lines"
);
impl Command for Fillet {
    impl_meta!(Fillet);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let r = args.number("fillet radius")?;
        let p1 = args.point("first line", None)?;
        let p2 = args.point("second line", None)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let (id1, _) = nearest(ctx, p1)?;
        let (id2, _) = nearest(ctx, p2)?;
        if id1 == id2 {
            return Err(CommandError::Invalid("pick two different lines".into()));
        }
        let line = |id: ObjectId| match &ctx.doc.object(id).expect("exists").geometry {
            Geometry::Line(l) => Ok((l.from, l.to)),
            _ => Err(CommandError::Invalid(
                "Fillet works on lines; use FilletCorners for polylines".into(),
            )),
        };
        let (a, b) = (line(id1)?, line(id2)?);
        let o1 = ctx.doc.object(id1).expect("exists").clone();
        let plane = Plane::from_normal(a.0, n);
        let (la, lb, arc) = geom::fillet_lines(a, b, r, &plane).map_err(bad)?;
        let mut t = ctx.doc.begin();
        t.replace(id1, geometry(Chain::new(vec![la]))?);
        t.replace(id2, geometry(Chain::new(vec![lb]))?);
        if let Some(arc) = arc {
            t.add_like(Geometry::Arc(arc), &o1);
        }
        t.commit();
        Ok("filleted".into())
    }
}

simple_command!(
    FilletCorners,
    "FilletCorners",
    &["FC"],
    "FilletCorners <radius> — round all corners of the selected polylines"
);
impl Command for FilletCorners {
    impl_meta!(FilletCorners);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let r = args.number("fillet radius")?;
        let ids = ctx.selected("FilletCorners")?;
        let tol = ctx.tolerance.absolute;
        let mut t = ctx.doc.begin();
        let mut done = 0;
        for id in ids {
            let Geometry::Polyline(p) = &t.doc().object(id).expect("selected").geometry else {
                continue;
            };
            let c = geom::fillet_corners(p, r, tol).map_err(bad)?;
            t.replace(id, geometry(c)?);
            done += 1;
        }
        if done == 0 {
            return Err(CommandError::Invalid("select polylines".into()));
        }
        t.commit();
        Ok(format!("filleted {done} polyline(s)"))
    }
}

simple_command!(
    Join,
    "Join",
    &["J"],
    "Join — join selected curves end to end, and selected meshes into one"
);
impl Command for Join {
    impl_meta!(Join);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Join")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        let objs: Vec<_> = ids
            .iter()
            .map(|id| ctx.doc.object(*id).expect("selected").clone())
            .collect();
        let curves: Vec<_> = objs.iter().filter(|o| o.geometry.is_curve()).collect();
        let meshes: Vec<_> = objs
            .iter()
            .filter(|o| matches!(o.geometry, Geometry::Mesh(_)))
            .collect();
        let mut t = ctx.doc.begin();
        let mut result = Vec::new();
        let mut before = 0;
        let mut after = 0;
        if curves.len() > 1 {
            let chains: Vec<Chain> = curves
                .iter()
                .filter_map(|o| o.geometry.to_chain())
                .collect();
            let joined = geom::join(chains, tol);
            if joined.len() < curves.len() {
                before += curves.len();
                after += joined.len();
                for o in &curves {
                    t.remove(o.id);
                }
                for c in joined {
                    result.push(t.add_like(geometry(c.simplified(tol))?, curves[0]));
                }
            }
        }
        if meshes.len() > 1 {
            let mut m = forma_geom::Mesh::default();
            for o in &meshes {
                if let Geometry::Mesh(x) = &o.geometry {
                    m.append(x);
                }
                t.remove(o.id);
            }
            before += meshes.len();
            after += 1;
            result.push(t.add_like(Geometry::Mesh(m), meshes[0]));
        }
        if result.is_empty() {
            return Err(CommandError::Invalid(
                "nothing to join (curves must touch end to end)".into(),
            ));
        }
        t.commit();
        ctx.selection = result.into_iter().collect();
        Ok(format!("{before} objects joined into {after}"))
    }
}

simple_command!(
    Explode,
    "Explode",
    &["X"],
    "Explode — split polylines and polycurves into segments, meshes into faces"
);
impl Command for Explode {
    impl_meta!(Explode);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Explode")?;
        let mut t = ctx.doc.begin();
        let mut made = Vec::new();
        for id in ids {
            let obj = t.doc().object(id).expect("selected").clone();
            let parts: Vec<Geometry> = match &obj.geometry {
                Geometry::Polyline(_) | Geometry::PolyCurve(_) => obj
                    .geometry
                    .to_chain()
                    .expect("curve")
                    .segs
                    .into_iter()
                    .filter_map(|s: Seg| Geometry::from_chain(Chain::new(vec![s])))
                    .collect(),
                Geometry::Mesh(m) => m.components().into_iter().map(Geometry::Mesh).collect(),
                _ => Vec::new(),
            };
            if parts.len() < 2 {
                continue;
            }
            t.remove(id);
            for g in parts {
                made.push(t.add_like(g, &obj));
            }
        }
        if made.is_empty() {
            return Err(CommandError::Invalid("nothing to explode".into()));
        }
        t.commit();
        ctx.selection.clear();
        Ok(format!("exploded into {} objects", made.len()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    fn near(a: Point3, b: Point3) -> bool {
        a.distance_to(b) < 1e-6
    }

    #[test]
    fn offset_rectangle_outside() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 100,50").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Offset 10 200,200").unwrap();
        assert_eq!(e.doc().len(), 2);
        let o = e.doc().objects().last().unwrap();
        let b = o.geometry.bounding_box();
        assert!(near(b.min, Point3::new(-10.0, -10.0, 0.0)), "{b:?}");
        assert!(near(b.max, Point3::new(110.0, 60.0, 0.0)), "{b:?}");
        assert!(o.geometry.is_closed_curve());
    }

    #[test]
    fn offset_line_on_front_plane() {
        let mut e = Engine::new();
        e.run_line("Line 0,0,0 100,0,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Offset 5 50,0,20 0,-1,0").unwrap();
        let Geometry::Line(l) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(near(l.from, Point3::new(0.0, 0.0, 5.0)), "{l:?}");
    }

    #[test]
    fn trim_and_extend_lines() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 100,0").unwrap(); // #1 target
        e.run_line("Line 50,-10 50,10").unwrap(); // #2 cutter
        e.run_line("Select #2").unwrap();
        e.run_line("Trim 80,1").unwrap();
        let Geometry::Line(l) = &e.doc().object(forma_doc::ObjectId(1)).unwrap().geometry else {
            panic!()
        };
        assert!(near(l.to, Point3::new(50.0, 0.0, 0.0)), "{l:?}");
        // Extend the cutter up to a new boundary line.
        e.run_line("Line -20,30 120,30").unwrap(); // #3
        e.run_line("SelNone").unwrap();
        e.run_line("Select #3").unwrap();
        e.run_line("Extend 50,9").unwrap();
        let Geometry::Line(l) = &e.doc().object(forma_doc::ObjectId(2)).unwrap().geometry else {
            panic!()
        };
        assert!(near(l.to, Point3::new(50.0, 30.0, 0.0)), "{l:?}");
        e.run_line("Extend 50,-9 5").unwrap();
        let Geometry::Line(l) = &e.doc().object(forma_doc::ObjectId(2)).unwrap().geometry else {
            panic!()
        };
        assert!(near(l.from, Point3::new(50.0, -15.0, 0.0)), "{l:?}");
        for _ in 0..4 {
            e.run_line("Undo").unwrap();
        }
        assert!(e
            .doc()
            .dump()
            .contains("#1 line [Default] 0,0,0 -> 100,0,0"));
    }

    #[test]
    fn join_explode_fillet() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 100,0").unwrap();
        e.run_line("Line 100,0 100,100").unwrap();
        e.run_line("Arc 100,50 100,100 100,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Join").unwrap();
        assert_eq!(e.doc().len(), 1);
        assert!(e.doc().dump().contains("polycurve"), "{}", e.doc().dump());
        e.run_line("Explode").unwrap();
        assert_eq!(e.doc().len(), 3);
        e.run_line("New").unwrap();
        e.run_line("Line 0,0 100,0").unwrap();
        e.run_line("Line 100,0 100,100").unwrap();
        e.run_line("Fillet 10 50,0 100,50").unwrap();
        assert_eq!(e.doc().len(), 3);
        assert!(
            e.doc().dump().contains("arc [Default] center 90,10,0 r 10"),
            "{}",
            e.doc().dump()
        );
        e.run_line("New").unwrap();
        e.run_line("Rectangle 0,0 100,50").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("FilletCorners 5").unwrap();
        assert!(
            e.doc().dump().contains("polycurve [Default] 8 segments"),
            "{}",
            e.doc().dump()
        );
    }

    #[test]
    fn explode_box_into_faces_and_join_back() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Explode").unwrap();
        assert_eq!(e.doc().len(), 6);
        e.run_line("SelAll").unwrap();
        e.run_line("Join").unwrap();
        assert_eq!(e.doc().len(), 1);
    }
}
