//! More analysis: Angle, Radius, EvaluatePt (reports only) and AreaCentroid,
//! VolumeCentroid (add point objects).

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, ObjectId};
use forma_geom::{polygon_centroid, Point3};
use std::fmt::Write as _;

simple_command!(
    Angle,
    "Angle",
    &[],
    "Angle <vertex> <first point> <second point> — angle between two directions from a vertex"
);
impl Command for Angle {
    impl_meta!(Angle);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("angle vertex", ctx.last_point)?;
        let a = args.point("first reference point", Some(c))?;
        let b = args.point("second reference point", Some(c))?;
        let (u, v) = (a - c, b - c);
        if u.length() < 1e-12 || v.length() < 1e-12 {
            return Err(CommandError::Invalid(
                "a point coincides with the vertex".into(),
            ));
        }
        let ang = u.cross(v).length().atan2(u.dot(v)).to_degrees();
        Ok(format!("angle = {ang:.4}°"))
    }
}

simple_command!(
    Radius,
    "Radius",
    &["Diameter"],
    "Radius — radius and diameter of the selected arcs and circles"
);
impl Command for Radius {
    impl_meta!(Radius);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Radius")?;
        let unit = ctx.doc.units.abbreviation();
        let mut out = String::new();
        for id in ids {
            if let Geometry::Arc(a) = &ctx.doc.object(id).expect("selected").geometry {
                if !out.is_empty() {
                    out.push('\n');
                }
                let _ = write!(
                    out,
                    "#{}: radius = {:.6} {unit}  diameter = {:.6} {unit}",
                    id.0,
                    a.radius,
                    2.0 * a.radius
                );
            }
        }
        if out.is_empty() {
            return Err(CommandError::Invalid(
                "Radius: select arcs or circles".into(),
            ));
        }
        Ok(out)
    }
}

simple_command!(
    EvaluatePt,
    "EvaluatePt",
    &["PtCoord"],
    "EvaluatePt <point> — report the coordinates of a point"
);
impl Command for EvaluatePt {
    impl_meta!(EvaluatePt);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let p = args.point("point to evaluate", ctx.last_point)?;
        ctx.last_point = Some(p);
        Ok(format!("{:.6},{:.6},{:.6}", p.x, p.y, p.z))
    }
}

fn add_points(ctx: &mut Context, pts: Vec<Point3>) -> Vec<ObjectId> {
    let mut t = ctx.doc.begin();
    let ids: Vec<ObjectId> = pts.into_iter().map(|p| t.add(Geometry::Point(p))).collect();
    t.commit();
    ctx.selection = ids.iter().copied().collect();
    ids
}

simple_command!(
    AreaCentroid,
    "AreaCentroid",
    &[],
    "AreaCentroid — add a point at the area centroid of each selected closed planar curve or mesh"
);
impl Command for AreaCentroid {
    impl_meta!(AreaCentroid);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("AreaCentroid")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let mut pts = Vec::new();
        for id in ids {
            let g = &ctx.doc.object(id).expect("selected").geometry;
            let c = match g {
                Geometry::Mesh(m) => m.area_centroid(),
                Geometry::Arc(a) if a.is_closed() => Some(a.center()),
                g => crate::surfaces::closed_plane(g, tol)
                    .and_then(|_| polygon_centroid(&g.curve_points())),
            };
            pts.extend(c);
        }
        if pts.is_empty() {
            return Err(CommandError::Invalid(
                "AreaCentroid: select closed planar curves or meshes".into(),
            ));
        }
        let msg = pts
            .iter()
            .map(|p| format!("{:.6},{:.6},{:.6}", p.x, p.y, p.z))
            .collect::<Vec<_>>()
            .join("; ");
        add_points(ctx, pts);
        Ok(format!("area centroid: {msg}"))
    }
}

simple_command!(
    VolumeCentroid,
    "VolumeCentroid",
    &[],
    "VolumeCentroid — add a point at the volume centroid of each selected closed mesh"
);
impl Command for VolumeCentroid {
    impl_meta!(VolumeCentroid);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("VolumeCentroid")?;
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let pts: Vec<Point3> = ids
            .iter()
            .filter_map(|id| match &ctx.doc.object(*id)?.geometry {
                Geometry::Mesh(m) if m.is_closed(tol) => m.volume_centroid(),
                _ => None,
            })
            .collect();
        if pts.is_empty() {
            return Err(CommandError::Invalid(
                "VolumeCentroid: select closed meshes".into(),
            ));
        }
        let msg = pts
            .iter()
            .map(|p| format!("{:.6},{:.6},{:.6}", p.x, p.y, p.z))
            .collect::<Vec<_>>()
            .join("; ");
        add_points(ctx, pts);
        Ok(format!("volume centroid: {msg}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::Point3;

    #[test]
    fn angle_radius_evaluate() {
        let mut e = Engine::new();
        assert_eq!(
            e.run_line("Angle 0,0 10,0 0,5").unwrap(),
            "angle = 90.0000°"
        );
        assert_eq!(
            e.run_line("Angle 0,0 10,0 -10,10").unwrap(),
            "angle = 135.0000°"
        );
        assert!(e.run_line("Angle 0,0 0,0 1,1").is_err());
        e.run_line("Circle 0,0 12.5").unwrap();
        e.run_line("SelAll").unwrap();
        let r = e.run_line("Radius").unwrap();
        assert!(
            r.contains("radius = 12.500000 mm") && r.contains("diameter = 25.000000"),
            "{r}"
        );
        assert_eq!(
            e.run_line("EvaluatePt 1,2,3").unwrap(),
            "1.000000,2.000000,3.000000"
        );
        assert!(e.doc().len() == 1);
    }

    #[test]
    fn centroids() {
        let mut e = Engine::new();
        e.run_line("Rectangle 0,0 40,20").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("AreaCentroid").unwrap();
        let Geometry::Point(p) = e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(p.distance_to(Point3::new(20.0, 10.0, 0.0)) < 1e-9);
        e.run_line("Box 0,0,0 10,20 30").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("VolumeCentroid").unwrap();
        let Geometry::Point(p) = e.doc().objects().last().unwrap().geometry else {
            panic!()
        };
        assert!(p.distance_to(Point3::new(5.0, 10.0, 15.0)) < 1e-9);
        e.run_line("SelNone").unwrap();
        e.run_line("Line 0,0 1,1").unwrap();
        e.run_line("SelLast").unwrap();
        assert!(e.run_line("AreaCentroid").is_err());
        assert!(e.run_line("VolumeCentroid").is_err());
    }
}
