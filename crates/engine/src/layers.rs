//! Object names, selection filters and layer management: SetObjectName, SelName,
//! SelLayer, SelColor, SelDup, SelOpenCrv, SelClosedCrv, SelPolyline, SelLine,
//! SelText, SelDim, Purge, RenameLayer, DeleteLayer, OneLayerOn, AllLayersOn,
//! ChangeToCurrentLayer, CopyObjectsToLayer.

use crate::attrs::parse_color;
use crate::edit::layer_named;
use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::{Geometry, LayerId, Object, ObjectId};

/// Simple wildcard match: `*` matches any run of characters (case-insensitive).
fn glob(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.to_lowercase(), text.to_lowercase());
    let parts: Vec<&str> = p.split('*').collect();
    if parts.len() == 1 {
        return p == t;
    }
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !t.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            return t.len() >= pos + part.len() && t[pos..].ends_with(part);
        } else {
            match t[pos..].find(part) {
                Some(k) => pos += k + part.len(),
                None => return false,
            }
        }
    }
    true
}

/// Add every selectable object matching `f` to the selection.
fn select_objects(ctx: &mut Context, f: impl Fn(&Object) -> bool) -> CommandResult {
    let doc = &ctx.doc;
    let ids: Vec<ObjectId> = doc
        .objects()
        .filter(|o| doc.is_selectable(o) && f(o))
        .map(|o| o.id)
        .collect();
    let n = ids.len();
    ctx.selection.extend(ids);
    Ok(format!("{n} added to the selection"))
}

/// The rest of the line as a name (`""` means none).
fn name_arg(args: &mut Args, what: &'static str) -> Result<String, CommandError> {
    let name = args.rest().join(" ");
    if name.is_empty() {
        return Err(CommandError::MissingInput(what));
    }
    Ok(name)
}

fn layer_arg(ctx: &Context, args: &mut Args) -> Result<LayerId, CommandError> {
    let name = name_arg(args, "layer name")?;
    ctx.doc
        .find_layer(&name)
        .ok_or_else(|| CommandError::Invalid(format!("no layer {name}")))
}

simple_command!(
    SetObjectName,
    "SetObjectName",
    &["Name"],
    "SetObjectName <name> — name the selected objects (\"\" clears the name)"
);
impl Command for SetObjectName {
    impl_meta!(SetObjectName);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = name_arg(args, "object name")?;
        let name = (name != "\"\"").then_some(name);
        let ids = ctx.selected("SetObjectName")?;
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_name(*id, name.clone());
        }
        t.commit();
        Ok(format!("named {} object(s)", ids.len()))
    }
}

simple_command!(
    SelName,
    "SelName",
    &[],
    "SelName <name> — select objects by name (* wildcards allowed)"
);
impl Command for SelName {
    impl_meta!(SelName);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let pattern = name_arg(args, "object name")?;
        select_objects(ctx, |o| {
            o.name.as_deref().is_some_and(|n| glob(&pattern, n))
        })
    }
}

simple_command!(
    SelLayer,
    "SelLayer",
    &[],
    "SelLayer <layer> — select all objects on a layer"
);
impl Command for SelLayer {
    impl_meta!(SelLayer);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let id = layer_arg(ctx, args)?;
        select_objects(ctx, |o| o.layer == id)
    }
}

simple_command!(
    SelColor,
    "SelColor",
    &[],
    "SelColor <r,g,b | #rrggbb | name> — select objects displayed in a colour"
);
impl Command for SelColor {
    impl_meta!(SelColor);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = parse_color(
            args.next_token()
                .ok_or(CommandError::MissingInput("colour"))?,
        )?;
        let doc = ctx.doc.clone();
        select_objects(ctx, |o| doc.display_color(o) == c)
    }
}

/// Points describing an object for duplicate detection.
fn signature(g: &Geometry) -> Vec<forma_geom::Point3> {
    match g {
        Geometry::Mesh(m) => m.positions.clone(),
        Geometry::Point(p) => vec![*p],
        g => g.curve_points(),
    }
}

simple_command!(
    SelDup,
    "SelDup",
    &[],
    "SelDup — select objects that duplicate an earlier object (same type and points within tolerance)"
);
impl Command for SelDup {
    impl_meta!(SelDup);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let tol = ctx.doc.absolute_tolerance.max(1e-9);
        let objs: Vec<(ObjectId, &'static str, Vec<forma_geom::Point3>)> = ctx
            .doc
            .objects()
            .filter(|o| ctx.doc.is_visible(o))
            .map(|o| (o.id, o.geometry.kind(), signature(&o.geometry)))
            .collect();
        let mut dups = Vec::new();
        for (i, (id, kind, pts)) in objs.iter().enumerate() {
            let same = objs[..i].iter().any(|(_, k, q)| {
                k == kind
                    && q.len() == pts.len()
                    && q.iter().zip(pts).all(|(a, b)| a.distance_to(*b) <= tol)
            });
            if same {
                dups.push(*id);
            }
        }
        let doc = &ctx.doc;
        dups.retain(|id| doc.object(*id).is_some_and(|o| doc.is_selectable(o)));
        let n = dups.len();
        ctx.selection.extend(dups);
        Ok(format!("{n} duplicate(s) selected"))
    }
}

simple_command!(
    SelOpenCrv,
    "SelOpenCrv",
    &[],
    "SelOpenCrv — select all open curves"
);
impl Command for SelOpenCrv {
    impl_meta!(SelOpenCrv);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_objects(ctx, |o| {
            o.geometry.is_curve() && !o.geometry.is_closed_curve()
        })
    }
}

simple_command!(
    SelClosedCrv,
    "SelClosedCrv",
    &[],
    "SelClosedCrv — select all closed curves"
);
impl Command for SelClosedCrv {
    impl_meta!(SelClosedCrv);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_objects(ctx, |o| o.geometry.is_closed_curve())
    }
}

simple_command!(
    SelPolyline,
    "SelPolyline",
    &[],
    "SelPolyline — select all polylines"
);
impl Command for SelPolyline {
    impl_meta!(SelPolyline);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_objects(ctx, |o| matches!(o.geometry, Geometry::Polyline(_)))
    }
}

simple_command!(SelLine, "SelLine", &[], "SelLine — select all lines");
impl Command for SelLine {
    impl_meta!(SelLine);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_objects(ctx, |o| matches!(o.geometry, Geometry::Line(_)))
    }
}

simple_command!(
    SelText,
    "SelText",
    &["SelDot"],
    "SelText — select all text and text dots"
);
impl Command for SelText {
    impl_meta!(SelText);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_objects(ctx, |o| matches!(o.geometry, Geometry::Text(_)))
    }
}

simple_command!(SelDim, "SelDim", &[], "SelDim — select all dimensions");
impl Command for SelDim {
    impl_meta!(SelDim);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        select_objects(ctx, |o| matches!(o.geometry, Geometry::Dimension(_)))
    }
}

/// Layer ids of the layer path `name` and its sub-layers (the path itself need
/// not be a layer: `arredi` finds `arredi::sedie`).
fn layers_under(ctx: &Context, name: &str) -> Vec<LayerId> {
    let prefix = format!("{name}::");
    ctx.doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.name == name || l.name.starts_with(&prefix))
        .map(|(i, _)| LayerId(i))
        .collect()
}

simple_command!(
    Purge,
    "Purge",
    &[],
    "Purge — remove empty layers (not the current layer or parents of used layers)"
);
impl Command for Purge {
    impl_meta!(Purge);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let doc = &ctx.doc;
        let used: Vec<bool> = (0..doc.layers.len())
            .map(|i| doc.objects().any(|o| o.layer.0 == i))
            .collect();
        let current = &doc.layer(doc.current_layer).name;
        let keep_name = |name: &str| {
            let prefix = format!("{name}::");
            current == name
                || current.starts_with(&prefix)
                || doc
                    .layers
                    .iter()
                    .enumerate()
                    .any(|(i, l)| used[i] && l.name.starts_with(&prefix))
        };
        let remove: Vec<LayerId> = doc
            .layers
            .iter()
            .enumerate()
            .filter(|(i, l)| !used[*i] && !keep_name(&l.name))
            .map(|(i, _)| LayerId(i))
            .collect();
        let names = ctx
            .doc
            .remove_layers(&remove)
            .map_err(CommandError::Invalid)?;
        Ok(format!("purged {} layer(s)", names.len()))
    }
}

simple_command!(
    RenameLayer,
    "RenameLayer",
    &["LayerRename"],
    "RenameLayer <old name> <new name> — rename a layer (and the paths of its sub-layers); names without spaces"
);
impl Command for RenameLayer {
    impl_meta!(RenameLayer);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let old = args
            .next_token()
            .ok_or(CommandError::MissingInput("layer name"))?;
        let new = args
            .next_token()
            .ok_or(CommandError::MissingInput("new layer name"))?;
        let ids = layers_under(ctx, old);
        if ids.is_empty() {
            return Err(CommandError::Invalid(format!("no layer {old}")));
        }
        if !layers_under(ctx, new).is_empty() {
            return Err(CommandError::Invalid(format!("layer {new} already exists")));
        }
        let prefix = format!("{old}::");
        for id in &ids {
            ctx.doc.edit_layer(*id, |l| {
                l.name = match l.name.strip_prefix(&prefix) {
                    Some(rest) => format!("{new}::{rest}"),
                    None => new.to_string(),
                };
            });
        }
        Ok(format!(
            "{} layer(s) renamed from {old} to {new}",
            ids.len()
        ))
    }
}

simple_command!(
    DeleteLayer,
    "DeleteLayer",
    &["LayerDelete"],
    "DeleteLayer <layer> — delete a layer, its sub-layers and their objects (objects undoable, the layer is not)"
);
impl Command for DeleteLayer {
    impl_meta!(DeleteLayer);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = name_arg(args, "layer name")?;
        let layers = layers_under(ctx, &name);
        if layers.is_empty() {
            return Err(CommandError::Invalid(format!("no layer {name}")));
        }
        if layers.contains(&ctx.doc.current_layer) {
            return Err(CommandError::Invalid(
                "cannot delete the current layer; make another layer current first".into(),
            ));
        }
        let ids: Vec<ObjectId> = ctx
            .doc
            .objects()
            .filter(|o| layers.contains(&o.layer))
            .map(|o| o.id)
            .collect();
        let mut t = ctx.doc.begin();
        for oid in &ids {
            t.remove(*oid);
        }
        t.commit();
        let names = ctx
            .doc
            .remove_layers(&layers)
            .map_err(CommandError::Invalid)?;
        Ok(format!(
            "deleted {} layer(s) and {} object(s)",
            names.len(),
            ids.len()
        ))
    }
}

simple_command!(
    OneLayerOn,
    "OneLayerOn",
    &[],
    "OneLayerOn <layer> — show only this layer (and its parents and sub-layers); it becomes current"
);
impl Command for OneLayerOn {
    impl_meta!(OneLayerOn);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let id = layer_arg(ctx, args)?;
        let name = ctx.doc.layer(id).name.clone();
        ctx.doc.current_layer = id;
        for i in 0..ctx.doc.layers.len() {
            let l = &ctx.doc.layers[i].name;
            let on = *l == name
                || l.starts_with(&format!("{name}::"))
                || name.starts_with(&format!("{l}::"));
            ctx.doc.edit_layer(LayerId(i), |l| l.visible = on);
        }
        let doc = &ctx.doc;
        ctx.selection
            .retain(|o| doc.object(*o).is_some_and(|o| doc.is_selectable(o)));
        Ok(format!("only {name} is on"))
    }
}

simple_command!(
    AllLayersOn,
    "AllLayersOn",
    &[],
    "AllLayersOn — show every layer"
);
impl Command for AllLayersOn {
    impl_meta!(AllLayersOn);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        for i in 0..ctx.doc.layers.len() {
            ctx.doc.edit_layer(LayerId(i), |l| l.visible = true);
        }
        Ok(format!("{} layer(s) on", ctx.doc.layers.len()))
    }
}

simple_command!(
    ChangeToCurrentLayer,
    "ChangeToCurrentLayer",
    &[],
    "ChangeToCurrentLayer — move the selected objects to the current layer"
);
impl Command for ChangeToCurrentLayer {
    impl_meta!(ChangeToCurrentLayer);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = ctx.selected("ChangeToCurrentLayer")?;
        let layer = ctx.doc.current_layer;
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_layer(*id, layer);
        }
        t.commit();
        Ok(format!(
            "{} object(s) moved to {}",
            ids.len(),
            ctx.doc.layer(layer).name
        ))
    }
}

simple_command!(
    CopyObjectsToLayer,
    "CopyObjectsToLayer",
    &["CopyToLayer"],
    "CopyObjectsToLayer <layer> — copy the selected objects to a layer (created if missing)"
);
impl Command for CopyObjectsToLayer {
    impl_meta!(CopyObjectsToLayer);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let name = name_arg(args, "layer name")?;
        let ids = ctx.selected("CopyObjectsToLayer")?;
        let layer = layer_named(&mut ctx.doc, &name);
        let mut t = ctx.doc.begin();
        let mut copies = Vec::new();
        for id in &ids {
            let obj = t.doc().object(*id).expect("selected").clone();
            let c = t.add_like(obj.geometry.clone(), &obj);
            t.set_layer(c, layer);
            copies.push(c);
        }
        t.commit();
        ctx.selection = copies.into_iter().collect();
        Ok(format!("{} object(s) copied to {name}", ids.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::glob;
    use crate::Engine;

    #[test]
    fn wildcards() {
        assert!(glob("sedia*", "Sedia_01"));
        assert!(glob("*01", "sedia_01"));
        assert!(glob("s*a*1", "sedia_01"));
        assert!(!glob("tavolo", "tavolino"));
        assert!(glob("tavol*", "tavolino"));
    }

    #[test]
    fn names_and_name_selection() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Box 20,0 30,10 10").unwrap();
        e.run_line("Select #1").unwrap();
        e.run_line("SetObjectName sedia 01").unwrap();
        assert_eq!(
            e.doc().objects().next().unwrap().name.as_deref(),
            Some("sedia 01")
        );
        e.run_line("SelNone").unwrap();
        e.run_line("SelName sedia*").unwrap();
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("Copy 0,0 0,50").unwrap();
        assert_eq!(
            e.doc()
                .objects()
                .filter(|o| o.name.as_deref() == Some("sedia 01"))
                .count(),
            2
        );
        e.run_line("SetObjectName \"\"").unwrap();
        assert!(e.doc().objects().next().unwrap().name.is_none());
        e.run_line("Undo").unwrap();
        assert!(e.doc().objects().next().unwrap().name.is_some());
    }

    #[test]
    fn selection_filters() {
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Polyline 0,0 10,0 10,10").unwrap();
        e.run_line("Rectangle 0,0 10,10").unwrap();
        e.run_line("Circle 0,0 5").unwrap();
        e.run_line("Line 0,0 10,0").unwrap(); // duplicate of #1
        e.run_line("Text 0,0 5 ciao").unwrap();
        e.run_line("Dim 0,0 10,0 5,-5").unwrap();
        let count = |e: &mut Engine, cmd: &str| {
            e.run_line("SelNone").unwrap();
            e.run_line(cmd).unwrap();
            e.ctx.selection.len()
        };
        assert_eq!(count(&mut e, "SelLine"), 2);
        assert_eq!(count(&mut e, "SelPolyline"), 2);
        assert_eq!(count(&mut e, "SelOpenCrv"), 3);
        assert_eq!(count(&mut e, "SelClosedCrv"), 2);
        assert_eq!(count(&mut e, "SelDup"), 1);
        assert!(e.ctx.selection.contains(&forma_doc::ObjectId(5)));
        assert_eq!(count(&mut e, "SelText"), 1);
        assert_eq!(count(&mut e, "SelDim"), 1);
        assert_eq!(count(&mut e, "SelCrv"), 5); // annotations are not curves
        e.run_line("SelNone").unwrap();
        e.run_line("Select #4").unwrap();
        e.run_line("SetObjectColor red").unwrap();
        assert_eq!(count(&mut e, "SelColor 255,0,0"), 1);
    }

    #[test]
    fn layer_management() {
        let mut e = Engine::new();
        e.run_line("Layer muri").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Layer arredi::sedie").unwrap();
        e.run_line("Box 20,0 30,10 10").unwrap();
        e.run_line("Layer vuoto").unwrap();
        e.run_line("Layer Default").unwrap();
        e.run_line("SelLayer muri").unwrap();
        assert_eq!(e.ctx.selection.len(), 1);
        e.run_line("ChangeToCurrentLayer").unwrap();
        e.run_line("SelNone").unwrap();
        e.run_line("SelLayer muri").unwrap();
        assert!(e.ctx.selection.is_empty());
        e.run_line("Undo").unwrap();
        e.run_line("SelLayer muri").unwrap();
        e.run_line("CopyObjectsToLayer copia").unwrap();
        assert_eq!(e.doc().len(), 3);
        e.run_line("Purge").unwrap();
        assert!(e.doc().find_layer("vuoto").is_none());
        assert!(e.doc().find_layer("arredi::sedie").is_some());
        e.run_line("RenameLayer arredi mobili").unwrap();
        assert!(e.doc().find_layer("mobili::sedie").is_some());
        e.run_line("OneLayerOn mobili::sedie").unwrap();
        let muri = e.doc().find_layer("muri").unwrap();
        assert!(!e.doc().layer(muri).visible);
        e.run_line("AllLayersOn").unwrap();
        assert!(e.doc().layer(muri).visible);
        assert!(e.run_line("DeleteLayer mobili").is_err()); // current (sub-layer)
        e.run_line("Layer Default").unwrap();
        e.run_line("DeleteLayer mobili").unwrap();
        assert!(e.doc().find_layer("mobili::sedie").is_none());
        assert_eq!(e.doc().len(), 2);
        e.run_line("Undo").unwrap(); // the objects come back (on the current layer)
        assert_eq!(e.doc().len(), 3);
    }
}
