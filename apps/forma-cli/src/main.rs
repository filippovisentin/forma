//! Headless Forma: run scripts and inspect the result.
//!
//! ```text
//! forma-cli run --script "Line 0,0 @100,0; Undo" [--file script.txt] [--dump]
//! forma-cli commands
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
