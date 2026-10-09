//! Rhino `.3dm` I/O through openNURBS.
//!
//! Spike S1 scope: read a [`Summary`] of a file and write a file with one line.
//! Full import into `forma-doc` arrives with milestones M1/M5.

mod display;
mod writer;

pub use display::{import_display, DisplayGeometry, Import, ImportedLayer, ImportedObject};
pub use writer::write_document;

use forma_geom::{BoundingBox, Point3};
use std::ffi::{c_char, c_int, CString};
use std::fmt;
use std::path::Path;

pub(crate) mod ffi {
    use std::ffi::{c_char, c_int};

    #[repr(C)]
    pub struct Model {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct Writer {
        _private: [u8; 0],
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct Object {
        pub kind: c_int,
        pub layer: c_int,
        pub brep_faces: c_int,
        pub is_solid: c_int,
        pub bbox: [f64; 6],
    }

    extern "C" {
        pub fn f3dm_read(path: *const c_char) -> *mut Model;
        pub fn f3dm_free(m: *mut Model);
        pub fn f3dm_archive_version(m: *const Model) -> c_int;
        pub fn f3dm_unit_system(m: *const Model) -> c_int;
        pub fn f3dm_abs_tolerance(m: *const Model) -> f64;
        pub fn f3dm_angle_tolerance_deg(m: *const Model) -> f64;
        pub fn f3dm_material_count(m: *const Model) -> c_int;
        pub fn f3dm_block_count(m: *const Model) -> c_int;
        pub fn f3dm_layer_count(m: *const Model) -> c_int;
        pub fn f3dm_layer_path(m: *const Model, i: c_int, buf: *mut c_char, cap: usize) -> usize;
        pub fn f3dm_object_count(m: *const Model) -> c_int;
        pub fn f3dm_object(m: *const Model, i: c_int, out: *mut Object) -> c_int;
        pub fn f3dm_layer_display(m: *const Model, i: c_int, rgb: *mut u8, visible: *mut c_int);
        pub fn f3dm_face_count(m: *const Model, obj: c_int) -> c_int;
        pub fn f3dm_face_info(
            m: *const Model,
            obj: c_int,
            face: c_int,
            domain: *mut f64,
            spans: *mut c_int,
            degree: *mut c_int,
            reversed: *mut c_int,
        ) -> c_int;
        pub fn f3dm_face_loops(m: *mut Model, obj: c_int, face: c_int) -> c_int;
        pub fn f3dm_loop_points(
            m: *const Model,
            k: c_int,
            uv: *mut f64,
            cap_points: c_int,
            is_outer: *mut c_int,
        ) -> c_int;
        pub fn f3dm_face_eval(
            m: *const Model,
            obj: c_int,
            face: c_int,
            n: c_int,
            uv: *const f64,
            xyz: *mut f64,
            nrm: *mut f64,
        ) -> c_int;
        pub fn f3dm_curve_points(m: *mut Model, obj: c_int) -> c_int;
        pub fn f3dm_points_copy(m: *const Model, xyz: *mut f64, cap_points: c_int) -> c_int;
        pub fn f3dm_mesh_data(m: *mut Model, obj: c_int, nv: *mut c_int, nt: *mut c_int) -> c_int;
        pub fn f3dm_mesh_copy(m: *const Model, xyz: *mut f64, tri: *mut u32);
        pub fn f3dm_writer_new(unit_system: c_int, abs_tol: f64) -> *mut Writer;
        pub fn f3dm_writer_free(w: *mut Writer);
        pub fn f3dm_writer_layer(
            w: *mut Writer,
            name: *const c_char,
            parent: c_int,
            rgb: *const u8,
            visible: c_int,
        ) -> c_int;
        pub fn f3dm_writer_line(
            w: *mut Writer,
            layer: c_int,
            a: *const f64,
            b: *const f64,
        ) -> c_int;
        pub fn f3dm_writer_polyline(
            w: *mut Writer,
            layer: c_int,
            xyz: *const f64,
            n: c_int,
        ) -> c_int;
        pub fn f3dm_writer_arc(
            w: *mut Writer,
            layer: c_int,
            center: *const f64,
            xaxis: *const f64,
            yaxis: *const f64,
            radius: f64,
            sweep: f64,
        ) -> c_int;
        pub fn f3dm_writer_mesh(
            w: *mut Writer,
            layer: c_int,
            xyz: *const f64,
            normals: *const f64,
            nv: c_int,
            tri: *const u32,
            nt: c_int,
        ) -> c_int;
        pub fn f3dm_object_color(m: *const Model, i: c_int, rgb: *mut u8) -> c_int;
        pub fn f3dm_writer_color(w: *mut Writer, has: c_int, rgb: *const u8);
        pub fn f3dm_writer_polycurve(
            w: *mut Writer,
            layer: c_int,
            data: *const f64,
            n: c_int,
        ) -> c_int;
        pub fn f3dm_writer_save(w: *mut Writer, path: *const c_char) -> c_int;
        pub fn f3dm_writer_nurbs(
            w: *mut Writer,
            layer: c_int,
            degree: c_int,
            cv_count: c_int,
            xyz: *const f64,
            weights: *const f64,
            knots: *const f64,
        ) -> c_int;
        pub fn f3dm_writer_point(w: *mut Writer, layer: c_int, xyz: *const f64) -> c_int;
        pub fn f3dm_point(m: *const Model, obj: c_int, xyz: *mut f64) -> c_int;
        pub fn f3dm_write_line(
            path: *const c_char,
            a: *const f64,
            b: *const f64,
            layer: *const c_char,
            unit_system: c_int,
            abs_tol: f64,
        ) -> c_int;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    BadPath(String),
    ReadFailed(String),
    WriteFailed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadPath(p) => write!(f, "invalid path: {p}"),
            Error::ReadFailed(p) => write!(f, "openNURBS could not read {p}"),
            Error::WriteFailed(p) => write!(f, "openNURBS could not write {p}"),
        }
    }
}

impl std::error::Error for Error {}

/// Length units, matching openNURBS `ON::LengthUnitSystem` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    None,
    Millimeters,
    Centimeters,
    Meters,
    Inches,
    Feet,
    Other(i32),
}

impl Units {
    fn from_on(v: i32) -> Self {
        match v {
            0 => Units::None,
            2 => Units::Millimeters,
            3 => Units::Centimeters,
            4 => Units::Meters,
            8 => Units::Inches,
            9 => Units::Feet,
            other => Units::Other(other),
        }
    }

    pub(crate) fn to_on(self) -> i32 {
        match self {
            Units::None => 0,
            Units::Millimeters => 2,
            Units::Centimeters => 3,
            Units::Meters => 4,
            Units::Inches => 8,
            Units::Feet => 9,
            Units::Other(v) => v,
        }
    }
}

/// Kind of a model object. Keep in sync with `F3dmKind` in `shim/shim.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectKind {
    Other,
    Line,
    Polyline,
    PolyCurve,
    NurbsCurve,
    Arc,
    Brep,
    Extrusion,
    Mesh,
    InstanceRef,
    Point,
    Surface,
    SubD,
    Annotation,
}

impl ObjectKind {
    fn from_ffi(v: c_int) -> Self {
        use ObjectKind::*;
        match v {
            1 => Line,
            2 => Polyline,
            3 => PolyCurve,
            4 => NurbsCurve,
            5 => Arc,
            6 => Brep,
            7 => Extrusion,
            8 => Mesh,
            9 => InstanceRef,
            10 => Point,
            11 => Surface,
            12 => SubD,
            13 => Annotation,
            _ => Other,
        }
    }
}

/// One object as seen by the summary reader.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectInfo {
    pub kind: ObjectKind,
    /// Index into [`Summary::layers`], if the layer was found.
    pub layer: Option<usize>,
    /// Number of faces, for breps only.
    pub brep_faces: Option<usize>,
    /// Whether a brep is a closed solid.
    pub is_solid: bool,
    pub bbox: Option<BoundingBox>,
}

/// Overview of a `.3dm` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// 3dm archive version (Rhino 8 writes 80).
    pub archive_version: i32,
    pub units: Units,
    pub absolute_tolerance: f64,
    pub angle_tolerance_deg: f64,
    /// Full layer paths, Rhino style: `parent::child`.
    pub layers: Vec<String>,
    pub material_count: usize,
    pub block_count: usize,
    pub objects: Vec<ObjectInfo>,
}

impl Summary {
    /// Number of objects of the given kind.
    pub fn count(&self, kind: ObjectKind) -> usize {
        self.objects.iter().filter(|o| o.kind == kind).count()
    }

    /// Bounding box of everything with a valid box.
    pub fn extents(&self) -> Option<BoundingBox> {
        let pts: Vec<Point3> = self
            .objects
            .iter()
            .filter_map(|o| o.bbox)
            .flat_map(|b| [b.min, b.max])
            .collect();
        BoundingBox::from_points(&pts)
    }
}

pub(crate) fn c_path(path: &Path) -> Result<CString, Error> {
    let s = path
        .to_str()
        .ok_or_else(|| Error::BadPath(path.display().to_string()))?;
    CString::new(s).map_err(|_| Error::BadPath(s.to_string()))
}

/// Owned openNURBS model, freed on drop.
pub(crate) struct ModelHandle(*mut ffi::Model);

impl ModelHandle {
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        let cpath = c_path(path)?;
        // SAFETY: valid NUL-terminated path; ownership of the result moves into self.
        let m = unsafe { ffi::f3dm_read(cpath.as_ptr()) };
        if m.is_null() {
            Err(Error::ReadFailed(path.display().to_string()))
        } else {
            Ok(Self(m))
        }
    }

    pub(crate) fn ptr(&self) -> *mut ffi::Model {
        self.0
    }

    pub(crate) fn summary(&self) -> Summary {
        // SAFETY: the model is valid for the lifetime of self.
        unsafe { summarise(self.0) }
    }
}

impl Drop for ModelHandle {
    fn drop(&mut self) {
        // SAFETY: allocated by f3dm_read and freed exactly once.
        unsafe { ffi::f3dm_free(self.0) }
    }
}

/// Read a `.3dm` file and summarise its contents.
pub fn read_summary(path: impl AsRef<Path>) -> Result<Summary, Error> {
    Ok(ModelHandle::open(path.as_ref())?.summary())
}

unsafe fn summarise(m: *const ffi::Model) -> Summary {
    let mut layers = Vec::new();
    for i in 0..ffi::f3dm_layer_count(m) {
        let len = ffi::f3dm_layer_path(m, i, std::ptr::null_mut(), 0);
        let mut buf = vec![0u8; len + 1];
        ffi::f3dm_layer_path(m, i, buf.as_mut_ptr().cast::<c_char>(), buf.len());
        buf.truncate(len);
        layers.push(String::from_utf8_lossy(&buf).into_owned());
    }

    let mut objects = Vec::new();
    for i in 0..ffi::f3dm_object_count(m) {
        let mut o = ffi::Object::default();
        if ffi::f3dm_object(m, i, &mut o) == 0 {
            continue;
        }
        let kind = ObjectKind::from_ffi(o.kind);
        let b = o.bbox;
        let bbox = (b != [0.0; 6]).then(|| BoundingBox {
            min: Point3::new(b[0], b[1], b[2]),
            max: Point3::new(b[3], b[4], b[5]),
        });
        objects.push(ObjectInfo {
            kind,
            layer: usize::try_from(o.layer).ok(),
            brep_faces: (kind == ObjectKind::Brep).then_some(o.brep_faces as usize),
            is_solid: o.is_solid != 0,
            bbox,
        });
    }

    Summary {
        archive_version: ffi::f3dm_archive_version(m),
        units: Units::from_on(ffi::f3dm_unit_system(m)),
        absolute_tolerance: ffi::f3dm_abs_tolerance(m),
        angle_tolerance_deg: ffi::f3dm_angle_tolerance_deg(m),
        layers,
        material_count: ffi::f3dm_material_count(m).max(0) as usize,
        block_count: ffi::f3dm_block_count(m).max(0) as usize,
        objects,
    }
}

/// Write a new `.3dm` containing a single line on `layer`.
pub fn write_line(
    path: impl AsRef<Path>,
    a: Point3,
    b: Point3,
    layer: &str,
    units: Units,
    absolute_tolerance: f64,
) -> Result<(), Error> {
    let path = path.as_ref();
    let cpath = c_path(path)?;
    let clayer = CString::new(layer).map_err(|_| Error::BadPath(layer.to_string()))?;
    let pa = [a.x, a.y, a.z];
    let pb = [b.x, b.y, b.z];
    // SAFETY: all pointers are valid for the duration of the call.
    let ok = unsafe {
        ffi::f3dm_write_line(
            cpath.as_ptr(),
            pa.as_ptr(),
            pb.as_ptr(),
            clayer.as_ptr(),
            units.to_on(),
            absolute_tolerance,
        )
    };
    if ok == 1 {
        Ok(())
    } else {
        Err(Error::WriteFailed(path.display().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geom::Tolerance;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("forma-io-3dm-{}-{name}", std::process::id()))
    }

    #[test]
    fn line_round_trip() {
        let path = tmp("line.3dm");
        let a = Point3::new(0.0, 0.0, 0.0);
        let b = Point3::new(100.0, 50.0, 25.0);
        write_line(&path, a, b, "Pareti", Units::Centimeters, 0.01).unwrap();

        let s = read_summary(&path).unwrap();
        assert_eq!(s.units, Units::Centimeters);
        assert!((s.absolute_tolerance - 0.01).abs() < 1e-12);
        assert!(s.layers.iter().any(|l| l == "Pareti"));
        assert_eq!(s.objects.len(), 1);
        let o = &s.objects[0];
        assert_eq!(o.kind, ObjectKind::Line);
        assert_eq!(s.layers[o.layer.unwrap()], "Pareti");
        let bb = o.bbox.unwrap();
        let tol = Tolerance::default();
        assert!(bb.min.almost_eq(a, tol) && bb.max.almost_eq(b, tol));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unicode_layer_name() {
        let path = tmp("unicode.3dm");
        write_line(
            &path,
            Point3::ORIGIN,
            Point3::new(1.0, 0.0, 0.0),
            "Muri è già",
            Units::Millimeters,
            0.001,
        )
        .unwrap();
        let s = read_summary(&path).unwrap();
        assert!(s.layers.iter().any(|l| l == "Muri è già"), "{:?}", s.layers);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(matches!(
            read_summary("/definitely/not/here.3dm"),
            Err(Error::ReadFailed(_))
        ));
    }
}
