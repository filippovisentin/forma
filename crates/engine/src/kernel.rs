//! Solid commands through the OpenCascade kernel (feature `occt` of forma-geom):
//! BooleanUnion, BooleanDifference, BooleanIntersection, BooleanSplit, FilletEdge,
//! ChamferEdge, Shell, OffsetSrf. Without the kernel they fail with a clear message.

use crate::edit::parse_ids;
use crate::exact::{editable_mesh, is_planar_brep, result_geometry};
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{
    exact_boolean, exact_fillet, exact_offset, exact_shell, fillet_edges, offset_solid,
    shell_solid, solid_boolean, BooleanOp, KernelError, Mesh, NeutralBrep, Point3, SolidOptions,
    Tolerance, KERNEL_AVAILABLE,
};

fn kernel_check(cmd: &str) -> Result<(), CommandError> {
    if KERNEL_AVAILABLE {
        Ok(())
    } else {
        Err(CommandError::Invalid(format!(
            "{cmd} {}",
            KernelError::Unavailable
        )))
    }
}

fn kernel_err(cmd: &str, e: KernelError) -> CommandError {
    CommandError::Invalid(format!("{cmd}: {e}"))
}

fn options(ctx: &Context) -> SolidOptions {
    SolidOptions::new(Tolerance {
        absolute: ctx.doc.absolute_tolerance.max(1e-7),
        ..ctx.tolerance
    })
}

/// Leading `#id` tokens.
fn leading_ids(args: &mut Args) -> Result<Vec<ObjectId>, CommandError> {
    let mut toks = Vec::new();
    while args.peek().is_some_and(|t| t.starts_with('#')) {
        toks.push(args.next_token().expect("peeked"));
    }
    parse_ids(toks)
}

/// Triangles of a solid: a mesh, or the welded display mesh of a polysurface.
fn mesh_of(ctx: &Context, cmd: &str, id: ObjectId) -> Result<Mesh, CommandError> {
    let tol = ctx.doc.absolute_tolerance.max(1e-9);
    match ctx.doc.object(id).map(|o| &o.geometry) {
        Some(g) if g.is_surface() => Ok(editable_mesh(g, tol).expect("surface")),
        Some(_) => Err(CommandError::Invalid(format!(
            "{cmd}: #{} is not a solid",
            id.0
        ))),
        None => Err(CommandError::Invalid(format!("{cmd}: no object #{}", id.0))),
    }
}

fn no_leftovers(args: &Args) -> Result<(), CommandError> {
    match args.remaining() {
        Some(extra) => Err(CommandError::BadInput(format!("unused input: {extra}"))),
        None => Ok(()),
    }
}

/// True when every object is a flat-faced polysurface (results of flat-faced
/// operations on them are rebuilt as exact polysurfaces).
fn all_planar(ctx: &Context, ids: &[ObjectId]) -> bool {
    ids.iter().all(|id| {
        ctx.doc
            .object(*id)
            .is_some_and(|o| is_planar_brep(&o.geometry))
    })
}

/// Replace `base` by the first result, add the others like it, delete `remove`, and
/// select the results. With `exact`, flat-faced results become polysurfaces.
fn apply(
    ctx: &mut Context,
    base: ObjectId,
    results: Vec<Mesh>,
    remove: &[ObjectId],
    exact: bool,
) -> Vec<ObjectId> {
    let tol = ctx.doc.absolute_tolerance.max(1e-9);
    let like = ctx.doc.object(base).expect("base exists").clone();
    let mut t = ctx.doc.begin();
    let mut ids = Vec::new();
    for (i, m) in results.into_iter().enumerate() {
        let g = result_geometry(m, exact, tol);
        if i == 0 {
            t.replace(base, g);
            ids.push(base);
        } else {
            ids.push(t.add_like(g, &like));
        }
    }
    for id in remove {
        t.remove(*id);
    }
    t.commit();
    ctx.selection = ids.iter().copied().collect();
    ids
}

// ------------------------------------------------------------ exact path (S3)
//
// When every input is an exact polysurface, the kernel works on the exact NURBS
// faces (no triangles in between) and the results come back as exact
// polysurfaces: a cylinder cut from a box keeps a round hole, a fillet is a real
// rolling-ball surface. If any step fails (a face without a usable NURBS form, a
// result openNURBS will not accept), the command falls back to the mesh path.

/// Neutral streams of the objects, when all are exact polysurfaces / surfaces.
fn exact_inputs(ctx: &Context, ids: &[ObjectId]) -> Option<Vec<NeutralBrep>> {
    ids.iter()
        .map(|id| match &ctx.doc.object(*id)?.geometry {
            Geometry::Brep(b) => forma_io_3dm::exact::to_neutral(b),
            _ => None,
        })
        .collect()
}

/// Kernel results as exact polysurfaces, or `None` when one does not convert.
fn exact_results(ctx: &Context, results: &[NeutralBrep]) -> Option<Vec<Geometry>> {
    let tol = ctx.doc.absolute_tolerance.max(1e-9);
    results
        .iter()
        .map(|s| forma_io_3dm::exact::from_neutral(s, tol).map(Geometry::Brep))
        .collect()
}

/// Like [`apply`], with ready geometry.
fn apply_geometry(
    ctx: &mut Context,
    base: ObjectId,
    results: Vec<Geometry>,
    remove: &[ObjectId],
) -> Vec<ObjectId> {
    let like = ctx.doc.object(base).expect("base exists").clone();
    let mut t = ctx.doc.begin();
    let mut ids = Vec::new();
    for (i, g) in results.into_iter().enumerate() {
        if i == 0 {
            t.replace(base, g);
            ids.push(base);
        } else {
            ids.push(t.add_like(g, &like));
        }
    }
    for id in remove {
        t.remove(*id);
    }
    t.commit();
    ctx.selection = ids.iter().copied().collect();
    ids
}

/// How far an edge / face pick may be from the solid (a tenth of its size).
fn pick_distance(ctx: &Context, id: ObjectId) -> f64 {
    ctx.doc.object(id).map_or(1.0, |o| {
        let b = o.geometry.bounding_box();
        b.min.distance_to(b.max) * 0.1
    })
}

fn tolerance(ctx: &Context) -> f64 {
    options(ctx).tolerance
}

fn id_list(ids: &[ObjectId]) -> String {
    ids.iter()
        .map(|i| format!("#{}", i.0))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Base and the other inputs of a two-group boolean: explicit `#base #other…`, or
/// `#base` with the rest of the selection, or the whole selection (lowest id first).
fn base_and_rest(
    ctx: &Context,
    cmd: &str,
    args: &mut Args,
) -> Result<(ObjectId, Vec<ObjectId>), CommandError> {
    let ids = leading_ids(args)?;
    let (base, rest) = match ids.as_slice() {
        [] => {
            let sel = ctx.selected(cmd)?;
            (sel[0], sel[1..].to_vec())
        }
        [b] => (
            *b,
            ctx.selection.iter().copied().filter(|i| i != b).collect(),
        ),
        [b, rest @ ..] => (*b, rest.to_vec()),
    };
    if rest.is_empty() {
        return Err(CommandError::Invalid(format!(
            "{cmd}: needs a base and at least one more solid"
        )));
    }
    if rest.contains(&base) {
        return Err(CommandError::Invalid(format!(
            "{cmd}: #{} is used twice",
            base.0
        )));
    }
    Ok((base, rest))
}

fn two_group(ctx: &mut Context, args: &mut Args, cmd: &str, op: BooleanOp) -> CommandResult {
    kernel_check(cmd)?;
    let (base, rest) = base_and_rest(ctx, cmd, args)?;
    no_leftovers(args)?;
    // BooleanSplit keeps the cutters (Rhino's default); the others consume them.
    let remove: &[ObjectId] = if op == BooleanOp::Split { &[] } else { &rest };
    if let (Some(a), Some(b)) = (exact_inputs(ctx, &[base]), exact_inputs(ctx, &rest)) {
        let a: Vec<&[f64]> = a.iter().map(Vec::as_slice).collect();
        let b: Vec<&[f64]> = b.iter().map(Vec::as_slice).collect();
        if let Some(geo) = exact_boolean(op, &a, &b, tolerance(ctx))
            .ok()
            .and_then(|r| exact_results(ctx, &r))
        {
            let n = geo.len();
            let ids = apply_geometry(ctx, base, geo, remove);
            return Ok(format!("{cmd}: {n} solid(s) {}", id_list(&ids)));
        }
    }
    let a = mesh_of(ctx, cmd, base)?;
    let b = rest
        .iter()
        .map(|id| mesh_of(ctx, cmd, *id))
        .collect::<Result<Vec<_>, _>>()?;
    let b: Vec<&Mesh> = b.iter().collect();
    let results = solid_boolean(op, &[&a], &b, options(ctx)).map_err(|e| kernel_err(cmd, e))?;
    let n = results.len();
    let exact = all_planar(ctx, &rest) && all_planar(ctx, &[base]);
    let ids = apply(ctx, base, results, remove, exact);
    Ok(format!("{cmd}: {n} solid(s) {}", id_list(&ids)))
}

simple_command!(
    BooleanUnion,
    "BooleanUnion",
    &["Union"],
    "BooleanUnion [#id …] — merge the selected (or listed) solids into one"
);
impl Command for BooleanUnion {
    impl_meta!(BooleanUnion);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let cmd = Self::NAME;
        kernel_check(cmd)?;
        let mut ids = leading_ids(args)?;
        no_leftovers(args)?;
        if ids.is_empty() {
            ids = ctx.selected(cmd)?;
        }
        ids.dedup();
        if ids.len() < 2 {
            return Err(CommandError::Invalid(format!(
                "{cmd}: needs at least two solids"
            )));
        }
        if let Some(all) = exact_inputs(ctx, &ids) {
            let all: Vec<&[f64]> = all.iter().map(Vec::as_slice).collect();
            if let Some(geo) = exact_boolean(BooleanOp::Union, &all, &[], tolerance(ctx))
                .ok()
                .and_then(|r| exact_results(ctx, &r))
            {
                let n = geo.len();
                let out = apply_geometry(ctx, ids[0], geo, &ids[1..]);
                return Ok(format!("{cmd}: {n} solid(s) {}", id_list(&out)));
            }
        }
        let meshes = ids
            .iter()
            .map(|id| mesh_of(ctx, cmd, *id))
            .collect::<Result<Vec<_>, _>>()?;
        let meshes: Vec<&Mesh> = meshes.iter().collect();
        let results = solid_boolean(BooleanOp::Union, &meshes, &[], options(ctx))
            .map_err(|e| kernel_err(cmd, e))?;
        let n = results.len();
        let exact = all_planar(ctx, &ids);
        let ids_out = apply(ctx, ids[0], results, &ids[1..], exact);
        Ok(format!("{cmd}: {n} solid(s) {}", id_list(&ids_out)))
    }
}

simple_command!(
    BooleanDifference,
    "BooleanDifference",
    &["Difference"],
    "BooleanDifference [#base #cutter …] — subtract the cutters from the base (selection: lowest id is the base)"
);
impl Command for BooleanDifference {
    impl_meta!(BooleanDifference);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        two_group(ctx, args, Self::NAME, BooleanOp::Difference)
    }
}

simple_command!(
    BooleanIntersection,
    "BooleanIntersection",
    &["Intersection"],
    "BooleanIntersection [#first #other …] — keep what the first solid shares with the others"
);
impl Command for BooleanIntersection {
    impl_meta!(BooleanIntersection);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        two_group(ctx, args, Self::NAME, BooleanOp::Intersection)
    }
}

simple_command!(
    BooleanSplit,
    "BooleanSplit",
    &[],
    "BooleanSplit [#base #cutter …] — cut the base into the pieces inside and outside the cutters (cutters kept)"
);
impl Command for BooleanSplit {
    impl_meta!(BooleanSplit);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        two_group(ctx, args, Self::NAME, BooleanOp::Split)
    }
}

/// The solid an edge/face pick refers to: a leading `#id`, else the single selected
/// mesh, else the selected mesh nearest to `near`.
fn target_solid(
    ctx: &Context,
    cmd: &str,
    explicit: Option<ObjectId>,
    near: Point3,
) -> Result<ObjectId, CommandError> {
    if let Some(id) = explicit {
        mesh_of(ctx, cmd, id)?;
        return Ok(id);
    }
    let sel = ctx.selected(cmd)?;
    let best = sel
        .iter()
        .filter_map(|id| {
            ctx.doc
                .object(*id)?
                .geometry
                .as_mesh()?
                .closest_triangle(near)
                .map(|(_, _, d)| (*id, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    best.map(|(id, _)| id)
        .ok_or_else(|| CommandError::Invalid(format!("{cmd}: select a solid first")))
}

/// `[#id] <number> <point> <point>…`
fn number_and_points(
    ctx: &Context,
    args: &mut Args,
    what: &'static str,
) -> Result<(Option<ObjectId>, f64, Vec<Point3>), CommandError> {
    let ids = leading_ids(args)?;
    if ids.len() > 1 {
        return Err(CommandError::BadInput("one object at a time".into()));
    }
    let n = args.number(what)?;
    let mut pts = vec![args.point("point", ctx.last_point)?];
    while args.peek().is_some() {
        let last = pts.last().copied();
        pts.push(args.point("point", last)?);
    }
    Ok((ids.first().copied(), n, pts))
}

fn edge_command(ctx: &mut Context, args: &mut Args, cmd: &str, chamfer: bool) -> CommandResult {
    kernel_check(cmd)?;
    let (id, r, pts) = number_and_points(ctx, args, if chamfer { "distance" } else { "radius" })?;
    let id = target_solid(ctx, cmd, id, pts[0])?;
    if let Some(s) = exact_inputs(ctx, &[id]) {
        let pick = pick_distance(ctx, id);
        match exact_fillet(&s[0], r, &pts, chamfer, tolerance(ctx), pick) {
            Ok(res) => {
                if let Some(geo) = exact_results(ctx, &res) {
                    apply_geometry(ctx, id, geo, &[]);
                    return Ok(format!("{cmd}: {} edge pick(s) on #{}", pts.len(), id.0));
                }
            }
            // A pick nowhere near an edge is the user's to fix, not a reason to mesh.
            Err(KernelError::Failed(e)) if e.contains("no edge near") => {
                return Err(kernel_err(cmd, KernelError::Failed(e)));
            }
            Err(_) => {}
        }
    }
    let m = mesh_of(ctx, cmd, id)?;
    let out = fillet_edges(&m, r, &pts, chamfer, options(ctx)).map_err(|e| kernel_err(cmd, e))?;
    // A chamfer keeps flat faces flat; a fillet is curved (a mesh until the
    // kernel works on exact breps).
    let exact = chamfer && all_planar(ctx, &[id]);
    apply(ctx, id, vec![out], &[], exact);
    Ok(format!("{cmd}: {} edge pick(s) on #{}", pts.len(), id.0))
}

simple_command!(
    FilletEdge,
    "FilletEdge",
    &["FE"],
    "FilletEdge [#id] <radius> <point near edge> … — round the edges nearest to the points (selected solid)"
);
impl Command for FilletEdge {
    impl_meta!(FilletEdge);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        edge_command(ctx, args, Self::NAME, false)
    }
}

simple_command!(
    ChamferEdge,
    "ChamferEdge",
    &[],
    "ChamferEdge [#id] <distance> <point near edge> … — bevel the edges nearest to the points (selected solid)"
);
impl Command for ChamferEdge {
    impl_meta!(ChamferEdge);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        edge_command(ctx, args, Self::NAME, true)
    }
}

simple_command!(
    Shell,
    "Shell",
    &[],
    "Shell [#id] <thickness> <point on face to remove> … — hollow the selected solid, walls inside"
);
impl Command for Shell {
    impl_meta!(Shell);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let cmd = Self::NAME;
        kernel_check(cmd)?;
        let (id, t, pts) = number_and_points(ctx, args, "thickness")?;
        let id = target_solid(ctx, cmd, id, pts[0])?;
        if let Some(s) = exact_inputs(ctx, &[id]) {
            let pick = pick_distance(ctx, id);
            if let Some(geo) = exact_shell(&s[0], t, &pts, tolerance(ctx), pick)
                .ok()
                .and_then(|r| exact_results(ctx, &r))
            {
                apply_geometry(ctx, id, geo, &[]);
                return Ok(format!("{cmd}: #{} hollowed, wall {t}", id.0));
            }
        }
        let m = mesh_of(ctx, cmd, id)?;
        let out = shell_solid(&m, t, &pts, options(ctx)).map_err(|e| kernel_err(cmd, e))?;
        let exact = all_planar(ctx, &[id]);
        apply(ctx, id, vec![out], &[], exact);
        Ok(format!("{cmd}: #{} hollowed, wall {t}", id.0))
    }
}

simple_command!(
    OffsetSrf,
    "OffsetSrf",
    &[],
    "OffsetSrf <distance> — offset all faces of the selected closed solids (positive = outwards)"
);
impl Command for OffsetSrf {
    impl_meta!(OffsetSrf);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let cmd = Self::NAME;
        kernel_check(cmd)?;
        let d = args.number("distance")?;
        no_leftovers(args)?;
        let ids = ctx.selected(cmd)?;
        let opts = options(ctx);
        let mut out = Vec::new();
        let mut exact_out: Vec<(ObjectId, Geometry)> = Vec::new();
        for id in &ids {
            if let Some(s) = exact_inputs(ctx, &[*id]) {
                if let Some(mut geo) = exact_offset(&s[0], d, tolerance(ctx))
                    .ok()
                    .and_then(|r| exact_results(ctx, &r))
                    .filter(|g| g.len() == 1)
                {
                    exact_out.push((*id, geo.remove(0)));
                    continue;
                }
            }
            let m = mesh_of(ctx, cmd, *id)?;
            out.push((
                *id,
                offset_solid(&m, d, opts).map_err(|e| kernel_err(cmd, e))?,
                all_planar(ctx, &[*id]),
            ));
        }
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut t = ctx.doc.begin();
        for (id, g) in exact_out {
            t.replace(id, g);
        }
        for (id, m, exact) in out {
            // Offsetting a box outwards rounds its edges: only inward (or zero)
            // offsets of flat-faced solids stay flat.
            t.replace(id, result_geometry(m, exact && d <= 0.0, tol));
        }
        t.commit();
        Ok(format!("{cmd}: offset {} solid(s) by {d}", ids.len()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::ObjectId;
    use forma_geom::KERNEL_AVAILABLE;

    fn volume(e: &Engine, id: u64) -> f64 {
        match e.doc().object(ObjectId(id)).unwrap().geometry.as_mesh() {
            Some(m) => m.volume(),
            None => panic!("not a solid"),
        }
    }

    fn rel(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs()
    }

    #[test]
    fn without_the_kernel_the_commands_explain_why() {
        if KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Box 5,5 15,15 10").unwrap();
        e.run_line("SelAll").unwrap();
        for line in [
            "BooleanUnion",
            "BooleanDifference",
            "BooleanIntersection",
            "BooleanSplit",
            "FilletEdge 1 5,0,10",
            "ChamferEdge 1 5,0,10",
            "Shell 1 5,5,10",
            "OffsetSrf 1",
        ] {
            let err = e.run_line(line).unwrap_err().to_string();
            assert!(err.contains("needs the solid kernel"), "{line}: {err}");
            assert!(err.contains("--features occt"), "{line}: {err}");
        }
        assert_eq!(e.doc().len(), 2);
    }

    #[test]
    fn boolean_union_of_selection() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Box 5,5 15,15 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("BooleanUnion").unwrap();
        assert_eq!(e.doc().len(), 1);
        assert!(rel(volume(&e, 1), 2000.0 - 250.0) < 1e-9);
        e.run_line("Undo").unwrap();
        assert_eq!(e.doc().len(), 2);
    }

    #[test]
    fn boolean_difference_with_ids_and_selection() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Box 5,5,5 15,15 10").unwrap();
        e.run_line("BooleanDifference #1 #2").unwrap();
        assert_eq!(e.doc().len(), 1);
        assert!(rel(volume(&e, 1), 1000.0 - 125.0) < 1e-9);
        e.run_line("Undo").unwrap();
        // Selection form: lowest id is the base.
        e.run_line("SelAll").unwrap();
        e.run_line("BooleanDifference").unwrap();
        assert!(rel(volume(&e, 1), 875.0) < 1e-9);
        assert!(e.run_line("BooleanDifference #1").is_err()); // nothing to subtract
    }

    #[test]
    fn boolean_intersection_and_split() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Box 5,5,5 15,15 10").unwrap();
        e.run_line("BooleanIntersection #1 #2").unwrap();
        assert_eq!(e.doc().len(), 1);
        assert!(rel(volume(&e, 1), 125.0) < 1e-9);
        e.run_line("Undo").unwrap();
        e.run_line("BooleanSplit #1 #2").unwrap();
        assert_eq!(e.doc().len(), 3); // two pieces + the kept cutter
        let total: f64 = [1, 3].iter().map(|i| volume(&e, *i)).sum();
        assert!(rel(total, 1000.0) < 1e-9);
    }

    #[test]
    fn fillet_and_chamfer_edges() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 40,20 20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("FilletEdge 3 20,0,20").unwrap();
        let removed = 16000.0 - volume(&e, 1);
        let exact = (1.0 - std::f64::consts::PI / 4.0) * 9.0 * 40.0;
        assert!(rel(removed, exact) < 0.02, "{removed} vs {exact}"); // display mesh
        e.run_line("Undo").unwrap();
        e.run_line("ChamferEdge #1 2 20,0,20 20,20,20").unwrap();
        assert!(rel(16000.0 - volume(&e, 1), 2.0 * 40.0 * 2.0) < 1e-6);
        assert!(e.run_line("FilletEdge 1 500,500,500").is_err());
    }

    #[test]
    fn shell_and_offset() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 40,30 20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Shell 2 20,15,20").unwrap();
        assert!(rel(volume(&e, 1), 24000.0 - 36.0 * 26.0 * 18.0) < 1e-6);
        e.run_line("Undo").unwrap();
        e.run_line("OffsetSrf 1").unwrap();
        assert!(rel(volume(&e, 1), 42.0 * 32.0 * 22.0) < 1e-6);
    }

    fn brep(e: &Engine, id: u64) -> forma_geom::Brep {
        match &e.doc().object(ObjectId(id)).unwrap().geometry {
            forma_doc::Geometry::Brep(b) => b.clone(),
            g => panic!("#{id} is a {}, not an exact polysurface", g.kind()),
        }
    }

    #[test]
    fn curved_booleans_and_fillets_stay_exact() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let pi = std::f64::consts::PI;
        let mut e = Engine::new();
        // A round hole through a box: the hole wall is an exact cylinder.
        e.run_line("Box 0,0,0 10,10,0 10").unwrap();
        e.run_line("Cylinder 5,5,-1 3 12").unwrap();
        e.run_line("BooleanDifference #1 #2").unwrap();
        let b = brep(&e, 1);
        assert_eq!(b.faces, 7);
        assert!(b.solid && !b.planar);
        assert!(rel(b.mesh.volume(), 1000.0 - pi * 90.0) < 0.005);
        e.run_line("Undo").unwrap();
        assert_eq!(e.doc().len(), 2);

        // A round window through a wall.
        let mut e = Engine::new();
        e.run_line("Box 0,0,0 300,20,0 270").unwrap();
        e.run_line("Cylinder 150,-5,135 40 30 0,1,0").unwrap();
        e.run_line("BooleanDifference #1 #2").unwrap();
        let v = brep(&e, 1).mesh.volume();
        assert!(
            rel(v, 300.0 * 20.0 * 270.0 - pi * 1600.0 * 20.0) < 0.001,
            "{v}"
        );

        // Two cylinders crossing, and a box minus a sphere.
        let mut e = Engine::new();
        e.run_line("Cylinder 0,0,0 5 20").unwrap();
        e.run_line("Cylinder 0,0,10 5 20 1,0,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("BooleanUnion").unwrap();
        assert!(brep(&e, 1).solid);
        let mut e = Engine::new();
        e.run_line("Box 0,0,0 20,20,0 20").unwrap();
        e.run_line("Sphere 10,10,20 6").unwrap();
        e.run_line("BooleanDifference #1 #2").unwrap();
        let v = brep(&e, 1).mesh.volume();
        assert!(rel(v, 8000.0 - 2.0 / 3.0 * pi * 216.0) < 0.01, "{v}");

        // A rolling-ball fillet on a box edge and on a cylinder rim.
        let mut e = Engine::new();
        e.run_line("Box 0,0 40,20 20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("FilletEdge 3 20,0,20").unwrap();
        let b = brep(&e, 1);
        assert_eq!(b.faces, 7);
        let removed = 16000.0 - b.mesh.volume();
        assert!(
            rel(removed, (1.0 - pi / 4.0) * 9.0 * 40.0) < 0.02,
            "{removed}"
        );
        let mut e = Engine::new();
        e.run_line("Cylinder 0,0,0 5 20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("FilletEdge 1 0,5,20").unwrap();
        assert_eq!(brep(&e, 1).faces, 4);
    }

    #[test]
    fn shells_and_offsets_of_curved_solids_stay_exact() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let pi = std::f64::consts::PI;
        // A cup: cylinder hollowed with 1 mm walls, top removed.
        let mut e = Engine::new();
        e.run_line("Cylinder 0,0,0 5 20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Shell 1 0,0,20").unwrap();
        let b = brep(&e, 1);
        assert_eq!(b.faces, 5);
        let cup = pi * 25.0 * 20.0 - pi * 16.0 * 19.0;
        assert!(rel(b.mesh.volume(), cup) < 0.01, "{}", b.mesh.volume());
        // Offsets: a cylinder and a sphere grow by 1.
        let mut e = Engine::new();
        e.run_line("Cylinder 0,0,0 5 20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("OffsetSrf 1").unwrap();
        let v = brep(&e, 1).mesh.volume();
        assert!(rel(v, pi * 36.0 * 22.0) < 0.01, "{v}");
        let mut e = Engine::new();
        e.run_line("Sphere 0,0,0 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("OffsetSrf 1").unwrap();
        let v = brep(&e, 1).mesh.volume();
        assert!(rel(v, 4.0 / 3.0 * pi * 1331.0) < 0.02, "{v}");
    }

    #[test]
    fn exact_results_survive_save_and_open() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let path = std::env::temp_dir().join(format!("forma-s3-{}.3dm", std::process::id()));
        let mut e = Engine::new();
        e.run_line("Box 0,0,0 10,10,0 10").unwrap();
        e.run_line("Cylinder 5,5,-1 3 12").unwrap();
        e.run_line("BooleanDifference #1 #2").unwrap();
        e.run_line(&format!("Save {}", path.display())).unwrap();
        let mut back = Engine::new();
        back.run_line(&format!("Open {}", path.display())).unwrap();
        let o = back.doc().objects().next().unwrap();
        let forma_doc::Geometry::Brep(b) = &o.geometry else {
            panic!("reopened as {}", o.geometry.kind())
        };
        assert_eq!(b.faces, 7);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn niche_in_a_wall_from_the_command_line() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut e = Engine::new();
        e.run_line("Box 0,0 300,20 270").unwrap();
        e.run_line("Box 110,0,90 190,10 120").unwrap();
        e.run_line("BooleanDifference #1 #2").unwrap();
        assert!(rel(volume(&e, 1), 300.0 * 20.0 * 270.0 - 80.0 * 10.0 * 120.0) < 1e-9);
    }
}
