//! Display geometry from `.3dm` files: triangle meshes for breps, extrusions and
//! meshes, polylines for curves.
//!
//! Public openNURBS cannot mesh breps and many files are saved without render meshes,
//! so each brep face is triangulated here: the trimming loops are sampled in the
//! face's (u, v) domain, a constrained Delaunay triangulation (spade) is built from the
//! loops plus an interior grid, triangles outside the trimmed region are dropped, and
//! the vertices are evaluated on the surface by openNURBS. Good enough for display;
//! the solid kernel (ADR 0001) will replace it for modelling.

use crate::{ffi, Error, ModelHandle, ObjectKind, Summary};
use forma_geom::{Mesh, Point3, Vec3};
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
use std::ffi::c_int;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedLayer {
    /// Full path, `parent::child`.
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayGeometry {
    Mesh(Mesh),
    Polyline(Vec<Point3>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedObject {
    pub kind: ObjectKind,
    /// Index into [`Import::layers`].
    pub layer: Option<usize>,
    /// `None` for objects that cannot be displayed yet (block instances, annotations…).
    pub geometry: Option<DisplayGeometry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub summary: Summary,
    pub layers: Vec<ImportedLayer>,
    pub objects: Vec<ImportedObject>,
}

/// Read a `.3dm` file into display geometry.
pub fn import_display(path: impl AsRef<Path>) -> Result<Import, Error> {
    let model = ModelHandle::open(path.as_ref())?;
    let summary = model.summary();
    let m = model.ptr();

    let layers = (0..summary.layers.len())
        .map(|i| {
            let mut rgb = [0u8; 3];
            let mut visible: c_int = 1;
            // SAFETY: model valid, index in range, out-pointers valid.
            unsafe { ffi::f3dm_layer_display(m, i as c_int, rgb.as_mut_ptr(), &mut visible) };
            ImportedLayer {
                name: summary.layers[i].clone(),
                color: rgb,
                visible: visible != 0,
            }
        })
        .collect();

    let objects = summary
        .objects
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let obj = i as c_int;
            // SAFETY (all calls below): model valid for this scope, obj in range.
            let geometry = match info.kind {
                ObjectKind::Brep | ObjectKind::Extrusion => {
                    let mesh = unsafe { brep_mesh(m, obj) };
                    (!mesh.triangles.is_empty()).then_some(DisplayGeometry::Mesh(mesh))
                }
                ObjectKind::Mesh => unsafe { mesh_object(m, obj) }.map(DisplayGeometry::Mesh),
                ObjectKind::Line
                | ObjectKind::Polyline
                | ObjectKind::PolyCurve
                | ObjectKind::NurbsCurve
                | ObjectKind::Arc => {
                    let pts = unsafe { curve_points(m, obj) };
                    (pts.len() >= 2).then_some(DisplayGeometry::Polyline(pts))
                }
                _ => None,
            };
            ImportedObject {
                kind: info.kind,
                layer: info.layer,
                geometry,
            }
        })
        .collect();

    Ok(Import {
        summary,
        layers,
        objects,
    })
}

unsafe fn curve_points(m: *mut ffi::Model, obj: c_int) -> Vec<Point3> {
    let n = ffi::f3dm_curve_points(m, obj);
    if n <= 0 {
        return Vec::new();
    }
    let mut buf = vec![0.0; n as usize * 3];
    ffi::f3dm_points_copy(m, buf.as_mut_ptr(), n);
    buf.chunks(3)
        .map(|c| Point3::new(c[0], c[1], c[2]))
        .collect()
}

unsafe fn mesh_object(m: *mut ffi::Model, obj: c_int) -> Option<Mesh> {
    let (mut nv, mut nt) = (0, 0);
    if ffi::f3dm_mesh_data(m, obj, &mut nv, &mut nt) == 0 || nv == 0 || nt == 0 {
        return None;
    }
    let mut v = vec![0.0; nv as usize * 3];
    let mut t = vec![0u32; nt as usize * 3];
    ffi::f3dm_mesh_copy(m, v.as_mut_ptr(), t.as_mut_ptr());
    Some(Mesh {
        positions: v.chunks(3).map(|c| Point3::new(c[0], c[1], c[2])).collect(),
        normals: Vec::new(),
        triangles: t.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
    })
}

unsafe fn brep_mesh(m: *mut ffi::Model, obj: c_int) -> Mesh {
    let mut out = Mesh::default();
    for face in 0..ffi::f3dm_face_count(m, obj) {
        if let Some(fm) = face_mesh(m, obj, face) {
            out.append(&fm);
        }
    }
    out
}

struct FaceInfo {
    domain: [f64; 4],
    spans: [i32; 2],
    degree: [i32; 2],
    reversed: bool,
}

unsafe fn face_mesh(m: *mut ffi::Model, obj: c_int, face: c_int) -> Option<Mesh> {
    let mut domain = [0.0; 4];
    let mut spans = [0 as c_int; 2];
    let mut degree = [0 as c_int; 2];
    let mut rev: c_int = 0;
    if ffi::f3dm_face_info(
        m,
        obj,
        face,
        domain.as_mut_ptr(),
        spans.as_mut_ptr(),
        degree.as_mut_ptr(),
        &mut rev,
    ) == 0
    {
        return None;
    }
    let info = FaceInfo {
        domain,
        spans,
        degree,
        reversed: rev != 0,
    };

    let mut loops = Vec::new();
    for k in 0..ffi::f3dm_face_loops(m, obj, face) {
        let mut outer: c_int = 0;
        let n = ffi::f3dm_loop_points(m, k, std::ptr::null_mut(), 0, &mut outer);
        let mut buf = vec![0.0; n as usize * 2];
        ffi::f3dm_loop_points(m, k, buf.as_mut_ptr(), n, &mut outer);
        let pts: Vec<[f64; 2]> = buf.chunks(2).map(|c| [c[0], c[1]]).collect();
        if pts.len() >= 3 {
            loops.push(pts);
        }
    }
    if loops.is_empty() {
        let [u0, u1, v0, v1] = info.domain;
        loops.push(vec![[u0, v0], [u1, v0], [u1, v1], [u0, v1]]);
    }

    let (uv, tris) = triangulate_trimmed(&loops, &info);
    if tris.is_empty() {
        return None;
    }

    let n = uv.len();
    let flat: Vec<f64> = uv.iter().flat_map(|p| [p[0], p[1]]).collect();
    let mut xyz = vec![0.0; n * 3];
    let mut nrm = vec![0.0; n * 3];
    ffi::f3dm_face_eval(
        m,
        obj,
        face,
        n as c_int,
        flat.as_ptr(),
        xyz.as_mut_ptr(),
        nrm.as_mut_ptr(),
    );
    let positions: Vec<Point3> = xyz
        .chunks(3)
        .map(|c| Point3::new(c[0], c[1], c[2]))
        .collect();
    let normals: Vec<Vec3> = nrm.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    let triangles = tris
        .into_iter()
        .map(|[a, b, c]| if info.reversed { [a, c, b] } else { [a, b, c] })
        .collect();
    Some(Mesh {
        positions,
        normals,
        triangles,
    })
}

/// Even-odd point-in-region test against all loops.
fn inside(p: [f64; 2], loops: &[Vec<[f64; 2]>]) -> bool {
    let mut inside = false;
    for l in loops {
        let n = l.len();
        for i in 0..n {
            let [x0, y0] = l[i];
            let [x1, y1] = l[(i + 1) % n];
            if (y0 > p[1]) != (y1 > p[1]) {
                let x = x0 + (p[1] - y0) / (y1 - y0) * (x1 - x0);
                if p[0] < x {
                    inside = !inside;
                }
            }
        }
    }
    inside
}

/// Number of interior grid intervals along one parameter direction.
fn intervals(spans: i32, degree: i32) -> usize {
    if degree <= 1 {
        spans.max(1) as usize
    } else {
        (spans.max(1) * degree * 2).clamp(4, 32) as usize
    }
}

/// Constrained triangulation of a trimmed (u, v) region. Returns the vertex
/// positions in (u, v) and counter-clockwise triangles inside the region.
fn triangulate_trimmed(loops: &[Vec<[f64; 2]>], info: &FaceInfo) -> (Vec<[f64; 2]>, Vec<[u32; 3]>) {
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();

    for l in loops {
        let handles: Vec<_> = l
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .filter_map(|p| cdt.insert(Point2::new(p[0], p[1])).ok())
            .collect();
        for i in 0..handles.len() {
            let (a, b) = (handles[i], handles[(i + 1) % handles.len()]);
            if a != b && cdt.can_add_constraint(a, b) {
                cdt.add_constraint(a, b);
            }
        }
    }

    let [u0, u1, v0, v1] = info.domain;
    let nu = intervals(info.spans[0], info.degree[0]);
    let nv = intervals(info.spans[1], info.degree[1]);
    if nu > 1 || nv > 1 {
        for i in 1..nu {
            for j in 1..nv {
                let p = [
                    u0 + (u1 - u0) * i as f64 / nu as f64,
                    v0 + (v1 - v0) * j as f64 / nv as f64,
                ];
                if inside(p, loops) {
                    let _ = cdt.insert(Point2::new(p[0], p[1]));
                }
            }
        }
    }

    let uv: Vec<[f64; 2]> = cdt
        .vertices()
        .map(|v| [v.position().x, v.position().y])
        .collect();
    let mut tris = Vec::new();
    for f in cdt.inner_faces() {
        let vs = f.vertices();
        let c = vs.iter().fold([0.0, 0.0], |acc, v| {
            [acc[0] + v.position().x / 3.0, acc[1] + v.position().y / 3.0]
        });
        if inside(c, loops) {
            tris.push(vs.map(|v| v.fix().index() as u32));
        }
    }
    (uv, tris)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, s: f64) -> Vec<[f64; 2]> {
        vec![[x0, y0], [x0 + s, y0], [x0 + s, y0 + s], [x0, y0 + s]]
    }

    fn area(uv: &[[f64; 2]], tris: &[[u32; 3]]) -> f64 {
        tris.iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| uv[i as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) / 2.0
            })
            .sum()
    }

    #[test]
    fn square_with_hole_keeps_only_the_ring() {
        let loops = vec![square(0.0, 0.0, 10.0), square(4.0, 4.0, 2.0)];
        let info = FaceInfo {
            domain: [0.0, 10.0, 0.0, 10.0],
            spans: [1, 1],
            degree: [3, 3],
            reversed: false,
        };
        let (uv, tris) = triangulate_trimmed(&loops, &info);
        let a = area(&uv, &tris);
        assert!((a - 96.0).abs() < 1e-9, "area {a}");
        assert!(tris.iter().all(|t| {
            let [a, b, c] = t.map(|i| uv[i as usize]);
            (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]) > 0.0
        }));
    }

    #[test]
    fn planar_face_has_no_interior_points() {
        let loops = vec![square(0.0, 0.0, 1.0)];
        let info = FaceInfo {
            domain: [0.0, 1.0, 0.0, 1.0],
            spans: [1, 1],
            degree: [1, 1],
            reversed: false,
        };
        let (uv, tris) = triangulate_trimmed(&loops, &info);
        assert_eq!(uv.len(), 4);
        assert_eq!(tris.len(), 2);
    }
}
