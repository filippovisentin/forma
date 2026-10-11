//! Keeping geometry exact through mesh-based tools.
//!
//! Exact surfaces and solids ([`Geometry::Brep`]) are shown and picked through
//! their display mesh. Tools that still work on triangles (push / pull, the
//! kernel booleans, mesh edits) take a welded copy of that mesh; when every
//! input was a flat-faced polysurface, the result is turned back into an exact
//! polysurface (trimmed planes joined at their edges), so boxes and walls stay
//! polysurfaces in Rhino after editing.

use forma_doc::Geometry;
use forma_geom::{box_mesh, Mesh, Plane};

/// Most faces a rebuilt polysurface may have (above this the mesh is a
/// tessellated curved shape, not a polyhedron).
const MAX_POLY_FACES: usize = 2000;

/// Triangles to edit: a mesh as is, the display mesh of an exact surface welded
/// (so faces share vertices). `None` for curves and annotations.
pub(crate) fn editable_mesh(g: &Geometry, tol: f64) -> Option<Mesh> {
    match g {
        Geometry::Mesh(m) => Some(m.clone()),
        Geometry::Brep(b) => Some(b.mesh.welded(tol)),
        _ => None,
    }
}

/// True for an exact polysurface whose faces are all flat.
pub(crate) fn is_planar_brep(g: &Geometry) -> bool {
    matches!(g, Geometry::Brep(b) if b.planar)
}

/// The result of a mesh tool: an exact polysurface when `exact` (the inputs
/// were flat-faced polysurfaces) and the mesh is a clean polyhedron, else the
/// mesh.
pub(crate) fn result_geometry(m: Mesh, exact: bool, tol: f64) -> Geometry {
    if exact {
        if let Some(b) = forma_io_3dm::exact::polyhedron_brep(&m, tol, MAX_POLY_FACES) {
            return Geometry::Brep(b);
        }
    }
    Geometry::Mesh(m)
}

/// Exact box on `plane` (u × v × h, negative extents go the other way); the
/// mesh box if openNURBS cannot make it.
pub(crate) fn box_geometry(plane: &Plane, u: f64, v: f64, h: f64) -> Geometry {
    let o = plane.point_at(u.min(0.0), v.min(0.0), h.min(0.0));
    let (x, y, z) = (plane.x * u.abs(), plane.y * v.abs(), plane.z * h.abs());
    let corners = [
        o,
        o + x,
        o + x + y,
        o + y,
        o + z,
        o + x + z,
        o + x + y + z,
        o + y + z,
    ];
    forma_io_3dm::exact::make_box(&corners)
        .map_or_else(|| Geometry::Mesh(box_mesh(plane, u, v, h)), Geometry::Brep)
}

use crate::{Args, Command, CommandError, CommandResult, Context};

simple_command!(
    MeshCmd,
    "Mesh",
    &["MeshFromSrf"],
    "Mesh — add a mesh copy of the selected surfaces and polysurfaces (the originals stay)"
);
impl Command for MeshCmd {
    impl_meta!(MeshCmd);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Mesh")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut t = ctx.doc.begin();
        let mut made = Vec::new();
        for id in ids {
            let o = t.doc().object(id).expect("selected").clone();
            if let Geometry::Brep(b) = &o.geometry {
                // Faces share their vertices, so the mesh is closed for solids.
                made.push(t.add_like(Geometry::Mesh(b.mesh.welded(tol)), &o));
            }
        }
        if made.is_empty() {
            return Err(CommandError::Invalid(
                "Mesh: select surfaces or polysurfaces".into(),
            ));
        }
        t.commit();
        let n = made.len();
        ctx.selection = made.into_iter().collect();
        Ok(format!("{n} mesh(es) added"))
    }
}

simple_command!(
    MeshToNURB,
    "MeshToNURB",
    &["MeshToPolysrf"],
    "MeshToNURB — replace flat-faced meshes by exact polysurfaces (coplanar triangles become one face)"
);
impl Command for MeshToNURB {
    impl_meta!(MeshToNURB);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("MeshToNURB")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        let mut skipped = 0;
        for id in ids {
            let Geometry::Mesh(m) = &ctx.doc.object(id).expect("selected").geometry else {
                continue;
            };
            match forma_io_3dm::exact::polyhedron_brep(m, tol, MAX_POLY_FACES) {
                Some(b) => out.push((id, b)),
                None => skipped += 1,
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "MeshToNURB: select clean meshes with flat faces (curved meshes stay meshes)"
                    .into(),
            ));
        }
        let n = out.len();
        let mut t = ctx.doc.begin();
        for (id, b) in out {
            t.replace(id, Geometry::Brep(b));
        }
        t.commit();
        Ok(if skipped > 0 {
            format!("{n} polysurface(s), {skipped} mesh(es) left as they are")
        } else {
            format!("{n} polysurface(s)")
        })
    }
}

simple_command!(
    MergeAllCoplanarFaces,
    "MergeAllCoplanarFaces",
    &["MergeAllFaces", "MergeCoplanarFaces"],
    "MergeAllCoplanarFaces — join the neighbouring coplanar faces of the selected flat-faced polysurfaces"
);
impl Command for MergeAllCoplanarFaces {
    impl_meta!(MergeAllCoplanarFaces);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("MergeAllCoplanarFaces")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut out = Vec::new();
        for id in ids {
            let Geometry::Brep(b) = &ctx.doc.object(id).expect("selected").geometry else {
                continue;
            };
            if !b.planar {
                continue;
            }
            let before = b.faces;
            if let Some(m) = forma_io_3dm::exact::polyhedron_brep(&b.mesh, tol, MAX_POLY_FACES) {
                if m.faces < before {
                    out.push((id, m, before));
                }
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "MergeAllCoplanarFaces: no coplanar faces to merge (flat-faced polysurfaces only)"
                    .into(),
            ));
        }
        let mut merged = 0;
        let mut t = ctx.doc.begin();
        for (id, b, before) in out {
            merged += before - b.faces;
            t.replace(id, Geometry::Brep(b));
        }
        t.commit();
        Ok(format!("{merged} face(s) merged"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;

    fn last(e: &Engine) -> &Geometry {
        &e.doc().objects().last().unwrap().geometry
    }

    #[test]
    fn mesh_and_back_to_a_polysurface() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,20 30").unwrap();
        assert_eq!(last(&e).kind(), "polysurface");
        e.run_line("SelAll").unwrap();
        e.run_line("Mesh").unwrap();
        let Geometry::Mesh(m) = last(&e) else {
            panic!("{:?}", last(&e))
        };
        assert!(m.is_closed(1e-9));
        assert!((m.volume() - 6000.0).abs() < 1e-6);
        e.run_line("MeshToNURB").unwrap();
        let Geometry::Brep(b) = last(&e) else {
            panic!()
        };
        assert_eq!(b.faces, 6);
        assert!(b.solid && b.planar);
        assert!(e.run_line("Sphere 0,0,0 5").is_ok());
        e.run_line("SelLast").unwrap();
        e.run_line("Mesh").unwrap();
        // A curved mesh stays a mesh.
        assert!(e.run_line("MeshToNURB").is_err());
    }

    #[test]
    fn push_pull_and_join_keep_boxes_exact_and_merge_faces() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        // Exploded into six exact faces, joined back into the exact solid.
        e.run_line("Explode").unwrap();
        assert_eq!(e.doc().len(), 6);
        assert!(e.doc().objects().all(|o| o.geometry.kind() == "surface"));
        e.run_line("SelAll").unwrap();
        e.run_line("Join").unwrap();
        let Geometry::Brep(b) = last(&e) else {
            panic!("{:?}", last(&e))
        };
        assert!(b.solid);
        assert_eq!(b.faces, 6);
        assert!((b.mesh.volume() - 1000.0).abs() < 1e-6);
        assert!(e.run_line("MergeAllCoplanarFaces").is_err());
        // Push the top up: still an exact box.
        e.run_line("PushPull 0,0,1 5").unwrap();
        let Geometry::Brep(b) = last(&e) else {
            panic!("{:?}", last(&e))
        };
        assert!(b.solid && b.planar);
        assert!((b.mesh.volume() - 1500.0).abs() < 1e-6);
    }
}

simple_command!(
    MeshBox,
    "MeshBox",
    &[],
    "MeshBox <corner> <opposite corner> <height> — box as a mesh (Box makes an exact polysurface)"
);
impl Command for MeshBox {
    impl_meta!(MeshBox);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first corner", ctx.last_point)?;
        let b = args.point("opposite corner", Some(a))?;
        let h = crate::create::positive(args.number("height")?, "height", ctx)?;
        let plane = Plane::from_normal(a, forma_geom::Vec3::Z);
        let (u, v, _) = plane.coords(b);
        crate::create::positive(u, "width", ctx)?;
        crate::create::positive(v, "depth", ctx)?;
        let ids =
            crate::create::finish(ctx, vec![Geometry::Mesh(box_mesh(&plane, u, v, h))], false)?;
        Ok(format!("added #{} mesh box", ids[0].0))
    }
}

simple_command!(
    MeshSphere,
    "MeshSphere",
    &[],
    "MeshSphere <center> <radius> — sphere as a mesh (Sphere makes an exact surface)"
);
impl Command for MeshSphere {
    impl_meta!(MeshSphere);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center", ctx.last_point)?;
        let r = crate::create::positive(args.number("radius")?, "radius", ctx)?.abs();
        let ids = crate::create::finish(
            ctx,
            vec![Geometry::Mesh(forma_geom::sphere_mesh(c, r, 48))],
            false,
        )?;
        Ok(format!("added #{} mesh sphere", ids[0].0))
    }
}

/// Selected exact surfaces and polysurfaces.
fn selected_breps(
    ctx: &Context,
    cmd: &str,
) -> Result<Vec<(forma_doc::ObjectId, forma_geom::Brep)>, CommandError> {
    let out: Vec<_> = ctx
        .selected(cmd)?
        .into_iter()
        .filter_map(|id| match &ctx.doc.object(id)?.geometry {
            Geometry::Brep(b) => Some((id, b.clone())),
            _ => None,
        })
        .collect();
    if out.is_empty() {
        return Err(CommandError::Invalid(format!(
            "{cmd}: select surfaces or polysurfaces"
        )));
    }
    Ok(out)
}

simple_command!(
    Untrim,
    "Untrim",
    &["UntrimAll"],
    "Untrim — remove the trims (holes and cut outlines) of the selected surfaces: each face becomes its whole underlying surface"
);
impl Command for Untrim {
    impl_meta!(Untrim);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let breps = selected_breps(ctx, "Untrim")?;
        let mut t = ctx.doc.begin();
        let mut n = 0;
        let mut made = Vec::new();
        for (id, b) in breps {
            let faces: Vec<forma_geom::Brep> = (0..b.faces)
                .filter_map(|i| forma_io_3dm::exact::untrimmed(&b, i))
                .collect();
            if faces.is_empty() {
                continue;
            }
            let like = t.doc().object(id).expect("selected").clone();
            t.remove(id);
            n += faces.len();
            for f in faces {
                made.push(t.add_like(Geometry::Brep(f), &like));
            }
        }
        if n == 0 {
            return Err(CommandError::Invalid("Untrim: nothing to untrim".into()));
        }
        t.commit();
        ctx.selection = made.into_iter().collect();
        Ok(format!("{n} untrimmed surface(s)"))
    }
}

simple_command!(
    ShrinkTrimmedSrf,
    "ShrinkTrimmedSrf",
    &["ShrinkTrimmedSrfToEdge"],
    "ShrinkTrimmedSrf — shrink the underlying surfaces of the selected surfaces close to their trims (lighter files)"
);
impl Command for ShrinkTrimmedSrf {
    impl_meta!(ShrinkTrimmedSrf);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let breps = selected_breps(ctx, "ShrinkTrimmedSrf")?;
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for (id, b) in breps {
            if let Some(s) = forma_io_3dm::exact::shrunk(&b) {
                t.replace(id, Geometry::Brep(s));
                n += 1;
            }
        }
        t.commit();
        Ok(format!("{n} surface(s) shrunk"))
    }
}

simple_command!(
    Check,
    "Check",
    &["CheckObject"],
    "Check — test the selected surfaces and polysurfaces for validity (what Rhino would reject)"
);
impl Command for Check {
    impl_meta!(Check);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let breps = selected_breps(ctx, "Check")?;
        let mut out = Vec::new();
        let mut bad = 0;
        for (id, b) in &breps {
            let (ok, log) = forma_io_3dm::exact::check(b);
            if ok {
                out.push(format!("#{} is valid", id.0));
            } else {
                bad += 1;
                let first = log.lines().take(3).collect::<Vec<_>>().join(" / ");
                out.push(format!("#{} is NOT valid: {first}", id.0));
            }
        }
        out.insert(
            0,
            format!("{} object(s) checked, {bad} with problems", breps.len()),
        );
        Ok(out.join("\n"))
    }
}

simple_command!(
    ExtractIsocurve,
    "ExtractIsocurve",
    &["ExtractIso"],
    "ExtractIsocurve #id <point> [u|v|both] — curves along the surface through the point (only inside the trims)"
);
impl Command for ExtractIsocurve {
    impl_meta!(ExtractIsocurve);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("surface #id"))?;
        let id = crate::edit::parse_ids(vec![tok])?[0];
        let p = args.point("point on the surface", ctx.last_point)?;
        let dir = if args.keyword("u") {
            0
        } else if args.keyword("v") {
            1
        } else {
            let _ = args.keyword("both");
            2
        };
        let Some(Geometry::Brep(b)) = ctx.doc.object(id).map(|o| &o.geometry) else {
            return Err(CommandError::Invalid(format!(
                "#{} is not a surface or polysurface",
                id.0
            )));
        };
        let curves = forma_io_3dm::exact::isocurves(b, p, dir);
        if curves.is_empty() {
            return Err(CommandError::Invalid(
                "ExtractIsocurve: no isocurve there".into(),
            ));
        }
        let n = curves.len();
        let ids = crate::create::finish(ctx, curves, true)?;
        Ok(format!("{n} isocurve(s) #{}", ids[0].0))
    }
}

simple_command!(
    CreateSolid,
    "CreateSolid",
    &[],
    "CreateSolid — join the selected surfaces into closed solids (they must enclose a volume)"
);
impl Command for CreateSolid {
    impl_meta!(CreateSolid);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let breps = selected_breps(ctx, "CreateSolid")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let refs: Vec<&forma_geom::Brep> = breps.iter().map(|(_, b)| b).collect();
        let (pieces, _) = forma_io_3dm::exact::join(&refs, tol)
            .ok_or_else(|| CommandError::Invalid("CreateSolid: could not join".into()))?;
        let solids: Vec<forma_geom::Brep> = pieces.into_iter().filter(|b| b.solid).collect();
        if solids.is_empty() {
            return Err(CommandError::Invalid(
                "CreateSolid: the surfaces do not close a volume (use Cap or add the missing faces)"
                    .into(),
            ));
        }
        let like = ctx.doc.object(breps[0].0).expect("selected").clone();
        let mut t = ctx.doc.begin();
        // Only the surfaces that went into a solid are replaced.
        for (id, _) in &breps {
            t.remove(*id);
        }
        let n = solids.len();
        let ids: Vec<_> = solids
            .into_iter()
            .map(|s| t.add_like(Geometry::Brep(s), &like))
            .collect();
        t.commit();
        ctx.selection = ids.into_iter().collect();
        Ok(format!("{n} solid(s)"))
    }
}

#[cfg(test)]
mod more_tests {
    use crate::Engine;
    use forma_doc::Geometry;

    #[test]
    fn untrim_check_isocurve_create_solid() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("Circle 5,5 2").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("PlanarSrf").unwrap();
        e.run_line("SelLast").unwrap();
        let r = e.run_line("Check").unwrap();
        assert!(r.contains("is valid"), "{r}");
        e.run_line("Untrim").unwrap();
        let Geometry::Brep(b) = &e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!((b.mesh.area() - 100.0).abs() < 1e-6);
        e.run_line("ShrinkTrimmedSrf").unwrap();
        e.run_line("New").unwrap();
        e.run_line("Sphere 0,0,0 10").unwrap();
        e.run_line("ExtractIsocurve #1 0,7,7 both").unwrap();
        assert_eq!(e.doc().len(), 3);
        // A box exploded then rebuilt with CreateSolid.
        e.run_line("New").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Explode").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("CreateSolid").unwrap();
        assert_eq!(e.doc().len(), 1);
        e.run_line("SelAll").unwrap();
        e.run_line("Explode").unwrap();
        e.run_line("SelNone").unwrap();
        let ids: Vec<String> = e
            .doc()
            .objects()
            .take(3)
            .map(|o| format!("#{}", o.id.0))
            .collect();
        e.run_line(&format!("Select {}", ids.join(" "))).unwrap();
        assert!(e.run_line("CreateSolid").is_err());
    }
}
