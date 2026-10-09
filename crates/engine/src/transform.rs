//! More transforms and object edits: Scale1D, Scale2D, Orient, Align, Flip,
//! MatchProperties, BoundingBox, ProjectToCPlane.

use crate::edit::{parse_ids, transform_selection};
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{box_mesh, BoundingBox, LineCurve, Plane, Point3, Vec3, Xform};

fn factor(args: &mut Args) -> Result<f64, CommandError> {
    let f = args.number("scale factor")?;
    if f.abs() < 1e-9 {
        return Err(CommandError::Invalid(
            "scale factor must not be zero".into(),
        ));
    }
    Ok(f)
}

simple_command!(
    Scale1D,
    "Scale1D",
    &["S1"],
    "Scale1D <origin> <factor> <direction point> — scale the selection along one direction (@dx,dy,dz for a vector)"
);
impl Command for Scale1D {
    impl_meta!(Scale1D);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let o = args.point("origin point", ctx.last_point)?;
        let f = factor(args)?;
        let d = args.point("direction point", Some(o))?;
        let dir = (d - o)
            .normalized()
            .ok_or_else(|| CommandError::Invalid("direction point is the origin".into()))?;
        let plane = Plane::from_normal(o, dir);
        // Plane z is the scaling direction.
        transform_selection(
            ctx,
            "Scale1D",
            &Xform::scale_axes(&plane, 1.0, 1.0, f),
            false,
        )
    }
}

simple_command!(
    Scale2D,
    "Scale2D",
    &["S2"],
    "Scale2D <origin> <factor> [normal] — scale the selection in the construction plane only"
);
impl Command for Scale2D {
    impl_meta!(Scale2D);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let o = args.point("origin point", ctx.last_point)?;
        let f = factor(args)?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let plane = Plane::from_normal(o, n);
        transform_selection(ctx, "Scale2D", &Xform::scale_axes(&plane, f, f, 1.0), false)
    }
}

simple_command!(
    Orient,
    "Orient",
    &["Or"],
    "Orient <ref1> <ref2> <target1> <target2> [copy] [scale] — move ref1 to target1 and turn ref1→ref2 towards target1→target2"
);
impl Command for Orient {
    impl_meta!(Orient);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a1 = args.point("reference point 1", ctx.last_point)?;
        let a2 = args.point("reference point 2", Some(a1))?;
        let b1 = args.point("target point 1", Some(a2))?;
        let b2 = args.point("target point 2", Some(b1))?;
        let (mut copy, mut scale) = (false, false);
        loop {
            if args.keyword("copy") {
                copy = true;
            } else if args.keyword("scale") {
                scale = true;
            } else {
                break;
            }
        }
        let x = Xform::orient(a1, a2, b1, b2, scale)
            .ok_or_else(|| CommandError::Invalid("reference points coincide".into()))?;
        transform_selection(ctx, "Orient", &x, copy)
    }
}

/// Bounding box of geometry in plane coordinates: (umin, vmin, umax, vmax).
fn plane_box(g: &Geometry, plane: &Plane) -> (f64, f64, f64, f64) {
    let pts = match g {
        Geometry::Mesh(m) => m.positions.clone(),
        Geometry::Point(p) => vec![*p],
        g => g.curve_points(),
    };
    pts.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(a, b, c, d), p| {
            let (u, v, _) = plane.coords(*p);
            (a.min(u), b.min(v), c.max(u), d.max(v))
        },
    )
}

simple_command!(
    Align,
    "Align",
    &[],
    "Align <left|right|top|bottom|hcenter|vcenter|center> [normal] — align the selected objects in the construction plane"
);
impl Command for Align {
    impl_meta!(Align);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mode = args
            .next_token()
            .ok_or(CommandError::MissingInput("alignment"))?
            .to_ascii_lowercase();
        let modes = [
            "left", "right", "top", "bottom", "hcenter", "vcenter", "center",
        ];
        if !modes.contains(&mode.as_str()) {
            return Err(CommandError::BadInput(format!("alignment {mode}")));
        }
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let ids = ctx.selected("Align")?;
        if ids.len() < 2 {
            return Err(CommandError::Invalid(
                "Align: select two or more objects".into(),
            ));
        }
        let plane = Plane::from_normal(Point3::ORIGIN, n);
        let boxes: Vec<(ObjectId, (f64, f64, f64, f64))> = ids
            .iter()
            .map(|id| {
                (
                    *id,
                    plane_box(&ctx.doc.object(*id).expect("selected").geometry, &plane),
                )
            })
            .collect();
        let all = boxes.iter().fold(
            (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
            |(a, b, c, d), (_, (u0, v0, u1, v1))| (a.min(*u0), b.min(*v0), c.max(*u1), d.max(*v1)),
        );
        let (cu, cv) = ((all.0 + all.2) / 2.0, (all.1 + all.3) / 2.0);
        let mut t = ctx.doc.begin();
        for (id, (u0, v0, u1, v1)) in boxes {
            let (du, dv) = match mode.as_str() {
                "left" => (all.0 - u0, 0.0),
                "right" => (all.2 - u1, 0.0),
                "bottom" => (0.0, all.1 - v0),
                "top" => (0.0, all.3 - v1),
                "hcenter" => (cu - (u0 + u1) / 2.0, 0.0),
                "vcenter" => (0.0, cv - (v0 + v1) / 2.0),
                _ => (cu - (u0 + u1) / 2.0, cv - (v0 + v1) / 2.0),
            };
            let x = Xform::translation(plane.x * du + plane.y * dv);
            let g = t
                .doc()
                .object(id)
                .expect("selected")
                .geometry
                .transformed(&x);
            t.replace(id, g);
        }
        t.commit();
        Ok(format!("aligned {} object(s) {mode}", ids.len()))
    }
}

simple_command!(
    Flip,
    "Flip",
    &["Dir"],
    "Flip — reverse the direction of the selected curves and the normals of selected meshes"
);
impl Command for Flip {
    impl_meta!(Flip);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Flip")?;
        let mut t = ctx.doc.begin();
        let mut n = 0;
        for id in &ids {
            let g = &t.doc().object(*id).expect("selected").geometry;
            if matches!(g, Geometry::Point(_)) {
                continue;
            }
            let r = g.reversed();
            t.replace(*id, r);
            n += 1;
        }
        if n == 0 {
            return Err(CommandError::Invalid(
                "Flip: select curves or meshes".into(),
            ));
        }
        t.commit();
        Ok(format!("flipped {n} object(s)"))
    }
}

simple_command!(
    MatchProperties,
    "MatchProperties",
    &["MatchProp", "MA"],
    "MatchProperties #source — give the selected objects the layer and colour of #source"
);
impl Command for MatchProperties {
    impl_meta!(MatchProperties);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("source object #id"))?;
        let src_id = parse_ids(vec![tok])?[0];
        let src = ctx
            .doc
            .object(src_id)
            .ok_or_else(|| CommandError::Invalid(format!("no object #{}", src_id.0)))?
            .clone();
        let ids: Vec<ObjectId> = ctx
            .selected("MatchProperties")?
            .into_iter()
            .filter(|id| *id != src_id)
            .collect();
        if ids.is_empty() {
            return Err(CommandError::Invalid(
                "MatchProperties: select objects to change".into(),
            ));
        }
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_layer(*id, src.layer);
            t.set_color(*id, src.color);
        }
        t.commit();
        Ok(format!("matched {} object(s) to #{}", ids.len(), src_id.0))
    }
}

/// Union bounding box of the selection.
pub(crate) fn selection_box(ctx: &Context, cmd: &str) -> Result<BoundingBox, CommandError> {
    let ids = ctx.selected(cmd)?;
    let mut bb: Option<BoundingBox> = None;
    for id in ids {
        let b = ctx
            .doc
            .object(id)
            .expect("selected")
            .geometry
            .bounding_box();
        bb = Some(bb.map_or(b, |a| a.union(b)));
    }
    bb.ok_or_else(|| CommandError::Invalid(format!("{cmd}: nothing selected")))
}

simple_command!(
    BoundingBoxCmd,
    "BoundingBox",
    &["BBox"],
    "BoundingBox — add a world-aligned box around the selection (a rectangle when flat)"
);
impl Command for BoundingBoxCmd {
    impl_meta!(BoundingBoxCmd);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let b = selection_box(ctx, "BoundingBox")?;
        let tol = ctx.tolerance.absolute;
        let d = b.max - b.min;
        let flat = [d.x <= tol, d.y <= tol, d.z <= tol];
        let g = match flat.iter().filter(|f| **f).count() {
            0 => Geometry::Mesh(box_mesh(&Plane::TOP.moved_to(b.min), d.x, d.y, d.z)),
            1 => {
                let (a, c) = (b.min, b.max);
                let pts = if flat[2] {
                    vec![
                        a,
                        Point3::new(c.x, a.y, a.z),
                        Point3::new(c.x, c.y, a.z),
                        Point3::new(a.x, c.y, a.z),
                    ]
                } else if flat[1] {
                    vec![
                        a,
                        Point3::new(c.x, a.y, a.z),
                        Point3::new(c.x, a.y, c.z),
                        Point3::new(a.x, a.y, c.z),
                    ]
                } else {
                    vec![
                        a,
                        Point3::new(a.x, c.y, a.z),
                        Point3::new(a.x, c.y, c.z),
                        Point3::new(a.x, a.y, c.z),
                    ]
                };
                let mut pts = pts;
                pts.push(a);
                Geometry::Polyline(pts)
            }
            2 => Geometry::Line(LineCurve::new(b.min, b.max)),
            _ => {
                return Err(CommandError::Invalid(
                    "BoundingBox: the selection is a single point".into(),
                ))
            }
        };
        let mut t = ctx.doc.begin();
        let id = t.add(g);
        t.commit();
        Ok(format!(
            "added #{} bounding box {:.4} x {:.4} x {:.4}",
            id.0, d.x, d.y, d.z
        ))
    }
}

simple_command!(
    ProjectToCPlane,
    "ProjectToCPlane",
    &["Flatten"],
    "ProjectToCPlane [normal] [origin] — flatten the selected objects onto the construction plane"
);
impl Command for ProjectToCPlane {
    impl_meta!(ProjectToCPlane);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let o = Point3::ORIGIN + args.optional_vector().unwrap_or(Vec3::new(0.0, 0.0, 0.0));
        let plane = Plane::from_normal(o, n);
        let proj = Xform::projection(&plane);
        let ids = ctx.selected("ProjectToCPlane")?;
        let mut t = ctx.doc.begin();
        for id in &ids {
            let g = &t.doc().object(*id).expect("selected").geometry;
            let p = match g {
                // Arcs in a parallel plane just move; others become exact NURBS.
                Geometry::Arc(a) if a.plane.z.cross(plane.z).length() < 1e-9 => {
                    let d = plane.coords(a.center()).2;
                    g.transformed(&Xform::translation(plane.z * -d))
                }
                _ => g.transformed(&proj),
            };
            t.replace(*id, p);
        }
        t.commit();
        Ok(format!("projected {} object(s)", ids.len()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::{Geometry, ObjectId};
    use forma_geom::Point3;

    fn g(e: &Engine, id: u64) -> Geometry {
        e.doc().object(ObjectId(id)).unwrap().geometry.clone()
    }

    #[test]
    fn scale_1d_and_2d() {
        let mut e = Engine::new();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Scale1D 0,0 2 @1,0").unwrap();
        assert_eq!(g(&e, 1).kind(), "nurbs");
        let b = g(&e, 1).bounding_box();
        assert!(
            (b.max.x - 20.0).abs() < 1e-6 && (b.max.y - 10.0).abs() < 1e-6,
            "{b:?}"
        );
        let b = g(&e, 2).bounding_box();
        assert!((b.max.x - 20.0).abs() < 1e-9 && (b.max.z - 10.0).abs() < 1e-9);
        e.run_line("Scale2D 0,0 0.5").unwrap();
        let b = g(&e, 2).bounding_box();
        assert!((b.max.x - 10.0).abs() < 1e-9 && (b.max.y - 5.0).abs() < 1e-9);
        assert!((b.max.z - 10.0).abs() < 1e-9);
        assert!(e.run_line("Scale1D 0,0 2 0,0").is_err());
    }

    #[test]
    fn orient_with_copy_and_scale() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Orient 0,0 10,0 100,100 100,120 copy scale")
            .unwrap();
        assert_eq!(e.doc().len(), 2);
        let Geometry::Line(l) = g(&e, 2) else {
            panic!()
        };
        assert!(l.from.distance_to(Point3::new(100.0, 100.0, 0.0)) < 1e-9);
        assert!(l.to.distance_to(Point3::new(100.0, 120.0, 0.0)) < 1e-9);
        e.run_line("Orient 0,0 10,0 0,0 0,0,1").unwrap();
        let Geometry::Line(l) = g(&e, 1) else {
            panic!()
        };
        assert!(
            l.to.distance_to(Point3::new(0.0, 0.0, 10.0)) < 1e-9,
            "{l:?}"
        );
    }

    #[test]
    fn align_left_and_center() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("Circle 50,30 5").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Align left").unwrap();
        let b = g(&e, 2).bounding_box();
        assert!(b.min.x.abs() < 1e-6, "{b:?}");
        e.run_line("Align vcenter").unwrap();
        // Union v range: 0..35, centre 17.5.
        let b1 = g(&e, 1).bounding_box();
        let b2 = g(&e, 2).bounding_box();
        assert!((b1.center().y - b2.center().y).abs() < 1e-6);
        assert!(e.run_line("Align sideways").is_err());
    }

    #[test]
    fn flip_curves_and_meshes() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Dir").unwrap();
        let Geometry::Line(l) = g(&e, 1) else {
            panic!()
        };
        assert_eq!(l.from, Point3::new(10.0, 0.0, 0.0));
        let Geometry::Mesh(m) = g(&e, 2) else {
            panic!()
        };
        assert!(m.volume() < 0.0);
    }

    #[test]
    fn match_properties() {
        let mut e = Engine::new();
        e.run_line("Layer Muri").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("SetObjectColor rosso").unwrap();
        e.run_line("Layer Default").unwrap();
        e.run_line("Line 0,0 0,10").unwrap();
        e.run_line("SelNone").unwrap();
        e.run_line("Select #2").unwrap();
        e.run_line("MatchProperties #1").unwrap();
        let o = e.doc().object(ObjectId(2)).unwrap();
        assert_eq!(e.doc().layer(o.layer).name, "Muri");
        assert_eq!(o.color, Some([255, 0, 0]));
        assert!(e.run_line("MatchProperties #99").is_err());
    }

    #[test]
    fn bounding_box_box_and_rectangle() {
        let mut e = Engine::new();
        e.run_line("Sphere 0,0,0 10").unwrap();
        e.run_line("Line 20,0 30,5,7").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("BoundingBox").unwrap();
        let Geometry::Mesh(m) = g(&e, 3) else {
            panic!()
        };
        let b = m.bounding_box().unwrap();
        assert!((b.max.x - 30.0).abs() < 1e-6 && (b.min.z + 10.0).abs() < 1e-6);
        e.run_line("New").unwrap();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("BoundingBox").unwrap();
        let Geometry::Polyline(p) = g(&e, 2) else {
            panic!()
        };
        assert_eq!(p.len(), 5);
    }

    #[test]
    fn project_to_cplane() {
        let mut e = Engine::new();
        e.run_line("Line 0,0,5 10,0,20").unwrap();
        e.run_line("Circle 0,0,30 10").unwrap();
        e.run_line("Circle 0,0,0 10 1,0,0").unwrap();
        e.run_line("Point 3,4,5").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("ProjectToCPlane").unwrap();
        for o in e.doc().objects() {
            let b = o.geometry.bounding_box();
            assert!(b.min.z.abs() < 1e-9 && b.max.z.abs() < 1e-9, "{o:?}");
        }
        assert_eq!(g(&e, 2).kind(), "circle");
        assert_eq!(g(&e, 3).kind(), "nurbs");
        assert_eq!(g(&e, 4), Geometry::Point(Point3::new(3.0, 4.0, 0.0)));
        e.run_line("ProjectToCPlane 0,1,0 0,-5,0").unwrap();
        assert_eq!(g(&e, 4), Geometry::Point(Point3::new(3.0, -5.0, 0.0)));
    }
}
