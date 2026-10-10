//! Internal clipboard (CopyToClipboard, Cut, Paste) and partial file exchange
//! (Import, Export).

use crate::{Args, ClipboardItem, Command, CommandError, CommandResult, Context};
use forma_doc::{Document, Geometry, LayerId, ObjectId};
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

/// Group and name of an imported object.
type Extra = (Option<u32>, Option<String>);

/// Add clipboard-style items to the document as one undo step; returns new ids.
/// `extra` (empty, or one per item) carries groups, offset by `group_base`, and names.
fn add_items(
    ctx: &mut Context,
    items: &[(ClipboardItem, bool)],
    extra: &[Extra],
    group_base: u32,
) -> Vec<ObjectId> {
    let layers: Vec<LayerId> = items
        .iter()
        .map(|(it, visible)| layer_for(&mut ctx.doc, &it.layer, it.layer_color, *visible))
        .collect();
    let mut t = ctx.doc.begin();
    let mut ids = Vec::new();
    for (k, ((it, _), layer)) in items.iter().zip(layers).enumerate() {
        let id = t.add_on_layer(it.geometry.clone(), layer);
        if it.color.is_some() {
            t.set_color(id, it.color);
        }
        if let Some((g, n)) = extra.get(k) {
            if let Some(g) = g {
                t.set_group(id, Some(g + group_base));
            }
            if n.is_some() {
                t.set_name(id, n.clone());
            }
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
        let ids = add_items(ctx, &items, &[], 0);
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
        // Block groups are renumbered after the existing ones; names are kept.
        let base = ctx.doc.objects().filter_map(|o| o.group).max().unwrap_or(0);
        let extra: Vec<(Option<u32>, Option<String>)> =
            src.objects().map(|o| (o.group, o.name.clone())).collect();
        let ids = add_items(ctx, &items, &extra, base);
        ctx.selection = ids.iter().copied().collect();
        Ok(format!("imported {} object(s) from {path}", ids.len()))
    }
}

simple_command!(
    Export,
    "Export",
    &["ExportSelected"],
    "Export <file.3dm|.obj|.stl> — save only the selected objects: a Rhino file (with their layers), Wavefront OBJ with layer-colour materials, Y up (for renderers), or binary STL in millimetres (for 3D printing)"
);
impl Command for Export {
    impl_meta!(Export);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let path = path_arg(args)?;
        let ids = ctx.selected("Export")?;
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        match ext.as_str() {
            "obj" => return export_obj(ctx, &ids, &path),
            "stl" => return export_stl(ctx, &ids, &path),
            _ => {}
        }
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

fn io_err(e: std::io::Error) -> CommandError {
    CommandError::Invalid(e.to_string())
}

/// Wavefront OBJ: one group per object (named after its layer), meshes as faces,
/// curves as polylines.
fn export_obj(ctx: &Context, ids: &[ObjectId], path: &str) -> CommandResult {
    use std::fmt::Write as _;
    // Y up, like Rhino's default "map Z to Y" and what Blender, D5 Render, SketchUp
    // and most renderers expect: (x, y, z) → (x, z, −y).
    let up = |x: f64, y: f64, z: f64| (x, z, -y);
    let mtl_path = std::path::Path::new(path).with_extension("mtl");
    let mtl_name = mtl_path
        .file_name()
        .map_or("materials.mtl".to_string(), |n| {
            n.to_string_lossy().into_owned()
        });
    let mut s = format!(
        "# Forma export, units: {}, Y up\nmtllib {mtl_name}\n",
        ctx.doc.units.abbreviation()
    );
    // One material per layer and display colour, named after the layer.
    let mut materials: Vec<(String, [u8; 3])> = Vec::new();
    let mut base = 1usize;
    let mut nbase = 1usize;
    let (mut meshes, mut curves) = (0, 0);
    for id in ids {
        let o = ctx.doc.object(*id).expect("selected");
        let layer = ctx
            .doc
            .layer(o.layer)
            .name
            .replace("::", "-")
            .replace(char::is_whitespace, "_");
        let color = ctx.doc.display_color(o);
        let mat = match materials
            .iter()
            .find(|(n, c)| *c == color && n.starts_with(&layer))
        {
            Some((n, _)) => n.clone(),
            None => {
                let taken = materials.iter().any(|(n, _)| *n == layer);
                let name = if taken {
                    format!("{layer}_{:02x}{:02x}{:02x}", color[0], color[1], color[2])
                } else {
                    layer.clone()
                };
                materials.push((name.clone(), color));
                name
            }
        };
        match &o.geometry {
            Geometry::Mesh(m) => {
                let _ = writeln!(s, "o {layer}_{}\nusemtl {mat}", id.0);
                for p in &m.positions {
                    let (x, y, z) = up(p.x, p.y, p.z);
                    let _ = writeln!(s, "v {x} {y} {z}");
                }
                let normals = m.normals.len() == m.positions.len();
                if normals {
                    for n in &m.normals {
                        let (x, y, z) = up(n.x, n.y, n.z);
                        let _ = writeln!(s, "vn {x} {y} {z}");
                    }
                }
                for t in &m.triangles {
                    let [a, b, c] = t.map(|i| i as usize + base);
                    if normals {
                        let [na, nb, nc] = t.map(|i| i as usize + nbase);
                        let _ = writeln!(s, "f {a}//{na} {b}//{nb} {c}//{nc}");
                    } else {
                        let _ = writeln!(s, "f {a} {b} {c}");
                    }
                }
                base += m.positions.len();
                if normals {
                    nbase += m.normals.len();
                }
                meshes += 1;
            }
            g if g.is_curve() => {
                let pts = g.curve_points();
                if pts.len() < 2 {
                    continue;
                }
                let _ = writeln!(s, "o {layer}_{}\nusemtl {mat}", id.0);
                for p in &pts {
                    let (x, y, z) = up(p.x, p.y, p.z);
                    let _ = writeln!(s, "v {x} {y} {z}");
                }
                s.push('l');
                for i in 0..pts.len() {
                    let _ = write!(s, " {}", base + i);
                }
                s.push('\n');
                base += pts.len();
                curves += 1;
            }
            _ => {}
        }
    }
    if meshes + curves == 0 {
        return Err(CommandError::Invalid(
            "Export: nothing to write as OBJ".into(),
        ));
    }
    let mut mtl = String::from("# Forma materials (layer colours)\n");
    for (name, [r, g, b]) in &materials {
        let f = |c: &u8| f64::from(*c) / 255.0;
        let _ = writeln!(
            mtl,
            "newmtl {name}\nKa 0 0 0\nKd {:.4} {:.4} {:.4}\nKs 0 0 0\nd 1\nillum 1\n",
            f(r),
            f(g),
            f(b)
        );
    }
    std::fs::write(path, s).map_err(io_err)?;
    std::fs::write(&mtl_path, mtl).map_err(io_err)?;
    Ok(format!(
        "exported {meshes} mesh(es) and {curves} curve(s) to {path} (+ {mtl_name}, {} material(s), Y up)",
        materials.len()
    ))
}

/// Binary STL of the selected meshes.
fn export_stl(ctx: &Context, ids: &[ObjectId], path: &str) -> CommandResult {
    let tris: Vec<[forma_geom::Point3; 3]> = ids
        .iter()
        .filter_map(|id| match &ctx.doc.object(*id)?.geometry {
            Geometry::Mesh(m) => Some(m),
            _ => None,
        })
        .flat_map(|m| {
            m.triangles
                .iter()
                .map(|t| t.map(|i| m.positions[i as usize]))
        })
        .collect();
    if tris.is_empty() {
        return Err(CommandError::Invalid(
            "Export: STL needs meshes or solids".into(),
        ));
    }
    // Slicers read STL in millimetres: convert from the document units.
    let k = ctx.doc.units.in_mm();
    let mut b: Vec<u8> = Vec::with_capacity(84 + tris.len() * 50);
    let mut header = [b' '; 80];
    let tag = b"Forma STL";
    header[..tag.len()].copy_from_slice(tag);
    b.extend_from_slice(&header);
    b.extend_from_slice(&u32::try_from(tris.len()).unwrap_or(u32::MAX).to_le_bytes());
    #[allow(clippy::cast_possible_truncation)]
    let f = |v: f64| (v as f32).to_le_bytes();
    for [p, q, r] in &tris {
        let n = (*q - *p)
            .cross(*r - *p)
            .normalized()
            .unwrap_or(forma_geom::Vec3::Z);
        for v in [n.x, n.y, n.z] {
            b.extend_from_slice(&f(v));
        }
        for pt in [p, q, r] {
            for v in [pt.x, pt.y, pt.z] {
                b.extend_from_slice(&f(v * k));
            }
        }
        b.extend_from_slice(&[0, 0]);
    }
    std::fs::write(path, b).map_err(io_err)?;
    Ok(format!(
        "exported {} triangle(s) to {path} (millimetres)",
        tris.len()
    ))
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    /// Block instances arrive expanded, one named group per instance. Needs
    /// rhino3dm (McNeel's Python reader/writer) to make the file; skipped without it.
    // Not on Windows: a child process inherits the open file handles of the
    // other tests running in parallel and keeps their files locked.
    #[cfg(not(windows))]
    #[test]
    fn open_expands_blocks() {
        let dir = std::env::temp_dir().join(format!("forma-blocks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("blk.3dm");
        let script = "import sys, rhino3dm as r\n\
            m = r.File3dm()\n\
            line = r.LineCurve(r.Point3d(0,0,0), r.Point3d(10,0,0))\n\
            i = m.InstanceDefinitions.Add('sedia','','','',r.Point3d(0,0,0),(line,),(r.ObjectAttributes(),))\n\
            d = m.InstanceDefinitions[i]\n\
            for t in [(100,0,0),(0,50,0)]:\n    m.Objects.AddInstanceObject(r.InstanceReference(d.Id, r.Transform.Translation(*t)))\n\
            assert m.Write(sys.argv[1], 8)\n";
        let made = std::process::Command::new("python3")
            .args(["-c", script, path.to_str().unwrap()])
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            eprintln!("rhino3dm not available: skipped");
            return;
        }
        let mut e = Engine::new();
        e.run_line(&format!("Open {}", path.display())).unwrap();
        assert_eq!(e.doc().len(), 2);
        let groups: Vec<_> = e.doc().objects().map(|o| o.group).collect();
        assert_eq!(groups, vec![Some(1), Some(2)]);
        e.run_line("SelName sedia").unwrap();
        assert_eq!(e.ctx.selection.len(), 2);
        let b = e.doc().objects().next().unwrap().geometry.bounding_box();
        assert!((b.min.x - 100.0).abs() < 1e-9 && (b.max.x - 110.0).abs() < 1e-9);
        // Import keeps the blocks as groups after the existing ones.
        e.run_line(&format!("Import {}", path.display())).unwrap();
        let groups: Vec<_> = e.doc().objects().map(|o| o.group).collect();
        assert_eq!(groups, vec![Some(1), Some(2), Some(3), Some(4)]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_keeps_groups_hidden_and_locked() {
        let dir = std::env::temp_dir().join(format!("forma-state-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stato.3dm");
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("Line 0,5 10,5").unwrap();
        e.run_line("Line 0,9 10,9").unwrap();
        e.run_line("Line 0,12 10,12").unwrap();
        e.run_line("Select #1 #2").unwrap();
        e.run_line("Group").unwrap();
        e.run_line("Hide #3").unwrap();
        e.run_line("Lock #4").unwrap();
        e.run_line("Text 0,20 5 Cucina abitabile").unwrap();
        e.run_line("Dot 50,50 A1").unwrap();
        e.run_line(&format!("Save {}", path.display())).unwrap();
        let mut f = Engine::new();
        f.run_line(&format!("Open {}", path.display())).unwrap();
        let o: Vec<_> = f
            .doc()
            .objects()
            .map(|o| (o.group, o.hidden, o.locked))
            .collect();
        assert_eq!(o.len(), 6, "{o:?}");
        let texts: Vec<(String, bool)> = f
            .doc()
            .objects()
            .filter_map(|o| match &o.geometry {
                forma_doc::Geometry::Text(t) => Some((t.text.clone(), t.dot)),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                ("Cucina abitabile".to_string(), false),
                ("A1".to_string(), true)
            ]
        );
        let forma_doc::Geometry::Text(t) = &f.doc().objects().nth(4).unwrap().geometry else {
            panic!()
        };
        assert!(
            (t.height - 5.0).abs() < 1e-9
                && t.plane
                    .origin
                    .distance_to(forma_geom::Point3::new(0.0, 20.0, 0.0))
                    < 1e-9,
            "{t:?}"
        );
        assert!(o[0].0.is_some() && o[0].0 == o[1].0, "{o:?}");
        assert_eq!((o[2].1, o[3].2), (true, true), "{o:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stl_is_written_in_millimetres() {
        let dir = std::env::temp_dir().join(format!("forma-stlmm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        e.run_line("Box 0,0 2,2 3").unwrap();
        e.run_line("SelAll").unwrap();
        let stl = dir.join("pezzo.stl");
        e.run_line(&format!("Export {}", stl.display())).unwrap();
        let b = std::fs::read(&stl).unwrap();
        let max_z = (0..12)
            .flat_map(|t| (0..3).map(move |v| 84 + t * 50 + 12 + v * 12 + 8))
            .map(|o| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]))
            .fold(f32::MIN, f32::max);
        assert!((max_z - 30.0).abs() < 1e-4, "{max_z}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_obj_and_stl() {
        let dir = std::env::temp_dir().join(format!("forma-objstl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("Line 0,0 5,5").unwrap();
        e.run_line("SelAll").unwrap();
        let obj = dir.join("a.obj");
        let out = e.run_line(&format!("Export {}", obj.display())).unwrap();
        assert!(out.contains("1 mesh(es) and 1 curve(s)"), "{out}");
        let text = std::fs::read_to_string(&obj).unwrap();
        assert_eq!(text.lines().filter(|l| l.starts_with("f ")).count(), 12);
        assert!(text.lines().any(|l| l.starts_with("l ")));
        assert!(text.contains("mtllib a.mtl") && text.contains("usemtl Default"));
        // Y up: the box top (z = 10) is at y = 10 in the file.
        assert!(text
            .lines()
            .any(|l| l.starts_with("v ") && l.split(' ').nth(2) == Some("10")));
        let mtl = std::fs::read_to_string(dir.join("a.mtl")).unwrap();
        assert!(
            mtl.contains("newmtl Default") && mtl.contains("Kd "),
            "{mtl}"
        );
        let stl = dir.join("a.stl");
        e.run_line(&format!("Export {}", stl.display())).unwrap();
        assert_eq!(std::fs::metadata(&stl).unwrap().len(), 84 + 12 * 50);
        std::fs::remove_dir_all(&dir).ok();
    }

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
        // (Not on Windows: see `open_expands_blocks`.)
        if let Ok(out) = std::process::Command::new(if cfg!(windows) { "-" } else { "python3" })
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
