//! Annotations: Text, TextDot, Dim (linear), DimAligned, DimRadius, DimDiameter,
//! DimAngle, Leader, Hatch, DimStyle. Heights and decimals come from the
//! document's dim style.

use crate::create::{finish, positive};
use crate::edit::parse_ids;
use crate::{parse_point, Args, Command, CommandError, CommandResult, Context};
use forma_doc::Geometry;
use forma_geom::{DimKind, Dimension, Plane, Text, Vec3};

fn add(ctx: &mut Context, g: Geometry) -> CommandResult {
    let what = match &g {
        Geometry::Dimension(d) => format!("dimension {}", d.text()),
        g => g.kind().to_string(),
    };
    let ids = finish(ctx, vec![g], false)?;
    Ok(format!("added #{} {what}", ids[0].0))
}

simple_command!(
    TextCmd,
    "Text",
    &["Txt"],
    "Text <origin> <height|*> [normal] <text…> — single-line text in the construction plane (origin = bottom left; * = the DimStyle height)"
);
impl Command for TextCmd {
    impl_meta!(TextCmd);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let o = args.point("text location", ctx.last_point)?;
        let h = if args.keyword("*") {
            ctx.doc.dim_style.text_height
        } else {
            positive(args.number("text height")?, "text height", ctx)?.abs()
        };
        // An optional normal before the text: a token that parses as a vector.
        let n = match args.peek() {
            Some(t) if t.contains(',') && parse_point(t, None).is_ok() => {
                args.optional_vector().unwrap_or(Vec3::Z)
            }
            _ => Vec3::Z,
        };
        let text = args.rest().join(" ");
        if text.is_empty() {
            return Err(CommandError::MissingInput("text"));
        }
        ctx.last_point = Some(o);
        add(
            ctx,
            Geometry::Text(Text::new(Plane::from_normal(o, n), &text, h, false)),
        )
    }
}

simple_command!(
    TextDot,
    "Dot",
    &["TextDot"],
    "Dot <point> <text…> — text dot: a label at a point, always facing the screen"
);
impl Command for TextDot {
    impl_meta!(TextDot);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let p = args.point("dot location", ctx.last_point)?;
        let text = args.rest().join(" ");
        if text.is_empty() {
            return Err(CommandError::MissingInput("text"));
        }
        ctx.last_point = Some(p);
        add(
            ctx,
            Geometry::Text(Text::new(Plane::TOP.moved_to(p), &text, 14.0, true)),
        )
    }
}

/// Style values for a new dimension.
fn style(ctx: &Context) -> (f64, usize) {
    (ctx.doc.dim_style.text_height, ctx.doc.dim_style.decimals)
}

simple_command!(
    Dim,
    "Dim",
    &["DimLinear"],
    "Dim <first point> <second point> <dimension line point> [normal] — horizontal or vertical dimension (direction from where the line point lies)"
);
impl Command for Dim {
    impl_meta!(Dim);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first dimension point", ctx.last_point)?;
        let b = args.point("second dimension point", Some(a))?;
        let l = args.point("dimension line location", Some(b))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let (h, dec) = style(ctx);
        let plane = Plane::from_normal(a, n);
        let d = Dimension::linear(&plane, a, b, l, h, dec);
        positive(d.value(), "dimension", ctx)?;
        ctx.last_point = Some(l);
        add(ctx, Geometry::Dimension(d))
    }
}

simple_command!(
    DimAligned,
    "DimAligned",
    &[],
    "DimAligned <first point> <second point> <dimension line point> [normal] — dimension parallel to the two points"
);
impl Command for DimAligned {
    impl_meta!(DimAligned);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("first dimension point", ctx.last_point)?;
        let b = args.point("second dimension point", Some(a))?;
        let l = args.point("dimension line location", Some(b))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let (h, dec) = style(ctx);
        let d = Dimension::aligned(n, a, b, l, h, dec).ok_or_else(|| {
            CommandError::Invalid("the points coincide (seen along the normal)".into())
        })?;
        positive(d.value(), "dimension", ctx)?;
        ctx.last_point = Some(l);
        add(ctx, Geometry::Dimension(d))
    }
}

/// Radius / diameter dimension of an arc or circle (listed `#id` or the single
/// selected one).
fn dim_round(ctx: &mut Context, args: &mut Args, kind: DimKind, cmd: &str) -> CommandResult {
    let l = args.point("dimension location", ctx.last_point)?;
    let listed = parse_ids(args.rest())?;
    let ids = if listed.is_empty() {
        ctx.selected(cmd)?
    } else {
        listed
    };
    let arc = ids
        .iter()
        .find_map(|id| match ctx.doc.object(*id).map(|o| &o.geometry) {
            Some(Geometry::Arc(a)) => Some(*a),
            _ => None,
        })
        .ok_or_else(|| CommandError::Invalid(format!("{cmd}: select an arc or circle")))?;
    let c = arc.center();
    let pl = arc.plane;
    // Point on the circle towards the location (in the arc plane).
    let dir = forma_geom::in_plane(l - c, pl.z).unwrap_or(pl.x);
    let on = c + dir * arc.radius;
    let (h, dec) = style(ctx);
    let d = Dimension {
        kind,
        plane: Plane { origin: c, ..pl },
        points: vec![c, on, pl.project(l)],
        text: None,
        height: h,
        decimals: dec,
    };
    ctx.last_point = Some(l);
    add(ctx, Geometry::Dimension(d))
}

simple_command!(
    DimRadius,
    "DimRadius",
    &[],
    "DimRadius <location> [#curve] — radius dimension of an arc or circle (listed or selected)"
);
impl Command for DimRadius {
    impl_meta!(DimRadius);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        dim_round(ctx, args, DimKind::Radius, "DimRadius")
    }
}

simple_command!(
    DimDiameter,
    "DimDiameter",
    &[],
    "DimDiameter <location> [#curve] — diameter dimension of an arc or circle (listed or selected)"
);
impl Command for DimDiameter {
    impl_meta!(DimDiameter);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        dim_round(ctx, args, DimKind::Diameter, "DimDiameter")
    }
}

simple_command!(
    DimAngle,
    "DimAngle",
    &[],
    "DimAngle <vertex> <point on first ray> <point on second ray> <arc location> [normal] — angle dimension"
);
impl Command for DimAngle {
    impl_meta!(DimAngle);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("angle vertex", ctx.last_point)?;
        let a = args.point("point on first ray", Some(c))?;
        let b = args.point("point on second ray", Some(c))?;
        let m = args.point("dimension arc location", Some(c))?;
        let n = args
            .optional_vector()
            .or_else(|| (a - c).cross(b - c).normalized())
            .unwrap_or(Vec3::Z);
        let n = if n.dot(Vec3::Z) < -1e-9 { -n } else { n };
        let (h, dec) = style(ctx);
        let d = Dimension {
            kind: DimKind::Angle,
            plane: Plane::from_normal(c, n),
            points: vec![c, a, b, m],
            text: None,
            height: h,
            decimals: dec,
        };
        if d.value() <= 1e-9 || d.lines().is_empty() {
            return Err(CommandError::Invalid("degenerate angle".into()));
        }
        ctx.last_point = Some(m);
        add(ctx, Geometry::Dimension(d))
    }
}

simple_command!(
    DimStyle,
    "DimStyle",
    &[],
    "DimStyle [text height] [decimals] — show or set the text height and decimals of new annotations"
);
impl Command for DimStyle {
    impl_meta!(DimStyle);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let h = args.optional_number();
        let d = args.optional_number();
        if let Some(h) = h {
            positive(h, "text height", ctx)?;
        }
        if let Some(d) = d {
            if !(0.0..=8.0).contains(&d) || d.fract().abs() > 1e-9 {
                return Err(CommandError::Invalid("decimals must be 0 to 8".into()));
            }
        }
        if let Some(h) = h {
            ctx.doc.dim_style.text_height = h.abs();
        }
        if let Some(d) = d {
            ctx.doc.dim_style.decimals = d as usize;
        }
        let s = ctx.doc.dim_style;
        Ok(format!(
            "text height {} {}, {} decimal(s)",
            s.text_height,
            ctx.doc.units.abbreviation(),
            s.decimals
        ))
    }
}

simple_command!(
    Leader,
    "Leader",
    &[],
    "Leader <arrow point> <point> [point…] <text…> — arrow polyline with a text at its end (DimStyle height)"
);
impl Command for Leader {
    impl_meta!(Leader);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let mut pts = vec![args.point("start of leader (arrow)", ctx.last_point)?];
        while let Some(t) = args.peek() {
            match parse_point(t, pts.last().copied()) {
                Ok(p) => {
                    args.next_token();
                    pts.push(p);
                }
                Err(_) => break,
            }
        }
        if pts.len() < 2 {
            return Err(CommandError::MissingInput("next point of leader"));
        }
        let text = args.rest().join(" ");
        if text.is_empty() {
            return Err(CommandError::MissingInput("leader text"));
        }
        let tol = ctx.tolerance.absolute;
        pts.dedup_by(|a, b| a.distance_to(*b) <= tol);
        if pts.len() < 2 {
            return Err(CommandError::Invalid("leader is degenerate".into()));
        }
        let n = forma_geom::newell_area(&pts)
            .normalized()
            .filter(|n| n.dot(Vec3::Z).abs() > 1e-9 || pts.len() > 2)
            .map_or(Vec3::Z, |n| if n.dot(Vec3::Z) < 0.0 { -n } else { n });
        let (h, _) = style(ctx);
        ctx.last_point = pts.last().copied();
        let d = Dimension::leader(&Plane::from_normal(pts[0], n), pts, &text, h);
        add(ctx, Geometry::Dimension(d))
    }
}

simple_command!(
    Hatch,
    "Hatch",
    &[],
    "Hatch <spacing> [angle°=45] — hatch lines (one group) inside the selected closed planar curves; curves inside others make holes"
);
impl Command for Hatch {
    impl_meta!(Hatch);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let spacing = positive(args.number("hatch spacing")?, "spacing", ctx)?.abs();
        let angle = args.optional_number().unwrap_or(45.0);
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let ids = ctx.selected("Hatch")?;
        let mut loops = Vec::new();
        let mut plane: Option<Plane> = None;
        for id in &ids {
            let g = &ctx.doc.object(*id).expect("selected").geometry;
            let Some((pl, _)) = crate::surfaces::closed_plane(g, tol) else {
                continue;
            };
            let pts = crate::surfaces::loop_points(g);
            let p = *plane.get_or_insert(pl);
            if p.z.cross(pl.z).length() > 1e-6 || pts.iter().any(|q| p.coords(*q).2.abs() > tol) {
                return Err(CommandError::Invalid(
                    "Hatch: the curves are not in the same plane".into(),
                ));
            }
            loops.push(pts);
        }
        let Some(plane) = plane else {
            return Err(CommandError::Invalid(
                "Hatch: select closed planar curves".into(),
            ));
        };
        // Hatch lines run along the world x axis when the plane allows it.
        let x = forma_geom::in_plane(Vec3::X, plane.z)
            .or_else(|| forma_geom::in_plane(Vec3::Y, plane.z))
            .unwrap_or(plane.x);
        let frame = Plane {
            origin: plane.project(forma_geom::Point3::ORIGIN),
            x,
            y: plane.z.cross(x),
            z: plane.z,
        };
        let segs = forma_geom::hatch_lines(&loops, &frame, spacing, angle.to_radians());
        if segs.is_empty() {
            return Err(CommandError::Invalid("Hatch: no hatch lines".into()));
        }
        if segs.len() > 20_000 {
            return Err(CommandError::Invalid(
                "Hatch: too many lines, use a larger spacing".into(),
            ));
        }
        let group = ctx
            .doc
            .objects()
            .filter_map(|o| o.group)
            .max()
            .map_or(1, |m| m + 1);
        let n = segs.len();
        let mut t = ctx.doc.begin();
        let new: Vec<_> = segs
            .into_iter()
            .map(|[a, b]| t.add(Geometry::Line(forma_geom::LineCurve::new(a, b))))
            .collect();
        for id in &new {
            t.set_group(*id, Some(group));
        }
        t.commit();
        ctx.selection = new.into_iter().collect();
        Ok(format!("hatch: {n} line(s) in group {group}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use forma_doc::Geometry;
    use forma_geom::{DimKind, Point3};

    fn last(e: &Engine) -> Geometry {
        e.doc().objects().last().unwrap().geometry.clone()
    }

    #[test]
    fn text_and_dots() {
        let mut e = Engine::new();
        e.run_line("Text 10,20 5 Soggiorno 25 m²").unwrap();
        let Geometry::Text(t) = last(&e) else {
            panic!()
        };
        assert_eq!(t.text, "Soggiorno 25 m²");
        assert!((t.height - 5.0).abs() < 1e-12 && !t.dot);
        e.run_line("Text 0,0,0 5 0,-1,0 Prospetto").unwrap();
        let Geometry::Text(t) = last(&e) else {
            panic!()
        };
        assert_eq!(t.text, "Prospetto");
        assert!((t.plane.z.y + 1.0).abs() < 1e-12);
        e.run_line("Dot 1,2,3 A").unwrap();
        assert_eq!(last(&e).kind(), "textdot");
        assert!(e.run_line("Text 0,0 5").is_err());
        assert!(e.run_line("Dot 0,0").is_err());
        let (p, text, _) = last(&e).label().unwrap();
        assert_eq!(text, "A");
        assert!(p.distance_to(Point3::new(1.0, 2.0, 3.0)) < 1e-12);
    }

    #[test]
    fn dimensions_in_a_cm_document() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Dim 0,0 120,45 60,-30").unwrap();
        let g = last(&e);
        assert_eq!(g.label().unwrap().1, "120");
        assert!((g.label().unwrap().2 - 10.0).abs() < 1e-12);
        e.run_line("Dim 0,0 120,45 150,20").unwrap();
        assert_eq!(last(&e).label().unwrap().1, "45");
        e.run_line("DimAligned 0,0 30,40 -10,10").unwrap();
        assert_eq!(last(&e).label().unwrap().1, "50");
        e.run_line("DimStyle 5 2").unwrap();
        e.run_line("DimAligned 0,0 10,10 0,10").unwrap();
        assert_eq!(last(&e).label().unwrap().1, "14.14");
        e.run_line("Circle 0,0 25").unwrap();
        e.run_line("DimRadius 40,40 #5").unwrap();
        assert_eq!(last(&e).label().unwrap().1, "R25");
        e.run_line("SelNone").unwrap();
        e.run_line("Select #5").unwrap();
        e.run_line("DimDiameter 40,-40").unwrap();
        assert_eq!(last(&e).label().unwrap().1, "Ø50");
        e.run_line("DimAngle 0,0 10,0 10,10 20,5").unwrap();
        let Geometry::Dimension(d) = last(&e) else {
            panic!()
        };
        assert_eq!(d.kind, DimKind::Angle);
        assert_eq!(d.text(), "45°");
        assert!(e.run_line("Dim 0,0 0,0 5,5").is_err());
        assert!(e.run_line("DimStyle 0").is_err());
        // Moving a dimension moves its label; the dump lists it.
        e.run_line("SelNone").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("Scale 0,0 2").unwrap();
        assert!(e
            .doc()
            .dump()
            .contains("#1 dimension [Default] Linear \"240\""));
    }

    #[test]
    fn leader_hatch_and_default_text_height() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Leader 0,0 50,50 80,50 Rovere naturale")
            .unwrap();
        let Geometry::Dimension(d) = last(&e) else {
            panic!()
        };
        assert_eq!(d.kind, DimKind::Leader);
        assert_eq!(d.points.len(), 3);
        assert_eq!(d.text(), "Rovere naturale");
        assert!(e.run_line("Leader 0,0 Testo").is_err());
        assert!(e.run_line("Leader 0,0 10,0").is_err());
        e.run_line("Text 0,0 * Bagno").unwrap();
        let Geometry::Text(t) = last(&e) else {
            panic!()
        };
        assert!((t.height - 10.0).abs() < 1e-12);
        // DimStyle leaves the style alone when one value is wrong.
        assert!(e.run_line("DimStyle 3 1.5").is_err());
        assert!((e.doc().dim_style.text_height - 10.0).abs() < 1e-12);
        e.run_line("SelNone").unwrap();
        e.run_line("Rectangle 0,0 100,100").unwrap();
        e.run_line("Rectangle 40,40 60,60").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("Select #3").unwrap();
        e.run_line("Hatch 10 0").unwrap();
        let lines: Vec<_> = e
            .doc()
            .objects()
            .filter(|o| matches!(o.geometry, Geometry::Line(_)))
            .collect();
        // y = 0, 10, …, 90; y = 40 and 50 are cut by the hole.
        assert_eq!(lines.len(), 12);
        assert!(lines
            .iter()
            .all(|o| o.group == lines[0].group && o.group.is_some()));
        let total: f64 = lines.iter().filter_map(|o| o.geometry.length()).sum();
        assert!((total - 960.0).abs() < 1e-6, "{total}");
        e.run_line("Undo").unwrap();
        assert!(e.run_line("Hatch 0").is_err());
    }

    #[test]
    fn annotations_survive_save_as_lines_and_dots() {
        let dir = std::env::temp_dir().join(format!("forma-annot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quote.3dm");
        let p = path.to_str().unwrap();
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Dim 0,0 400,0 200,-50").unwrap();
        e.run_line("Text 0,0 10 Cucina").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelLast").unwrap();
        e.run_line("SetObjectName tavolo").unwrap();
        e.run_line(&format!("Save {p}")).unwrap();
        let mut f = Engine::new();
        f.run_line(&format!("Open {p}")).unwrap();
        // 7 dimension lines + the box; dots are not read back yet.
        let lines = f.doc().objects().filter(|o| o.geometry.is_curve()).count();
        assert_eq!(lines, 7);
        assert!(f
            .doc()
            .objects()
            .any(|o| o.name.as_deref() == Some("tavolo")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
