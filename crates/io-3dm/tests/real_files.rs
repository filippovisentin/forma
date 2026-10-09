//! Checks against Filippo's real models in `tests/data/private/` (not in git).
//! Each test is skipped when its file is missing, so CI stays green without them.
//! Expected values were cross-checked with McNeel's rhino3dm (Python).

use forma_io_3dm::{read_summary, ObjectKind as K, Units};
use std::path::PathBuf;

fn private(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/private")
        .join(name);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skipping: {} not present", p.display());
        None
    }
}

#[test]
fn binario() {
    let Some(p) = private("binario.3dm") else {
        return;
    };
    let s = read_summary(p).unwrap();
    assert_eq!(s.archive_version, 80);
    assert_eq!(s.units, Units::Centimeters);
    assert!((s.absolute_tolerance - 0.01).abs() < 1e-9);
    assert_eq!(s.objects.len(), 1167);
    assert_eq!(s.count(K::Line), 904);
    assert_eq!(s.count(K::Brep), 256);
    assert_eq!(s.count(K::Extrusion), 5);
    assert_eq!(s.count(K::PolyCurve), 2);
    assert_eq!(s.layers.len(), 9);
    assert_eq!(s.material_count, 4);
    assert_eq!(s.block_count, 3);
    let solids = s.objects.iter().filter(|o| o.is_solid).count();
    assert_eq!(solids, 256);
}

#[test]
fn gggg() {
    let Some(p) = private("gggg.3dm") else { return };
    let s = read_summary(p).unwrap();
    assert_eq!(s.units, Units::Centimeters);
    assert_eq!(s.objects.len(), 953);
    assert_eq!(s.count(K::Extrusion), 368);
    assert_eq!(s.count(K::Brep), 262);
    assert_eq!(s.count(K::PolyCurve), 205);
    assert_eq!(s.count(K::Polyline), 73);
    assert_eq!(s.count(K::NurbsCurve), 30);
    assert_eq!(s.count(K::Mesh), 11);
    assert_eq!(s.count(K::InstanceRef), 3);
    assert_eq!(s.count(K::Line), 1);
    assert_eq!(s.layers.len(), 14);
    assert_eq!(s.material_count, 25);
    assert!(s.layers.iter().any(|l| l == "muri::SOSTEGNI DA TERRA"));
    assert!(s.layers.iter().any(|l| l == "STRUTTURA::CORDE"));
    let on_supports = s
        .objects
        .iter()
        .filter(|o| o.layer.map(|i| s.layers[i].as_str()) == Some("muri::SOSTEGNI DA TERRA"))
        .count();
    assert_eq!(on_supports, 349);
    let e = s.extents().unwrap();
    assert!((e.max.x - e.min.x - 7002.0).abs() < 1.0);
}

#[test]
fn gggg_display() {
    use forma_io_3dm::{import_display, DisplayGeometry};
    let Some(p) = private("gggg.3dm") else {
        return;
    };
    let t = std::time::Instant::now();
    let imp = import_display(p).unwrap();
    let elapsed = t.elapsed();
    let mut meshes = 0;
    let mut tris = 0;
    let mut lines = 0;
    let mut missing = std::collections::BTreeMap::new();
    for o in &imp.objects {
        match &o.geometry {
            Some(DisplayGeometry::Mesh(m)) => {
                meshes += 1;
                tris += m.triangles.len();
                assert!(m
                    .positions
                    .iter()
                    .all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite()));
            }
            Some(DisplayGeometry::Polyline(_)) => lines += 1,
            None => *missing.entry(format!("{:?}", o.kind)).or_insert(0) += 1,
        }
    }
    eprintln!("gggg: {meshes} meshes ({tris} triangles), {lines} polylines, not displayed: {missing:?}, {elapsed:?}");
    assert_eq!(imp.layers.len(), 14);
    assert_eq!(meshes + lines + missing.values().sum::<usize>(), 953);
    // Every brep, extrusion and mesh must produce triangles.
    assert_eq!(meshes, 262 + 368 + 11);
    assert_eq!(lines, 1 + 73 + 205 + 30);
}
