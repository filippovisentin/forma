//! Writes a small `.3dm` for the black-box check in Rhino:
//! `cargo run -p forma-io-3dm --example write_line -- out.3dm`
//! Expected in Rhino: units cm, one line from 0,0,0 to 100,50,25 on layer "Forma prova".

use forma_geom::Point3;
use forma_io_3dm::{write_line, Units};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "forma_prova.3dm".into());
    write_line(
        &path,
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(100.0, 50.0, 25.0),
        "Forma prova",
        Units::Centimeters,
        0.01,
    )
    .expect("write failed");
    println!("wrote {path}");
}
