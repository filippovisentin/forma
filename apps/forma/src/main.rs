//! Forma desktop app. Usage: `forma [file.3dm]`.
//! The interactive text shell moved to `forma-cli`.

// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let open = std::env::args().nth(1);
    if let Err(e) = forma_ui::run(open) {
        eprintln!("Forma could not start: {e}");
        std::process::exit(1);
    }
}
