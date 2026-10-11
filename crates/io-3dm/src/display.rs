//! Display geometry from `.3dm` files: triangle meshes for breps, extrusions and
//! meshes, polylines for curves, points for point objects.
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
use std::collections::HashMap;
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
    /// Exact surface or polysurface (brep, extrusion, surface): the native
    /// object as bytes (see [`crate::exact`]) with its display mesh.
    Brep(forma_geom::Brep),
    Polyline(Vec<Point3>),
    Point(Point3),
    /// A curve read exactly: line, arc / circle, polyline, polycurve of lines
    /// and arcs, or NURBS curve.
    Curve(forma_doc::Geometry),
    /// Text (`dot` = text dot): plane origin and axes, height (0 for dots).
    Text {
        origin: Point3,
        x: Vec3,
        y: Vec3,
        height: f64,
        text: String,
        dot: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedObject {
    pub kind: ObjectKind,
    /// Index into [`Import::layers`].
    pub layer: Option<usize>,
    /// Object colour, `None` = by layer.
    pub color: Option<[u8; 3]>,
    /// Object name, if any.
    pub name: Option<String>,
    /// `None` for objects that cannot be displayed yet (annotations…).
    pub geometry: Option<DisplayGeometry>,
    /// Block instance this object comes from (members of one instance share the
    /// number); `None` for plain objects. Instances are expanded on import.
    pub instance: Option<usize>,
    /// Hidden / locked in the file.
    pub hidden: bool,
    pub locked: bool,
    /// The file's group of the object (Rhino group index), if any.
    pub group: Option<usize>,
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
                ObjectKind::Brep | ObjectKind::Extrusion | ObjectKind::Surface => {
                    let mut len = 0usize;
                    let p = unsafe { ffi::f3dm_object_bytes(m, obj, &mut len) };
                    let data = unsafe { crate::exact::take_bytes(p, len) }.unwrap_or_default();
                    let mut raw = ffi::Object::default();
                    unsafe { ffi::f3dm_object(m, obj, &mut raw) };
                    let b = unsafe { brep_of_object(m, obj, &raw, data) };
                    if b.mesh.triangles.is_empty() {
                        None
                    } else if b.data.is_empty() {
                        Some(DisplayGeometry::Mesh(b.mesh))
                    } else {
                        Some(DisplayGeometry::Brep(b))
                    }
                }
                ObjectKind::Mesh => unsafe { mesh_object(m, obj) }.map(DisplayGeometry::Mesh),
                ObjectKind::Line
                | ObjectKind::Polyline
                | ObjectKind::PolyCurve
                | ObjectKind::NurbsCurve
                | ObjectKind::Arc => {
                    let mut n = 0usize;
                    let p = unsafe { ffi::f3dm_curve_exact(m, obj, &mut n) };
                    let exact = (!p.is_null())
                        .then(|| {
                            // SAFETY: n doubles from the shim, freed right after.
                            let s = unsafe { std::slice::from_raw_parts(p, n) }.to_vec();
                            unsafe { ffi::f3dm_bytes_free(p.cast()) };
                            crate::exact::curve_from_stream(&s)
                        })
                        .flatten();
                    match exact {
                        Some(g) => Some(DisplayGeometry::Curve(g)),
                        None => {
                            let pts = unsafe { curve_points(m, obj) };
                            (pts.len() >= 2).then_some(DisplayGeometry::Polyline(pts))
                        }
                    }
                }
                ObjectKind::Point => {
                    let mut xyz = [0.0; 3];
                    (unsafe { ffi::f3dm_point(m, obj, xyz.as_mut_ptr()) } != 0)
                        .then(|| DisplayGeometry::Point(Point3::new(xyz[0], xyz[1], xyz[2])))
                }
                ObjectKind::Annotation | ObjectKind::TextDot => unsafe { text_object(m, obj) },
                _ => None,
            };
            let mut rgb = [0u8; 3];
            let has = unsafe { ffi::f3dm_object_color(m, obj, rgb.as_mut_ptr()) } != 0;
            let mut buf = vec![0u8; 256];
            // SAFETY: buffer of the given capacity.
            let n = unsafe {
                ffi::f3dm_object_name(m, obj, buf.as_mut_ptr().cast(), buf.len() as c_int)
            };
            let name = (n > 0).then(|| {
                let len = (n as usize).min(buf.len() - 1);
                String::from_utf8_lossy(&buf[..len]).into_owned()
            });
            // SAFETY: model valid, obj in range.
            let instance = usize::try_from(unsafe { ffi::f3dm_object_instance(m, obj) }).ok();
            let mut group: c_int = -1;
            // SAFETY: model valid, obj in range, out-pointer valid.
            let state = unsafe { ffi::f3dm_object_state(m, obj, &mut group) };
            ImportedObject {
                instance,
                hidden: state & 1 != 0,
                locked: state & 2 != 0,
                group: usize::try_from(group).ok(),
                kind: info.kind,
                layer: info.layer,
                color: has.then_some(rgb),
                name,
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

unsafe fn text_object(m: *mut ffi::Model, obj: c_int) -> Option<DisplayGeometry> {
    let mut plane = [0.0f64; 9];
    let mut height = 0.0;
    let mut len: c_int = 0;
    let mut buf = vec![0u8; 1024];
    let kind = ffi::f3dm_text(
        m,
        obj,
        plane.as_mut_ptr(),
        &mut height,
        buf.as_mut_ptr().cast(),
        buf.len() as c_int,
        &mut len,
    );
    if kind == 0 {
        return None;
    }
    let n = usize::try_from(len).unwrap_or(0).min(buf.len() - 1);
    let text = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(DisplayGeometry::Text {
        origin: Point3::new(plane[0], plane[1], plane[2]),
        x: Vec3::new(plane[3], plane[4], plane[5]),
        y: Vec3::new(plane[6], plane[7], plane[8]),
        height,
        text,
        dot: kind == 2,
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

/// A [`Brep`](forma_geom::Brep) for brep object `obj` of the model: display
/// mesh, face data and snap points, carrying `data` (its serialized form).
pub(crate) unsafe fn brep_of_object(
    m: *mut ffi::Model,
    obj: c_int,
    info: &ffi::Object,
    data: Vec<u8>,
) -> forma_geom::Brep {
    let (mesh, face_ends, wires) = brep_mesh_faces(m, obj);
    let mut n_edges = 0usize;
    let p = ffi::f3dm_object_edges(m, obj, &mut n_edges);
    let arcs = crate::exact::take_stream_pub(p, n_edges)
        .map(|s| crate::exact::arcs_in_stream(&s))
        .unwrap_or_default();
    let mut nverts: c_int = 0;
    let n = ffi::f3dm_brep_points(m, obj, &mut nverts);
    let mut buf = vec![0.0; n.max(0) as usize * 3];
    ffi::f3dm_points_copy(m, buf.as_mut_ptr(), n);
    let mut pts: Vec<Point3> = buf
        .chunks(3)
        .map(|c| Point3::new(c[0], c[1], c[2]))
        .collect();
    let edge_mids = pts.split_off((nverts.max(0) as usize).min(pts.len()));
    forma_geom::Brep {
        data: std::sync::Arc::new(data),
        xform: forma_geom::Xform::IDENTITY,
        mesh,
        faces: usize::try_from(info.brep_faces).unwrap_or(0),
        solid: info.is_solid != 0,
        planar: info.all_planar != 0,
        face_ends,
        vertices: pts,
        edge_mids,
        arcs,
        wires,
    }
}

/// Display mesh of a brep, per face the end of its triangles, and the
/// isocurves of its curved faces.
pub(crate) unsafe fn brep_mesh_faces(
    m: *mut ffi::Model,
    obj: c_int,
) -> (Mesh, Vec<u32>, Vec<Vec<Point3>>) {
    let mut out = Mesh::default();
    let mut ends = Vec::new();
    let mut wires = Vec::new();
    for face in 0..ffi::f3dm_face_count(m, obj) {
        if let Some((fm, w)) = face_mesh(m, obj, face) {
            out.append(&fm);
            wires.extend(w);
        }
        ends.push(out.triangles.len() as u32);
    }
    (out, ends, wires)
}

/// Isocurves inside the trimmed region of a curved face: three across each
/// parameter direction (like Rhino's default density), as 3D polylines.
unsafe fn face_isocurves(
    m: *mut ffi::Model,
    obj: c_int,
    face: c_int,
    info: &FaceInfo,
    loops: &[Vec<[f64; 2]>],
) -> Vec<Vec<Point3>> {
    if info.degree[0] <= 1 && info.degree[1] <= 1 {
        return Vec::new(); // flat faces show their edges only
    }
    const SAMPLES: usize = 48;
    let [u0, u1, v0, v1] = info.domain;
    let mut runs: Vec<Vec<[f64; 2]>> = Vec::new();
    for k in 1..4 {
        let f = k as f64 / 4.0;
        for dir in 0..2 {
            let mut run = Vec::new();
            for i in 0..=SAMPLES {
                let t = i as f64 / SAMPLES as f64;
                let uv = if dir == 0 {
                    [u0 + (u1 - u0) * f, v0 + (v1 - v0) * t]
                } else {
                    [u0 + (u1 - u0) * t, v0 + (v1 - v0) * f]
                };
                if inside(uv, loops) {
                    run.push(uv);
                } else if run.len() >= 2 {
                    runs.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
            }
            if run.len() >= 2 {
                runs.push(run);
            }
        }
    }
    runs.into_iter()
        .map(|r| {
            let flat: Vec<f64> = r.iter().flat_map(|p| [p[0], p[1]]).collect();
            let mut xyz = vec![0.0; r.len() * 3];
            let mut nrm = vec![0.0; r.len() * 3];
            ffi::f3dm_face_eval(
                m,
                obj,
                face,
                r.len() as c_int,
                flat.as_ptr(),
                xyz.as_mut_ptr(),
                nrm.as_mut_ptr(),
            );
            xyz.chunks(3)
                .map(|c| Point3::new(c[0], c[1], c[2]))
                .collect()
        })
        .collect()
}

struct FaceInfo {
    domain: [f64; 4],
    spans: [i32; 2],
    degree: [i32; 2],
    reversed: bool,
}

unsafe fn face_mesh(
    m: *mut ffi::Model,
    obj: c_int,
    face: c_int,
) -> Option<(Mesh, Vec<Vec<Point3>>)> {
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
    // Loop samples lie on the brep edges: their 3D points come from the edge, so
    // neighbouring faces share them exactly (a closed brep gives a closed mesh).
    let mut on_edges: HashMap<(u64, u64), Point3> = HashMap::new();
    for k in 0..ffi::f3dm_face_loops(m, obj, face) {
        let mut outer: c_int = 0;
        let n = ffi::f3dm_loop_points(m, k, std::ptr::null_mut(), 0, &mut outer);
        let mut buf = vec![0.0; n as usize * 2];
        ffi::f3dm_loop_points(m, k, buf.as_mut_ptr(), n, &mut outer);
        let mut xyz = vec![f64::NAN; n as usize * 3];
        ffi::f3dm_loop_xyz(m, k, xyz.as_mut_ptr(), n);
        let pts: Vec<[f64; 2]> = buf.chunks(2).map(|c| [c[0], c[1]]).collect();
        for (p, q) in pts.iter().zip(xyz.chunks(3)) {
            if q.iter().all(|x| x.is_finite()) {
                on_edges.insert(
                    (p[0].to_bits(), p[1].to_bits()),
                    Point3::new(q[0], q[1], q[2]),
                );
            }
        }
        if pts.len() >= 3 {
            loops.push(pts);
        }
    }
    if loops.is_empty() {
        let [u0, u1, v0, v1] = info.domain;
        loops.push(vec![[u0, v0], [u1, v0], [u1, v1], [u0, v1]]);
    }
    let (nu, nv) = grid_size(m, obj, face, &info);

    let (uv, tris) = triangulate_trimmed(&loops, &info, nu, nv);
    if tris.is_empty() {
        return None;
    }
    let wires = face_isocurves(m, obj, face, &info, &loops);

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
        .zip(&uv)
        .map(|(c, p)| {
            on_edges
                .get(&(p[0].to_bits(), p[1].to_bits()))
                .copied()
                .unwrap_or(Point3::new(c[0], c[1], c[2]))
        })
        .collect();
    let mut normals: Vec<Vec3> = nrm.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    let triangles: Vec<[u32; 3]> = tris
        .into_iter()
        .map(|[a, b, c]| if info.reversed { [a, c, b] } else { [a, b, c] })
        .collect();
    // Where the surface has no normal (poles, degenerate points) use the
    // average of the triangles around the vertex, so shading has no dark spots.
    let bad: Vec<bool> = normals
        .iter()
        .map(|n| !(n.x.is_finite() && n.y.is_finite() && n.z.is_finite()) || n.length() < 0.5)
        .collect();
    if bad.iter().any(|b| *b) {
        let mut acc = vec![Vec3::new(0.0, 0.0, 0.0); positions.len()];
        for t in &triangles {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            let n = (b - a).cross(c - a);
            for i in t {
                acc[*i as usize] = acc[*i as usize] + n;
            }
        }
        for (i, n) in normals.iter_mut().enumerate() {
            if bad[i] {
                *n = acc[i].normalized().unwrap_or(Vec3::Z);
            }
        }
    }
    Some((
        Mesh {
            positions,
            normals,
            triangles,
        },
        wires,
    ))
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

/// Number of interior grid intervals along one parameter direction, from the
/// surface structure alone (used when the face does not turn).
fn intervals(spans: i32, degree: i32) -> usize {
    if degree <= 1 {
        spans.max(1) as usize
    } else {
        (spans.max(1) * degree).clamp(2, 32) as usize
    }
}

/// Largest normal turning angle allowed across one grid interval.
const TURN_PER_INTERVAL: f64 = 10.0 * std::f64::consts::PI / 180.0;
const MAX_INTERVALS: usize = 64;

/// Grid intervals in u and v: enough for the face's spans, and more where its
/// normal turns (one interval per 10°), measured on a sample grid.
unsafe fn grid_size(
    m: *mut ffi::Model,
    obj: c_int,
    face: c_int,
    info: &FaceInfo,
) -> (usize, usize) {
    let base = (
        intervals(info.spans[0], info.degree[0]),
        intervals(info.spans[1], info.degree[1]),
    );
    if info.degree[0] <= 1 && info.degree[1] <= 1 {
        return base;
    }
    let [u0, u1, v0, v1] = info.domain;
    let k =
        |spans: i32, degree: i32| ((spans.max(1) * degree.max(1) * 2 + 1) as usize).clamp(9, 33);
    let (ku, kv) = (
        k(info.spans[0], info.degree[0]),
        k(info.spans[1], info.degree[1]),
    );
    let mut uv = Vec::with_capacity(ku * kv * 2);
    for j in 0..kv {
        for i in 0..ku {
            uv.push(u0 + (u1 - u0) * i as f64 / (ku - 1) as f64);
            uv.push(v0 + (v1 - v0) * j as f64 / (kv - 1) as f64);
        }
    }
    let n = ku * kv;
    let mut xyz = vec![0.0; n * 3];
    let mut nrm = vec![0.0; n * 3];
    ffi::f3dm_face_eval(
        m,
        obj,
        face,
        n as c_int,
        uv.as_ptr(),
        xyz.as_mut_ptr(),
        nrm.as_mut_ptr(),
    );
    let normal = |i: usize, j: usize| {
        let o = 3 * (j * ku + i);
        Vec3::new(nrm[o], nrm[o + 1], nrm[o + 2]).normalized()
    };
    let angle = |a: Option<Vec3>, b: Option<Vec3>| match (a, b) {
        (Some(a), Some(b)) => a.dot(b).clamp(-1.0, 1.0).acos(),
        _ => 0.0,
    };
    let mut turn_u: f64 = 0.0;
    for j in 0..kv {
        let t: f64 = (1..ku).map(|i| angle(normal(i - 1, j), normal(i, j))).sum();
        turn_u = turn_u.max(t);
    }
    let mut turn_v: f64 = 0.0;
    for i in 0..ku {
        let t: f64 = (1..kv).map(|j| angle(normal(i, j - 1), normal(i, j))).sum();
        turn_v = turn_v.max(t);
    }
    let by_turn = |t: f64| (t / TURN_PER_INTERVAL).ceil() as usize;
    (
        base.0.max(by_turn(turn_u)).clamp(1, MAX_INTERVALS),
        base.1.max(by_turn(turn_v)).clamp(1, MAX_INTERVALS),
    )
}

/// Constrained triangulation of a trimmed (u, v) region. Returns the vertex
/// positions in (u, v) and counter-clockwise triangles inside the region.
fn triangulate_trimmed(
    loops: &[Vec<[f64; 2]>],
    info: &FaceInfo,
    nu: usize,
    nv: usize,
) -> (Vec<[f64; 2]>, Vec<[u32; 3]>) {
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
        let (uv, tris) = triangulate_trimmed(&loops, &info, 6, 6);
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
        let (uv, tris) = triangulate_trimmed(&loops, &info, 1, 1);
        assert_eq!(uv.len(), 4);
        assert_eq!(tris.len(), 2);
    }
}
