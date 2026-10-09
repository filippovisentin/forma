//! Debug helper: heaviest display meshes in a file.
//! `cargo run --release -p forma-io-3dm --example top_meshes -- file.3dm`
use forma_io_3dm::{import_display, DisplayGeometry};

fn main() {
    let path = std::env::args().nth(1).expect("path");
    let imp = import_display(&path).expect("read");
    let mut v: Vec<(usize, usize, String)> = imp
        .objects
        .iter()
        .enumerate()
        .filter_map(|(i, o)| match &o.geometry {
            Some(DisplayGeometry::Mesh(m)) => Some((
                m.triangles.len(),
                i,
                format!(
                    "{:?} on {}",
                    o.kind,
                    o.layer.map_or("?", |l| imp.layers[l].name.as_str())
                ),
            )),
            _ => None,
        })
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (n, i, what) in v.iter().take(12) {
        println!("{n:>8} triangles  #{i}  {what}");
    }
    let total: usize = v.iter().map(|x| x.0).sum();
    println!("total {total}");
}
