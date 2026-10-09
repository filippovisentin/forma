//! Internal clipboard (CopyToClipboard, Cut, Paste) and partial file exchange
//! (Import, Export).

use crate::{Args, ClipboardItem, Command, CommandError, CommandResult, Context};
use forma_doc::{Document, LayerId, ObjectId};
use std::collections::HashMap;

fn path_arg(args: &mut Args) -> Result<String, CommandError> {
    let path = args.rest().join(" ");
    let path = path.trim().trim_matches('"');
    if path.is_empty() {
        return Err(CommandError::MissingInput("file path"));
    }
    Ok(path.to_string())
}

/// Layer with this name, created (with this colour and visibility) if missing.
fn layer_for(doc: &mut Document, name: &str, color: [u8; 3], visible: bool) -> LayerId {
    doc.find_layer(name)
        .unwrap_or_else(|| doc.add_layer(name, color, visible))
}

/// Add clipboard-style items to the document as one undo step; returns new ids.
fn add_items(ctx: &mut Context, items: &[(ClipboardItem, bool)]) -> Vec<ObjectId> {
    let layers: Vec<LayerId> = items
        .iter()
        .map(|(it, visible)| layer_for(&mut ctx.doc, &it.layer, it.layer_color, *visible))
        .collect();
    let mut t = ctx.doc.begin();
    let mut ids = Vec::new();
    for ((it, _), layer) in items.iter().zip(layers) {
        let id = t.add_on_layer(it.geometry.clone(), layer);
        if it.color.is_some() {
            t.set_color(id, it.color);
        }
        ids.push(id);
    }
    t.commit();
    ids
}

fn copy_selection(ctx: &mut Context, cmd: &str) -> Result<Vec<ObjectId>, CommandError> {
    let ids = ctx.selected(cmd)?;
    ctx.clipboard = ids
        .iter()
        .map(|id| {
            let o = ctx.doc.object(*id).expect("selected");
            let l = ctx.doc.layer(o.layer);
            ClipboardItem {
                geometry: o.geometry.clone(),
                layer: l.name.clone(),
                layer_color: l.color,
                color: o.color,
            }
        })
        .collect();
    Ok(ids)
}

simple_command!(
    CopyToClipboard,
    "CopyToClipboard",
    &["CopyClip"],
    "CopyToClipboard — copy the selected objects to Forma's clipboard"
);
impl Command for CopyToClipboard {
    impl_meta!(CopyToClipboard);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = copy_selection(ctx, "CopyToClipboard")?;
        Ok(format!("{} object(s) copied to the clipboard", ids.len()))
    }
}

simple_command!(
    Cut,
    "Cut",
    &[],
    "Cut — move the selected objects to Forma's clipboard"
);
impl Command for Cut {
    impl_meta!(Cut);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        let ids = copy_selection(ctx, "Cut")?;
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.remove(*id);
        }
        t.commit();
        ctx.selection.clear();
        Ok(format!("{} object(s) cut", ids.len()))
    }
}

simple_command!(
    Paste,
    "Paste",
    &[],
    "Paste — add the clipboard objects (layers by name) and select them"
);
impl Command for Paste {
    impl_meta!(Paste);
    fn run(&self, ctx: &mut Context, _args: &mut Args) -> CommandResult {
        if ctx.clipboard.is_empty() {
            return Err(CommandError::Invalid("the clipboard is empty".into()));
        }
        let items: Vec<(ClipboardItem, bool)> =
            ctx.clipboard.iter().map(|c| (c.clone(), true)).collect();
        let ids = add_items(ctx, &items);
        ctx.selection = ids.iter().copied().collect();
        Ok(format!("{} object(s) pasted", ids.len()))
    }
}

simple_command!(
    Import,
    "Import",
    &[],
    "Import <file.3dm> — merge a Rhino file into the document (layers matched by name) and select it"
);
impl Command for Import {
    impl_meta!(Import);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let path = path_arg(args)?;
        let src = crate::import::open_3dm(&path)?;
        let items: Vec<(ClipboardItem, bool)> = src
            .objects()
            .map(|o| {
                let l = src.layer(o.layer);
                (
                    ClipboardItem {
                        geometry: o.geometry.clone(),
                        layer: l.name.clone(),
                        layer_color: l.color,
                        color: o.color,
                    },
                    l.visible,
                )
            })
            .collect();
        if items.is_empty() {
            return Err(CommandError::Invalid(format!("{path}: nothing to import")));
        }
        let ids = add_items(ctx, &items);
        ctx.selection = ids.iter().copied().collect();
        Ok(format!("imported {} object(s) from {path}", ids.len()))
    }
}

simple_command!(
    Export,
    "Export",
    &["ExportSelected"],
    "Export <file.3dm> — save only the selected objects (with their layers) as a Rhino file"
);
impl Command for Export {
    impl_meta!(Export);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let path = path_arg(args)?;
        let ids = ctx.selected("Export")?;
        let mut out = Document::new();
        out.units = ctx.doc.units;
        out.absolute_tolerance = ctx.doc.absolute_tolerance;
        out.layers.clear();
        let mut map: HashMap<LayerId, LayerId> = HashMap::new();
        for id in &ids {
            let o = ctx.doc.object(*id).expect("selected");
            map.entry(o.layer).or_insert_with(|| {
                let l = ctx.doc.layer(o.layer);
                let new = out.add_layer(&l.name, l.color, l.visible);
                out.edit_layer(new, |x| x.locked = l.locked);
                new
            });
        }
        {
            let mut t = out.begin();
            for id in &ids {
                let o = ctx.doc.object(*id).expect("selected");
                let n = t.add_on_layer(o.geometry.clone(), map[&o.layer]);
                if o.color.is_some() {
                    t.set_color(n, o.color);
                }
            }
            t.commit();
        }
        forma_io_3dm::write_document(&path, &out)
            .map_err(|e| CommandError::Invalid(e.to_string()))?;
        Ok(format!("exported {} object(s) to {path}", ids.len()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn copy_cut_paste() {
        let mut e = Engine::new();
        assert!(e.run_line("Paste").is_err());
        e.run_line("Layer Arredi").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Circle 0,0 5").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("SetObjectColor 1,2,3").unwrap();
        e.run_line("CopyToClipboard").unwrap();
        e.run_line("Paste").unwrap();
        assert_eq!(e.doc().len(), 4);
        assert_eq!(e.ctx.selection.len(), 2);
        assert!(e.ctx.selection.iter().all(|id| id.0 > 2));
        e.run_line("Cut").unwrap();
        assert_eq!(e.doc().len(), 2);
        // Paste into a new document re-creates the layer.
        e.run_line("New").unwrap();
        e.run_line("Paste").unwrap();
        assert_eq!(e.doc().len(), 2);
        let o = e.doc().objects().next().unwrap();
        assert_eq!(e.doc().layer(o.layer).name, "Arredi");
        assert_eq!(o.color, Some([1, 2, 3]));
        e.run_line("Undo").unwrap();
        assert!(e.doc().is_empty());
    }

    #[test]
    fn save_and_reopen_nurbs_and_points() {
        let dir = std::env::temp_dir().join(format!("forma-nurbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nurbs.3dm");
        let p = path.to_str().unwrap();
        let mut e = Engine::new();
        e.run_line("Ellipse 0,0 100,0 0,40").unwrap();
        e.run_line("InterpCrv 0,0,0 50,30,0 100,0,10 150,20,0")
            .unwrap();
        e.run_line("Points 1,2,3 4,5,6").unwrap();
        e.run_line(&format!("Save {p}")).unwrap();
        let mut f = Engine::new();
        f.run_line(&format!("Open {p}")).unwrap();
        assert_eq!(f.doc().len(), 4);
        let kinds: Vec<&str> = f.doc().objects().map(|o| o.geometry.kind()).collect();
        assert_eq!(
            kinds.iter().filter(|k| **k == "point").count(),
            2,
            "{kinds:?}"
        );
        assert_eq!(
            kinds.iter().filter(|k| **k == "polyline").count(),
            2,
            "{kinds:?}"
        );
        // The reader imports curves as display polylines: the ellipse keeps its size.
        let b = f.doc().objects().next().unwrap().geometry.bounding_box();
        assert!(
            (b.max.x - 100.0).abs() < 1e-3 && (b.max.y - 40.0).abs() < 1e-3,
            "{b:?}"
        );
        // Validate with rhino3dm (McNeel's own reader) when it is installed.
        let script = "import sys, rhino3dm\n\
            m = rhino3dm.File3dm.Read(sys.argv[1])\n\
            cs = [o.Geometry for o in m.Objects if isinstance(o.Geometry, rhino3dm.NurbsCurve)]\n\
            assert len(cs) == 2, len(cs)\n\
            assert all(c.IsValid for c in cs)\n\
            assert cs[0].IsClosed and cs[0].IsRational and cs[0].Degree == 2\n\
            assert not cs[1].IsClosed and cs[1].Degree == 3\n\
            assert sum(isinstance(o.Geometry, rhino3dm.Point) for o in m.Objects) == 2\n\
            print('ok')\n";
        if let Ok(out) = std::process::Command::new("python3")
            .args(["-c", script, p])
            .output()
        {
            let err = String::from_utf8_lossy(&out.stderr);
            if !err.contains("No module named") && !err.is_empty() || out.status.success() {
                assert!(out.status.success(), "rhino3dm check failed: {err}");
                assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn export_selected_and_import() {
        let dir = std::env::temp_dir().join(format!("forma-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parte.3dm");
        let p = path.to_str().unwrap();
        let mut e = Engine::new();
        e.run_line("Layer muri").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Layer arredi").unwrap();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Point 1,2,3").unwrap();
        e.run_line("Select #2 #3").unwrap();
        e.run_line(&format!("Export {p}")).unwrap();
        let mut f = Engine::new();
        f.run_line("Line 0,0 0,50").unwrap();
        f.run_line(&format!("Import {p}")).unwrap();
        assert_eq!(f.doc().len(), 3);
        assert_eq!(f.ctx.selection.len(), 2);
        assert!(f.doc().find_layer("arredi").is_some());
        assert!(f.doc().find_layer("muri").is_none());
        let kinds: Vec<&str> = f.doc().objects().map(|o| o.geometry.kind()).collect();
        assert!(kinds.contains(&"point"), "{kinds:?}");
        assert!(f.run_line("Import /not/there.3dm").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
