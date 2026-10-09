//! Push / pull the planar faces of solids (meshes until the solid kernel lands).

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};

fn face_tol(ctx: &Context) -> f64 {
    ctx.doc.absolute_tolerance.max(1e-6)
}

simple_command!(
    MoveFace,
    "MoveFace",
    &["PushFace"],
    "MoveFace #id <point on face> <distance> — move a flat face along its normal; the faces around it stretch"
);
impl Command for MoveFace {
    impl_meta!(MoveFace);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("object id"))?;
        let id = tok
            .trim_start_matches('#')
            .parse()
            .map(ObjectId)
            .map_err(|_| CommandError::BadInput(tok.to_string()))?;
        let p = args.point("point on the face", None)?;
        let d = args.number("distance")?;
        let tol = face_tol(ctx);
        let Some(Geometry::Mesh(m)) = ctx.doc.object(id).map(|o| &o.geometry) else {
            return Err(CommandError::Invalid(format!(
                "#{} is not a solid or surface",
                id.0
            )));
        };
        let (t, _, dist) = m
            .closest_triangle(p)
            .ok_or_else(|| CommandError::Invalid("empty mesh".into()))?;
        let size = m
            .bounding_box()
            .map_or(1.0, |b| b.min.distance_to(b.max))
            .max(tol);
        if dist > size * 0.05 + tol {
            return Err(CommandError::Invalid("the point is not on a face".into()));
        }
        let face = m
            .planar_face(t, tol)
            .ok_or_else(|| CommandError::Invalid("degenerate face".into()))?;
        let moved = m.move_face(&face, face.normal * d, tol);
        let mut t = ctx.doc.begin();
        t.replace(id, Geometry::Mesh(moved));
        t.commit();
        Ok(format!("moved face by {d}"))
    }
}

simple_command!(
    PushPull,
    "PushPull",
    &[],
    "PushPull <direction> <distance> — move the face of the selected solids that faces <direction> (gumball extrude dot)"
);
impl Command for PushPull {
    impl_meta!(PushPull);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let dir = args.vector("direction")?;
        let d = args.number("distance")?;
        let dir = dir
            .normalized()
            .ok_or_else(|| CommandError::Invalid("direction is zero".into()))?;
        let ids = ctx.selected("PushPull")?;
        let tol = face_tol(ctx);
        let mut out = Vec::new();
        for id in ids {
            let Some(Geometry::Mesh(m)) = ctx.doc.object(id).map(|o| &o.geometry) else {
                continue;
            };
            if let Some(face) = m.extreme_face(dir, tol) {
                out.push((id, m.move_face(&face, dir * d, tol)));
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "no flat face of the selection points that way".into(),
            ));
        }
        let n = out.len();
        let mut t = ctx.doc.begin();
        for (id, m) in out {
            t.replace(id, Geometry::Mesh(m));
        }
        t.commit();
        Ok(format!("moved {n} face(s) by {d}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;

    fn bbox(e: &Engine) -> forma_geom::BoundingBox {
        e.doc().objects().next().unwrap().geometry.bounding_box()
    }

    #[test]
    fn push_pull_box_faces() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 100,50 30").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("PushPull 0,0,1 20").unwrap();
        assert!((bbox(&e).max.z - 50.0).abs() < 1e-9);
        e.run_line("PushPull 1,0,0 -40").unwrap();
        assert!((bbox(&e).max.x - 60.0).abs() < 1e-9);
        e.run_line("MoveFace #1 0,25,10 15").unwrap(); // the x = 0 face, outwards
        let b = bbox(&e);
        assert!((b.min.x + 15.0).abs() < 1e-9, "{b:?}");
        let Geometry::Mesh(m) = &e.doc().objects().next().unwrap().geometry else {
            panic!()
        };
        assert!((m.volume() - 75.0 * 50.0 * 50.0).abs() < 1e-6);
        e.run_line("Undo").unwrap();
        e.run_line("Undo").unwrap();
        e.run_line("Undo").unwrap();
        assert!((bbox(&e).max.z - 30.0).abs() < 1e-9);
        assert!(e.run_line("MoveFace #1 500,500,500 5").is_err());
    }
}
