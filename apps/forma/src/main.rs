//! Forma desktop app. The egui window arrives in milestone M1; until then this
//! is a minimal interactive command line over the engine.

use std::io::{self, BufRead, Write};

fn main() {
    let mut engine = forma_engine::Engine::new();
    println!(
        "Forma {} — type a command (Help, Exit)",
        env!("CARGO_PKG_VERSION")
    );
    let stdin = io::stdin();
    loop {
        print!("Command: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        match line.trim() {
            "" => continue,
            l if l.eq_ignore_ascii_case("exit") => break,
            l if l.eq_ignore_ascii_case("help") => {
                for (name, help) in engine.command_list() {
                    println!("  {name:<10} {help}");
                }
            }
            l if l.eq_ignore_ascii_case("list") => print!("{}", engine.doc().dump()),
            l => match engine.run_line(l) {
                Ok(msg) => println!("{msg}"),
                Err(e) => println!("error: {e}"),
            },
        }
    }
}
