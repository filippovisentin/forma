//! Building a document from a `.3dm` file.

use crate::CommandError;
use forma_doc::{Document, Geometry, LayerId, LengthUnit};
use forma_io_3dm::{import_display, DisplayGeometry, Units};

fn unit(u: Units) -> LengthUnit {
    match u {
        Units::Centimeters => LengthUnit::Centimeters,
        Units::Meters => LengthUnit::Meters,
        Units::Inches => LengthUnit::Inches,
        Units::Feet => LengthUnit::Feet,
        _ => LengthUnit::Millimeters,
    }
}

/// Read a Rhino file into a fresh document. Objects that cannot be displayed yet
/// (block instances, annotations) are skipped.
pub fn open_3dm(path: &str) -> Result<Document, CommandError> {
    let imp = import_display(path).map_err(|e| CommandError::Invalid(e.to_string()))?;
    let mut doc = Document::new();
    doc.units = unit(imp.summary.units);
    doc.absolute_tolerance = imp.summary.absolute_tolerance;
    doc.path = Some(path.to_string());
    // The file's layers replace the default one.
    doc.layers.clear();
    let ids: Vec<LayerId> = imp
        .layers
        .iter()
        .map(|l| doc.add_layer(&l.name, l.color, l.visible))
        .collect();
    if doc.layers.is_empty() {
        doc.add_layer("Default", [0, 0, 0], true);
    }
    doc.current_layer = LayerId(0);

    {
        let mut t = doc.begin();
        for o in imp.objects {
            let layer = o.layer.map_or(LayerId(0), |i| ids[i]);
            let geometry = match o.geometry {
                Some(DisplayGeometry::Mesh(m)) => Geometry::Mesh(m),
                Some(DisplayGeometry::Polyline(p)) => Geometry::Polyline(p),
                None => continue,
            };
            let id = t.add_on_layer(geometry, layer);
            if o.color.is_some() {
                t.set_color(id, o.color);
            }
        }
        t.commit();
    }
    // Opening is not an undoable step.
    doc.clear_history();
    Ok(doc)
}
