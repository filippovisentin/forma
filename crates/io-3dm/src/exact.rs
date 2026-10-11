//! Exact surfaces and solids through openNURBS: making breps (box, cylinder,
//! sphere, planar regions and their extrusions), reading them back from bytes
//! and meshing them for display. See [`forma_geom::Brep`].

use crate::{display, ffi, ModelHandle};
use forma_doc::Geometry;
use forma_geom::{Brep, Mesh, Plane, Point3, Seg, Vec3, Xform};
use std::ffi::c_int;
use std::sync::Arc;

/// Copies a malloc'd byte buffer from the shim and frees it.
pub(crate) unsafe fn take_bytes(p: *mut u8, len: usize) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    let v = std::slice::from_raw_parts(p, len).to_vec();
    ffi::f3dm_bytes_free(p);
    (!v.is_empty()).then_some(v)
}

/// The object stored in `data` read back as a [`Brep`] (display mesh, faces,
/// snap points), with `xform` applied to it first (None = as stored). The
/// returned brep carries the bytes as given (and no pending transform).
fn read_bytes(data: Vec<u8>, xform: Option<&Xform>) -> Option<Brep> {
    let x16 = xform.map(xform16);
    let xp = x16.as_ref().map_or(std::ptr::null(), |x| x.as_ptr());
    // SAFETY: data/xform buffers outlive the call; the model is owned below.
    let m = unsafe { ffi::f3dm_bytes_model(data.as_ptr(), data.len(), xp) };
    if m.is_null() {
        return None;
    }
    let model = ModelHandle::from_raw(m);
    let mut info = ffi::Object::default();
    // SAFETY: valid model with one object.
    unsafe { ffi::f3dm_object(model.ptr(), 0, &mut info) };
    // SAFETY: valid model, object 0 exists.
    Some(unsafe { display::brep_of_object(model.ptr(), 0, &info, data) })
}

/// Display mesh, face count, closedness and flatness of serialized exact
/// geometry, with `xform` applied (None = as stored).
pub fn mesh_bytes(data: &[u8], xform: Option<&Xform>) -> Option<(Mesh, usize, bool, bool)> {
    read_bytes(data.to_vec(), xform).map(|b| (b.mesh, b.faces, b.solid, b.planar))
}

/// Signed volume enclosed by a closed mesh (positive when it faces outwards).
fn signed_volume(m: &Mesh) -> f64 {
    m.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.positions[i as usize].to_vec());
            a.dot(b.cross(c)) / 6.0
        })
        .sum()
}

/// A brep from serialized exact geometry (mesh made now, no transform pending).
/// Closed solids are turned outward-facing when they come inside out (mirrors,
/// some constructions).
pub fn brep_from_bytes(data: Vec<u8>) -> Option<Brep> {
    let b = read_bytes(data, None)?;
    if b.mesh.triangles.is_empty() {
        return None;
    }
    if b.solid && signed_volume(&b.mesh) < 0.0 {
        let mut len = 0;
        // SAFETY: buffer outlives the call; result freed by take_bytes.
        if let Some(flipped) = unsafe {
            take_bytes(
                ffi::f3dm_bytes_flipped(b.data.as_ptr(), b.data.len(), &mut len),
                len,
            )
        } {
            if let Some(f) = read_bytes(flipped, None) {
                return Some(f);
            }
        }
    }
    Some(b)
}

/// The exact geometry with its pending transform applied (identity transform,
/// fresh display mesh). `None` if openNURBS cannot transform it.
pub fn baked(b: &Brep) -> Option<Brep> {
    if !b.is_moved() {
        return Some(b.clone());
    }
    let x = b.xform16();
    let mut len = 0usize;
    // SAFETY: buffers outlive the call; result freed by take_bytes.
    let bytes = unsafe {
        take_bytes(
            ffi::f3dm_bytes_transformed(b.data.as_ptr(), b.data.len(), x.as_ptr(), &mut len),
            len,
        )
    }?;
    brep_from_bytes(bytes)
}

/// Copy with the faces reversed (an open surface's normals flipped).
pub fn flipped(b: &Brep) -> Option<Brep> {
    let b = baked(b)?;
    let mut len = 0;
    // SAFETY: buffer outlives the call; owned bytes out.
    made(
        unsafe { ffi::f3dm_bytes_flipped(b.data.as_ptr(), b.data.len(), &mut len) },
        len,
    )
}

/// The surface swept along `dir` into a closed solid (ExtrudeSrf).
pub fn extruded(b: &Brep, dir: Vec3) -> Option<Brep> {
    let b = baked(b)?;
    let d = [dir.x, dir.y, dir.z];
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe { ffi::f3dm_bytes_extruded(b.data.as_ptr(), b.data.len(), d.as_ptr(), &mut len) },
        len,
    )
}

/// Planar holes closed with trimmed planes (Cap): the capped surface and the
/// number of caps, or `None` when there was nothing to cap.
pub fn capped(b: &Brep, tol: f64) -> Option<(Brep, usize)> {
    let b = baked(b)?;
    let mut len = 0;
    let mut caps: c_int = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    let out = made(
        unsafe { ffi::f3dm_bytes_capped(b.data.as_ptr(), b.data.len(), tol, &mut caps, &mut len) },
        len,
    )?;
    Some((out, usize::try_from(caps).unwrap_or(0)))
}

/// The exact geometry (pending transform applied) as a neutral stream for the
/// solid kernel (`forma_geom::neutral`). `None` when a surface has no NURBS form
/// that matches its trims.
pub fn to_neutral(b: &Brep) -> Option<Vec<f64>> {
    let b = baked(b)?;
    let mut n = 0;
    // SAFETY: buffer outlives the call; the stream is freed by take_stream.
    unsafe {
        take_stream(
            ffi::f3dm_bytes_to_neutral(b.data.as_ptr(), b.data.len(), &mut n),
            n,
        )
    }
}

/// A brep from a neutral stream (results of the solid kernel). `None` when the
/// stream does not make a valid openNURBS brep.
pub fn from_neutral(s: &[f64], tol: f64) -> Option<Brep> {
    let mut len = 0;
    // SAFETY: the stream outlives the call; owned bytes out.
    made(
        unsafe { ffi::f3dm_neutral_to_bytes(s.as_ptr(), s.len(), tol, &mut len) },
        len,
    )
}

/// Row-major 4×4 of an affine transform.
pub fn xform16(x: &Xform) -> [f64; 16] {
    Brep {
        data: Arc::new(Vec::new()),
        xform: *x,
        mesh: Mesh::default(),
        faces: 0,
        solid: false,
        planar: false,
        face_ends: Vec::new(),
        vertices: Vec::new(),
        edge_mids: Vec::new(),
        arcs: Vec::new(),
        wires: Vec::new(),
    }
    .xform16()
}

fn plane9(p: &Plane) -> [f64; 9] {
    [
        p.origin.x, p.origin.y, p.origin.z, p.x.x, p.x.y, p.x.z, p.y.x, p.y.y, p.y.z,
    ]
}

fn made(p: *mut u8, len: usize) -> Option<Brep> {
    // SAFETY: p comes from a shim constructor with its length.
    brep_from_bytes(unsafe { take_bytes(p, len) }?)
}

/// Box from 8 corners: bottom 0–3 counter-clockwise seen from the top side,
/// then top 4–7 above them. Six planar faces.
pub fn make_box(corners: &[Point3; 8]) -> Option<Brep> {
    let c: Vec<f64> = corners.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    let mut len = 0;
    // SAFETY: 24 doubles in, owned bytes out.
    made(unsafe { ffi::f3dm_make_box(c.as_ptr(), &mut len) }, len)
}

/// Closed cylinder standing on `base` (origin = centre of the bottom circle).
pub fn make_cylinder(base: &Plane, radius: f64, height: f64) -> Option<Brep> {
    let p = plane9(base);
    let mut len = 0;
    // SAFETY: plain values in, owned bytes out.
    made(
        unsafe { ffi::f3dm_make_cylinder(p.as_ptr(), radius, height, &mut len) },
        len,
    )
}

/// Closed cone with its base circle on `base` and the apex `height` above it.
pub fn make_cone(base: &Plane, radius: f64, height: f64) -> Option<Brep> {
    let p = plane9(base);
    let mut len = 0;
    // SAFETY: plain values in, owned bytes out.
    made(
        unsafe { ffi::f3dm_make_cone(p.as_ptr(), radius, height, &mut len) },
        len,
    )
}

pub fn make_sphere(center: Point3, radius: f64) -> Option<Brep> {
    let c = [center.x, center.y, center.z];
    let mut len = 0;
    // SAFETY: plain values in, owned bytes out.
    made(
        unsafe { ffi::f3dm_make_sphere(c.as_ptr(), radius, &mut len) },
        len,
    )
}

pub fn make_torus(plane: &Plane, major: f64, minor: f64) -> Option<Brep> {
    let p = plane9(plane);
    let mut len = 0;
    // SAFETY: plain values in, owned bytes out.
    made(
        unsafe { ffi::f3dm_make_torus(p.as_ptr(), major, minor, &mut len) },
        len,
    )
}

fn push_seg(out: &mut Vec<f64>, s: &Seg) {
    match s {
        Seg::Line(a, b) => out.extend([0.0, a.x, a.y, a.z, b.x, b.y, b.z]),
        Seg::Arc(a) => {
            let c = a.center();
            let (x, y) = (a.plane.x, a.plane.y);
            out.extend([
                1.0, c.x, c.y, c.z, x.x, x.y, x.z, y.x, y.y, y.z, a.radius, a.sweep,
            ]);
        }
    }
}

/// Appends one closed curve as a segment stream (see the shim's `read_loop`).
/// Lines and arcs stay separate edges, as Rhino makes them.
fn push_loop(out: &mut Vec<f64>, g: &Geometry) -> bool {
    match g {
        Geometry::Nurbs(n) => {
            out.extend([1.0, 2.0, n.degree as f64, n.points.len() as f64]);
            out.extend(n.points.iter().flat_map(|p| [p.x, p.y, p.z]));
            out.extend(&n.weights);
            out.extend(&n.knots);
            true
        }
        _ => {
            let Some(chain) = g.to_chain() else {
                return false;
            };
            let segs: Vec<&Seg> = chain
                .segs
                .iter()
                .filter(|s| s.start().distance_to(s.end()) > 1e-12 || matches!(s, Seg::Arc(_)))
                .collect();
            if segs.is_empty() {
                return false;
            }
            out.push(segs.len() as f64);
            for s in segs {
                push_seg(out, s);
            }
            true
        }
    }
}

/// Planar surface bounded by `outer` with `holes`, on the plane with normal
/// `normal`; extruded along `extrude` into a closed solid when given.
pub fn make_planar(
    outer: &Geometry,
    holes: &[&Geometry],
    normal: Vec3,
    extrude: Option<Vec3>,
) -> Option<Brep> {
    let pts = outer.curve_points();
    let origin = *pts.first()?;
    let plane = Plane::from_normal(origin, normal);
    let mut stream = Vec::new();
    if !push_loop(&mut stream, outer) {
        return None;
    }
    for h in holes {
        if !push_loop(&mut stream, h) {
            return None;
        }
    }
    let p = plane9(&plane);
    let d = extrude.map(|v| [v.x, v.y, v.z]);
    let dp = d.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_make_planar(
                p.as_ptr(),
                stream.as_ptr(),
                stream.len(),
                (1 + holes.len()) as c_int,
                dp,
                &mut len,
            )
        },
        len,
    )
}

/// Surface swept by a curve moving along `dir` (Rhino's ExtrudeCrv of an open
/// curve, or of a closed curve without caps): one face per line / arc segment.
pub fn make_extruded_curve(curve: &Geometry, dir: Vec3) -> Option<Brep> {
    let mut stream = Vec::new();
    if !push_loop(&mut stream, curve) {
        return None;
    }
    let d = [dir.x, dir.y, dir.z];
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_make_extruded_curve(stream.as_ptr(), stream.len(), d.as_ptr(), &mut len)
        },
        len,
    )
}

/// One segment of a stream (see the shim's `read_loop`) as a curve.
fn read_segment(s: &[f64], k: &mut usize) -> Option<Geometry> {
    let p = |i: usize| Point3::new(s[i], s[i + 1], s[i + 2]);
    let kind = *s.get(*k)? as i64;
    *k += 1;
    match kind {
        0 => {
            s.get(*k + 5)?;
            let g = Geometry::Line(forma_geom::LineCurve::new(p(*k), p(*k + 3)));
            *k += 6;
            Some(g)
        }
        1 => {
            s.get(*k + 10)?;
            let o = p(*k);
            let x = Vec3::new(s[*k + 3], s[*k + 4], s[*k + 5]).normalized()?;
            let y = Vec3::new(s[*k + 6], s[*k + 7], s[*k + 8]).normalized()?;
            let (radius, sweep) = (s[*k + 9], s[*k + 10]);
            *k += 11;
            Some(Geometry::Arc(forma_geom::CircleArc {
                plane: Plane {
                    origin: o,
                    x,
                    y,
                    z: x.cross(y),
                },
                radius,
                sweep,
            }))
        }
        2 => {
            let degree = *s.get(*k)? as usize;
            let n = *s.get(*k + 1)? as usize;
            *k += 2;
            let need = 4 * n + n + degree - 1;
            s.get((*k + need).checked_sub(1)?)?;
            let points = (0..n).map(|i| p(*k + 3 * i)).collect();
            let weights = s[*k + 3 * n..*k + 4 * n].to_vec();
            let knots = s[*k + 4 * n..*k + need].to_vec();
            *k += need;
            forma_geom::NurbsCurve::new(degree, points, weights, knots).map(Geometry::Nurbs)
        }
        _ => None,
    }
}

/// Copies a malloc'd stream of doubles from the shim and frees it.
unsafe fn take_stream(p: *mut f64, n: usize) -> Option<Vec<f64>> {
    if p.is_null() {
        return None;
    }
    let v = std::slice::from_raw_parts(p, n).to_vec();
    ffi::f3dm_bytes_free(p.cast());
    Some(v)
}

/// A curve stream (count, then segments) as one Forma curve: line, arc,
/// polyline or polycurve of lines and arcs, or a NURBS curve.
pub(crate) fn curve_from_stream(s: &[f64]) -> Option<Geometry> {
    // Every segment takes at least one value: a bogus count cannot over-allocate.
    let count = (*s.first()? as usize).min(s.len());
    let mut k = 1;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        parts.push(read_segment(s, &mut k)?);
    }
    if let [g @ Geometry::Nurbs(_)] = parts.as_slice() {
        return Some(g.clone());
    }
    let segs: Vec<Seg> = parts
        .iter()
        .filter_map(Geometry::to_chain)
        .flat_map(|c| c.segs)
        .collect();
    if segs.len() != parts.len() {
        return None;
    }
    Geometry::from_chain(forma_geom::Chain::new(segs))
}

/// [`take_stream`] for the display module.
pub(crate) unsafe fn take_stream_pub(p: *mut f64, n: usize) -> Option<Vec<f64>> {
    take_stream(p, n)
}

/// The circles and arcs of an edge stream (count, then segments).
pub(crate) fn arcs_in_stream(s: &[f64]) -> Vec<forma_geom::CircleArc> {
    let count = s.first().map_or(0, |c| *c as usize);
    let mut k = 1;
    let mut out = Vec::new();
    for _ in 0..count {
        match read_segment(s, &mut k) {
            Some(Geometry::Arc(a)) => out.push(a),
            Some(_) => {}
            None => break,
        }
    }
    out
}

/// Edge curves of an exact surface (with its pending transform): all edges, or
/// only the naked (border) ones. Lines, arcs and NURBS curves, as in the model.
pub fn edges(b: &Brep, naked_only: bool) -> Vec<Geometry> {
    let Some(b) = baked(b) else {
        return Vec::new();
    };
    let mut n = 0;
    // SAFETY: buffer outlives the call; the stream is freed by take_stream.
    let Some(s) = (unsafe {
        take_stream(
            ffi::f3dm_bytes_edges(
                b.data.as_ptr(),
                b.data.len(),
                c_int::from(naked_only),
                &mut n,
            ),
            n,
        )
    }) else {
        return Vec::new();
    };
    let count = s.first().map_or(0, |c| *c as usize);
    let mut k = 1;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        match read_segment(&s, &mut k) {
            Some(g) => out.push(g),
            None => break,
        }
    }
    out
}

/// Boundary loops of face `i` (outer first), each as its edge curves in order.
pub fn face_loops(b: &Brep, i: usize) -> Vec<Vec<Geometry>> {
    let Some(b) = baked(b) else {
        return Vec::new();
    };
    let mut n = 0;
    // SAFETY: buffer outlives the call; the stream is freed by take_stream.
    let Some(s) = (unsafe {
        take_stream(
            ffi::f3dm_bytes_face_loops(b.data.as_ptr(), b.data.len(), i as c_int, &mut n),
            n,
        )
    }) else {
        return Vec::new();
    };
    let loops = s.first().map_or(0, |c| *c as usize);
    let mut k = 1;
    let mut out = Vec::new();
    for _ in 0..loops {
        let Some(count) = s.get(k).map(|c| *c as usize) else {
            break;
        };
        k += 1;
        let mut l = Vec::with_capacity(count);
        for _ in 0..count {
            match read_segment(&s, &mut k) {
                Some(g) => l.push(g),
                None => return out,
            }
        }
        out.push(l);
    }
    out
}

/// The faces of an exact surface, each as an exact surface of its own (Explode).
pub fn faces(b: &Brep) -> Vec<Brep> {
    let Some(b) = baked(b) else {
        return Vec::new();
    };
    (0..b.faces)
        .filter_map(|i| {
            let mut len = 0;
            // SAFETY: buffer outlives the call; owned bytes out.
            made(
                unsafe {
                    ffi::f3dm_bytes_face(b.data.as_ptr(), b.data.len(), i as c_int, &mut len)
                },
                len,
            )
        })
        .collect()
}

/// Breps joined where their naked edges meet (Rhino's Join): the joined
/// polysurfaces (one per connected piece) and the number of edges joined.
pub fn join(breps: &[&Brep], tol: f64) -> Option<(Vec<Brep>, usize)> {
    let baked: Vec<Brep> = breps.iter().map(|b| baked(b)).collect::<Option<_>>()?;
    let ptrs: Vec<*const u8> = baked.iter().map(|b| b.data.as_ptr()).collect();
    let lens: Vec<usize> = baked.iter().map(|b| b.data.len()).collect();
    let mut joined: c_int = 0;
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    let packed = unsafe {
        take_bytes(
            ffi::f3dm_bytes_join(
                ptrs.as_ptr(),
                lens.as_ptr(),
                c_int::try_from(ptrs.len()).ok()?,
                tol,
                &mut joined,
                &mut len,
            ),
            len,
        )
    }?;
    let word = |at: usize| -> Option<usize> {
        let b: [u8; 8] = packed.get(at..at + 8)?.try_into().ok()?;
        usize::try_from(u64::from_ne_bytes(b)).ok()
    };
    let count = word(0)?;
    let mut at = 8;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let l = word(at)?;
        at += 8;
        out.push(brep_from_bytes(packed.get(at..at + l)?.to_vec())?);
        at += l;
    }
    Some((out, usize::try_from(joined).unwrap_or(0)))
}

/// Surface of revolution of `profile` around the axis through `origin` along
/// `axis`, by `angle` radians (2π: full turn). One face per segment of the
/// profile, joined; a profile closing on the axis gives a closed solid.
pub fn make_revolved(
    profile: &Geometry,
    origin: Point3,
    axis: Vec3,
    angle: f64,
    tol: f64,
) -> Option<Brep> {
    let mut stream = Vec::new();
    if !push_loop(&mut stream, profile) {
        return None;
    }
    let (o, a) = ([origin.x, origin.y, origin.z], [axis.x, axis.y, axis.z]);
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_make_revolved(
                stream.as_ptr(),
                stream.len(),
                o.as_ptr(),
                a.as_ptr(),
                angle,
                tol,
                &mut len,
            )
        },
        len,
    )
}

/// Loft through curves (in order): one smooth NURBS surface through all of
/// them, or with `straight` ruled faces between neighbours (a polysurface).
pub fn make_loft(curves: &[&Geometry], straight: bool, tol: f64) -> Option<Brep> {
    let mut stream = Vec::new();
    for c in curves {
        if !push_loop(&mut stream, c) {
            return None;
        }
    }
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_make_loft(
                stream.as_ptr(),
                stream.len(),
                c_int::try_from(curves.len()).ok()?,
                c_int::from(straight),
                tol,
                &mut len,
            )
        },
        len,
    )
}

/// The surface swept by `profile` moving along `path` without turning
/// (ExtrudeCrvAlongCrv).
pub fn make_along(profile: &Geometry, path: &Geometry, tol: f64) -> Option<Brep> {
    let mut stream = Vec::new();
    if !push_loop(&mut stream, profile) || !push_loop(&mut stream, path) {
        return None;
    }
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe { ffi::f3dm_make_along(stream.as_ptr(), stream.len(), tol, &mut len) },
        len,
    )
}

/// The cone from `curve` to `apex` (ExtrudeCrvToPoint); a closed planar curve
/// (normal given) also gets its flat base, making a closed solid.
pub fn make_to_point(
    curve: &Geometry,
    apex: Point3,
    base_normal: Option<Vec3>,
    tol: f64,
) -> Option<Brep> {
    let mut stream = Vec::new();
    if !push_loop(&mut stream, curve) {
        return None;
    }
    let a = [apex.x, apex.y, apex.z];
    let plane = base_normal.and_then(|n| {
        curve
            .curve_points()
            .first()
            .map(|o| plane9(&Plane::from_normal(*o, n)))
    });
    let pp = plane.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_make_to_point(stream.as_ptr(), stream.len(), a.as_ptr(), pp, tol, &mut len)
        },
        len,
    )
}

/// Face `i` without its trims (Untrim): its whole underlying surface.
pub fn untrimmed(b: &Brep, i: usize) -> Option<Brep> {
    let b = baked(b)?;
    let mut len = 0;
    // SAFETY: buffer outlives the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_bytes_untrimmed(
                b.data.as_ptr(),
                b.data.len(),
                c_int::try_from(i).ok()?,
                &mut len,
            )
        },
        len,
    )
}

/// The surfaces shrunk to their trims (ShrinkTrimmedSrf).
pub fn shrunk(b: &Brep) -> Option<Brep> {
    let b = baked(b)?;
    let mut len = 0;
    // SAFETY: buffer outlives the call; owned bytes out.
    made(
        unsafe { ffi::f3dm_bytes_shrunk(b.data.as_ptr(), b.data.len(), &mut len) },
        len,
    )
}

/// openNURBS validity check (Check): valid, and the report when it is not.
pub fn check(b: &Brep) -> (bool, String) {
    let Some(b) = baked(b) else {
        return (false, "the transformed object could not be made".into());
    };
    let mut buf = vec![0u8; 4096];
    // SAFETY: buffer of the given capacity.
    let ok = unsafe {
        ffi::f3dm_bytes_check(b.data.as_ptr(), b.data.len(), buf.as_mut_ptr().cast(), 4096)
    } != 0;
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    (ok, String::from_utf8_lossy(&buf[..end]).trim().to_string())
}

/// Isocurves through the point of the surface nearest to `p` (inside the
/// trims): `dir` 0 along u, 1 along v, 2 both.
pub fn isocurves(b: &Brep, p: Point3, dir: u8) -> Vec<Geometry> {
    let Some(b) = baked(b) else {
        return Vec::new();
    };
    let xyz = [p.x, p.y, p.z];
    let mut n = 0;
    // SAFETY: buffers outlive the call; the stream is freed by take_stream.
    let Some(s) = (unsafe {
        take_stream(
            ffi::f3dm_bytes_isocurves(
                b.data.as_ptr(),
                b.data.len(),
                xyz.as_ptr(),
                c_int::from(dir),
                &mut n,
            ),
            n,
        )
    }) else {
        return Vec::new();
    };
    let count = s.first().map_or(0, |c| *c as usize);
    let mut k = 1;
    (0..count).map_while(|_| read_segment(&s, &mut k)).collect()
}

/// Exact polysurface with the flat faces of `mesh` (see
/// [`Mesh::polyhedron`]); `None` when the mesh is not a clean polyhedron of at
/// most `max_faces` faces.
pub fn polyhedron_brep(mesh: &Mesh, tol: f64, max_faces: usize) -> Option<Brep> {
    let p = mesh.polyhedron(tol, max_faces)?;
    let pts: Vec<f64> = p.points.iter().flat_map(|q| [q.x, q.y, q.z]).collect();
    let mut data: Vec<c_int> = vec![c_int::try_from(p.faces.len()).ok()?];
    for f in &p.faces {
        data.push(c_int::try_from(f.loops.len()).ok()?);
        for l in &f.loops {
            data.push(c_int::try_from(l.len()).ok()?);
            data.extend(l.iter().map(|v| *v as c_int));
        }
    }
    let mut len = 0;
    // SAFETY: buffers outlive the call; owned bytes out.
    made(
        unsafe {
            ffi::f3dm_make_polyhedron(
                pts.as_ptr(),
                c_int::try_from(p.points.len()).ok()?,
                data.as_ptr(),
                data.len(),
                &mut len,
            )
        },
        len,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geom::CircleArc;

    #[test]
    fn neutral_round_trip_keeps_every_primitive() {
        let base = Plane::from_normal(Point3::ORIGIN, Vec3::new(0.0, 0.0, 1.0));
        let corners = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 6.0, 0.0),
            Point3::new(0.0, 6.0, 0.0),
            Point3::new(0.0, 0.0, 4.0),
            Point3::new(10.0, 0.0, 4.0),
            Point3::new(10.0, 6.0, 4.0),
            Point3::new(0.0, 6.0, 4.0),
        ];
        let cases = [
            ("box", make_box(&corners).unwrap(), 240.0),
            (
                "cylinder",
                make_cylinder(&base, 3.0, 10.0).unwrap(),
                std::f64::consts::PI * 90.0,
            ),
            (
                "sphere",
                make_sphere(Point3::ORIGIN, 5.0).unwrap(),
                4.0 / 3.0 * std::f64::consts::PI * 125.0,
            ),
            (
                "cone",
                make_cone(&base, 4.0, 9.0).unwrap(),
                std::f64::consts::PI * 16.0 * 3.0,
            ),
            (
                "torus",
                make_torus(&base, 10.0, 2.0).unwrap(),
                2.0 * std::f64::consts::PI.powi(2) * 10.0 * 4.0,
            ),
        ];
        for (name, b, vol) in cases {
            let s = to_neutral(&b).unwrap_or_else(|| panic!("{name}: no neutral form"));
            let back = from_neutral(&s, 1e-6).unwrap_or_else(|| panic!("{name}: not rebuilt"));
            assert_eq!(back.faces, b.faces, "{name}");
            assert!(back.solid, "{name} stays closed");
            let v = volume(&back.mesh);
            assert!((v - vol).abs() / vol < 0.02, "{name}: volume {v} vs {vol}");
        }
        // Damaged streams are refused, not crashed on.
        let s = to_neutral(&make_box(&corners).unwrap()).unwrap();
        assert!(from_neutral(&s[..s.len() - 3], 1e-6).is_none());
        let mut bad = s.clone();
        bad[1] = 1e18;
        assert!(from_neutral(&bad, 1e-6).is_none());
        assert!(from_neutral(&[], 1e-6).is_none());
    }

    fn volume(m: &Mesh) -> f64 {
        m.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize].to_vec());
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }

    fn corners(x: f64, y: f64, z: f64) -> [Point3; 8] {
        let p = |a, b, c| Point3::new(a, b, c);
        [
            p(0.0, 0.0, 0.0),
            p(x, 0.0, 0.0),
            p(x, y, 0.0),
            p(0.0, y, 0.0),
            p(0.0, 0.0, z),
            p(x, 0.0, z),
            p(x, y, z),
            p(0.0, y, z),
        ]
    }

    #[test]
    fn box_is_a_closed_six_face_solid() {
        let b = make_box(&corners(10.0, 20.0, 30.0)).expect("box");
        assert_eq!(b.faces, 6);
        assert!(b.solid);
        assert!(
            (volume(&b.mesh) - 6000.0).abs() < 1e-6,
            "{}",
            volume(&b.mesh)
        );
    }

    #[test]
    fn bytes_round_trip_and_transform() {
        let b = make_box(&corners(10.0, 10.0, 10.0)).expect("box");
        let mirrored = b
            .transformed(&Xform::translation(Vec3::new(100.0, 0.0, 0.0)))
            .transformed(&Xform::mirror(Point3::ORIGIN, Vec3::new(1.0, 0.0, 0.0)));
        let baked = baked(&mirrored).expect("baked");
        assert!(!baked.is_moved());
        assert!(baked.solid);
        let bb = baked.mesh.bounding_box().expect("bbox");
        assert!(
            bb.min.distance_to(Point3::new(-110.0, 0.0, 0.0)) < 1e-9,
            "{bb:?}"
        );
        // Still outward-facing after the mirror, without any fixing up.
        assert!((volume(&baked.mesh) - 1000.0).abs() < 1e-6);
        let (m, ..) = mesh_bytes(&b.data, Some(&mirrored.xform)).expect("mesh");
        assert!((volume(&m) - 1000.0).abs() < 1e-6, "{}", volume(&m));
        // A mirrored extrusion too.
        let p = |x, y| Point3::new(x, y, 0.0);
        let sq = Geometry::Polyline(vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 0.0),
        ]);
        let e = make_planar(&sq, &[], Vec3::Z, Some(Vec3::Z)).expect("solid");
        let x = Xform::mirror(Point3::ORIGIN, Vec3::X);
        let (m, ..) = mesh_bytes(&e.data, Some(&x)).expect("mesh");
        assert!((volume(&m) - 16.0).abs() < 1e-9, "{}", volume(&m));
        // Non-uniform scale works too.
        let s = Xform::scale_axes(&Plane::from_normal(Point3::ORIGIN, Vec3::Z), 2.0, 1.0, 1.0);
        let stretched = super::baked(&b.transformed(&s)).expect("stretched");
        assert!((volume(&stretched.mesh) - 2000.0).abs() < 1e-6);
    }

    #[test]
    fn sphere_cylinder_cone_torus() {
        let s = make_sphere(Point3::ORIGIN, 10.0).expect("sphere");
        assert!(s.solid && s.faces == 1);
        let v = volume(&s.mesh);
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
        assert!((v - exact).abs() / exact < 0.05, "{v}");
        let base = Plane::from_normal(Point3::ORIGIN, Vec3::Z);
        let c = make_cylinder(&base, 5.0, 10.0).expect("cylinder");
        assert!(c.solid && c.faces == 3, "{} {}", c.solid, c.faces);
        assert!(c.mesh.bounding_box().unwrap().max.z > 9.99);
        let k = make_cone(&base, 5.0, 10.0).expect("cone");
        assert!(k.solid, "cone");
        let kb = k.mesh.bounding_box().unwrap();
        assert!(
            kb.min.z.abs() < 1e-9 && (kb.max.z - 10.0).abs() < 1e-9,
            "{kb:?}"
        );
        assert!(volume(&k.mesh) > 0.0);
        let t = make_torus(&base, 10.0, 2.0).expect("torus");
        assert!(t.solid);
        // Display meshes of closed breps are watertight once welded.
        for (name, b) in [
            ("sphere", &s),
            ("cylinder", &c),
            ("cone", &k),
            ("torus", &t),
        ] {
            assert!(b.mesh.is_closed(1e-9), "{name} mesh has cracks");
        }
    }

    #[test]
    fn polyhedron_from_a_mesh_box_is_an_exact_solid() {
        let m = forma_geom::box_mesh(&Plane::from_normal(Point3::ORIGIN, Vec3::Z), 4.0, 5.0, 6.0)
            .subdivided(3);
        let b = polyhedron_brep(&m, 1e-6, 1000).expect("polyhedron");
        assert_eq!(b.faces, 6);
        assert!(b.solid && b.planar);
        assert!((volume(&b.mesh) - 120.0).abs() < 1e-9);
        // A frame (hole through it) too.
        let sq = |s: f64, o: f64| {
            vec![
                Point3::new(o, o, 0.0),
                Point3::new(o + s, o, 0.0),
                Point3::new(o + s, o + s, 0.0),
                Point3::new(o, o + s, 0.0),
            ]
        };
        let f = forma_geom::extrude_region(&sq(10.0, 0.0), &[sq(4.0, 3.0)], Vec3::Z * 2.0);
        let b = polyhedron_brep(&f, 1e-6, 1000).expect("frame");
        assert_eq!(b.faces, 10);
        assert!(b.solid);
        assert!(
            (volume(&b.mesh) - 168.0).abs() < 1e-9,
            "{}",
            volume(&b.mesh)
        );
    }

    #[test]
    fn extruded_open_and_closed_curves() {
        let p = |x, y| Point3::new(x, y, 0.0);
        let l = Geometry::Polyline(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0)]);
        let s = make_extruded_curve(&l, Vec3::Z * 3.0).expect("open");
        assert_eq!(s.faces, 2);
        assert!(!s.solid);
        let area: f64 = s
            .mesh
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| s.mesh.positions[i as usize]);
                (b - a).cross(c - a).length() / 2.0
            })
            .sum();
        assert!((area - 45.0).abs() < 1e-9, "{area}");
        let c = Geometry::Arc(CircleArc::circle(
            Plane::from_normal(p(0.0, 0.0), Vec3::Z),
            2.0,
        ));
        let tube = make_extruded_curve(&c, Vec3::Z * 3.0).expect("closed");
        assert_eq!(tube.faces, 1);
        assert!(!tube.solid);
    }

    #[test]
    fn edges_faces_and_loops_of_a_cylinder() {
        let base = Plane::from_normal(Point3::ORIGIN, Vec3::Z);
        let c = make_cylinder(&base, 5.0, 10.0).expect("cylinder");
        let e = edges(&c, false);
        // Two circles and the seam.
        assert_eq!(e.len(), 3, "{e:?}");
        assert_eq!(
            e.iter()
                .filter(|g| matches!(g, Geometry::Arc(a) if a.is_closed()))
                .count(),
            2
        );
        assert!(edges(&c, true).is_empty());
        let f = faces(&c);
        assert_eq!(f.len(), 3);
        assert!(f.iter().all(|x| x.faces == 1 && !x.solid));
        // The moved box: a top face outline of 4 lines.
        let b = make_box(&corners(2.0, 3.0, 4.0))
            .expect("box")
            .transformed(&Xform::translation(Vec3::new(0.0, 0.0, 10.0)));
        let loops = face_loops(&b, 5);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 4);
        assert!(loops[0]
            .iter()
            .all(|g| matches!(g, Geometry::Line(l) if l.from.z > 9.99)));
        let n = edges(&b, false);
        assert_eq!(n.len(), 12);
    }

    #[test]
    fn extrude_and_cap_surfaces() {
        let p = |x, y| Point3::new(x, y, 0.0);
        let sq = Geometry::Polyline(vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 0.0),
        ]);
        let flat = make_planar(&sq, &[], Vec3::Z, None).expect("surface");
        let solid = extruded(&flat, Vec3::new(0.0, 0.0, -2.0)).expect("extruded");
        assert!(solid.solid);
        assert_eq!(solid.faces, 6);
        assert!((volume(&solid.mesh) - 32.0).abs() < 1e-9);
        // An open tube (uncapped cylinder side) gets two round caps.
        let c = Geometry::Arc(CircleArc::circle(
            Plane::from_normal(p(0.0, 0.0), Vec3::Z),
            2.0,
        ));
        let tube = make_extruded_curve(&c, Vec3::Z * 3.0).expect("tube");
        let (capped, n) = capped(&tube, 1e-6).expect("capped");
        assert_eq!(n, 2);
        assert!(capped.solid, "capped tube is closed");
        assert!(capped.mesh.is_closed(1e-9));
        let v = volume(&capped.mesh);
        let exact = std::f64::consts::PI * 4.0 * 3.0;
        assert!((v - exact).abs() / exact < 0.02, "{v} vs {exact}");
        assert!(super::capped(&capped, 1e-6).is_none());
    }

    #[test]
    fn join_exploded_faces_back_into_solids() {
        let base = Plane::from_normal(Point3::ORIGIN, Vec3::Z);
        for (name, b) in [
            ("box", make_box(&corners(3.0, 4.0, 5.0)).unwrap()),
            ("cylinder", make_cylinder(&base, 2.0, 5.0).unwrap()),
        ] {
            let parts = faces(&b);
            let refs: Vec<&Brep> = parts.iter().collect();
            let (joined, n) = join(&refs, 1e-6).expect("join");
            assert_eq!(joined.len(), 1, "{name}");
            assert!(joined[0].solid, "{name} joined into a solid ({n} edges)");
            assert_eq!(joined[0].faces, b.faces, "{name}");
            assert!(
                (volume(&joined[0].mesh) - volume(&b.mesh)).abs() < 1e-6,
                "{name}"
            );
        }
        // Two separate boxes stay two pieces.
        let a = make_box(&corners(1.0, 1.0, 1.0)).unwrap();
        let b = a.transformed(&Xform::translation(Vec3::new(5.0, 0.0, 0.0)));
        let (pieces, _) = join(&[&a, &b], 1e-6).unwrap();
        assert_eq!(pieces.len(), 2);
    }

    #[test]
    fn revolve_profiles_into_solids_and_surfaces() {
        let p = |x, z| Point3::new(x, 0.0, z);
        // Truncated cone: bottom disc, slanted side, top disc.
        let profile = Geometry::Polyline(vec![p(0.0, 0.0), p(4.0, 0.0), p(2.0, 3.0), p(0.0, 3.0)]);
        let tc = make_revolved(
            &profile,
            Point3::ORIGIN,
            Vec3::Z,
            std::f64::consts::TAU,
            1e-6,
        )
        .expect("tcone");
        assert_eq!(tc.faces, 3);
        assert!(tc.solid, "closed on the axis");
        let exact = std::f64::consts::PI * 3.0 / 3.0 * (16.0 + 8.0 + 4.0);
        let v = volume(&tc.mesh);
        assert!((v - exact).abs() / exact < 0.02, "{v} vs {exact}");
        assert!(tc.mesh.is_closed(1e-9));
        // A quarter turn of a line: an open surface.
        let l = Geometry::Line(forma_geom::LineCurve::new(p(5.0, 0.0), p(5.0, 2.0)));
        let s = make_revolved(
            &l,
            Point3::ORIGIN,
            Vec3::Z,
            std::f64::consts::FRAC_PI_2,
            1e-6,
        )
        .expect("quarter");
        assert_eq!(s.faces, 1);
        assert!(!s.solid);
    }

    #[test]
    fn stretched_and_mirrored_round_solids_stay_exact_and_outward() {
        let base = Plane::from_normal(Point3::ORIGIN, Vec3::Z);
        let stretch = Xform::scale_axes(&base, 1.0, 2.0, 3.0);
        let s = make_sphere(Point3::ORIGIN, 10.0).unwrap();
        let e = baked(&s.transformed(&stretch)).expect("ellipsoid");
        let bb = e.mesh.bounding_box().unwrap();
        assert!((bb.max.z - 30.0).abs() < 1e-6, "{bb:?}");
        assert!((bb.max.y - 20.0).abs() < 0.5, "{bb:?}");
        assert!((bb.max.x - 10.0).abs() < 0.5, "{bb:?}");
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 6000.0;
        assert!((volume(&e.mesh) - exact).abs() / exact < 0.03);
        for (name, b) in [
            ("cylinder", make_cylinder(&base, 2.0, 5.0).unwrap()),
            ("cone", make_cone(&base, 2.0, 5.0).unwrap()),
            ("sphere", s.clone()),
            ("torus", make_torus(&base, 6.0, 1.0).unwrap()),
        ] {
            let v = volume(&b.mesh);
            let x = Xform::mirror(Point3::ORIGIN, Vec3::X);
            let (m, ..) = mesh_bytes(&b.data, Some(&x)).expect(name);
            assert!(
                (volume(&m) - v).abs() / v < 1e-6,
                "{name} mirrored: {}",
                volume(&m)
            );
            let (m, ..) = mesh_bytes(&b.data, Some(&stretch)).expect(name);
            assert!((volume(&m) - 6.0 * v).abs() / v < 0.2, "{name} stretched");
        }
    }

    #[test]
    fn loft_sweep_along_and_cone_to_point() {
        let c = |z: f64, r: f64| {
            Geometry::Arc(CircleArc::circle(
                Plane::from_normal(Point3::new(0.0, 0.0, z), Vec3::Z),
                r,
            ))
        };
        let (a, b, d) = (c(0.0, 5.0), c(5.0, 3.0), c(10.0, 5.0));
        let smooth = make_loft(&[&a, &b, &d], false, 1e-6).expect("loft");
        assert_eq!(smooth.faces, 1);
        let bb = smooth.mesh.bounding_box().unwrap();
        assert!(
            (bb.max.z - 10.0).abs() < 1e-6 && (bb.max.x - 5.0).abs() < 0.1,
            "{bb:?}"
        );
        // The middle section passes through the middle circle.
        let near_mid = smooth
            .mesh
            .positions
            .iter()
            .filter(|p| (p.z - 5.0).abs() < 0.05)
            .all(|p| ((p.x * p.x + p.y * p.y).sqrt() - 3.0).abs() < 0.05);
        assert!(near_mid);
        let straight = make_loft(&[&a, &b, &d], true, 1e-6).expect("straight loft");
        assert_eq!(straight.faces, 2);
        // Polylines with the same corners: one ruled face per side.
        let sq = |z: f64, s: f64| {
            let p = |x, y| Point3::new(x, y, z);
            Geometry::Polyline(vec![
                p(0.0, 0.0),
                p(s, 0.0),
                p(s, s),
                p(0.0, s),
                p(0.0, 0.0),
            ])
        };
        let box_side = make_loft(&[&sq(0.0, 4.0), &sq(3.0, 4.0)], true, 1e-6).expect("ruled");
        assert_eq!(box_side.faces, 4);
        // Profile along a path.
        let prof = Geometry::Line(forma_geom::LineCurve::new(
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, 2.0),
        ));
        let path = Geometry::Arc(
            CircleArc::from_center_start_end(
                Point3::new(10.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(20.0, 0.0, 0.0),
                -Vec3::Z,
            )
            .unwrap(),
        );
        let along = make_along(&prof, &path, 1e-6).expect("along");
        assert_eq!(along.faces, 1);
        let bb = along.mesh.bounding_box().unwrap();
        assert!(
            (bb.max.z - 2.0).abs() < 1e-9 && (bb.max.x - 20.0).abs() < 1e-6,
            "{bb:?}"
        );
        // A square to a point: a closed pyramid.
        let pyr = make_to_point(
            &sq(0.0, 4.0),
            Point3::new(2.0, 2.0, 6.0),
            Some(Vec3::Z),
            1e-6,
        )
        .expect("pyramid");
        assert!(pyr.solid, "{pyr:?}");
        assert_eq!(pyr.faces, 5);
        assert!(
            (volume(&pyr.mesh) - 32.0).abs() < 1e-6,
            "{}",
            volume(&pyr.mesh)
        );
    }

    #[test]
    fn untrim_shrink_check_and_isocurves() {
        let p = |x, y| Point3::new(x, y, 0.0);
        let outer = Geometry::Polyline(vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 0.0),
        ]);
        let hole = Geometry::Arc(CircleArc::circle(
            Plane::from_normal(p(5.0, 5.0), Vec3::Z),
            2.0,
        ));
        let s = make_planar(&outer, &[&hole], Vec3::Z, None).unwrap();
        let full = untrimmed(&s, 0).expect("untrimmed");
        let area = |b: &Brep| -> f64 {
            b.mesh
                .triangles
                .iter()
                .map(|t| {
                    let [a, b2, c] = t.map(|i| b.mesh.positions[i as usize]);
                    (b2 - a).cross(c - a).length() / 2.0
                })
                .sum()
        };
        assert!((area(&full) - 100.0).abs() < 1e-6, "{}", area(&full));
        assert!(shrunk(&s).is_some());
        let (ok, _) = check(&s);
        assert!(ok);
        // Isocurves of a sphere through a point on its equator.
        let sph = make_sphere(Point3::ORIGIN, 10.0).unwrap();
        let isos = isocurves(&sph, Point3::new(0.0, 7.0, 7.0), 2);
        assert_eq!(isos.len(), 2, "{isos:?}");
        // The plate with a hole: the line across the middle is cut in two.
        let cut = isocurves(&s, p(5.0, 5.0), 0);
        assert_eq!(cut.len(), 2, "{cut:?}");
    }

    #[test]
    fn extruded_region_with_a_hole() {
        let p = |x, y| Point3::new(x, y, 0.0);
        let outer = Geometry::Polyline(vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 0.0),
        ]);
        let hole = Geometry::Arc(CircleArc::circle(
            Plane::from_normal(p(5.0, 5.0), Vec3::Z),
            2.0,
        ));
        let b =
            make_planar(&outer, &[&hole], Vec3::Z, Some(Vec3::new(0.0, 0.0, 3.0))).expect("solid");
        assert!(b.solid);
        // 4 sides + hole wall + 2 caps.
        assert_eq!(b.faces, 7);
        assert!(b.mesh.is_closed(1e-9));
        let v = volume(&b.mesh);
        let exact = (100.0 - std::f64::consts::PI * 4.0) * 3.0;
        assert!((v - exact).abs() / exact < 0.01, "{v} vs {exact}");
        let flat = make_planar(&outer, &[], Vec3::Z, None).expect("surface");
        assert_eq!(flat.faces, 1);
        assert!(!flat.solid);
    }
}
