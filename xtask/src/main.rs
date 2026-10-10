//! Dev automation. `cargo xtask ci` is the gate every agent must pass.
//!
//! - `cargo xtask ci`        fmt check, clippy -D warnings, tests, layering
//! - `cargo xtask ci --occt` the same with the OpenCascade solid kernel
//!   (`forma-geom/occt`; set `FORMA_OCCT_DIR` to reuse an OCCT build)
//! - `cargo xtask layering`  check that crates only depend on lower layers

use std::path::Path;
use std::process::{Command, ExitCode};

/// Layer of every workspace crate. A crate may depend only on strictly lower layers.
const LAYERS: &[(&str, u8)] = &[
    ("forma-geom", 0),
    ("forma-doc", 1),
    ("forma-io-3dm", 2),
    ("forma-io-mesh", 2),
    ("forma-render", 2),
    ("forma-engine", 3),
    ("forma-ui", 4),
    ("forma-mcp", 4),
    ("forma", 5),
    ("forma-cli", 5),
];

const MANIFESTS: &[&str] = &[
    "crates/geom",
    "crates/doc",
    "crates/io-3dm",
    "crates/io-mesh",
    "crates/render",
    "crates/engine",
    "crates/ui",
    "crates/mcp",
    "apps/forma",
    "apps/forma-cli",
];

fn layer_of(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

/// Very small TOML reader: package name and `forma-*` keys in `[dependencies]`.
fn read_manifest(path: &Path) -> Result<(String, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut section = String::new();
    let mut name = None;
    let mut deps = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if section == "[package]" && key == "name" {
            name = Some(value.trim().trim_matches('"').to_string());
        }
        let base = key.split('.').next().unwrap_or(key);
        if section.contains("dependencies") && base.starts_with("forma") {
            deps.push(base.to_string());
        }
    }
    Ok((name.ok_or("missing package name")?, deps))
}

fn layering() -> Result<(), String> {
    let mut errors = Vec::new();
    for dir in MANIFESTS {
        let (name, deps) = read_manifest(&Path::new(dir).join("Cargo.toml"))?;
        let own = layer_of(&name).ok_or(format!("{name} has no layer in xtask LAYERS"))?;
        for d in deps {
            match layer_of(&d) {
                Some(l) if l < own => {}
                Some(l) => errors.push(format!("{name} (layer {own}) depends on {d} (layer {l})")),
                None => errors.push(format!("{name} depends on unknown crate {d}")),
            }
        }
    }
    if errors.is_empty() {
        println!("layering: ok");
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn cargo(args: &[&str]) -> Result<(), String> {
    println!("$ cargo {}", args.join(" "));
    let status = Command::new(env!("CARGO"))
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

fn ci(occt: bool) -> Result<(), String> {
    let features: &[&str] = if occt {
        &["--features", "forma-geom/occt"]
    } else {
        &[]
    };
    cargo(&["fmt", "--all", "--check"])?;
    let mut clippy = vec!["clippy", "--workspace", "--all-targets"];
    clippy.extend_from_slice(features);
    clippy.extend_from_slice(&["--", "-D", "warnings"]);
    cargo(&clippy)?;
    let mut test = vec!["test", "--workspace"];
    test.extend_from_slice(features);
    cargo(&test)?;
    layering()
}

fn main() -> ExitCode {
    // Run from the workspace root regardless of where cargo was invoked.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    std::env::set_current_dir(root).expect("chdir to workspace root");

    let occt = std::env::args().skip(2).any(|a| a == "--occt");
    let result = match std::env::args().nth(1).as_deref() {
        Some("ci") => ci(occt),
        Some("layering") => layering(),
        _ => Err("usage: cargo xtask <ci [--occt]|layering>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
