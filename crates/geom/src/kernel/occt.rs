//! FFI to `occt/shim.cpp`. Shapes are owned handles freed on drop.

use super::{weld_smooth_seams, BooleanOp, KernelError, SolidInfo, SolidOptions};
use crate::{Mesh, Point3, Vec3};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

#[repr(C)]
struct FoShape {
    _private: [u8; 0],
}

#[repr(C)]
struct FoTess {
    _private: [u8; 0],
}

extern "C" {
    fn fo_last_error() -> *const c_char;
    fn fo_shape_free(s: *mut FoShape);
    fn fo_solid_count(s: *const FoShape) -> c_int;
    fn fo_solid_at(s: *const FoShape, i: c_int) -> *mut FoShape;
    fn fo_face_count(s: *const FoShape) -> c_int;
    fn fo_volume(s: *const FoShape) -> f64;
    fn fo_is_valid(s: *const FoShape) -> c_int;
    fn fo_shape_from_mesh(
        xyz: *const f64,
        nv: u32,
        tris: *const u32,
        nt: u32,
        tol: f64,
    ) -> *mut FoShape;
    fn fo_boolean(
        op: c_int,
        a: *const *const FoShape,
        na: u32,
        b: *const *const FoShape,
        nb: u32,
        fuzzy: f64,
    ) -> *mut FoShape;
    fn fo_fillet_edges(
        s: *const FoShape,
        r: f64,
        pts: *const f64,
        np: u32,
        max_dist: f64,
        chamfer: c_int,
    ) -> *mut FoShape;
    fn fo_shell(
        s: *const FoShape,
        thickness: f64,
        pts: *const f64,
        np: u32,
        max_dist: f64,
        tol: f64,
    ) -> *mut FoShape;
    fn fo_offset(s: *const FoShape, d: f64, tol: f64) -> *mut FoShape;
    fn fo_tessellate(s: *const FoShape, lin: f64, ang: f64) -> *mut FoTess;
    fn fo_tess_vertex_count(t: *const FoTess) -> u32;
    fn fo_tess_triangle_count(t: *const FoTess) -> u32;
    fn fo_tess_copy(
        t: *const FoTess,
        positions: *mut f64,
        normals: *mut f64,
        faces: *mut u32,
        triangles: *mut u32,
    );
    fn fo_tess_free(t: *mut FoTess);
}

struct Shape(*mut FoShape);

impl Drop for Shape {
    fn drop(&mut self) {
        // SAFETY: the pointer came from the shim and is freed exactly once.
        unsafe { fo_shape_free(self.0) }
    }
}

fn last_error(what: &str) -> KernelError {
    // SAFETY: the shim returns a valid, NUL-terminated thread-local string.
    let msg = unsafe { CStr::from_ptr(fo_last_error()) }
        .to_string_lossy()
        .into_owned();
    if msg.is_empty() {
        KernelError::Failed(format!("{what} failed"))
    } else {
        KernelError::Failed(format!("{what}: {msg}"))
    }
}

fn wrap(p: *mut FoShape, what: &str) -> Result<Shape, KernelError> {
    if p.is_null() {
        Err(last_error(what))
    } else {
        Ok(Shape(p))
    }
}

fn to_shape(m: &Mesh, opts: SolidOptions) -> Result<Shape, KernelError> {
    let xyz: Vec<f64> = m.positions.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    let tris: Vec<u32> = m.triangles.iter().flatten().copied().collect();
    // SAFETY: the slices outlive the call; lengths match the counts passed.
    let p = unsafe {
        fo_shape_from_mesh(
            xyz.as_ptr(),
            m.positions.len() as u32,
            tris.as_ptr(),
            m.triangles.len() as u32,
            opts.tolerance,
        )
    };
    wrap(p, "making a solid from the mesh")
}

fn solids(s: &Shape) -> Result<Vec<Shape>, KernelError> {
    // SAFETY: `s` is a live handle.
    let n = unsafe { fo_solid_count(s.0) };
    (0..n)
        .map(|i| wrap(unsafe { fo_solid_at(s.0, i) }, "reading the result"))
        .collect()
}

fn deflection(m: &Mesh, opts: SolidOptions) -> f64 {
    if opts.deflection > 0.0 {
        return opts.deflection;
    }
    let diag = m.bounding_box().map_or(1.0, |b| b.min.distance_to(b.max));
    (diag / 2000.0).max(opts.tolerance)
}

fn to_mesh(s: &Shape, lin: f64, opts: SolidOptions) -> Result<Mesh, KernelError> {
    // SAFETY: `s` is a live handle; the buffers are sized from the shim's counts.
    unsafe {
        let t = fo_tessellate(s.0, lin, opts.angle);
        if t.is_null() {
            return Err(last_error("triangulating the result"));
        }
        let nv = fo_tess_vertex_count(t) as usize;
        let nt = fo_tess_triangle_count(t) as usize;
        let mut pos = vec![0.0; nv * 3];
        let mut nrm = vec![0.0; nv * 3];
        let mut face = vec![0u32; nv];
        let mut tri = vec![0u32; nt * 3];
        fo_tess_copy(
            t,
            pos.as_mut_ptr(),
            nrm.as_mut_ptr(),
            face.as_mut_ptr(),
            tri.as_mut_ptr(),
        );
        fo_tess_free(t);
        let mut m = Mesh {
            positions: pos
                .chunks_exact(3)
                .map(|c| Point3::new(c[0], c[1], c[2]))
                .collect(),
            normals: nrm
                .chunks_exact(3)
                .map(|c| Vec3::new(c[0], c[1], c[2]))
                .collect(),
            triangles: tri.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        };
        weld_smooth_seams(&mut m, &face, 30f64.to_radians());
        Ok(m)
    }
}

/// Deflection for results of an operation on `inputs`.
fn deflection_for(inputs: &[&Mesh], opts: SolidOptions) -> f64 {
    let mut all = Mesh::default();
    for m in inputs {
        all.positions.extend_from_slice(&m.positions);
    }
    deflection(&all, opts)
}

pub(super) fn boolean(
    op: BooleanOp,
    a: &[&Mesh],
    b: &[&Mesh],
    opts: SolidOptions,
) -> Result<Vec<Mesh>, KernelError> {
    let sa = a
        .iter()
        .map(|m| to_shape(m, opts))
        .collect::<Result<Vec<_>, _>>()?;
    let sb = b
        .iter()
        .map(|m| to_shape(m, opts))
        .collect::<Result<Vec<_>, _>>()?;
    let pa: Vec<*const FoShape> = sa.iter().map(|s| s.0 as *const FoShape).collect();
    let pb: Vec<*const FoShape> = sb.iter().map(|s| s.0 as *const FoShape).collect();
    let code = match op {
        BooleanOp::Union => 0,
        BooleanOp::Difference => 1,
        BooleanOp::Intersection => 2,
        BooleanOp::Split => 3,
    };
    // SAFETY: the handle arrays outlive the call.
    let r = wrap(
        unsafe {
            fo_boolean(
                code,
                pa.as_ptr(),
                pa.len() as u32,
                pb.as_ptr(),
                pb.len() as u32,
                opts.tolerance,
            )
        },
        "boolean",
    )?;
    let lin = deflection_for(&[a, b].concat(), opts);
    solids(&r)?.iter().map(|s| to_mesh(s, lin, opts)).collect()
}

fn points(p: &[Point3]) -> Vec<f64> {
    p.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

/// How far a pick point may be from the edge / face it picks.
fn pick_distance(m: &Mesh) -> f64 {
    m.bounding_box()
        .map_or(1.0, |b| b.min.distance_to(b.max) * 0.1)
}

pub(super) fn fillet(
    m: &Mesh,
    r: f64,
    near: &[Point3],
    chamfer: bool,
    opts: SolidOptions,
) -> Result<Mesh, KernelError> {
    if r <= opts.tolerance {
        return Err(KernelError::Failed("the radius must be positive".into()));
    }
    if near.is_empty() {
        return Err(KernelError::Failed("no edge picked".into()));
    }
    let s = to_shape(m, opts)?;
    let pts = points(near);
    // SAFETY: `s` and `pts` outlive the call.
    let out = wrap(
        unsafe {
            fo_fillet_edges(
                s.0,
                r,
                pts.as_ptr(),
                near.len() as u32,
                pick_distance(m),
                c_int::from(chamfer),
            )
        },
        if chamfer { "chamfer" } else { "fillet" },
    )?;
    to_mesh(&out, deflection(m, opts), opts)
}

pub(super) fn shell(
    m: &Mesh,
    thickness: f64,
    near: &[Point3],
    opts: SolidOptions,
) -> Result<Mesh, KernelError> {
    if thickness <= opts.tolerance {
        return Err(KernelError::Failed("the thickness must be positive".into()));
    }
    let s = to_shape(m, opts)?;
    let pts = points(near);
    // SAFETY: `s` and `pts` outlive the call.
    let out = wrap(
        unsafe {
            fo_shell(
                s.0,
                thickness,
                pts.as_ptr(),
                near.len() as u32,
                pick_distance(m),
                opts.tolerance,
            )
        },
        "shell",
    )?;
    to_mesh(&out, deflection(m, opts), opts)
}

pub(super) fn offset(m: &Mesh, d: f64, opts: SolidOptions) -> Result<Mesh, KernelError> {
    if d.abs() <= opts.tolerance {
        return Err(KernelError::Failed("the distance is too small".into()));
    }
    let s = to_shape(m, opts)?;
    // SAFETY: `s` is a live handle.
    let out = wrap(unsafe { fo_offset(s.0, d, opts.tolerance) }, "offset")?;
    to_mesh(&out, deflection(m, opts), opts)
}

pub(super) fn info(m: &Mesh, opts: SolidOptions) -> Result<SolidInfo, KernelError> {
    let s = to_shape(m, opts)?;
    // SAFETY: `s` is a live handle.
    unsafe {
        Ok(SolidInfo {
            solids: fo_solid_count(s.0) as usize,
            faces: fo_face_count(s.0) as usize,
            volume: fo_volume(s.0),
            valid: fo_is_valid(s.0) == 1,
        })
    }
}

pub(super) fn rebuild(m: &Mesh, opts: SolidOptions) -> Result<Mesh, KernelError> {
    let s = to_shape(m, opts)?;
    to_mesh(&s, deflection(m, opts), opts)
}
