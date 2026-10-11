//! Small dialog windows for commands that need a few values (Array, BoxEdit,
//! layer operations, selection by name / colour…), and the Import / Export file
//! dialogs. Each form only builds an engine command line: the modelling stays
//! in `forma-engine`, and the line is logged like a typed command.

use crate::{FormaApp, LogKind};
use eframe::egui::{self, Key, RichText};
use forma_engine::Engine;

/// One input of a form.
#[derive(Clone, Copy)]
pub(crate) enum Field {
    /// Free text.
    Text(&'static str),
    /// A number.
    Num(&'static str),
    /// An existing layer (drop-down) — `true` also accepts a new name.
    Layer(&'static str, bool),
    /// On / off.
    Check(&'static str),
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Field::Text(l) | Field::Num(l) | Field::Layer(l, _) | Field::Check(l) => l,
        }
    }
}

/// A dialog: title, one line of help, its fields, their initial values (from
/// the document) and the command line it produces.
pub(crate) struct Form {
    pub title: &'static str,
    pub help: &'static str,
    pub fields: &'static [Field],
    pub init: fn(&Engine) -> Vec<String>,
    /// The command line, or an error to show in the dialog.
    pub emit: fn(&[String]) -> Result<String, String>,
    pub needs_selection: bool,
}

/// An open dialog.
pub(crate) struct OpenForm {
    form: &'static Form,
    values: Vec<String>,
    error: Option<String>,
    focus_first: bool,
}

/// Wrap a name in quotes when it has spaces.
fn q(s: &str) -> String {
    let s = s.trim();
    if s.contains(char::is_whitespace) {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

fn need(v: &str, what: &str) -> Result<String, String> {
    let v = v.trim();
    if v.is_empty() {
        Err(format!("{what}: required"))
    } else {
        Ok(v.to_string())
    }
}

fn num(v: &str, what: &str) -> Result<f64, String> {
    v.trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| format!("{what}: a number is expected"))
}

fn one_blank(_: &Engine) -> Vec<String> {
    vec![String::new()]
}

fn current_layer(e: &Engine) -> Vec<String> {
    let d = e.doc();
    vec![d.layer(d.current_layer).name.clone()]
}

fn selection_box(e: &Engine) -> Option<forma_geom::BoundingBox> {
    let pts: Vec<forma_geom::Point3> = e
        .ctx
        .selection
        .iter()
        .filter_map(|id| e.doc().object(*id))
        .flat_map(|o| {
            let b = o.geometry.bounding_box();
            [b.min, b.max]
        })
        .collect();
    forma_geom::BoundingBox::from_points(&pts)
}

fn r(x: f64) -> String {
    crate::tools::round(x).to_string()
}

pub(crate) static ARRAY: Form = Form {
    title: "Rectangular array",
    help: "Copies of the selection on a grid: how many along X, Y, Z and the spacing.",
    fields: &[
        Field::Num("Number in X"),
        Field::Num("Number in Y"),
        Field::Num("Number in Z"),
        Field::Num("Spacing X"),
        Field::Num("Spacing Y"),
        Field::Num("Spacing Z"),
    ],
    init: |e| {
        let s = selection_box(e).map_or([100.0; 3], |b| {
            let d = b.max - b.min;
            [d.x.max(1.0) * 1.5, d.y.max(1.0) * 1.5, d.z.max(1.0) * 1.5]
        });
        vec![
            "3".into(),
            "1".into(),
            "1".into(),
            r(s[0]),
            r(s[1]),
            r(s[2]),
        ]
    },
    emit: |v| {
        let n: Vec<f64> = (0..3)
            .map(|i| num(&v[i], "number"))
            .collect::<Result<_, _>>()?;
        if n.iter().any(|x| *x < 1.0) {
            return Err("numbers must be 1 or more".into());
        }
        let d: Vec<f64> = (3..6)
            .map(|i| num(&v[i], "spacing"))
            .collect::<Result<_, _>>()?;
        Ok(format!(
            "Array {} {} {} {},{},{}",
            n[0].round(),
            n[1].round(),
            n[2].round(),
            r(d[0]),
            r(d[1]),
            r(d[2])
        ))
    },
    needs_selection: true,
};

pub(crate) static BOX_EDIT: Form = Form {
    title: "Box Edit",
    help: "Exact size and position of the selection's bounding box (empty = unchanged).",
    fields: &[
        Field::Num("Size X"),
        Field::Num("Size Y"),
        Field::Num("Size Z"),
        Field::Check("Keep proportions"),
        Field::Check("Scale about the centre"),
        Field::Text("Move minimum corner to (x,y,z)"),
    ],
    init: |e| match selection_box(e) {
        Some(b) => {
            let d = b.max - b.min;
            vec![
                r(d.x),
                r(d.y),
                r(d.z),
                "false".into(),
                "false".into(),
                String::new(),
            ]
        }
        None => vec![String::new(); 6],
    },
    emit: |v| {
        let mut line = String::from("BoxEdit");
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            if !v[i].trim().is_empty() {
                line.push_str(&format!(" {axis}={}", r(num(&v[i], "size")?)));
            }
        }
        if v[3] == "true" {
            line.push_str(" uniform");
        }
        if v[4] == "true" {
            line.push_str(" center");
        }
        if !v[5].trim().is_empty() {
            line.push_str(&format!(" at={}", v[5].trim().replace(' ', "")));
        }
        Ok(line)
    },
    needs_selection: true,
};

pub(crate) static DIM_STYLE: Form = Form {
    title: "Annotation style",
    help: "Text height and decimals of new text and dimensions.",
    fields: &[Field::Num("Text height"), Field::Num("Decimals")],
    init: |e| {
        let s = &e.doc().dim_style;
        vec![r(s.text_height), s.decimals.to_string()]
    },
    emit: |v| {
        let h = num(&v[0], "text height")?;
        let d = num(&v[1], "decimals")?;
        Ok(format!("DimStyle {} {}", r(h), d.round()))
    },
    needs_selection: false,
};

pub(crate) static SAVE_VIEW: Form = Form {
    title: "Save named view",
    help: "Saves the active view's camera (and CPlane) under a name, to restore it later.",
    fields: &[Field::Text("Name")],
    init: one_blank,
    emit: |v| Ok(format!("NamedView Save {}", need(&v[0], "name")?)),
    needs_selection: false,
};

pub(crate) static SAVE_LAYER_STATE: Form = Form {
    title: "Save layer state",
    help: "Remembers which layers are on and locked (e.g. \"pianta arredi\"), to restore later.",
    fields: &[Field::Text("Name")],
    init: one_blank,
    emit: |v| Ok(format!("LayerState Save {}", q(&need(&v[0], "name")?))),
    needs_selection: false,
};

pub(crate) static UNITS: Form = Form {
    title: "Units",
    help: "Model units (mm, cm, m, in, ft). With \"Scale the model\" sizes stay real (100 cm become 1 m).",
    fields: &[Field::Text("Units"), Field::Check("Scale the model")],
    init: |e| vec![e.doc().units.abbreviation().to_string(), "true".into()],
    emit: |v| {
        let keep = if v[1] == "true" { "scale" } else { "keep" };
        Ok(format!("Units {} {keep}", need(&v[0], "units")?))
    },
    needs_selection: false,
};

pub(crate) static SEL_LAYER: Form = Form {
    title: "Select by layer",
    help: "Selects every visible object on the layer.",
    fields: &[Field::Layer("Layer", false)],
    init: current_layer,
    emit: |v| Ok(format!("SelLayer {}", need(&v[0], "layer")?)),
    needs_selection: false,
};

pub(crate) static SEL_COLOR: Form = Form {
    title: "Select by colour",
    help: "r,g,b · #rrggbb · or a name (red, blue…): objects shown in that colour.",
    fields: &[Field::Text("Colour")],
    init: |_| vec!["255,0,0".into()],
    emit: |v| Ok(format!("SelColor {}", need(&v[0], "colour")?)),
    needs_selection: false,
};

pub(crate) static SEL_NAME: Form = Form {
    title: "Select by name",
    help: "Object name; * matches anything (sedia*).",
    fields: &[Field::Text("Name")],
    init: one_blank,
    emit: |v| Ok(format!("SelName {}", need(&v[0], "name")?)),
    needs_selection: false,
};

pub(crate) static SEL_SMALL: Form = Form {
    title: "Select small objects",
    help: "Objects whose bounding box is smaller than this size.",
    fields: &[Field::Num("Size")],
    init: |_| vec!["1".into()],
    emit: |v| Ok(format!("SelSmall {}", r(num(&v[0], "size")?))),
    needs_selection: false,
};

pub(crate) static COPY_TO_LAYER: Form = Form {
    title: "Copy objects to layer",
    help: "Copies the selection to a layer (a new name creates it).",
    fields: &[Field::Layer("Layer", true)],
    init: current_layer,
    emit: |v| Ok(format!("CopyObjectsToLayer {}", need(&v[0], "layer")?)),
    needs_selection: true,
};

pub(crate) static ONE_LAYER_ON: Form = Form {
    title: "One layer on",
    help: "Shows only this layer (with its parents and sub-layers) and makes it current.",
    fields: &[Field::Layer("Layer", false)],
    init: current_layer,
    emit: |v| Ok(format!("OneLayerOn {}", need(&v[0], "layer")?)),
    needs_selection: false,
};

pub(crate) static ONE_LAYER_OFF: Form = Form {
    title: "One layer off",
    help: "Hides one layer.",
    fields: &[Field::Layer("Layer", false)],
    init: one_blank,
    emit: |v| Ok(format!("OneLayerOff {}", need(&v[0], "layer")?)),
    needs_selection: false,
};

pub(crate) static RENAME_LAYER: Form = Form {
    title: "Rename layer",
    help: "Sub-layers follow their parent.",
    fields: &[Field::Layer("Layer", false), Field::Text("New name")],
    init: |e| {
        let mut v = current_layer(e);
        v.push(String::new());
        v
    },
    emit: |v| {
        Ok(format!(
            "RenameLayer {} {}",
            q(&need(&v[0], "layer")?),
            q(&need(&v[1], "new name")?)
        ))
    },
    needs_selection: false,
};

pub(crate) static DELETE_LAYER: Form = Form {
    title: "Delete layer",
    help: "Deletes the layer, its sub-layers and their objects (objects can be undone).",
    fields: &[Field::Layer("Layer", false)],
    init: one_blank,
    emit: |v| Ok(format!("DeleteLayer {}", need(&v[0], "layer")?)),
    needs_selection: false,
};

pub(crate) static OBJECT_NAME: Form = Form {
    title: "Object name",
    help: "Name of the selected objects (empty clears it).",
    fields: &[Field::Text("Name")],
    init: |e| {
        let name = e
            .ctx
            .selection
            .iter()
            .find_map(|id| e.doc().object(*id).and_then(|o| o.name.clone()))
            .unwrap_or_default();
        vec![name]
    },
    emit: |v| {
        let n = v[0].trim();
        Ok(if n.is_empty() {
            "SetObjectName \"\"".into()
        } else {
            format!("SetObjectName {n}")
        })
    },
    needs_selection: true,
};

impl FormaApp {
    pub(crate) fn open_form(&mut self, form: &'static Form) {
        if form.needs_selection && self.engine.ctx.selection.is_empty() {
            self.log(
                LogKind::Error,
                format!("{}: select objects first", form.title),
            );
            return;
        }
        self.tool = None;
        self.form = Some(OpenForm {
            form,
            values: (form.init)(&self.engine),
            error: None,
            focus_first: true,
        });
    }

    /// Open a dialog with given starting values (e.g. the layer right-clicked).
    pub(crate) fn open_form_with(&mut self, form: &'static Form, values: Vec<String>) {
        self.open_form(form);
        if let Some(f) = self.form.as_mut() {
            for (slot, v) in f.values.iter_mut().zip(values) {
                *slot = v;
            }
        }
    }

    /// The open dialog, if any.
    pub(crate) fn ui_form(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.form.take() else {
            return;
        };
        let layers: Vec<String> = self
            .engine
            .doc()
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        let mut ok = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("forma-form")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(RichText::new(f.form.title).strong());
            ui.label(
                RichText::new(f.form.help)
                    .small()
                    .color(crate::theme::weak()),
            );
            ui.add_space(6.0);
            egui::Grid::new("form-grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for (i, field) in f.form.fields.iter().enumerate() {
                        ui.label(field.label());
                        let v = &mut f.values[i];
                        let resp = match field {
                            Field::Text(_) | Field::Num(_) => {
                                ui.add(egui::TextEdit::singleline(v).desired_width(180.0))
                            }
                            Field::Check(_) => {
                                let mut on = v == "true";
                                let r = ui.checkbox(&mut on, "");
                                *v = on.to_string();
                                r
                            }
                            Field::Layer(_, free) => {
                                let r = ui
                                    .horizontal(|ui| {
                                        let r = if *free {
                                            ui.add(
                                                egui::TextEdit::singleline(v).desired_width(140.0),
                                            )
                                        } else {
                                            ui.add(
                                                egui::Label::new(if v.is_empty() {
                                                    "(choose)".to_string()
                                                } else {
                                                    v.clone()
                                                })
                                                .truncate(),
                                            )
                                        };
                                        egui::ComboBox::from_id_salt(("form-layer", i))
                                            .selected_text("")
                                            .width(24.0)
                                            .show_ui(ui, |ui| {
                                                for l in &layers {
                                                    ui.selectable_value(v, l.clone(), l);
                                                }
                                            });
                                        r
                                    })
                                    .inner;
                                r
                            }
                        };
                        if i == 0 && std::mem::take(&mut f.focus_first) {
                            resp.request_focus();
                        }
                        ui.end_row();
                    }
                });
            if let Some(e) = &f.error {
                ui.add_space(4.0);
                ui.colored_label(crate::theme::error(), e);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("OK").strong())
                            .min_size(egui::vec2(80.0, 24.0)),
                    )
                    .clicked()
                {
                    ok = true;
                }
                if ui
                    .add(egui::Button::new("Cancel").min_size(egui::vec2(80.0, 24.0)))
                    .clicked()
                {
                    cancel = true;
                }
            });
        });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)) {
            ok = true;
        }
        if modal.should_close() {
            cancel = true;
        }
        if cancel {
            return;
        }
        if ok {
            match (f.form.emit)(&f.values) {
                Ok(line) => {
                    self.run_engine(&line);
                    return;
                }
                Err(e) => f.error = Some(e),
            }
        }
        self.form = Some(f);
    }

    /// File ▸ Import: pick a Rhino file and merge it.
    pub(crate) fn import_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Rhino 3D model", &["3dm"]);
        if let Some(dir) = self.start_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(p) = dialog.pick_file() {
            self.run_engine(&format!("Import {}", p.display()));
        }
    }

    /// File ▸ Export Selected: Rhino, OBJ or STL, chosen by the file type.
    pub(crate) fn export_dialog(&mut self) {
        if self.engine.ctx.selection.is_empty() {
            self.log(LogKind::Error, "Export: select the objects to export first");
            return;
        }
        let stem = self
            .engine
            .doc()
            .path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map_or("export".to_string(), |s| {
                format!("{}-export", s.to_string_lossy())
            });
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Rhino 3D model", &["3dm"])
            .add_filter("Wavefront OBJ", &["obj"])
            .add_filter("STL (3D printing)", &["stl"])
            .set_file_name(format!("{stem}.3dm"));
        if let Some(dir) = self.start_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(mut p) = dialog.save_file() {
            if p.extension().is_none() {
                p.set_extension("3dm");
            }
            self.run_engine(&format!("Export {}", p.display()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(form: &Form, e: &mut Engine, values: &[&str]) -> String {
        let v: Vec<String> = values.iter().map(|s| s.to_string()).collect();
        let line = (form.emit)(&v).unwrap();
        e.run_line(&line).unwrap();
        line
    }

    #[test]
    fn forms_build_working_commands() {
        let mut e = Engine::new();
        e.run_line("Layer muri esterni").unwrap();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        assert_eq!((ARRAY.init)(&e).len(), 6);
        run(&ARRAY, &mut e, &["2", "2", "1", "20", "20", "0"]);
        assert_eq!(e.doc().len(), 4);
        e.run_line("SelAll").unwrap();
        run(&BOX_EDIT, &mut e, &["60", "", "", "false", "false", ""]);
        run(&OBJECT_NAME, &mut e, &["tavolo grande"]);
        e.run_line("SelNone").unwrap();
        run(&SEL_NAME, &mut e, &["tavolo*"]);
        assert_eq!(e.ctx.selection.len(), 4);
        let line = run(&RENAME_LAYER, &mut e, &["muri esterni", "facciata nord"]);
        assert_eq!(line, "RenameLayer \"muri esterni\" \"facciata nord\"");
        assert!(e.doc().find_layer("facciata nord").is_some());
        run(&DIM_STYLE, &mut e, &["12,5", "1"]);
        assert!((e.doc().dim_style.text_height - 12.5).abs() < 1e-12);
        assert!((ARRAY.emit)(&[
            "0".into(),
            "1".into(),
            "1".into(),
            "1".into(),
            "1".into(),
            "1".into()
        ])
        .is_err());
    }
}
