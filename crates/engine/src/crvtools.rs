//! Curve tools: Divide, DivideByLength, Contour, Section, DupBorder, DupEdge,
//! CurveBoolean, Convert, Rebuild, CloseCrv, ExtractPt, Project, Pull.

use crate::create::positive;
use crate::surfaces::loop_points;
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{
    chain_points_at_lengths, chain_segments, division_lengths, mesh_border_loops,
    mesh_edge_segments, mesh_plane_section, polyline_plane_points, region_boolean, resample,
    simplify_polyline, step_lengths, CircleArc, NurbsCurve, Plane, Point3, RegionOp, Seg, Vec3,
};

/// Add geometry on the current layer as one undo step and select it.
fn add_and_select(ctx: &mut Context, items: Vec<Geometry>) -> Vec<ObjectId> {
    let mut t = ctx.doc.begin();
    let ids: Vec<ObjectId> = items.into_iter().map(|g| t.add(g)).collect();
    t.commit();
    ctx.selection = ids.iter().copied().collect();
    ids
}

/// Selected curve geometry (ids and geometry), or an error.
fn curves(ctx: &Context, cmd: &str) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let out: Vec<(ObjectId, Geometry)> = ctx
        .selected(cmd)?
        .into_iter()
        .filter_map(|id| ctx.doc.object(id).map(|o| (id, o.geometry.clone())))
        .filter(|(_, g)| g.is_curve())
        .collect();
    if out.is_empty() {
        return Err(CommandError::Invalid(format!("{cmd}: select curves")));
    }
    Ok(out)
}

/// Points at distances along a curve (exact for lines, arcs and NURBS).
fn points_at_lengths(g: &Geometry, lengths: &[f64]) -> Vec<Point3> {
    match g {
        Geometry::Nurbs(n) => n.points_at_lengths(lengths),
        g => g
            .to_chain()
            .map(|c| chain_points_at_lengths(&c, lengths))
            .unwrap_or_default(),
    }
}

/// Fine polyline of a curve (arcs with 1024 segments per turn).
fn dense_points(g: &Geometry) -> Vec<Point3> {
    let arc = |a: &CircleArc| a.points(1024);
    match g {
        Geometry::Arc(a) => arc(a),
        Geometry::PolyCurve(segs) => {
            let mut out: Vec<Point3> = Vec::new();
            for s in segs {
                let p = match s {
                    Seg::Line(a, b) => vec![*a, *b],
                    Seg::Arc(a) => arc(a),
                };
                let skip = usize::from(!out.is_empty());
                out.extend_from_slice(&p[skip..]);
            }
            out
        }
        Geometry::Nurbs(n) => n.sample(64),
        g => g.curve_points(),
    }
}

fn divide(ctx: &mut Context, cmd: &str, lengths: impl Fn(f64, bool) -> Vec<f64>) -> CommandResult {
    let list = curves(ctx, cmd)?;
    let mut pts = Vec::new();
    for (_, g) in &list {
        let total = g.length().unwrap_or(0.0);
        if total <= ctx.tolerance.absolute {
            continue;
        }
        pts.extend(points_at_lengths(g, &lengths(total, g.is_closed_curve())));
    }
    let n = pts.len();
    add_and_select(ctx, pts.into_iter().map(Geometry::Point).collect());
    Ok(format!("{n} point(s) added"))
}

simple_command!(
    Divide,
    "Divide",
    &[],
    "Divide <segments> — points dividing the selected curves into equal-length segments"
);
impl Command for Divide {
    impl_meta!(Divide);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let n = args.number("number of segments")?;
        if n < 1.0 || n.fract().abs() > 1e-9 || n > 100_000.0 {
            return Err(CommandError::Invalid(
                "number of segments must be a whole number ≥ 1".into(),
            ));
        }
        divide(ctx, "Divide", |total, closed| {
            division_lengths(total, n as usize, closed)
        })
    }
}

simple_command!(
    DivideByLength,
    "DivideByLength",
    &["DivideLength"],
    "DivideByLength <length> — points every <length> along the selected curves"
);
impl Command for DivideByLength {
    impl_meta!(DivideByLength);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let step = positive(args.number("segment length")?, "length", ctx)?.abs();
        divide(ctx, "DivideByLength", |total, _| {
            if total / step > 100_000.0 {
                Vec::new()
            } else {
                step_lengths(total, step)
            }
        })
    }
}

/// Sections of the selected meshes (polylines) and curves (points) by `planes`.
fn cut(ctx: &mut Context, cmd: &str, planes: &[Plane]) -> CommandResult {
    let ids = ctx.selected(cmd)?;
    let tol = ctx.doc.absolute_tolerance.max(1e-9);
    let mut out = Vec::new();
    for id in ids {
        let g = ctx.doc.object(id).expect("selected").geometry.clone();
        for plane in planes {
            match &g {
                Geometry::Mesh(m) => {
                    for pl in mesh_plane_section(m, plane, tol) {
                        out.push(Geometry::Polyline(pl));
                    }
                }
                g if g.is_curve() => {
                    out.extend(
                        polyline_plane_points(&dense_points(g), plane)
                            .into_iter()
                            .map(Geometry::Point),
                    );
                }
                _ => {}
            }
        }
    }
    if out.is_empty() {
        return Err(CommandError::Invalid(format!(
            "{cmd}: the plane(s) do not cut the selected objects"
        )));
    }
    let n = out.len();
    add_and_select(ctx, out);
    Ok(format!("{n} section object(s)"))
}

simple_command!(
    Contour,
    "Contour",
    &[],
    "Contour <base point> <direction point> <spacing> — sections of the selected meshes (and points on curves) by parallel planes"
);
impl Command for Contour {
    impl_meta!(Contour);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let base = args.point("contour plane base point", ctx.last_point)?;
        let dir_pt = args.point("direction perpendicular to contour planes", Some(base))?;
        let spacing = positive(args.number("distance between contours")?, "spacing", ctx)?.abs();
        let dir = (dir_pt - base)
            .normalized()
            .ok_or_else(|| CommandError::Invalid("direction points coincide".into()))?;
        let bb = crate::transform::selection_box(ctx, "Contour")?;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for c in bb.corners() {
            let d = (c - base).dot(dir);
            lo = lo.min(d);
            hi = hi.max(d);
        }
        let (k0, k1) = ((lo / spacing).ceil() as i64, (hi / spacing).floor() as i64);
        if k1 - k0 > 10_000 {
            return Err(CommandError::Invalid("too many contours".into()));
        }
        let planes: Vec<Plane> = (k0..=k1)
            .map(|k| Plane::from_normal(base + dir * (k as f64 * spacing), dir))
            .collect();
        cut(ctx, "Contour", &planes)
    }
}

simple_command!(
    Section,
    "Section",
    &[],
    "Section <start> <end> [view normal] — section of the selected meshes by the plane through the line and the view direction"
);
impl Command for Section {
    impl_meta!(Section);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of section", ctx.last_point)?;
        let b = args.point("end of section", Some(a))?;
        let view = args.optional_vector().unwrap_or(Vec3::Z);
        let n = (b - a)
            .cross(view)
            .normalized()
            .ok_or_else(|| CommandError::Invalid("section line is parallel to the view".into()))?;
        cut(ctx, "Section", &[Plane::from_normal(a, n)])
    }
}

/// Selected meshes, or an error.
fn meshes(ctx: &Context, cmd: &str) -> Result<Vec<(ObjectId, forma_geom::Mesh)>, CommandError> {
    let out: Vec<_> = ctx
        .selected(cmd)?
        .into_iter()
        .filter_map(|id| match &ctx.doc.object(id)?.geometry {
            Geometry::Mesh(m) => Some((id, m.clone())),
            _ => None,
        })
        .collect();
    if out.is_empty() {
        return Err(CommandError::Invalid(format!("{cmd}: select meshes")));
    }
    Ok(out)
}

simple_command!(
    DupBorder,
    "DupBorder",
    &[],
    "DupBorder — closed polylines along the open borders of the selected meshes"
);
impl Command for DupBorder {
    impl_meta!(DupBorder);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let out: Vec<Geometry> = meshes(ctx, "DupBorder")?
            .iter()
            .flat_map(|(_, m)| mesh_border_loops(m, tol))
            .map(Geometry::Polyline)
            .collect();
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "DupBorder: the selected meshes are closed".into(),
            ));
        }
        let n = out.len();
        add_and_select(ctx, out);
        Ok(format!("{n} border curve(s)"))
    }
}

simple_command!(
    DupEdge,
    "DupEdge",
    &["DupAllEdges", "ExtractEdges"],
    "DupEdge — curves along all visible edges (creases and borders) of the selected meshes"
);
impl Command for DupEdge {
    impl_meta!(DupEdge);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        for (_, m) in meshes(ctx, "DupEdge")? {
            let mut segs = mesh_edge_segments(&m);
            // A crease is seen from both faces: keep one copy.
            let mut unique: Vec<[Point3; 2]> = Vec::new();
            segs.sort_by(|a, b| {
                (a[0].x + a[1].x, a[0].y + a[1].y, a[0].z + a[1].z)
                    .partial_cmp(&(b[0].x + b[1].x, b[0].y + b[1].y, b[0].z + b[1].z))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for s in segs {
                let dup = unique.iter().rev().take(16).any(|u| {
                    (u[0].distance_to(s[0]) <= tol && u[1].distance_to(s[1]) <= tol)
                        || (u[0].distance_to(s[1]) <= tol && u[1].distance_to(s[0]) <= tol)
                });
                if !dup {
                    unique.push(s);
                }
            }
            for pl in chain_segments(&unique, tol) {
                out.push(
                    Geometry::from_chain(forma_geom::Chain::from_points(&pl)).expect("2+ points"),
                );
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid("DupEdge: no visible edges".into()));
        }
        let n = out.len();
        add_and_select(ctx, out);
        Ok(format!("{n} edge curve(s)"))
    }
}

simple_command!(
    CurveBoolean,
    "CurveBoolean",
    &["CrvBoolean"],
    "CurveBoolean <union|intersection|difference> [delete] — boolean of the regions inside the selected closed planar curves (difference: the largest minus the others)"
);
impl Command for CurveBoolean {
    impl_meta!(CurveBoolean);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let op = match args
            .next_token()
            .ok_or(CommandError::MissingInput(
                "union, intersection or difference",
            ))?
            .to_ascii_lowercase()
            .as_str()
        {
            "union" | "u" => RegionOp::Union,
            "intersection" | "i" => RegionOp::Intersection,
            "difference" | "d" => RegionOp::Difference,
            o => return Err(CommandError::BadInput(format!("operation {o}"))),
        };
        let delete = args.keyword("delete");
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let list = curves(ctx, "CurveBoolean")?;
        let mut loops: Vec<(ObjectId, Vec<Point3>, f64)> = Vec::new();
        let mut plane: Option<Plane> = None;
        for (id, g) in &list {
            let Some((pl, _)) = crate::surfaces::closed_plane(g, tol) else {
                return Err(CommandError::Invalid(format!(
                    "CurveBoolean: #{} is not a closed planar curve",
                    id.0
                )));
            };
            let pts = loop_points(g);
            let p = *plane.get_or_insert(pl);
            if p.z.cross(pl.z).length() > 1e-6 || pts.iter().any(|q| p.coords(*q).2.abs() > tol) {
                return Err(CommandError::Invalid(
                    "CurveBoolean: the curves are not in the same plane".into(),
                ));
            }
            let area = forma_geom::newell_area(&pts).length();
            loops.push((*id, pts, area));
        }
        if loops.len() < 2 {
            return Err(CommandError::Invalid(
                "CurveBoolean: select two or more closed curves".into(),
            ));
        }
        loops.sort_by(|a, b| b.2.total_cmp(&a.2));
        let plane = plane.expect("at least one loop");
        let polys: Vec<Vec<Point3>> = loops.iter().map(|l| l.1.clone()).collect();
        let result = region_boolean(&polys, op, &plane, tol);
        if result.is_empty() {
            return Err(CommandError::Invalid("CurveBoolean: empty result".into()));
        }
        let n = result.len();
        let mut t = ctx.doc.begin();
        let ids: Vec<ObjectId> = result
            .into_iter()
            .map(|p| t.add(Geometry::Polyline(p)))
            .collect();
        if delete {
            for (id, _, _) in &loops {
                t.remove(*id);
            }
        }
        t.commit();
        ctx.selection = ids.into_iter().collect();
        Ok(format!("{n} curve(s)"))
    }
}

simple_command!(
    Convert,
    "Convert",
    &["ConvertToPolyline"],
    "Convert [tolerance] — replace the selected curves by polylines within the tolerance (default: 10 × document tolerance)"
);
impl Command for Convert {
    impl_meta!(Convert);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tol = args
            .optional_number()
            .unwrap_or(ctx.doc.absolute_tolerance * 10.0)
            .abs()
            .max(1e-9);
        let list = curves(ctx, "Convert")?;
        let mut t = ctx.doc.begin();
        let mut total = 0;
        for (id, g) in &list {
            let pts = simplify_polyline(&dense_points(g), tol);
            if pts.len() >= 2 {
                total += pts.len() - 1;
                t.replace(*id, Geometry::Polyline(pts));
            }
        }
        t.commit();
        Ok(format!(
            "converted {} curve(s) to polylines, {total} segments",
            list.len()
        ))
    }
}

simple_command!(
    Rebuild,
    "Rebuild",
    &[],
    "Rebuild <point count> [degree=3] — replace the selected curves by smooth NURBS curves with that many control points"
);
impl Command for Rebuild {
    impl_meta!(Rebuild);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let n = args.number("point count")?;
        let degree = args.optional_number().unwrap_or(3.0);
        if !(1.0..=11.0).contains(&degree) || degree.fract().abs() > 1e-9 {
            return Err(CommandError::Invalid("degree must be 1 to 11".into()));
        }
        if n < degree + 1.0 || n.fract().abs() > 1e-9 || n > 500.0 {
            return Err(CommandError::Invalid(
                "point count must be a whole number from degree + 1 to 500".into(),
            ));
        }
        let (n, degree) = (n as usize, degree as usize);
        let list = curves(ctx, "Rebuild")?;
        let mut t = ctx.doc.begin();
        for (id, g) in &list {
            let closed = g.is_closed_curve();
            let mut pts = resample(&dense_points(g), if closed { n - 1 } else { n }, closed);
            if closed {
                pts.push(pts[0]);
            }
            let c = NurbsCurve::interpolate(&pts, degree)
                .ok_or_else(|| CommandError::Invalid(format!("cannot rebuild #{}", id.0)))?;
            t.replace(*id, Geometry::Nurbs(c));
        }
        t.commit();
        Ok(format!("rebuilt {} curve(s) with {n} points", list.len()))
    }
}

simple_command!(
    CloseCrv,
    "CloseCrv",
    &[],
    "CloseCrv — close the selected open curves (a closing line, or moving the end of a NURBS curve)"
);
impl Command for CloseCrv {
    impl_meta!(CloseCrv);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let list = curves(ctx, "CloseCrv")?;
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for (id, g) in &list {
            if g.is_closed_curve() {
                continue;
            }
            let closed = match g {
                Geometry::Polyline(p) if p.len() >= 3 => {
                    let mut p = p.clone();
                    p.push(p[0]);
                    Geometry::Polyline(p)
                }
                Geometry::Arc(a) => {
                    Geometry::PolyCurve(vec![Seg::Arc(*a), Seg::Line(a.end(), a.start())])
                }
                Geometry::PolyCurve(s) => {
                    let mut s = s.clone();
                    s.push(Seg::Line(s[s.len() - 1].end(), s[0].start()));
                    Geometry::PolyCurve(s)
                }
                Geometry::Nurbs(c) if c.points.len() >= 3 => {
                    let mut c = c.clone();
                    let first = c.points[0];
                    *c.points.last_mut().expect("len") = first;
                    let w0 = c.weights[0];
                    *c.weights.last_mut().expect("len") = w0;
                    Geometry::Nurbs(c)
                }
                _ => continue,
            };
            t.replace(*id, closed);
            n += 1;
        }
        if n == 0 {
            return Err(CommandError::Invalid(
                "CloseCrv: no open curves that can be closed".into(),
            ));
        }
        t.commit();
        Ok(format!("closed {n} curve(s)"))
    }
}

simple_command!(
    ExtractPt,
    "ExtractPt",
    &[],
    "ExtractPt — point objects at the vertices / control points of the selected curves and meshes"
);
impl Command for ExtractPt {
    impl_meta!(ExtractPt);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("ExtractPt")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut pts: Vec<Point3> = Vec::new();
        for id in ids {
            let g = &ctx.doc.object(id).expect("selected").geometry;
            let mut p = match g {
                Geometry::Line(l) => vec![l.from, l.to],
                Geometry::Polyline(p) => p.clone(),
                Geometry::Arc(a) if a.is_closed() => vec![a.center()],
                Geometry::Arc(a) => vec![a.start(), a.end()],
                Geometry::PolyCurve(s) => {
                    let mut v = vec![s[0].start()];
                    v.extend(s.iter().map(Seg::end));
                    v
                }
                Geometry::Nurbs(n) => n.points.clone(),
                Geometry::Mesh(m) => m.welded(tol).positions,
                _ => Vec::new(),
            };
            if g.is_closed_curve() && p.len() > 1 && p[0].distance_to(p[p.len() - 1]) <= tol {
                p.pop();
            }
            pts.extend(p);
        }
        if pts.is_empty() {
            return Err(CommandError::Invalid("ExtractPt: no points".into()));
        }
        let n = pts.len();
        add_and_select(ctx, pts.into_iter().map(Geometry::Point).collect());
        Ok(format!("{n} point(s) extracted"))
    }
}

/// Selected curves and meshes, split (both must be present).
#[allow(clippy::type_complexity)]
fn curves_and_meshes(
    ctx: &Context,
    cmd: &str,
) -> Result<(Vec<Geometry>, Vec<forma_geom::Mesh>), CommandError> {
    let mut crvs = Vec::new();
    let mut ms = Vec::new();
    for id in ctx.selected(cmd)? {
        match &ctx.doc.object(id).expect("selected").geometry {
            Geometry::Mesh(m) => ms.push(m.clone()),
            g if g.is_curve() => crvs.push(g.clone()),
            Geometry::Point(p) => crvs.push(Geometry::Point(*p)),
            _ => {}
        }
    }
    if crvs.is_empty() || ms.is_empty() {
        return Err(CommandError::Invalid(format!(
            "{cmd}: select the curves (or points) and the meshes / surfaces to put them on"
        )));
    }
    Ok((crvs, ms))
}

/// Sample points of a curve fine enough to follow a surface (about 1/200 of its
/// length, at least the curve's own vertices).
fn follow_points(g: &Geometry) -> Vec<Point3> {
    if let Geometry::Point(p) = g {
        return vec![*p];
    }
    let pts = dense_points(g);
    let len = g.length().unwrap_or(0.0);
    forma_geom::densify(&pts, (len / 200.0).max(1e-6))
}

/// Polylines from runs of `Some` points (a `None` breaks the run); single
/// points become point objects.
fn runs(points: Vec<Option<Point3>>, tol: f64) -> Vec<Geometry> {
    let mut out = Vec::new();
    let mut cur: Vec<Point3> = Vec::new();
    let flush = |cur: &mut Vec<Point3>, out: &mut Vec<Geometry>| {
        cur.dedup_by(|a, b| a.distance_to(*b) <= tol);
        if cur.len() >= 2 {
            let simple = simplify_polyline(cur, tol * 0.5);
            out.push(Geometry::Polyline(simple));
        } else if cur.len() == 1 {
            out.push(Geometry::Point(cur[0]));
        }
        cur.clear();
    };
    for p in points {
        match p {
            Some(p) => cur.push(p),
            None => flush(&mut cur, &mut out),
        }
    }
    flush(&mut cur, &mut out);
    out
}

simple_command!(
    Project,
    "Project",
    &[],
    "Project [direction=0,0,-1] — project the selected curves and points onto the selected meshes / surfaces (the first one met along the direction)"
);
impl Command for Project {
    impl_meta!(Project);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let dir = args
            .optional_vector()
            .unwrap_or(Vec3::new(0.0, 0.0, -1.0))
            .normalized()
            .ok_or_else(|| CommandError::Invalid("direction is zero".into()))?;
        let (crvs, ms) = curves_and_meshes(ctx, "Project")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        // Start every ray far behind everything.
        let bb = crate::transform::selection_box(ctx, "Project")?;
        let far = bb.min.distance_to(bb.max) + 1.0;
        let mut out = Vec::new();
        for g in &crvs {
            let hits: Vec<Option<Point3>> = follow_points(g)
                .into_iter()
                .map(|p| {
                    let o = p - dir * far;
                    ms.iter()
                        .flat_map(|m| m.ray_hits(o, dir))
                        .filter(|t| *t >= 0.0)
                        .min_by(f64::total_cmp)
                        .map(|t| o + dir * t)
                })
                .collect();
            out.extend(runs(hits, tol));
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "Project: nothing lands on the meshes".into(),
            ));
        }
        let n = out.len();
        add_and_select(ctx, out);
        Ok(format!("{n} projected object(s)"))
    }
}

simple_command!(
    Pull,
    "Pull",
    &[],
    "Pull — pull the selected curves and points to the closest points of the selected meshes / surfaces"
);
impl Command for Pull {
    impl_meta!(Pull);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let (crvs, ms) = curves_and_meshes(ctx, "Pull")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        for g in &crvs {
            let pts: Vec<Option<Point3>> = follow_points(g)
                .into_iter()
                .map(|p| {
                    ms.iter()
                        .filter_map(|m| m.closest_triangle(p))
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(_, q, _)| q)
                })
                .collect();
            out.extend(runs(pts, tol));
        }
        let n = out.len();
        add_and_select(ctx, out);
        Ok(format!("{n} pulled object(s)"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    fn points(e: &Engine) -> Vec<Point3> {
        e.doc()
            .objects()
            .filter_map(|o| match o.geometry {
                Geometry::Point(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn divide_by_count_and_length() {
        let mut e = Engine::new();
        e.run_line("Polyline 0,0 100,0 100,50").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Divide 3").unwrap();
        let p = points(&e);
        assert_eq!(p.len(), 4);
        assert!(p[2].distance_to(Point3::new(100.0, 0.0, 0.0)) < 1e-9);
        assert_eq!(e.ctx.selection.len(), 4);
        e.run_line("Delete").unwrap();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("SelNone").unwrap();
        e.run_line("SelCrv").unwrap();
        e.run_line("DivideByLength 40").unwrap();
        // Polyline: 0, 40, 80, 120 (4); circle of length 62.8: 0, 40 (2).
        assert_eq!(points(&e).len(), 6);
        assert!(points(&e)
            .iter()
            .skip(4)
            .all(|q| (q.distance_to(Point3::ORIGIN) - 10.0).abs() < 1e-9));
        assert!(e.run_line("Divide 0").is_err());
    }

    #[test]
    fn contour_and_section_of_a_box() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 100,50 270").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Contour 0,0,0 0,0,1 100").unwrap();
        // Planes at z = 0 (face lies in it: no section), 100, 200.
        let polys: Vec<_> = e
            .doc()
            .objects()
            .filter(|o| matches!(o.geometry, Geometry::Polyline(_)))
            .collect();
        assert_eq!(polys.len(), 2);
        assert!(polys.iter().all(|o| o.geometry.is_closed_curve()));
        assert!((polys[0].geometry.length().unwrap() - 300.0).abs() < 1e-6);
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Section 50,-10 50,60").unwrap();
        let s = &e.doc().objects().last().unwrap().geometry;
        assert!((s.length().unwrap() - 2.0 * (50.0 + 270.0)).abs() < 1e-6);
        assert!(e.run_line("Section 500,0 500,10").is_err());
    }

    #[test]
    fn borders_and_edges() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        assert!(e.run_line("DupBorder").is_err()); // closed
        e.run_line("DupEdge").unwrap();
        let total: f64 = e.doc().objects().filter_map(|o| o.geometry.length()).sum();
        assert!((total - 120.0).abs() < 1e-6, "{total}");
        e.run_line("SelNone").unwrap();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("PlanarSrf").unwrap();
        e.run_line("DupBorder").unwrap();
        let g = &e.doc().objects().last().unwrap().geometry;
        assert!(g.is_closed_curve() && (g.length().unwrap() - 40.0).abs() < 1e-6);
    }

    #[test]
    fn project_and_pull_onto_a_box() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 100,100 50").unwrap(); // #1
        e.run_line("Line -50,50,200 150,50,200").unwrap(); // #2, longer than the box
        e.run_line("Point 20,20,300").unwrap(); // #3
        e.run_line("SelAll").unwrap();
        e.run_line("Project").unwrap();
        let new: Vec<_> = e
            .doc()
            .objects()
            .skip(3)
            .map(|o| o.geometry.clone())
            .collect();
        assert_eq!(new.len(), 2, "{new:?}");
        let b = new[0].bounding_box();
        // Only the part over the box, on its top face.
        assert!((b.min.z - 50.0).abs() < 1e-6 && (b.max.z - 50.0).abs() < 1e-6);
        assert!(
            b.min.x > -1.0 && b.min.x < 1.5 && b.max.x > 98.5 && b.max.x < 101.0,
            "{b:?}"
        );
        let Geometry::Point(p) = new[1] else { panic!() };
        assert!(p.distance_to(Point3::new(20.0, 20.0, 50.0)) < 1e-9);
        e.run_line("Undo").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Pull").unwrap();
        let new: Vec<_> = e
            .doc()
            .objects()
            .skip(3)
            .map(|o| o.geometry.clone())
            .collect();
        assert_eq!(new.len(), 2);
        // The line ends beyond the box are pulled to its top edges.
        let b = new[0].bounding_box();
        assert!(
            b.min.x.abs() < 1e-6 && (b.max.x - 100.0).abs() < 1e-6,
            "{b:?}"
        );
        e.run_line("SelNone").unwrap();
        e.run_line("Select #2").unwrap();
        assert!(e.run_line("Project").is_err());
    }

    #[test]
    fn curve_boolean_rooms() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 400,300").unwrap();
        e.run_line("Rectangle 400,0 700,300").unwrap();
        e.run_line("Circle 100,100 20").unwrap();
        e.run_line("Select #1 #2").unwrap();
        e.run_line("CurveBoolean union").unwrap();
        let g = e.doc().objects().last().unwrap().geometry.clone();
        let Geometry::Polyline(p) = &g else { panic!() };
        assert_eq!(p.len(), 5);
        assert!((g.length().unwrap() - 2.0 * (700.0 + 300.0)).abs() < 1e-6);
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1 #3").unwrap();
        e.run_line("CurveBoolean difference delete").unwrap();
        assert!(e.doc().object(forma_doc::ObjectId(1)).is_none());
        assert_eq!(e.ctx.selection.len(), 2); // outline + hole
        e.run_line("PlanarSrf").unwrap();
        e.run_line("Area").unwrap();
        e.run_line("Undo").unwrap();
        assert!(e.run_line("CurveBoolean xor").is_err());
    }

    #[test]
    fn convert_rebuild_close_extract() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Circle 0,0 50").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Convert 0.5").unwrap();
        let Geometry::Polyline(p) = &e.doc().objects().next().unwrap().geometry else {
            panic!()
        };
        // Sag r(1 − cos(θ/2)) ≤ 0.5 → about 23 segments.
        assert!(p.len() > 15 && p.len() < 40, "{}", p.len());
        e.run_line("Rebuild 12").unwrap();
        let g = e.doc().objects().next().unwrap().geometry.clone();
        let Geometry::Nurbs(n) = &g else { panic!() };
        assert_eq!(n.points.len(), 12);
        assert!(g.is_closed_curve());
        e.run_line("SelNone").unwrap();
        e.run_line("Polyline 0,0 10,0 10,10").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("CloseCrv").unwrap();
        assert!(e.doc().objects().last().unwrap().geometry.is_closed_curve());
        e.run_line("ExtractPt").unwrap();
        assert_eq!(points(&e).len(), 3);
    }
}
