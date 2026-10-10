//! Solid commands through the OpenCascade kernel (feature `occt` of forma-geom):
//! BooleanUnion, BooleanDifference, BooleanIntersection, BooleanSplit, FilletEdge,
//! ChamferEdge, Shell, OffsetSrf. Without the kernel they fail with a clear message.

use crate::edit::parse_ids;
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{
    fillet_edges, offset_solid, shell_solid, solid_boolean, BooleanOp, KernelError, Mesh, Point3,
    SolidOptions, Tolerance, KERNEL_AVAILABLE,
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

fn mesh_of<'a>(ctx: &'a Context, cmd: &str, id: ObjectId) -> Result<&'a Mesh, CommandError> {
    match ctx.doc.object(id).map(|o| &o.geometry) {
        Some(Geometry::Mesh(m)) => Ok(m),
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

/// Replace `base` by the first result, add the others like it, delete `remove`, and
/// select the results.
fn apply(
    ctx: &mut Context,
    base: ObjectId,
    results: Vec<Mesh>,
    remove: &[ObjectId],
) -> Vec<ObjectId> {
    let like = ctx.doc.object(base).expect("base exists").clone();
    let mut t = ctx.doc.begin();
    let mut ids = Vec::new();
    for (i, m) in results.into_iter().enumerate() {
        if i == 0 {
            t.replace(base, Geometry::Mesh(m));
            ids.push(base);
        } else {
            ids.push(t.add_like(Geometry::Mesh(m), &like));
        }
    }
    for id in remove {
        t.remove(*id);
    }
    t.commit();
    ctx.selection = ids.iter().copied().collect();
    ids
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
    let a = mesh_of(ctx, cmd, base)?;
    let b = rest
        .iter()
        .map(|id| mesh_of(ctx, cmd, *id))
        .collect::<Result<Vec<_>, _>>()?;
    let results = solid_boolean(op, &[a], &b, options(ctx)).map_err(|e| kernel_err(cmd, e))?;
    let n = results.len();
    // BooleanSplit keeps the cutters (Rhino's default); the others consume them.
    let remove: &[ObjectId] = if op == BooleanOp::Split { &[] } else { &rest };
    let ids = apply(ctx, base, results, remove);
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
        let meshes = ids
            .iter()
            .map(|id| mesh_of(ctx, cmd, *id))
            .collect::<Result<Vec<_>, _>>()?;
        let results = solid_boolean(BooleanOp::Union, &meshes, &[], options(ctx))
            .map_err(|e| kernel_err(cmd, e))?;
        let n = results.len();
        let ids_out = apply(ctx, ids[0], results, &ids[1..]);
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
        .filter_map(|id| match &ctx.doc.object(*id)?.geometry {
            Geometry::Mesh(m) => m.closest_triangle(near).map(|(_, _, d)| (*id, d)),
            _ => None,
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
    let m = mesh_of(ctx, cmd, id)?;
    let out = fillet_edges(m, r, &pts, chamfer, options(ctx)).map_err(|e| kernel_err(cmd, e))?;
    apply(ctx, id, vec![out], &[]);
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
        let m = mesh_of(ctx, cmd, id)?;
        let out = shell_solid(m, t, &pts, options(ctx)).map_err(|e| kernel_err(cmd, e))?;
        apply(ctx, id, vec![out], &[]);
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
        for id in &ids {
            let m = mesh_of(ctx, cmd, *id)?;
            out.push((
                *id,
                offset_solid(m, d, opts).map_err(|e| kernel_err(cmd, e))?,
            ));
        }
        let mut t = ctx.doc.begin();
        for (id, m) in out {
            t.replace(id, Geometry::Mesh(m));
        }
        t.commit();
        Ok(format!("{cmd}: offset {} solid(s) by {d}", ids.len()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::{Geometry, ObjectId};
    use forma_geom::KERNEL_AVAILABLE;

    fn volume(e: &Engine, id: u64) -> f64 {
        match &e.doc().object(ObjectId(id)).unwrap().geometry {
            Geometry::Mesh(m) => m.volume(),
            g => panic!("{g:?}"),
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
