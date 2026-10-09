//! Headless Forma: run scripts and inspect the result.
//!
//! ```text
//! forma-cli run --script "Line 0,0 @100,0; Undo" [--file script.txt] [--dump]
//! forma-cli commands
//! forma-cli info model.3dm
//! ```

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  forma-cli run [--script <text>] [--file <path>] [--dump]\n  forma-cli commands"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut engine = forma_engine::Engine::new();
    match args.first().map(String::as_str) {
        Some("commands") => {
            for (name, help) in engine.command_list() {
                println!("{name:<10} {help}");
            }
            ExitCode::SUCCESS
        }
        Some("info") => match args.get(1) {
            Some(path) => info(path),
            None => usage(),
        },
        Some("run") => {
            let mut script = String::new();
            let mut dump = false;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--script" => match it.next() {
                        Some(s) => script.push_str(&format!("{s}\n")),
                        None => return usage(),
                    },
                    "--file" => match it.next().map(std::fs::read_to_string) {
                        Some(Ok(s)) => script.push_str(&s),
                        Some(Err(e)) => {
                            eprintln!("cannot read script: {e}");
                            return ExitCode::FAILURE;
                        }
                        None => return usage(),
                    },
                    "--dump" => dump = true,
                    _ => return usage(),
                }
            }
            match engine.run_script(&script) {
                Ok(msgs) => {
                    for m in msgs {
                        eprintln!("{m}");
                    }
                    if dump {
                        print!("{}", engine.doc().dump());
                    }
                    ExitCode::SUCCESS
                }
                Err((line, e)) => {
                    eprintln!("line {line}: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}

fn info(path: &str) -> ExitCode {
    use std::collections::BTreeMap;
    let s = match forma_io_3dm::read_summary(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    println!("{path}");
    println!(
        "  version {} | units {:?} | tolerance {} | angle {:.2}°",
        s.archive_version, s.units, s.absolute_tolerance, s.angle_tolerance_deg
    );
    println!(
        "  {} objects, {} layers, {} materials, {} blocks",
        s.objects.len(),
        s.layers.len(),
        s.material_count,
        s.block_count
    );
    let mut kinds: BTreeMap<_, usize> = BTreeMap::new();
    let mut per_layer: BTreeMap<&str, usize> = BTreeMap::new();
    for o in &s.objects {
        *kinds.entry(o.kind).or_default() += 1;
        let layer = o.layer.map_or("?", |i| s.layers[i].as_str());
        *per_layer.entry(layer).or_default() += 1;
    }
    println!("  by type:");
    for (k, n) in kinds {
        println!("    {:<12} {n}", format!("{k:?}"));
    }
    println!("  by layer:");
    for (l, n) in per_layer {
        println!("    {n:>6}  {l}");
    }
    if let Some(e) = s.extents() {
        println!(
            "  extents {:.0} x {:.0} x {:.0}",
            e.max.x - e.min.x,
            e.max.y - e.min.y,
            e.max.z - e.min.z
        );
    }
    ExitCode::SUCCESS
}
