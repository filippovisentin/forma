//! Everyday workflow commands: SelPrev, ShowSelected, UnlockSelected, Lines,
//! Stretch, ClosestPt, DupFaceBorder, UnifyMeshNormals, Block, Insert.

use crate::edit::parse_ids;
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{Chain, Dimension, LineCurve, Mesh, Point3, Seg, Vec3, Xform};

simple_command!(
    SelPrev,
    "SelPrev",
    &[],
    "SelPrev — select again the objects that were selected before the last change of selection"
);
impl Command for SelPrev {
    impl_meta!(SelPrev);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let doc = &ctx.doc;
        let ids: Vec<ObjectId> = ctx
            .prev_selection
            .iter()
            .copied()
            .filter(|id| doc.object(*id).is_some_and(|o| doc.is_selectable(o)))
            .collect();
        if ids.is_empty() {
            return Err(CommandError::Invalid("no previous selection".into()));
        }
        ctx.selection = ids.into_iter().collect();
        Ok(format!("{} selected", ctx.selection.len()))
    }
}

simple_command!(
    ShowSelected,
    "ShowSelected",
    &[],
    "ShowSelected [#id …] — show the listed hidden objects, or every hidden object, and select what was shown"
);
impl Command for ShowSelected {
    impl_meta!(ShowSelected);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let listed = parse_ids(args.rest())?;
        let ids: Vec<ObjectId> = ctx
            .doc
            .objects()
            .filter(|o| o.hidden && (listed.is_empty() || listed.contains(&o.id)))
            .map(|o| o.id)
            .collect();
        if ids.is_empty() {
            return Err(CommandError::Invalid("no hidden objects to show".into()));
        }
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_hidden(*id, false);
        }
        t.commit();
        let doc = &ctx.doc;
        ctx.selection = ids
            .iter()
            .copied()
            .filter(|id| doc.object(*id).is_some_and(|o| doc.is_selectable(o)))
            .collect();
        Ok(format!("{} object(s) shown and selected", ids.len()))
    }
}

simple_command!(
    UnlockSelected,
    "UnlockSelected",
    &[],
    "UnlockSelected [#id …] — unlock the listed locked objects, or every locked object, and select them"
);
impl Command for UnlockSelected {
    impl_meta!(UnlockSelected);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let listed = parse_ids(args.rest())?;
        let ids: Vec<ObjectId> = ctx
            .doc
            .objects()
            .filter(|o| o.locked && (listed.is_empty() || listed.contains(&o.id)))
            .map(|o| o.id)
            .collect();
        if ids.is_empty() {
            return Err(CommandError::Invalid("no locked objects to unlock".into()));
        }
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_locked(*id, false);
        }
        t.commit();
        let doc = &ctx.doc;
        ctx.selection = ids
            .iter()
            .copied()
            .filter(|id| doc.object(*id).is_some_and(|o| doc.is_selectable(o)))
            .collect();
        Ok(format!("{} object(s) unlocked and selected", ids.len()))
    }
}

simple_command!(
    Lines,
    "Lines",
    &[],
    "Lines <p1> <p2> [p3 …] — chain of separate line segments (not joined)"
);
impl Command for Lines {
    impl_meta!(Lines);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut pts = vec![args.point("first point", ctx.last_point)?];
        while args.peek().is_some() {
            let last = *pts.last().expect("non-empty");
            pts.push(args.point("next point", Some(last))?);
        }
        if pts.len() < 2 {
            return Err(CommandError::MissingInput("second point"));
        }
        pts.dedup_by(|a, b| a.distance_to(*b) <= ctx.tolerance.absolute);
        if pts.len() < 2 {
            return Err(CommandError::Invalid("lines are degenerate".into()));
        }
        let mut t = ctx.doc.begin();
        for w in pts.windows(2) {
            t.add(Geometry::Line(LineCurve::new(w[0], w[1])));
        }
        t.commit();
        ctx.last_point = pts.last().copied();
        Ok(format!("added {} line(s)", pts.len() - 1))
    }
}

/// Axis-aligned stretch window. When both corners share the same height the
/// window is a rectangle seen from above and extends through every height.
struct Window {
    min: Point3,
    max: Point3,
    any_z: bool,
}

impl Window {
    fn new(a: Point3, b: Point3, tol: f64) -> Window {
        Window {
            min: Point3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            max: Point3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
            any_z: (a.z - b.z).abs() <= tol,
        }
    }
    fn contains(&self, p: Point3) -> bool {
        (self.min.x..=self.max.x).contains(&p.x)
            && (self.min.y..=self.max.y).contains(&p.y)
            && (self.any_z || (self.min.z..=self.max.z).contains(&p.z))
    }
    fn shift(&self, p: Point3, v: Vec3) -> Point3 {
        if self.contains(p) {
            p + v
        } else {
            p
        }
    }
}

/// Stretched copy of `g`, or `None` when no point of it lies in the window.
fn stretched(g: &Geometry, w: &Window, v: Vec3) -> Option<Geometry> {
    let mv = Xform::translation(v);
    let out = match g {
        Geometry::Point(p) => w.contains(*p).then(|| Geometry::Point(*p + v))?,
        Geometry::Line(l) => {
            if !w.contains(l.from) && !w.contains(l.to) {
                return None;
            }
            Geometry::Line(LineCurve::new(w.shift(l.from, v), w.shift(l.to, v)))
        }
        Geometry::Polyline(p) => {
            if !p.iter().any(|q| w.contains(*q)) {
                return None;
            }
            Geometry::Polyline(p.iter().map(|q| w.shift(*q, v)).collect())
        }
        Geometry::Nurbs(n) => {
            if !n.points.iter().any(|q| w.contains(*q)) {
                return None;
            }
            Geometry::Nurbs(forma_geom::NurbsCurve {
                points: n.points.iter().map(|q| w.shift(*q, v)).collect(),
                ..n.clone()
            })
        }
        Geometry::Arc(a) => {
            let pts = a.points(32);
            let inside = pts.iter().filter(|q| w.contains(**q)).count();
            match inside {
                0 => return None,
                n if n == pts.len() => g.transformed(&mv),
                // Partly inside: the arc cannot stay an arc.
                _ => Geometry::Polyline(a.points(96).iter().map(|q| w.shift(*q, v)).collect()),
            }
        }
        Geometry::PolyCurve(segs) => {
            let pts = Chain::new(segs.clone()).points();
            if !pts.iter().any(|q| w.contains(*q)) {
                return None;
            }
            let mut out = Vec::new();
            for s in segs {
                match s {
                    Seg::Line(a, b) => out.push(Seg::Line(w.shift(*a, v), w.shift(*b, v))),
                    Seg::Arc(a) => {
                        let ap = a.points(32);
                        let n = ap.iter().filter(|q| w.contains(**q)).count();
                        if n == 0 {
                            out.push(*s);
                        } else if n == ap.len() {
                            out.push(Seg::Arc(a.transformed(&mv)));
                        } else {
                            let fine: Vec<Point3> =
                                a.points(48).iter().map(|q| w.shift(*q, v)).collect();
                            out.extend(fine.windows(2).map(|p| Seg::Line(p[0], p[1])));
                        }
                    }
                }
            }
            Geometry::PolyCurve(out)
        }
        Geometry::Mesh(m) => {
            if !m.positions.iter().any(|q| w.contains(*q)) {
                return None;
            }
            Geometry::Mesh(Mesh {
                positions: m.positions.iter().map(|q| w.shift(*q, v)).collect(),
                // Moved vertices change the face normals: let the display recompute.
                normals: Vec::new(),
                triangles: m.triangles.clone(),
            })
        }
        Geometry::Text(t) => w.contains(t.plane.origin).then(|| g.transformed(&mv))?,
        Geometry::Dimension(d) => {
            if !d.points.iter().any(|q| w.contains(*q)) {
                return None;
            }
            Geometry::Dimension(Dimension {
                points: d.points.iter().map(|q| w.shift(*q, v)).collect(),
                ..d.clone()
            })
        }
    };
    Some(out)
}

simple_command!(
    Stretch,
    "Stretch",
    &[],
    "Stretch <window corner> <opposite corner> <from> <to> — move the points inside the window (curve points, mesh vertices, dimension points), stretching what crosses it; acts on the selection, or on every visible object"
);
impl Command for Stretch {
    impl_meta!(Stretch);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("window corner", ctx.last_point)?;
        let b = args.point("opposite corner", Some(a))?;
        let from = args.point("point to stretch from", Some(b))?;
        let to = args.point("point to stretch to", Some(from))?;
        let tol = ctx.tolerance.absolute;
        let w = Window::new(a, b, tol);
        let v = to - from;
        let doc = &ctx.doc;
        let ids: Vec<ObjectId> = if ctx.selection.is_empty() {
            doc.objects()
                .filter(|o| doc.is_selectable(o))
                .map(|o| o.id)
                .collect()
        } else {
            ctx.selection.iter().copied().collect()
        };
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for id in ids {
            let g = t.doc().object(id).expect("listed").geometry.clone();
            if let Some(out) = stretched(&g, &w, v) {
                t.replace(id, out);
                n += 1;
            }
        }
        if n == 0 {
            return Err(CommandError::Invalid(
                "Stretch: no object has points inside the window".into(),
            ));
        }
        t.commit();
        ctx.last_point = Some(to);
        Ok(format!("stretched {n} object(s)"))
    }
}

/// Closest point to `p` on the segments joining `pts`.
fn closest_on_polyline(pts: &[Point3], p: Point3) -> Option<Point3> {
    if pts.len() == 1 {
        return Some(pts[0]);
    }
    pts.windows(2)
        .map(|s| {
            let d = s[1] - s[0];
            let len2 = d.dot(d);
            let t = if len2 > 0.0 {
                ((p - s[0]).dot(d) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            s[0] + d * t
        })
        .min_by(|a, b| a.distance_to(p).total_cmp(&b.distance_to(p)))
}

/// Closest point to `p` on an object.
fn closest_on(g: &Geometry, p: Point3) -> Option<Point3> {
    match g {
        Geometry::Point(q) => Some(*q),
        Geometry::Mesh(m) => m.closest_triangle(p).map(|(_, q, _)| q),
        Geometry::Arc(a) => closest_on_polyline(&a.points(720), p),
        Geometry::Nurbs(n) => closest_on_polyline(&n.sample(64), p),
        g => closest_on_polyline(&g.curve_points(), p),
    }
}

simple_command!(
    ClosestPt,
    "ClosestPt",
    &[],
    "ClosestPt <point> — add a point on each selected object where it comes closest to <point>"
);
impl Command for ClosestPt {
    impl_meta!(ClosestPt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let p = args.point("point to measure from", ctx.last_point)?;
        let ids = ctx.selected("ClosestPt")?;
        let mut found = Vec::new();
        for id in &ids {
            let g = &ctx.doc.object(*id).expect("selected").geometry;
            if let Some(q) = closest_on(g, p) {
                found.push(q);
            }
        }
        if found.is_empty() {
            return Err(CommandError::Invalid(
                "ClosestPt: no curve or mesh selected".into(),
            ));
        }
        let mut t = ctx.doc.begin();
        for q in &found {
            t.add(Geometry::Point(*q));
        }
        t.commit();
        let d = found
            .iter()
            .map(|q| q.distance_to(p))
            .fold(f64::INFINITY, f64::min);
        Ok(format!(
            "{} point(s) added; nearest at distance {d:.3} {}",
            found.len(),
            ctx.doc.units.abbreviation()
        ))
    }
}

simple_command!(
    DupFaceBorder,
    "DupFaceBorder",
    &[],
    "DupFaceBorder #id <point on face> — copy the outline of the flat face of a mesh under the point as curves"
);
impl Command for DupFaceBorder {
    impl_meta!(DupFaceBorder);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("mesh"))?;
        let id = parse_ids(vec![tok])?
            .first()
            .copied()
            .ok_or_else(|| CommandError::BadInput(tok.to_string()))?;
        let p = args.point("point on the face", ctx.last_point)?;
        let tol = ctx.tolerance.absolute;
        let Some(Geometry::Mesh(m)) = ctx.doc.object(id).map(|o| &o.geometry) else {
            return Err(CommandError::Invalid(format!("#{} is not a mesh", id.0)));
        };
        let (tri, _, _) = m
            .closest_triangle(p)
            .ok_or_else(|| CommandError::Invalid("empty mesh".into()))?;
        let face = m
            .planar_face(tri, tol)
            .ok_or_else(|| CommandError::Invalid("no flat face there".into()))?;
        let edges = m.face_outline(&face, tol);
        let chains: Vec<Chain> = edges
            .iter()
            .map(|e| Chain::new(vec![Seg::Line(e[0], e[1])]))
            .collect();
        let loops = forma_geom::join(chains, tol);
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for c in loops {
            let pts = c.points();
            t.add(Geometry::Polyline(merge_collinear(&pts, tol)));
            n += 1;
        }
        t.commit();
        Ok(format!("{n} border curve(s) added"))
    }
}

/// Drop interior points of straight runs (mesh face outlines are split at every
/// triangle vertex).
fn merge_collinear(pts: &[Point3], tol: f64) -> Vec<Point3> {
    let closed = pts.len() > 3 && pts[0].distance_to(*pts.last().expect("len>3")) <= tol;
    let mut out: Vec<Point3> = Vec::with_capacity(pts.len());
    for &p in pts {
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            if off_line(a, p, b) <= tol {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    if closed && out.len() > 4 && off_line(out[out.len() - 2], out[1], out[0]) <= tol {
        // The start point sits in the middle of a straight run: restart at out[1].
        out.remove(0);
        let last = out.len() - 1;
        out[last] = out[0];
    }
    out
}

/// Distance of `p` from the line through `a` and `b`.
fn off_line(a: Point3, b: Point3, p: Point3) -> f64 {
    let d = b - a;
    let len = d.length();
    if len <= f64::EPSILON {
        return p.distance_to(a);
    }
    (p - a).cross(d).length() / len
}

simple_command!(
    UnifyMeshNormals,
    "UnifyMeshNormals",
    &[],
    "UnifyMeshNormals — make the triangles of each selected mesh face the same way (outwards for closed meshes)"
);
impl Command for UnifyMeshNormals {
    impl_meta!(UnifyMeshNormals);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("UnifyMeshNormals")?;
        let tol = ctx.tolerance.absolute;
        let mut t = ctx.doc.begin();
        let mut flipped = 0;
        for id in ids {
            let Geometry::Mesh(m) = &t.doc().object(id).expect("selected").geometry else {
                continue;
            };
            let (out, n) = m.unified(tol);
            if n > 0 {
                flipped += n;
                t.replace(id, Geometry::Mesh(out));
            }
        }
        t.commit();
        Ok(format!("{flipped} triangle(s) flipped"))
    }
}

simple_command!(
    Block,
    "Block",
    &[],
    "Block <name> — turn the selected objects into a block: one group named <name> (Rhino blocks open the same way)"
);
impl Command for Block {
    impl_meta!(Block);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = args.rest().join(" ");
        let name = name.trim().trim_matches('"').to_string();
        if name.is_empty() {
            return Err(CommandError::MissingInput("name"));
        }
        let ids = ctx.selected("Block")?;
        let g = ctx
            .doc
            .objects()
            .filter_map(|o| o.group)
            .max()
            .map_or(1, |m| m + 1);
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_group(*id, Some(g));
            t.set_name(*id, Some(name.clone()));
        }
        t.commit();
        Ok(format!("block \"{name}\": {} object(s)", ids.len()))
    }
}

simple_command!(
    Insert,
    "Insert",
    &[],
    "Insert <name> <point> [scale] [angle°] — place a copy of a block (a group named <name>) with its bottom centre at <point>"
);
impl Command for Insert {
    impl_meta!(Insert);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = args
            .next_token()
            .ok_or(CommandError::MissingInput("name"))?
            .trim_matches('"')
            .to_string();
        let at = args.point("insertion point", ctx.last_point)?;
        let scale = args.optional_number().unwrap_or(1.0);
        let angle = args.optional_number().unwrap_or(0.0);
        if scale.abs() <= f64::EPSILON {
            return Err(CommandError::Invalid("scale must not be 0".into()));
        }
        // The first group whose members carry this name is the definition.
        let doc = &ctx.doc;
        let group = doc
            .objects()
            .filter(|o| {
                o.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&name))
            })
            .find_map(|o| o.group)
            .ok_or_else(|| CommandError::Invalid(format!("no block named \"{name}\"")))?;
        let members: Vec<forma_doc::Object> = doc
            .objects()
            .filter(|o| o.group == Some(group))
            .cloned()
            .collect();
        let pts: Vec<Point3> = members
            .iter()
            .flat_map(|o| {
                let b = o.geometry.bounding_box();
                [b.min, b.max]
            })
            .collect();
        let bb = forma_geom::BoundingBox::from_points(&pts)
            .ok_or_else(|| CommandError::Invalid("empty block".into()))?;
        let base = Point3::new(
            (bb.min.x + bb.max.x) / 2.0,
            (bb.min.y + bb.max.y) / 2.0,
            bb.min.z,
        );
        let x = Xform::translation(Point3::ORIGIN - base)
            .then(&Xform::scale(Point3::ORIGIN, scale))
            .then(&Xform::rotation(
                Point3::ORIGIN,
                Vec3::Z,
                angle.to_radians(),
            ))
            .then(&Xform::translation(at - Point3::ORIGIN));
        let new_group = doc.objects().filter_map(|o| o.group).max().unwrap_or(0) + 1;
        let mut t = ctx.doc.begin();
        let mut ids = Vec::new();
        for o in &members {
            let id = t.add_like(o.geometry.transformed(&x), o);
            t.set_group(id, Some(new_group));
            ids.push(id);
        }
        t.commit();
        ctx.selection = ids.iter().copied().collect();
        ctx.last_point = Some(at);
        Ok(format!("inserted \"{name}\" ({} object(s))", ids.len()))
    }
}

simple_command!(
    MoveGrips,
    "MoveGrips",
    &["MovePts"],
    "MoveGrips #id <i,j,…> [#id <i,…> …] <dx,dy,dz> — move control points / vertices (grips, see PointsOn) of objects by a vector"
);
impl Command for MoveGrips {
    impl_meta!(MoveGrips);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let toks = args.rest();
        let (vec_tok, pairs) = toks
            .split_last()
            .ok_or(CommandError::MissingInput("object id"))?;
        let v = crate::args::parse_point(vec_tok, None)? - Point3::ORIGIN;
        let mut edits: Vec<(ObjectId, Vec<usize>)> = Vec::new();
        for t in pairs {
            if let Some(id) = t.strip_prefix('#') {
                let id: u64 = id
                    .parse()
                    .map_err(|_| CommandError::BadInput(t.to_string()))?;
                edits.push((ObjectId(id), Vec::new()));
            } else {
                let last = edits
                    .last_mut()
                    .ok_or_else(|| CommandError::BadInput(format!("{t}: object id first")))?;
                for i in t.split(',').filter(|s| !s.is_empty()) {
                    last.1.push(
                        i.parse()
                            .map_err(|_| CommandError::BadInput(t.to_string()))?,
                    );
                }
            }
        }
        if edits.is_empty() {
            return Err(CommandError::MissingInput("object id"));
        }
        let mut t = ctx.doc.begin();
        let mut moved = 0;
        for (id, idx) in &edits {
            let g = t
                .doc()
                .object(*id)
                .ok_or_else(|| CommandError::Invalid(format!("no object #{}", id.0)))?
                .geometry
                .clone();
            let n = g.grips().len();
            if let Some(bad) = idx.iter().find(|i| **i >= n) {
                return Err(CommandError::Invalid(format!(
                    "#{} has {n} grip(s), no grip {bad}",
                    id.0
                )));
            }
            let moves: Vec<(usize, Vec3)> = idx.iter().map(|i| (*i, v)).collect();
            t.replace(*id, g.with_grips_moved(&moves));
            moved += idx.len();
        }
        t.commit();
        Ok(format!("moved {moved} point(s)"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;

    #[test]
    fn move_grips_of_a_polyline_and_a_box() {
        let mut e = Engine::new();
        e.run_line("Polyline 0,0 10,0 10,10").unwrap();
        e.run_line("Box 20,0 30,10 10").unwrap();
        e.run_line("MoveGrips #1 1 0,0,5").unwrap();
        let Geometry::Polyline(p) = &e.doc().objects().next().unwrap().geometry else {
            panic!()
        };
        assert!((p[1].z - 5.0).abs() < 1e-12 && p[0].z.abs() < 1e-12);
        // Raise the four top corners of the box together with the first point.
        let grips = e
            .doc()
            .object(forma_doc::ObjectId(2))
            .unwrap()
            .geometry
            .grips();
        let top: Vec<String> = grips
            .iter()
            .enumerate()
            .filter(|(_, p)| p.z > 5.0)
            .map(|(i, _)| i.to_string())
            .collect();
        e.run_line(&format!("MoveGrips #1 0 #2 {} 0,0,2", top.join(",")))
            .unwrap();
        let b = e
            .doc()
            .object(forma_doc::ObjectId(2))
            .unwrap()
            .geometry
            .bounding_box();
        assert!((b.max.z - 12.0).abs() < 1e-12);
        assert!(e.run_line("MoveGrips #1 9 1,0,0").is_err());
        e.run_line("Undo").unwrap();
        let b = e
            .doc()
            .object(forma_doc::ObjectId(2))
            .unwrap()
            .geometry
            .bounding_box();
        assert!((b.max.z - 10.0).abs() < 1e-12);
    }

    #[test]
    fn block_insert_explode() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 5").unwrap();
        e.run_line("Circle 5,5,5 2").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Block tavolo").unwrap();
        e.run_line("SelNone").unwrap();
        e.run_line("Insert tavolo 100,0 2 90").unwrap();
        assert_eq!(e.doc().len(), 4);
        assert_eq!(e.ctx.selection.len(), 2);
        let pts: Vec<_> = e
            .ctx
            .selection
            .iter()
            .flat_map(|id| {
                let b = e.doc().object(*id).unwrap().geometry.bounding_box();
                [b.min, b.max]
            })
            .collect();
        let bb = forma_geom::BoundingBox::from_points(&pts).unwrap();
        assert!(
            (bb.min.x - 90.0).abs() < 1e-9 && (bb.max.x - 110.0).abs() < 1e-9,
            "{bb:?}"
        );
        assert!(
            bb.min.z.abs() < 1e-9 && (bb.max.z - 10.0).abs() < 1e-9,
            "{bb:?}"
        );
        let g: Vec<_> = e.doc().objects().map(|o| o.group).collect();
        assert_eq!(g, vec![Some(1), Some(1), Some(2), Some(2)]);
        e.run_line("ExplodeBlock").unwrap();
        assert!(e.run_line("Insert sedia 0,0").is_err());
    }

    #[test]
    fn sel_prev_after_deselect() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Line 0,5 10,5").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Move 0,0 0,1").unwrap();
        e.run_line("SelNone").unwrap();
        assert!(e.ctx.selection.is_empty());
        e.run_line("SelPrev").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
    }

    #[test]
    fn show_and_unlock_selected() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Line 0,5 10,5").unwrap();
        e.run_line("Hide #1 #2").unwrap();
        e.run_line("ShowSelected #2").unwrap();
        assert!(e.doc().objects().next().unwrap().hidden);
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("Lock #2").unwrap();
        e.run_line("UnlockSelected").unwrap();
        assert_eq!(e.ctx.selection.len(), 1);
        assert!(e.run_line("UnlockSelected").is_err());
    }

    #[test]
    fn lines_are_separate() {
        let mut e = Engine::new();
        e.run_line("Lines 0,0 10,0 10,10 0,10").unwrap();
        assert_eq!(e.doc().len(), 3);
        assert!(e
            .doc()
            .objects()
            .all(|o| matches!(o.geometry, Geometry::Line(_))));
    }

    #[test]
    fn stretch_a_wall() {
        let mut e = Engine::new();
        // A 400 x 30 x 300 wall and a plan rectangle; stretch the right end by 100.
        e.run_line("Box 0,0 400,30 300").unwrap();
        e.run_line("Rectangle 0,0 400,30").unwrap();
        e.run_line("Stretch 350,-10 450,50 0,0 100,0").unwrap();
        for o in e.doc().objects() {
            let b = o.geometry.bounding_box();
            assert!((b.max.x - 500.0).abs() < 1e-9, "{b:?}");
            assert!(b.min.x.abs() < 1e-9, "{b:?}");
        }
        // Nothing in the window: an error, the document is unchanged.
        assert!(e.run_line("Stretch 900,900 950,950 0,0 1,0").is_err());
        e.run_line("Undo").unwrap();
        let b = e.doc().objects().next().unwrap().geometry.bounding_box();
        assert!((b.max.x - 400.0).abs() < 1e-9);
    }

    #[test]
    fn closest_point_and_list() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelAll").unwrap();
        let out = e.run_line("List").unwrap();
        assert!(out.contains("#1 line"), "{out}");
        e.run_line("ClosestPt 4,3").unwrap();
        let p = e.doc().objects().last().unwrap();
        let Geometry::Point(p) = p.geometry else {
            panic!()
        };
        assert!(p.distance_to(forma_geom::Point3::new(4.0, 0.0, 0.0)) < 1e-9);
    }

    #[test]
    fn dup_face_border_of_box_top() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,20 5").unwrap();
        e.run_line("DupFaceBorder #1 5,10,5").unwrap();
        let o = e.doc().objects().last().unwrap();
        let Geometry::Polyline(p) = &o.geometry else {
            panic!("{:?}", o.geometry.kind())
        };
        assert_eq!(p.len(), 5, "{p:?}");
        assert!(p.iter().all(|q| (q.z - 5.0).abs() < 1e-9));
        assert!((o.geometry.length().unwrap() - 60.0).abs() < 1e-9);
    }

    #[test]
    fn unify_mesh_normals_fixes_a_flipped_triangle() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        let Geometry::Mesh(mut m) = e.doc().objects().next().unwrap().geometry.clone() else {
            panic!()
        };
        m.triangles[0].swap(1, 2);
        m.normals.clear();
        {
            let mut t = e.ctx.doc.begin();
            t.replace(forma_doc::ObjectId(1), Geometry::Mesh(m));
            t.commit();
        }
        e.run_line("SelAll").unwrap();
        let out = e.run_line("UnifyMeshNormals").unwrap();
        assert_eq!(out, "1 triangle(s) flipped");
        assert_eq!(
            e.run_line("UnifyMeshNormals").unwrap(),
            "0 triangle(s) flipped"
        );
    }
}
