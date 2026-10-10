//! Selection, transforms, layers and file commands.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Document, LayerId, LengthUnit, ObjectId};
use forma_geom::{Vec3, Xform};

/// Parse `#id` tokens (the `#` is optional).
pub(crate) fn parse_ids(tokens: Vec<&str>) -> Result<Vec<ObjectId>, CommandError> {
    tokens
        .into_iter()
        .map(|t| {
            t.trim_start_matches('#')
                .parse()
                .map(ObjectId)
                .map_err(|_| CommandError::BadInput(t.to_string()))
        })
        .collect()
}

/// Apply `x` to the selection; `copy` adds transformed copies instead of moving.
pub(crate) fn transform_selection(
    ctx: &mut Context,
    cmd: &str,
    x: &Xform,
    copy: bool,
) -> CommandResult {
    let ids = ctx.selected(cmd)?;
    let mut t = ctx.doc.begin();
    for id in &ids {
        let obj = t.doc().object(*id).expect("selected exists").clone();
        let g = obj.geometry.transformed(x);
        if copy {
            t.add_like(g, &obj);
        } else {
            t.replace(*id, g);
        }
    }
    t.commit();
    Ok(format!(
        "{} {} object(s)",
        if copy { "copied" } else { "transformed" },
        ids.len()
    ))
}

simple_command!(
    Move,
    "Move",
    &["M"],
    "Move <from> <to> — move selected objects"
);
impl Command for Move {
    impl_meta!(Move);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("point to move from", ctx.last_point)?;
        let b = args.point("point to move to", Some(a))?;
        transform_selection(ctx, "Move", &Xform::translation(b - a), false)
    }
}

simple_command!(
    Copy,
    "Copy",
    &["Co", "Cp"],
    "Copy <from> <to> — copy selected objects"
);
impl Command for Copy {
    impl_meta!(Copy);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("point to copy from", ctx.last_point)?;
        let b = args.point("point to copy to", Some(a))?;
        transform_selection(ctx, "Copy", &Xform::translation(b - a), true)
    }
}

simple_command!(
    Rotate,
    "Rotate",
    &["Ro"],
    "Rotate <center> <angle°> [axis] — rotate selected objects"
);
impl Command for Rotate {
    impl_meta!(Rotate);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("center of rotation", ctx.last_point)?;
        let a = args.number("angle")?;
        let axis = args.optional_vector().unwrap_or(Vec3::Z);
        transform_selection(
            ctx,
            "Rotate",
            &Xform::rotation(c, axis, a.to_radians()),
            false,
        )
    }
}

simple_command!(
    Scale,
    "Scale",
    &["Sc"],
    "Scale <base point> <factor> — scale selected objects uniformly"
);
impl Command for Scale {
    impl_meta!(Scale);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("base point", ctx.last_point)?;
        let f = args.number("scale factor")?;
        if f.abs() < 1e-9 {
            return Err(CommandError::Invalid(
                "scale factor must not be zero".into(),
            ));
        }
        transform_selection(ctx, "Scale", &Xform::scale(c, f), false)
    }
}

simple_command!(
    Mirror,
    "Mirror",
    &["Mi"],
    "Mirror <start> <end> [plane normal] — mirrored copies across the line (in the plane)"
);
impl Command for Mirror {
    impl_meta!(Mirror);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let a = args.point("start of mirror plane", ctx.last_point)?;
        let b = args.point("end of mirror plane", Some(a))?;
        let n = args.optional_vector().unwrap_or(Vec3::Z);
        let mirror_normal = (b - a)
            .cross(n)
            .normalized()
            .ok_or_else(|| CommandError::Invalid("mirror line is degenerate".into()))?;
        transform_selection(ctx, "Mirror", &Xform::mirror(a, mirror_normal), true)
    }
}

simple_command!(
    Delete,
    "Delete",
    &["Del", "Erase"],
    "Delete [#id …] — delete selected (or listed) objects"
);
impl Command for Delete {
    impl_meta!(Delete);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let listed = parse_ids(args.rest())?;
        let ids = if listed.is_empty() {
            ctx.selected("Delete")?
        } else {
            listed
        };
        let mut t = ctx.doc.begin();
        for id in &ids {
            if !t.remove(*id) {
                return Err(CommandError::Invalid(format!("no object #{}", id.0)));
            }
        }
        t.commit();
        Ok(format!("deleted {}", ids.len()))
    }
}

simple_command!(
    SelAll,
    "SelAll",
    &[],
    "SelAll — select all visible, unlocked objects"
);
impl Command for SelAll {
    impl_meta!(SelAll);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let doc = &ctx.doc;
        ctx.selection = doc
            .objects()
            .filter(|o| doc.is_selectable(o))
            .map(|o| o.id)
            .collect();
        Ok(format!("{} selected", ctx.selection.len()))
    }
}

simple_command!(SelNone, "SelNone", &[], "SelNone — clear the selection");
impl Command for SelNone {
    impl_meta!(SelNone);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        ctx.selection.clear();
        Ok(String::new())
    }
}

simple_command!(
    Select,
    "Select",
    &["SelId"],
    "Select #id … — add objects (and the rest of their groups) to the selection"
);
impl Command for Select {
    impl_meta!(Select);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let ids = parse_ids(args.rest())?;
        for id in &ids {
            if ctx.doc.object(*id).is_none() {
                return Err(CommandError::Invalid(format!("no object #{}", id.0)));
            }
        }
        let ids = crate::select::with_groups(&ctx.doc, &ids);
        ctx.selection.extend(ids);
        Ok(format!("{} selected", ctx.selection.len()))
    }
}

pub(crate) fn layer_named(doc: &mut Document, name: &str) -> LayerId {
    doc.find_layer(name).unwrap_or_else(|| {
        // Pleasant distinct colours for new layers.
        const PALETTE: [[u8; 3]; 6] = [
            [200, 60, 60],
            [60, 140, 200],
            [70, 160, 80],
            [210, 150, 40],
            [150, 80, 190],
            [40, 170, 170],
        ];
        let c = PALETTE[doc.layers.len() % PALETTE.len()];
        doc.add_layer(name, c, true)
    })
}

simple_command!(
    LayerCmd,
    "Layer",
    &[],
    "Layer <name> — make a layer current (created if missing)"
);
impl Command for LayerCmd {
    impl_meta!(LayerCmd);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = args.rest().join(" ");
        if name.is_empty() {
            return Err(CommandError::MissingInput("layer name"));
        }
        let id = layer_named(&mut ctx.doc, &name);
        ctx.doc.current_layer = id;
        Ok(format!("current layer: {name}"))
    }
}

simple_command!(
    ChangeLayer,
    "ChangeLayer",
    &[],
    "ChangeLayer <name> — move selected objects to a layer"
);
impl Command for ChangeLayer {
    impl_meta!(ChangeLayer);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = args.rest().join(" ");
        if name.is_empty() {
            return Err(CommandError::MissingInput("layer name"));
        }
        let ids = ctx.selected("ChangeLayer")?;
        let layer = layer_named(&mut ctx.doc, &name);
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_layer(*id, layer);
        }
        t.commit();
        Ok(format!("{} object(s) moved to {name}", ids.len()))
    }
}

simple_command!(
    New,
    "New",
    &[],
    "New [cm|mm|m] — start an empty document (centimetres by default)"
);
impl Command for New {
    impl_meta!(New);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let units = match args.next_token().map(str::to_lowercase).as_deref() {
            Some("mm") => LengthUnit::Millimeters,
            None | Some("cm") => LengthUnit::Centimeters,
            Some("m") => LengthUnit::Meters,
            Some(u) => return Err(CommandError::BadInput(format!("unknown unit {u}"))),
        };
        let mut doc = Document::new();
        doc.units = units;
        doc.dim_style = forma_doc::DimStyle::for_units(units);
        doc.absolute_tolerance = match units {
            LengthUnit::Millimeters => 0.001,
            LengthUnit::Centimeters => 0.01,
            _ => 0.0001,
        };
        ctx.doc = doc;
        ctx.selection.clear();
        ctx.last_point = None;
        Ok(format!("new document ({})", units.abbreviation()))
    }
}

simple_command!(
    Save,
    "Save",
    &["SaveAs"],
    "Save [file.3dm] — save as a Rhino file"
);
impl Command for Save {
    impl_meta!(Save);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let typed = args.rest().join(" ");
        let path = if typed.trim().is_empty() {
            ctx.doc
                .path
                .clone()
                .ok_or(CommandError::MissingInput("file path"))?
        } else {
            typed.trim().trim_matches('"').to_string()
        };
        forma_io_3dm::write_document(&path, &ctx.doc)
            .map_err(|e| CommandError::Invalid(e.to_string()))?;
        ctx.doc.path = Some(path.clone());
        Ok(format!("saved {path}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn move_rotate_scale_mirror_copy_undo() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Move 0,0 0,5").unwrap();
        assert!(
            e.doc().dump().contains("0,5,0 -> 10,5,0"),
            "{}",
            e.doc().dump()
        );
        e.run_line("Rotate 0,5 90").unwrap();
        e.run_line("Scale 0,5 2").unwrap();
        let b = e.doc().objects().next().unwrap().geometry.bounding_box();
        assert!((b.max.y - 25.0).abs() < 1e-9, "{b:?}");
        e.run_line("Mirror 5,0 5,10").unwrap();
        e.run_line("Copy 0,0 100,0").unwrap();
        assert_eq!(e.doc().len(), 3); // original + mirror + copy
        e.run_line("Undo").unwrap();
        e.run_line("Undo").unwrap();
        assert_eq!(e.doc().len(), 1);
    }

    #[test]
    fn delete_selection_and_layers() {
        let mut e = Engine::new();
        e.run_line("Layer Arredi").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        assert_eq!(
            e.doc().layer(e.doc().objects().next().unwrap().layer).name,
            "Arredi"
        );
        e.run_line("SelAll").unwrap();
        e.run_line("ChangeLayer Muri").unwrap();
        assert_eq!(
            e.doc().layer(e.doc().objects().next().unwrap().layer).name,
            "Muri"
        );
        e.run_line("Delete").unwrap();
        assert!(e.doc().is_empty());
        assert!(e.ctx.selection.is_empty());
        assert!(e.run_line("Delete").is_err());
    }

    #[test]
    fn save_and_reopen() {
        let dir = std::env::temp_dir().join(format!("forma-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("prova.3dm");
        let p = path.to_str().unwrap();
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Layer muri::pareti").unwrap();
        e.run_line("Box 0,0 400,30 270").unwrap();
        e.run_line("Circle 0,0 50").unwrap();
        e.run_line("Polyline 0,0 100,0 100,100").unwrap();
        e.run_line("Line 0,0 0,0,100").unwrap();
        e.run_line("SelNone").unwrap();
        e.run_line("Select #3").unwrap();
        e.run_line("FilletCorners 10").unwrap();
        e.run_line("SetObjectColor 10,200,30").unwrap();
        e.run_line(&format!("Save {p}")).unwrap();
        let mut f = Engine::new();
        f.run_line(&format!("Open {p}")).unwrap();
        assert_eq!(f.doc().units.abbreviation(), "cm");
        assert_eq!(f.doc().len(), 4);
        assert!(
            f.doc().find_layer("muri::pareti").is_some(),
            "{:?}",
            f.doc().layers
        );
        let kinds: Vec<&str> = f.doc().objects().map(|o| o.geometry.kind()).collect();
        assert!(kinds.contains(&"mesh"), "{kinds:?}");
        assert_eq!(
            f.doc()
                .objects()
                .filter(|o| o.color == Some([10, 200, 30]))
                .count(),
            1
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
