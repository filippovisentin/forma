//! Object and layer attributes (colour, visibility, lock) and arrays.

use crate::{Args, Command, CommandError, CommandResult, Context};
use forma_doc::LayerId;
use forma_geom::{Vec3, Xform};

/// Parse a colour: `r,g,b`, `#rrggbb` or a few names.
pub fn parse_color(s: &str) -> Result<[u8; 3], CommandError> {
    let bad = || CommandError::BadInput(format!("colour {s}"));
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(bad());
        }
        let v = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
        return Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    }
    let named = match s.to_ascii_lowercase().as_str() {
        "black" | "nero" => Some([0, 0, 0]),
        "white" | "bianco" => Some([255, 255, 255]),
        "red" | "rosso" => Some([255, 0, 0]),
        "green" | "verde" => Some([0, 160, 0]),
        "blue" | "blu" => Some([0, 0, 255]),
        "yellow" | "giallo" => Some([255, 220, 0]),
        "orange" | "arancione" => Some([255, 127, 0]),
        "magenta" => Some([255, 0, 255]),
        "cyan" | "ciano" => Some([0, 200, 200]),
        "gray" | "grey" | "grigio" => Some([128, 128, 128]),
        "brown" | "marrone" => Some([130, 80, 40]),
        _ => None,
    };
    if let Some(c) = named {
        return Ok(c);
    }
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err(bad());
    }
    let mut c = [0u8; 3];
    for (i, p) in parts.iter().enumerate() {
        c[i] = p.trim().parse().map_err(|_| bad())?;
    }
    Ok(c)
}

fn layer_arg(ctx: &Context, args: &mut Args) -> Result<LayerId, CommandError> {
    let name = args.rest().join(" ");
    if name.is_empty() {
        return Ok(ctx.doc.current_layer);
    }
    ctx.doc
        .find_layer(&name)
        .ok_or_else(|| CommandError::Invalid(format!("no layer {name}")))
}

simple_command!(
    ObjectColor,
    "SetObjectColor",
    &["ObjectColor", "Color"],
    "SetObjectColor <r,g,b | #rrggbb | name | ByLayer> — display colour of the selected objects"
);
impl Command for ObjectColor {
    impl_meta!(ObjectColor);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("colour"))?;
        let color = if tok.eq_ignore_ascii_case("bylayer") || tok.eq_ignore_ascii_case("layer") {
            None
        } else {
            Some(parse_color(tok)?)
        };
        let ids = ctx.selected("SetObjectColor")?;
        let mut t = ctx.doc.begin();
        for id in &ids {
            t.set_color(*id, color);
        }
        t.commit();
        Ok(format!("colour set on {} object(s)", ids.len()))
    }
}

simple_command!(
    LayerColor,
    "LayerColor",
    &[],
    "LayerColor <r,g,b | #rrggbb | name> [layer] — colour of a layer (default: current)"
);
impl Command for LayerColor {
    impl_meta!(LayerColor);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let tok = args
            .next_token()
            .ok_or(CommandError::MissingInput("colour"))?;
        let c = parse_color(tok)?;
        let id = layer_arg(ctx, args)?;
        ctx.doc.edit_layer(id, |l| l.color = c);
        Ok(format!("layer {} colour set", ctx.doc.layer(id).name))
    }
}

fn on_off(args: &mut Args) -> Result<bool, CommandError> {
    match args.next_token().map(str::to_ascii_lowercase).as_deref() {
        Some("on" | "yes" | "1") => Ok(true),
        Some("off" | "no" | "0") => Ok(false),
        _ => Err(CommandError::MissingInput("on or off")),
    }
}

simple_command!(
    LayerVisible,
    "LayerVisible",
    &["LayerOn"],
    "LayerVisible <on|off> [layer] — show or hide a layer"
);
impl Command for LayerVisible {
    impl_meta!(LayerVisible);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let on = on_off(args)?;
        let id = layer_arg(ctx, args)?;
        if !on && id == ctx.doc.current_layer {
            return Err(CommandError::Invalid(
                "cannot hide the current layer".into(),
            ));
        }
        ctx.doc.edit_layer(id, |l| l.visible = on);
        let doc = &ctx.doc;
        ctx.selection
            .retain(|o| doc.object(*o).is_some_and(|o| doc.layer(o.layer).visible));
        Ok(String::new())
    }
}

simple_command!(
    LayerLock,
    "LayerLock",
    &[],
    "LayerLock <on|off> [layer] — lock or unlock a layer"
);
impl Command for LayerLock {
    impl_meta!(LayerLock);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let on = on_off(args)?;
        let id = layer_arg(ctx, args)?;
        if on && id == ctx.doc.current_layer {
            return Err(CommandError::Invalid(
                "cannot lock the current layer".into(),
            ));
        }
        ctx.doc.edit_layer(id, |l| l.locked = on);
        let doc = &ctx.doc;
        ctx.selection
            .retain(|o| doc.object(*o).is_some_and(|o| !doc.layer(o.layer).locked));
        Ok(String::new())
    }
}

fn count(args: &mut Args, what: &'static str) -> Result<usize, CommandError> {
    let n = args.number(what)?;
    if n < 1.0 || n.fract().abs() > 1e-9 || n > 10_000.0 {
        return Err(CommandError::Invalid(format!(
            "{what} must be a whole number ≥ 1"
        )));
    }
    Ok(n as usize)
}

/// Add transformed copies of the selection, one per transform.
fn copies(ctx: &mut Context, cmd: &str, xs: &[Xform]) -> CommandResult {
    let ids = ctx.selected(cmd)?;
    let mut t = ctx.doc.begin();
    let mut n = 0;
    for id in &ids {
        let obj = t.doc().object(*id).expect("selected").clone();
        for x in xs {
            t.add_like(obj.geometry.transformed(x), &obj);
            n += 1;
        }
    }
    t.commit();
    Ok(format!("{n} copies"))
}

simple_command!(
    Array,
    "Array",
    &["Ar"],
    "Array <nx> <ny> <nz> <dx,dy,dz> — rectangular array of the selection"
);
impl Command for Array {
    impl_meta!(Array);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let nx = count(args, "X count")?;
        let ny = count(args, "Y count")?;
        let nz = count(args, "Z count")?;
        let d = args.vector("spacing")?;
        let mut xs = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    if i + j + k > 0 {
                        xs.push(Xform::translation(Vec3::new(
                            d.x * i as f64,
                            d.y * j as f64,
                            d.z * k as f64,
                        )));
                    }
                }
            }
        }
        copies(ctx, "Array", &xs)
    }
}

simple_command!(
    ArrayLinear,
    "ArrayLinear",
    &["AL"],
    "ArrayLinear <count> <from> <to> — copies along a direction"
);
impl Command for ArrayLinear {
    impl_meta!(ArrayLinear);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let n = count(args, "number of elements")?;
        let a = args.point("first reference point", ctx.last_point)?;
        let b = args.point("second reference point", Some(a))?;
        let xs: Vec<Xform> = (1..n)
            .map(|i| Xform::translation((b - a) * i as f64))
            .collect();
        copies(ctx, "ArrayLinear", &xs)
    }
}

simple_command!(
    ArrayPolar,
    "ArrayPolar",
    &["AP"],
    "ArrayPolar <center> <count> [angle=360] [axis] — copies around a centre"
);
impl Command for ArrayPolar {
    impl_meta!(ArrayPolar);
    fn run(&self, ctx: &mut Context, args: &mut Args) -> CommandResult {
        let c = args.point("centre of polar array", ctx.last_point)?;
        let n = count(args, "number of elements")?;
        let total = args.optional_number().unwrap_or(360.0);
        let axis = args.optional_vector().unwrap_or(Vec3::Z);
        let full = (total.abs() - 360.0).abs() < 1e-9;
        let step = if full || n < 2 {
            total / n as f64
        } else {
            total / (n - 1) as f64
        };
        let xs: Vec<Xform> = (1..n)
            .map(|i| Xform::rotation(c, axis, (step * i as f64).to_radians()))
            .collect();
        copies(ctx, "ArrayPolar", &xs)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_color;
    use crate::Engine;

    #[test]
    fn colours_and_layers() {
        assert_eq!(parse_color("#ff8000").unwrap(), [255, 128, 0]);
        assert_eq!(parse_color("10,20,30").unwrap(), [10, 20, 30]);
        assert!(parse_color("10,20").is_err());
        let mut e = Engine::new();
        e.run_line("Line 0,0 10,0").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("SetObjectColor rosso").unwrap();
        let o = e.doc().objects().next().unwrap().clone();
        assert_eq!(e.doc().display_color(&o), [255, 0, 0]);
        e.run_line("Copy 0,0 0,10").unwrap();
        assert!(e.doc().objects().all(|o| o.color == Some([255, 0, 0])));
        e.run_line("Undo").unwrap();
        e.run_line("SetObjectColor ByLayer").unwrap();
        e.run_line("LayerColor 0,0,255").unwrap();
        let o = e.doc().objects().next().unwrap().clone();
        assert_eq!(e.doc().display_color(&o), [0, 0, 255]);
        e.run_line("Layer Muri").unwrap();
        e.run_line("LayerVisible off Default").unwrap();
        assert!(e.ctx.selection.is_empty());
        assert!(e.run_line("LayerVisible off Muri").is_err());
        e.run_line("LayerLock on Default").unwrap();
        assert!(e.doc().layer(forma_doc::LayerId(0)).locked);
    }

    #[test]
    fn arrays() {
        let mut e = Engine::new();
        e.run_line("Box 0,0 10,10 10").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("Array 3 2 1 20,20,0").unwrap();
        assert_eq!(e.doc().len(), 6);
        e.run_line("New").unwrap();
        e.run_line("Circle 100,0 5").unwrap();
        e.run_line("SelAll").unwrap();
        e.run_line("ArrayPolar 0,0 4").unwrap();
        assert_eq!(e.doc().len(), 4);
        assert!(e.doc().dump().contains("center 0,100,0") || e.doc().dump().contains("r 5"));
        e.run_line("ArrayLinear 3 0,0 0,50").unwrap();
        assert_eq!(e.doc().len(), 6);
    }
}
