//! Curve tools: Offset, Trim, Split, Extend, Fillet, Chamfer, FilletCorners, Join,
//! Explode, Intersect.
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
        .filter(|o| ctx.doc.is_selectable(o))
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

simple_command!(
    Split,
    "Split",
    &[],
    "Split [#cutter …] — cut the selected curves at their crossings with the listed cutters (default: with each other)"
);
impl Command for Split {
    impl_meta!(Split);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let listed = crate::edit::parse_ids(args.rest())?;
        for id in &listed {
            if ctx.doc.object(*id).is_none() {
                return Err(CommandError::Invalid(format!("no object #{}", id.0)));
            }
        }
        let targets: Vec<(ObjectId, Chain)> = selected_chains(ctx, "Split")?
            .into_iter()
            .filter(|(id, _)| !listed.contains(id))
            .collect();
        if targets.is_empty() {
            return Err(CommandError::Invalid(
                "Split: select curves to split".into(),
            ));
        }
        let tol = ctx.tolerance.absolute;
        let mut results = Vec::new();
        for (id, chain) in &targets {
            let cutter_ids: Vec<ObjectId> = if listed.is_empty() {
                targets
                    .iter()
                    .map(|(i, _)| *i)
                    .filter(|i| i != id)
                    .collect()
            } else {
                listed.clone()
            };
            let cutters: Vec<Chain> = cutter_ids
                .iter()
                .filter_map(|c| ctx.doc.object(*c).and_then(|o| o.geometry.to_chain()))
                .collect();
            let obj = ctx.doc.object(*id).expect("selected").clone();
            let plane = plane_of(&obj.geometry, chain.start(), None);
            if let Ok(pieces) = geom::split(chain, &cutters, &plane, tol) {
                if pieces.len() > 1 {
                    results.push((obj, pieces));
                }
            }
        }
        if results.is_empty() {
            return Err(CommandError::Invalid(
                "Split: the curves do not cross the cutters".into(),
            ));
        }
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for (obj, pieces) in results {
            let mut it = pieces.into_iter();
            if let Some(first) = it.next() {
                t.replace(obj.id, geometry(first)?);
                n += 1;
            }
            for p in it {
                t.add_like(geometry(p)?, &obj);
                n += 1;
            }
        }
        t.commit();
        Ok(format!("split into {n} pieces"))
    }
}

simple_command!(
    Chamfer,
    "Chamfer",
    &["Cha"],
    "Chamfer <distance 1> [distance 2] <pick line 1> <pick line 2> [normal] — bevel the corner between two lines"
);
impl Command for Chamfer {
    impl_meta!(Chamfer);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let d1 = args.number("first chamfer distance")?;
        let d2 = args.optional_number().unwrap_or(d1);
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
            _ => Err(CommandError::Invalid("Chamfer works on lines".into())),
        };
        let (a, b) = (line(id1)?, line(id2)?);
        let o1 = ctx.doc.object(id1).expect("exists").clone();
        let plane = Plane::from_normal(a.0, n);
        let (la, lb, cut) = geom::chamfer_lines(a, b, d1, d2, &plane).map_err(bad)?;
        let mut t = ctx.doc.begin();
        t.replace(id1, geometry(Chain::new(vec![la]))?);
        t.replace(id2, geometry(Chain::new(vec![lb]))?);
        if let Some(c) = cut {
            t.add_like(geometry(Chain::new(vec![c]))?, &o1);
        }
        t.commit();
        Ok("chamfered".into())
    }
}

/// A plane containing both curves (within `tol`), if there is one.
fn common_plane(a: &Geometry, b: &Geometry, tol: f64) -> Option<Plane> {
    let pa = a.curve_points();
    let pb = b.curve_points();
    let origin = *pa.first()?;
    let dir = |p: &[Point3]| p.get(1).map(|q| *q - p[0]);
    let mut candidates: Vec<Vec3> = Vec::new();
    candidates.extend(a.curve_normal());
    candidates.extend(b.curve_normal());
    if let (Some(da), Some(db)) = (dir(&pa), dir(&pb)) {
        candidates.extend(da.cross(db).normalized());
        // Parallel lines: the plane through both.
        candidates.extend(da.cross(*pb.first()? - origin).normalized());
    }
    candidates.into_iter().find_map(|n| {
        let plane = Plane::from_normal(origin, n);
        pa.iter()
            .chain(pb.iter())
            .all(|p| plane.coords(*p).2.abs() <= tol)
            .then_some(plane)
    })
}

simple_command!(
    Intersect,
    "Intersect",
    &["Int"],
    "Intersect — add points where the selected coplanar curves cross"
);
impl Command for Intersect {
    impl_meta!(Intersect);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Intersect")?;
        let tol = ctx.tolerance.absolute;
        let curves: Vec<(Geometry, Chain)> = ids
            .iter()
            .filter_map(|id| {
                let g = ctx.doc.object(*id)?.geometry.clone();
                let c = g.to_chain()?;
                Some((g, c))
            })
            .collect();
        if curves.len() < 2 {
            return Err(CommandError::Invalid(
                "Intersect: select two or more curves".into(),
            ));
        }
        let mut pts: Vec<Point3> = Vec::new();
        for i in 0..curves.len() {
            for j in i + 1..curves.len() {
                let Some(plane) = common_plane(&curves[i].0, &curves[j].0, tol.max(1e-9)) else {
                    continue;
                };
                for p in geom::crossings(&curves[i].1, &curves[j].1, &plane, tol) {
                    if !pts.iter().any(|q| q.distance_to(p) <= tol) {
                        pts.push(p);
                    }
                }
            }
        }
        if pts.is_empty() {
            return Err(CommandError::Invalid("no intersections found".into()));
        }
        let mut t = ctx.doc.begin();
        for p in &pts {
            t.add(Geometry::Point(*p));
        }
        t.commit();
        Ok(format!("{} intersection point(s)", pts.len()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    #[test]
    fn split_with_cutters_and_with_each_other() {
        let mut e = Engine::new();
        e.run_line("Line 0,5 20,5").unwrap(); // #1
        e.run_line("Line 5,0 5,10").unwrap(); // #2
        e.run_line("Line 12,0 12,10").unwrap(); // #3
        e.run_line("Select #1").unwrap();
        e.run_line("Split #2 #3").unwrap();
        assert_eq!(e.doc().len(), 5);
        let Geometry::Line(l) = &e.doc().object(forma_doc::ObjectId(1)).unwrap().geometry else {
            panic!()
        };
        assert!(near(l.to, Point3::new(5.0, 5.0, 0.0)), "{l:?}");
        e.run_line("Undo").unwrap();
        // Without cutters the selected curves split each other.
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1 #2").unwrap();
        e.run_line("Split").unwrap();
        assert_eq!(e.doc().len(), 5);
        // A closed rectangle split by a line.
        e.run_line("New").unwrap();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("Line 5,-5 5,15").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Split #2").unwrap();
        assert_eq!(e.doc().len(), 3);
        let total: f64 = e
            .doc()
            .objects()
            .filter(|o| o.id.0 != 2)
            .map(|o| o.geometry.length().unwrap())
            .sum();
        assert!((total - 40.0).abs() < 1e-9);
        assert!(e.run_line("Split #2").is_err());
    }

    #[test]
    fn chamfer_corner() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 100,0").unwrap();
        e.run_line("Line 100,0 100,100").unwrap();
        e.run_line("Chamfer 10 50,0 100,50").unwrap();
        assert_eq!(e.doc().len(), 3);
        let Geometry::Line(c) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(near(c.from, Point3::new(90.0, 0.0, 0.0)), "{c:?}");
        assert!(near(c.to, Point3::new(100.0, 10.0, 0.0)), "{c:?}");
        e.run_line("Undo").unwrap();
        e.run_line("Chamfer 10 20 50,0 100,50").unwrap();
        let Geometry::Line(c) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(near(c.to, Point3::new(100.0, 20.0, 0.0)), "{c:?}");
        assert!(e.run_line("Chamfer 500 50,0 100,50").is_err());
    }

    #[test]
    fn intersect_adds_points() {
        let mut e = Engine::new();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("Line -20,0 20,0").unwrap();
        e.run_line("Line 0,-20,5 0,20,5").unwrap(); // above the plane: no crossing
        e.run_line("SelAll").unwrap();
        e.run_line("Intersect").unwrap();
        let pts: Vec<Point3> = e
            .doc()
            .objects()
            .filter_map(|o| match o.geometry {
                Geometry::Point(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(pts.len(), 2, "{pts:?}");
        assert!(pts.iter().any(|p| near(*p, Point3::new(10.0, 0.0, 0.0))));
        assert!(pts.iter().any(|p| near(*p, Point3::new(-10.0, 0.0, 0.0))));
        e.run_line("SelNone").unwrap();
        e.run_line("Select #3").unwrap();
        assert!(e.run_line("Intersect").is_err());
    }

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
        let arc = e
            .doc()
            .objects()
            .find_map(|o| match &o.geometry {
                Geometry::Arc(a) => Some(*a),
                _ => None,
            })
            .expect("fillet arc");
        assert!(near(arc.center(), Point3::new(90.0, 10.0, 0.0)), "{arc:?}");
        assert!((arc.radius - 10.0).abs() < 1e-9);
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
