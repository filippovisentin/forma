//! Writing a Forma document as a Rhino `.3dm` file.

use crate::{c_path, ffi, Error, Units};
use forma_doc::{Document, Geometry, LengthUnit};
use forma_geom::Seg;
use std::collections::HashMap;
use std::ffi::{c_int, CString};
use std::path::Path;

struct Writer(*mut ffi::Writer);

impl Drop for Writer {
    fn drop(&mut self) {
        // SAFETY: created by f3dm_writer_new, freed once.
        unsafe { ffi::f3dm_writer_free(self.0) }
    }
}

fn units(u: LengthUnit) -> Units {
    match u {
        LengthUnit::Millimeters => Units::Millimeters,
        LengthUnit::Centimeters => Units::Centimeters,
        LengthUnit::Meters => Units::Meters,
        LengthUnit::Inches => Units::Inches,
        LengthUnit::Feet => Units::Feet,
    }
}

/// Save `doc` to `path`. Curves are written as Rhino curves (lines, polylines,
/// arcs/circles, polycurves, NURBS curves), points as point objects; solids and
/// surfaces are written as meshes. Annotations are written in a simplified form
/// Rhino can display: text and text dots become text dots, dimensions become their
/// lines plus a text dot with the measured value (not editable Rhino dimensions).
pub fn write_document(path: impl AsRef<Path>, doc: &Document) -> Result<(), Error> {
    let path = path.as_ref();
    let cpath = c_path(path)?;
    // SAFETY: plain values in, owned handle out.
    let w =
        Writer(unsafe { ffi::f3dm_writer_new(units(doc.units).to_on(), doc.absolute_tolerance) });

    // Layers: Forma stores full paths ("muri::colonne"); Rhino needs a parent chain.
    let mut by_path: HashMap<String, c_int> = HashMap::new();
    let ensure =
        |path: &str, rgb: [u8; 3], visible: bool, by_path: &mut HashMap<String, c_int>| -> c_int {
            let mut parent: c_int = -1;
            let mut acc = String::new();
            let parts: Vec<&str> = path.split("::").collect();
            for (i, part) in parts.iter().enumerate() {
                if !acc.is_empty() {
                    acc.push_str("::");
                }
                acc.push_str(part);
                if let Some(&idx) = by_path.get(&acc) {
                    parent = idx;
                    continue;
                }
                let last = i + 1 == parts.len();
                let name =
                    CString::new(*part).unwrap_or_else(|_| CString::new("Layer").expect("static"));
                let color = if last { rgb } else { [0, 0, 0] };
                // SAFETY: valid writer, NUL-terminated name, 3-byte colour.
                let idx = unsafe {
                    ffi::f3dm_writer_layer(
                        w.0,
                        name.as_ptr(),
                        parent,
                        color.as_ptr(),
                        c_int::from(!last || visible),
                    )
                };
                by_path.insert(acc.clone(), idx);
                parent = idx;
            }
            parent
        };
    let layer_index: Vec<c_int> = doc
        .layers
        .iter()
        .map(|l| ensure(&l.name, l.color, l.visible, &mut by_path))
        .collect();

    for o in doc.objects() {
        let layer = layer_index.get(o.layer.0).copied().unwrap_or(-1);
        let rgb = o.color.unwrap_or([0, 0, 0]);
        // SAFETY: valid writer; rgb outlives the call.
        unsafe { ffi::f3dm_writer_color(w.0, c_int::from(o.color.is_some()), rgb.as_ptr()) };
        let name = o
            .name
            .as_deref()
            .and_then(|n| CString::new(n).ok())
            .unwrap_or_default();
        // SAFETY: valid writer, NUL-terminated name.
        unsafe { ffi::f3dm_writer_name(w.0, name.as_ptr()) };
        // SAFETY (all calls): valid writer; buffers outlive the call.
        let ok = unsafe {
            match &o.geometry {
                Geometry::Line(l) => {
                    let a = [l.from.x, l.from.y, l.from.z];
                    let b = [l.to.x, l.to.y, l.to.z];
                    ffi::f3dm_writer_line(w.0, layer, a.as_ptr(), b.as_ptr())
                }
                Geometry::Polyline(p) => {
                    let xyz: Vec<f64> = p.iter().flat_map(|q| [q.x, q.y, q.z]).collect();
                    ffi::f3dm_writer_polyline(w.0, layer, xyz.as_ptr(), p.len() as c_int)
                }
                Geometry::Arc(a) => {
                    let c = a.center();
                    let (x, y) = (a.plane.x, a.plane.y);
                    ffi::f3dm_writer_arc(
                        w.0,
                        layer,
                        [c.x, c.y, c.z].as_ptr(),
                        [x.x, x.y, x.z].as_ptr(),
                        [y.x, y.y, y.z].as_ptr(),
                        a.radius,
                        a.sweep,
                    )
                }
                Geometry::PolyCurve(segs) => {
                    let mut data: Vec<f64> = Vec::with_capacity(segs.len() * 12);
                    for s in segs {
                        match s {
                            Seg::Line(a, b) => {
                                data.extend([0.0, a.x, a.y, a.z, b.x, b.y, b.z]);
                                data.extend([0.0; 5]);
                            }
                            Seg::Arc(a) => {
                                let c = a.center();
                                let (x, y) = (a.plane.x, a.plane.y);
                                data.extend([1.0, c.x, c.y, c.z, x.x, x.y, x.z, y.x, y.y, y.z]);
                                data.extend([a.radius, a.sweep]);
                            }
                        }
                    }
                    ffi::f3dm_writer_polycurve(w.0, layer, data.as_ptr(), segs.len() as c_int)
                }
                Geometry::Point(p) => ffi::f3dm_writer_point(w.0, layer, [p.x, p.y, p.z].as_ptr()),
                Geometry::Text(_) | Geometry::Dimension(_) => {
                    let mut ok = 1;
                    for [a, b] in o.geometry.annotation_lines() {
                        ok &= ffi::f3dm_writer_line(
                            w.0,
                            layer,
                            [a.x, a.y, a.z].as_ptr(),
                            [b.x, b.y, b.z].as_ptr(),
                        );
                    }
                    if let Some((p, text, _)) = o.geometry.label() {
                        let text = CString::new(text).unwrap_or_default();
                        ok &= ffi::f3dm_writer_textdot(
                            w.0,
                            layer,
                            [p.x, p.y, p.z].as_ptr(),
                            text.as_ptr(),
                            14,
                        );
                    }
                    ok
                }
                Geometry::Nurbs(n) => {
                    let xyz: Vec<f64> = n.points.iter().flat_map(|q| [q.x, q.y, q.z]).collect();
                    ffi::f3dm_writer_nurbs(
                        w.0,
                        layer,
                        n.degree as c_int,
                        n.points.len() as c_int,
                        xyz.as_ptr(),
                        n.weights.as_ptr(),
                        n.knots.as_ptr(),
                    )
                }
                Geometry::Mesh(m) => {
                    let xyz: Vec<f64> = m.positions.iter().flat_map(|q| [q.x, q.y, q.z]).collect();
                    let nrm: Vec<f64> = m.normals.iter().flat_map(|n| [n.x, n.y, n.z]).collect();
                    let tri: Vec<u32> = m.triangles.iter().flatten().copied().collect();
                    let normals = if m.normals.len() == m.positions.len() {
                        nrm.as_ptr()
                    } else {
                        std::ptr::null()
                    };
                    ffi::f3dm_writer_mesh(
                        w.0,
                        layer,
                        xyz.as_ptr(),
                        normals,
                        m.positions.len() as c_int,
                        tri.as_ptr(),
                        m.triangles.len() as c_int,
                    )
                }
            }
        };
        if ok == 0 {
            return Err(Error::WriteFailed(format!(
                "{} (object #{})",
                path.display(),
                o.id.0
            )));
        }
    }

    // SAFETY: valid writer and path.
    if unsafe { ffi::f3dm_writer_save(w.0, cpath.as_ptr()) } == 1 {
        Ok(())
    } else {
        Err(Error::WriteFailed(path.display().to_string()))
    }
}
