//! Surface commands: PlanarSrf, ExtrudeSrf, Cap, Loft, Revolve, Sweep1.
//!
//! There are no NURBS surfaces yet (ADR 0001): every result is a triangle mesh.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, Object, ObjectId};
use forma_geom::{
    self as geom, cap_planar_holes, extrude_open_mesh, loft_mesh, planar_mesh, point_in_polygon,
    resample, revolve_mesh, sweep1_mesh, Plane, Point3, Vec3,
};

/// Add `items` (geometry with the attributes of its source object) as one undo
/// step and select the new objects.
pub(crate) fn add_selected(ctx: &mut Context, items: Vec<(Geometry, Object)>) -> Vec<ObjectId> {
    let mut t = ctx.doc.begin();
    let ids: Vec<ObjectId> = items.into_iter().map(|(g, o)| t.add_like(g, &o)).collect();
    t.commit();
    ctx.selection = ids.iter().copied().collect();
    ids
}

/// Points of a closed curve without the repeated closing point.
pub(crate) fn loop_points(g: &Geometry) -> Vec<Point3> {
    let mut p = g.curve_points();
    if p.len() > 1 && p[0].distance_to(*p.last().expect("len")) < 1e-9 {
        p.pop();
    }
    p
}

/// Plane of a closed planar curve, if it is planar within `tol`.
pub(crate) fn closed_plane(g: &Geometry, tol: f64) -> Option<(Plane, Vec<Point3>)> {
    if !g.is_closed_curve() {
        return None;
    }
    let pts = loop_points(g);
    let n = g.curve_normal()?;
    let plane = Plane::from_normal(pts[0], n);
    pts.iter()
        .all(|p| plane.coords(*p).2.abs() <= tol)
        .then_some((plane, pts))
}

simple_command!(
    PlanarSrf,
    "PlanarSrf",
    &[],
    "PlanarSrf — planar surfaces (meshes) from the selected closed planar curves; inner curves become holes"
);
impl Command for PlanarSrf {
    impl_meta!(PlanarSrf);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("PlanarSrf")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        // Group coplanar loops.
        struct Group {
            plane: Plane,
            loops: Vec<(Vec<Point3>, f64)>,
            source: Object,
        }
        let mut groups: Vec<Group> = Vec::new();
        for id in &ids {
            let obj = ctx.doc.object(*id).expect("selected");
            let Some((plane, pts)) = closed_plane(&obj.geometry, tol) else {
                continue;
            };
            let area = geom::newell_area(&pts).length();
            let same = groups.iter_mut().find(|g| {
                g.plane.z.cross(plane.z).length() < 1e-6 && g.plane.coords(pts[0]).2.abs() <= tol
            });
            match same {
                Some(g) => g.loops.push((pts, area)),
                None => groups.push(Group {
                    plane,
                    loops: vec![(pts, area)],
                    source: obj.clone(),
                }),
            }
        }
        if groups.is_empty() {
            return Err(CommandError::Invalid(
                "PlanarSrf: select closed planar curves".into(),
            ));
        }
        let mut out = Vec::new();
        for g in groups {
            let n = g.loops.len();
            let contains = |i: usize, j: usize| {
                // Loop j inside loop i.
                i != j
                    && g.loops[i].1 > g.loops[j].1
                    && point_in_polygon(g.loops[j].0[0], &g.loops[i].0, &g.plane)
            };
            let depth: Vec<usize> = (0..n)
                .map(|j| (0..n).filter(|i| contains(*i, j)).count())
                .collect();
            for o in (0..n).filter(|i| depth[*i].is_multiple_of(2)) {
                let holes: Vec<Vec<Point3>> = (0..n)
                    .filter(|h| depth[*h] == depth[o] + 1 && contains(o, *h))
                    .map(|h| g.loops[h].0.clone())
                    .collect();
                let m = planar_mesh(&g.loops[o].0, &holes, g.plane.z);
                out.push((Geometry::Mesh(m), g.source.clone()));
            }
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("{n} planar surface(s)"))
    }
}

simple_command!(
    ExtrudeSrf,
    "ExtrudeSrf",
    &[],
    "ExtrudeSrf <distance> [direction] — extrude the selected open meshes (e.g. PlanarSrf) into closed solids"
);
impl Command for ExtrudeSrf {
    impl_meta!(ExtrudeSrf);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let d = args.number("extrusion distance")?;
        if d.abs() <= ctx.tolerance.absolute {
            return Err(CommandError::Invalid("distance is too small".into()));
        }
        let dir = args.optional_vector();
        let ids = ctx.selected("ExtrudeSrf")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        let mut out = Vec::new();
        for id in ids {
            let obj = ctx.doc.object(id).expect("selected").clone();
            let Geometry::Mesh(m) = &obj.geometry else {
                continue;
            };
            let along = dir
                .or_else(|| m.area_vector().normalized())
                .and_then(Vec3::normalized)
                .unwrap_or(Vec3::Z);
            if let Some(solid) = extrude_open_mesh(m, along * d, tol) {
                out.push((Geometry::Mesh(solid), obj));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "ExtrudeSrf: select open surfaces (meshes)".into(),
            ));
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("extruded {n} surface(s) by {d}"))
    }
}

simple_command!(
    Cap,
    "Cap",
    &[],
    "Cap — close the planar holes of the selected meshes"
);
impl Command for Cap {
    impl_meta!(Cap);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Cap")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        let mut t = ctx.doc.begin();
        let mut caps = 0;
        for id in ids {
            let Geometry::Mesh(m) = &t.doc().object(id).expect("selected").geometry else {
                continue;
            };
            let (capped, n) = cap_planar_holes(m, tol);
            if n > 0 {
                t.replace(id, Geometry::Mesh(capped));
                caps += n;
            }
        }
        if caps == 0 {
            return Err(CommandError::Invalid(
                "Cap: no planar holes to close".into(),
            ));
        }
        t.commit();
        Ok(format!("{caps} cap(s) added"))
    }
}

/// Selected curves (objects, in id order).
pub(crate) fn selected_curves(ctx: &Context, cmd: &str) -> Result<Vec<Object>, CommandError> {
    Ok(ctx
        .selected(cmd)?
        .into_iter()
        .filter_map(|id| ctx.doc.object(id))
        .filter(|o| o.geometry.is_curve())
        .cloned()
        .collect())
}

simple_command!(
    Loft,
    "Loft",
    &[],
    "Loft — mesh surface through the selected curves (ordered along their common normal, else by id)"
);
impl Command for Loft {
    impl_meta!(Loft);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let mut curves = selected_curves(ctx, "Loft")?;
        if curves.len() < 2 {
            return Err(CommandError::Invalid(
                "Loft: select two or more curves".into(),
            ));
        }
        let closed = curves[0].geometry.is_closed_curve();
        if curves
            .iter()
            .any(|c| c.geometry.is_closed_curve() != closed)
        {
            return Err(CommandError::Invalid(
                "Loft: curves must be all open or all closed".into(),
            ));
        }
        // Order along the average normal when the curves are stacked along it.
        let normals: Vec<Vec3> = curves
            .iter()
            .filter_map(|c| c.geometry.curve_normal())
            .collect();
        if normals.len() == curves.len() {
            let n0 = normals[0];
            let avg = normals
                .iter()
                .fold(Vec3::new(0.0, 0.0, 0.0), |a, n| {
                    a + if n.dot(n0) < 0.0 { -*n } else { *n }
                })
                .normalized()
                .unwrap_or(n0);
            let key = |o: &Object| o.geometry.bounding_box().center().to_vec().dot(avg);
            let keys: Vec<f64> = curves.iter().map(key).collect();
            let spread = keys.iter().fold(f64::MIN, |a, k| a.max(*k))
                - keys.iter().fold(f64::MAX, |a, k| a.min(*k));
            if spread > ctx.tolerance.absolute {
                curves.sort_by(|a, b| key(a).total_cmp(&key(b)));
            }
        }
        let raw: Vec<Vec<Point3>> = curves
            .iter()
            .map(|c| {
                if closed {
                    loop_points(&c.geometry)
                } else {
                    c.geometry.curve_points()
                }
            })
            .collect();
        let n = raw.iter().map(Vec::len).max().unwrap_or(2).clamp(2, 256);
        let mut sections: Vec<Vec<Point3>> = Vec::new();
        for (k, r) in raw.iter().enumerate() {
            let mut s = resample(r, n, closed);
            if let Some(prev) = sections.last() {
                if closed {
                    let (a, b) = (geom::newell_area(prev), geom::newell_area(&s));
                    if a.dot(b) < 0.0 {
                        s.reverse();
                    }
                    let start = (0..s.len())
                        .min_by(|i, j| {
                            s[*i]
                                .distance_to(prev[0])
                                .total_cmp(&s[*j].distance_to(prev[0]))
                        })
                        .unwrap_or(0);
                    s.rotate_left(start);
                } else if s[0].distance_to(prev[0]) + s[n - 1].distance_to(prev[n - 1])
                    > s[0].distance_to(prev[n - 1]) + s[n - 1].distance_to(prev[0])
                {
                    s.reverse();
                }
            }
            if s.len() != n {
                return Err(CommandError::Invalid(format!(
                    "Loft: curve #{} is degenerate",
                    curves[k].id.0
                )));
            }
            sections.push(s);
        }
        let m = loft_mesh(&sections, closed);
        if m.triangles.is_empty() {
            return Err(CommandError::Invalid("Loft: degenerate result".into()));
        }
        let ids = add_selected(ctx, vec![(Geometry::Mesh(m), curves[0].clone())]);
        Ok(format!(
            "added #{} loft through {} curves",
            ids[0].0,
            curves.len()
        ))
    }
}

simple_command!(
    Revolve,
    "Revolve",
    &["Rev"],
    "Revolve <axis start> <axis end> [angle=360] — revolve the selected curves into mesh surfaces"
);
impl Command for Revolve {
    impl_meta!(Revolve);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of revolve axis", ctx.last_point)?;
        let b = args.point("end of revolve axis", Some(a))?;
        let angle = args.optional_number().unwrap_or(360.0);
        if (b - a).length() <= ctx.tolerance.absolute {
            return Err(CommandError::Invalid("revolve axis is too short".into()));
        }
        if angle.abs() < 1e-9 {
            return Err(CommandError::Invalid("angle must not be zero".into()));
        }
        let curves = selected_curves(ctx, "Revolve")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        let mut out = Vec::new();
        for c in curves {
            let m = revolve_mesh(
                &c.geometry.curve_points(),
                a,
                b - a,
                angle.to_radians(),
                tol,
            );
            if !m.triangles.is_empty() {
                out.push((Geometry::Mesh(m), c));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "Revolve: select curves (not lying on the axis)".into(),
            ));
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("revolved {n} curve(s) by {angle}°"))
    }
}

simple_command!(
    Sweep1,
    "Sweep1",
    &[],
    "Sweep1 #rail — sweep the selected profile curves along the rail curve"
);
impl Command for Sweep1 {
    impl_meta!(Sweep1);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("rail curve #id"))?;
        let rail_id = crate::edit::parse_ids(vec![tok])?[0];
        let rail = ctx
            .doc
            .object(rail_id)
            .filter(|o| o.geometry.is_curve())
            .ok_or_else(|| CommandError::Invalid(format!("#{} is not a curve", rail_id.0)))?
            .geometry
            .curve_points();
        let profiles: Vec<Object> = selected_curves(ctx, "Sweep1")?
            .into_iter()
            .filter(|o| o.id != rail_id)
            .collect();
        if profiles.is_empty() {
            return Err(CommandError::Invalid(
                "Sweep1: select profile curves".into(),
            ));
        }
        let mut out = Vec::new();
        for p in profiles {
            let m = sweep1_mesh(&rail, &p.geometry.curve_points());
            if !m.triangles.is_empty() {
                out.push((Geometry::Mesh(m), p));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid("Sweep1: degenerate result".into()));
        }
        let n = out.len();
        add_selected(ctx, out);
        Ok(format!("swept {n} profile(s)"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::{Geometry, ObjectId};
    use std::f64::consts::PI;

    fn mesh(e: &Engine, id: u64) -> forma_geom::Mesh {
        match &e.doc().object(ObjectId(id)).unwrap().geometry {
            Geometry::Mesh(m) => m.clone(),
            g => panic!("{g:?}"),
        }
    }

    #[test]
    fn planar_srf_with_hole_then_extrude() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 100,50").unwrap(); // #1
        e.run_line("Circle 30,25 10").unwrap(); // #2 hole
        e.run_line("Rectangle 200,0 210,10").unwrap(); // #3 separate region
        e.run_line("SelAll").unwrap();
        e.run_line("PlanarSrf").unwrap();
        assert_eq!(e.doc().len(), 5);
        let m = mesh(&e, 4);
        let hole = 96.0 / 2.0 * 100.0 * (2.0 * PI / 96.0).sin(); // inscribed 96-gon
        assert!((m.area() - (5000.0 - hole)).abs() < 1e-6, "{}", m.area());
        assert!((mesh(&e, 5).area() - 100.0).abs() < 1e-9);
        assert_eq!(e.ctx.selection.len(), 2);
        e.run_line("ExtrudeSrf 20").unwrap();
        let s = mesh(&e, 6);
        assert!(s.is_closed(1e-6));
        assert!(
            (s.volume() - (5000.0 - hole) * 20.0).abs() < 1e-4,
            "{}",
            s.volume()
        );
        assert!((s.bounding_box().unwrap().max.z - 20.0).abs() < 1e-9);
        assert!(e.run_line("ExtrudeSrf 20").is_err()); // closed solids cannot be extruded
    }

    #[test]
    fn cap_extruded_open_curve_walls() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 10,20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Extrude 5").unwrap(); // closed solid already
        assert!(e.run_line("Cap").is_err());
        // An open tube: extrude walls of an open-then-joined profile.
        e.run_line("New").unwrap();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("Circle 0,0,30 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Loft").unwrap();
        assert!(!mesh(&e, 3).is_closed(1e-6));
        e.run_line("Cap").unwrap();
        let m = mesh(&e, 3);
        assert!(m.is_closed(1e-6));
        let exact = PI * 100.0 * 30.0;
        assert!((m.volume() - exact).abs() / exact < 0.01, "{}", m.volume());
    }

    #[test]
    fn loft_orders_by_height_and_open_curves() {
        let mut e = Engine::new();
        e.run_line("Circle 0,0,20 5").unwrap();
        e.run_line("Circle 0,0,0 10").unwrap();
        e.run_line("Rectangle -5,-5,10 5,5,10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Loft").unwrap();
        let m = mesh(&e, 4);
        let b = m.bounding_box().unwrap();
        assert!((b.max.z - 20.0).abs() < 1e-9 && b.min.z.abs() < 1e-9);
        assert!(m.normals.len() == m.positions.len());
        // Two open lines → a flat strip.
        e.run_line("New").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Line 10,10 0,10").unwrap(); // opposite direction on purpose
        e.run_line("SelAll").unwrap();
        e.run_line("Loft").unwrap();
        assert!((mesh(&e, 3).area() - 100.0).abs() < 1e-9);
        e.run_line("New").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("SelAll").unwrap();
        assert!(e.run_line("Loft").is_err());
    }

    #[test]
    fn revolve_rectangle_into_cylinder() {
        let mut e = Engine::new();
        e.run_line("Polyline 0,0,0 10,0,0 10,0,20 0,0,20 c")
            .unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Revolve 0,0,0 0,0,1").unwrap();
        let m = mesh(&e, 2);
        assert!(m.is_closed(1e-6));
        let exact = PI * 100.0 * 20.0;
        assert!((m.volume() - exact).abs() / exact < 0.01, "{}", m.volume());
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Revolve 0,0,0 0,0,1 90").unwrap();
        assert!(!mesh(&e, 3).is_closed(1e-6));
        assert!(e.run_line("Revolve 0,0 0,0").is_err());
    }

    #[test]
    fn sweep_circle_along_line() {
        let mut e = Engine::new();
        e.run_line("Line 0,0,0 0,0,100").unwrap(); // rail #1
        e.run_line("Circle 0,0,0 5").unwrap(); // profile #2
        e.run_line("Select #2").unwrap();
        e.run_line("Sweep1 #1").unwrap();
        let m = mesh(&e, 3);
        let b = m.bounding_box().unwrap();
        assert!((b.max.z - 100.0).abs() < 1e-9 && (b.max.x - 5.0).abs() < 1e-9);
        let lateral = 2.0 * PI * 5.0 * 100.0;
        assert!((m.area() - lateral).abs() / lateral < 0.01);
        assert!(e.run_line("Sweep1 #2").is_err()); // only the rail itself selected
    }
}
