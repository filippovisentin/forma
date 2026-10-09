//! Analysis commands: they report measurements and never change the document.
//! Distance, Length, Area, Volume, What.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::Geometry;
use forma_geom::{newell_area, Plane};
use std::fmt::Write as _;

/// Area of a closed planar curve (exact for circles), `None` otherwise.
fn curve_area(g: &Geometry, tol: f64) -> Option<f64> {
    if !g.is_closed_curve() {
        return None;
    }
    if let Geometry::Arc(a) = g {
        return Some(std::f64::consts::PI * a.radius * a.radius);
    }
    let pts = match g {
        Geometry::Nurbs(n) => n.sample(64),
        g => g.curve_points(),
    };
    let n = g.curve_normal()?;
    let plane = Plane::from_normal(pts[0], n);
    if pts.iter().any(|p| plane.coords(*p).2.abs() > tol) {
        return None;
    }
    Some(newell_area(&pts).length())
}

simple_command!(
    Distance,
    "Distance",
    &["Dist"],
    "Distance <p1> <p2> — distance and dx, dy, dz between two points"
);
impl Command for Distance {
    impl_meta!(Distance);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first point", ctx.last_point)?;
        let b = args.point("second point", Some(a))?;
        let d = b - a;
        Ok(format!(
            "distance = {:.6}  dx = {:.6}  dy = {:.6}  dz = {:.6}",
            d.length(),
            d.x,
            d.y,
            d.z
        ))
    }
}

simple_command!(
    Length,
    "Length",
    &["Len"],
    "Length — total length of the selected curves"
);
impl Command for Length {
    impl_meta!(Length);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Length")?;
        let mut total = 0.0;
        let mut n = 0;
        for id in ids {
            if let Some(l) = ctx.doc.object(id).and_then(|o| o.geometry.length()) {
                total += l;
                n += 1;
            }
        }
        if n == 0 {
            return Err(CommandError::Invalid("Length: select curves".into()));
        }
        Ok(format!(
            "length = {total:.6} {} ({n} curve(s))",
            ctx.doc.units.abbreviation()
        ))
    }
}

simple_command!(
    Area,
    "Area",
    &[],
    "Area — area of the selected closed planar curves and meshes"
);
impl Command for Area {
    impl_meta!(Area);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Area")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        let mut total = 0.0;
        let mut n = 0;
        for id in ids {
            let g = &ctx.doc.object(id).expect("selected").geometry;
            let a = match g {
                Geometry::Mesh(m) => Some(m.area()),
                g => curve_area(g, tol),
            };
            if let Some(a) = a {
                total += a;
                n += 1;
            }
        }
        if n == 0 {
            return Err(CommandError::Invalid(
                "Area: select closed planar curves or meshes".into(),
            ));
        }
        Ok(format!(
            "area = {total:.6} {}² ({n} object(s))",
            ctx.doc.units.abbreviation()
        ))
    }
}

simple_command!(
    Volume,
    "Volume",
    &["Vol"],
    "Volume — volume of the selected closed meshes"
);
impl Command for Volume {
    impl_meta!(Volume);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("Volume")?;
        let tol = ctx.tolerance.absolute.max(1e-9);
        let mut total = 0.0;
        let mut n = 0;
        for id in ids {
            if let Geometry::Mesh(m) = &ctx.doc.object(id).expect("selected").geometry {
                if m.is_closed(tol) {
                    total += m.volume().abs();
                    n += 1;
                }
            }
        }
        if n == 0 {
            return Err(CommandError::Invalid("Volume: select closed meshes".into()));
        }
        Ok(format!(
            "volume = {total:.6} {}³ ({n} object(s))",
            ctx.doc.units.abbreviation()
        ))
    }
}

simple_command!(
    What,
    "What",
    &[],
    "What — describe the selected objects (type, layer, attributes, bounding box)"
);
impl Command for What {
    impl_meta!(What);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("What")?;
        let mut out = String::new();
        for id in ids {
            let o = ctx.doc.object(id).expect("selected");
            let b = o.geometry.bounding_box();
            let _ = write!(
                out,
                "#{} {} on layer {}",
                o.id.0,
                o.geometry.kind(),
                ctx.doc.layer(o.layer).name
            );
            match &o.geometry {
                Geometry::Nurbs(n) => {
                    let _ = write!(
                        out,
                        ", degree {}, {} control points",
                        n.degree,
                        n.points.len()
                    );
                }
                Geometry::Polyline(p) => {
                    let _ = write!(out, ", {} points", p.len());
                }
                Geometry::Mesh(m) => {
                    let _ = write!(
                        out,
                        ", {} vertices, {} triangles{}",
                        m.positions.len(),
                        m.triangles.len(),
                        if m.is_closed(ctx.tolerance.absolute.max(1e-9)) {
                            ", closed"
                        } else {
                            ", open"
                        }
                    );
                }
                _ => {}
            }
            if let Some(c) = o.color {
                let _ = write!(out, ", colour {},{},{}", c[0], c[1], c[2]);
            }
            if let Some(g) = o.group {
                let _ = write!(out, ", group {g}");
            }
            if o.locked {
                out.push_str(", locked");
            }
            let _ = writeln!(out, "; box {} .. {}", b.min, b.max);
        }
        Ok(out.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use std::f64::consts::PI;

    /// First number after `key = ` in a report.
    fn value(s: &str, key: &str) -> f64 {
        let rest = &s[s.find(&format!("{key} = ")).unwrap() + key.len() + 3..];
        rest.split_whitespace().next().unwrap().parse().unwrap()
    }

    #[test]
    fn distance_length_area_volume() {
        let mut e = Engine::new();
        let r = e.run_line("Distance 0,0,0 3,4,12").unwrap();
        assert!((value(&r, "distance") - 13.0).abs() < 1e-9);
        assert!((value(&r, "dz") - 12.0).abs() < 1e-9);
        assert!(e.doc().is_empty());
        e.run_line("Line 0,0 100,0").unwrap();
        e.run_line("Circle 0,0 10").unwrap();
        e.run_line("SelAll").unwrap();
        let r = e.run_line("Length").unwrap();
        assert!(
            (value(&r, "length") - (100.0 + 20.0 * PI)).abs() < 1e-5,
            "{r}"
        );
        let r = e.run_line("Area").unwrap();
        assert!((value(&r, "area") - 100.0 * PI).abs() < 1e-5, "{r}");
        assert!(e.run_line("Volume").is_err());
        e.run_line("New").unwrap();
        e.run_line("Ellipse 0,0 10,0 0,4").unwrap();
        e.run_line("Rectangle 0,0 30,20").unwrap();
        e.run_line("Box 0,0 10,20 30").unwrap();
        e.run_line("SelAll").unwrap();
        let r = e.run_line("Area").unwrap();
        let expected = PI * 40.0 + 600.0 + 2.0 * (200.0 + 300.0 + 600.0);
        assert!(
            (value(&r, "area") - expected).abs() / expected < 1e-3,
            "{r}"
        );
        let r = e.run_line("Volume").unwrap();
        assert!((value(&r, "volume") - 6000.0).abs() < 1e-6, "{r}");
        assert_eq!(e.doc().len(), 3);
    }

    #[test]
    fn what_describes_objects() {
        let mut e = Engine::new();
        e.run_line("Curve 0,0 10,10 20,0 30,10").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Group").unwrap();
        let r = e.run_line("What").unwrap();
        assert!(
            r.contains("#1 nurbs on layer Default, degree 3, 4 control points"),
            "{r}"
        );
        assert!(r.contains("#2 mesh"), "{r}");
        assert!(r.contains("closed"), "{r}");
        assert!(r.contains("group 1"), "{r}");
    }
}
