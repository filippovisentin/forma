//! Solid modelling on closed meshes through the OpenCascade kernel (ADR 0001, 0004).
//!
//! Every function takes and returns plain [`Mesh`]es: the mesh is sewn into an exact
//! solid (coplanar triangles merged into planar faces), the operation runs on the exact
//! solid, and the result is triangulated again with one set of vertices per face, so
//! sharp edges stay visible and smooth seams (fillets, facets of a cylinder) are welded.
//! No OpenCascade type leaves this module.
//!
//! Without the cargo feature `occt` every function returns
//! [`KernelError::Unavailable`].

use crate::{Mesh, Point3, Tolerance};
use std::fmt;

#[cfg(feature = "occt")]
mod occt;

/// True when this build includes the solid kernel.
pub const KERNEL_AVAILABLE: bool = cfg!(feature = "occt");

/// Error from a solid operation.
#[derive(Debug, Clone, PartialEq)]
pub enum KernelError {
    /// Built without the `occt` feature.
    Unavailable,
    /// The kernel refused or failed; the message says why.
    Failed(String),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::Unavailable => {
                write!(f, "needs the solid kernel (build with --features occt)")
            }
            KernelError::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for KernelError {}

/// Tolerances for solid operations and for triangulating their results.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidOptions {
    /// Sewing / boolean fuzzy tolerance in model units.
    pub tolerance: f64,
    /// Maximum chord deviation of the result mesh; `0` picks one from the size of the
    /// result (1/2000 of its diagonal, at least `tolerance`).
    pub deflection: f64,
    /// Maximum angle between neighbouring facets of a curved face, radians.
    pub angle: f64,
}

impl SolidOptions {
    pub fn new(tol: Tolerance) -> Self {
        Self {
            tolerance: tol.absolute.max(1e-7),
            deflection: 0.0,
            angle: 12f64.to_radians(),
        }
    }

    /// Fine triangulation, for tests that compare volumes with exact values.
    pub fn fine(tol: Tolerance) -> Self {
        Self {
            deflection: tol.absolute.max(1e-7) * 0.1,
            angle: 1f64.to_radians(),
            ..Self::new(tol)
        }
    }
}

impl Default for SolidOptions {
    fn default() -> Self {
        Self::new(Tolerance::default())
    }
}

/// Boolean operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    /// Everything inside any input.
    Union,
    /// The first inputs minus the second ones.
    Difference,
    /// What is inside both groups.
    Intersection,
    /// The first inputs cut into pieces by the second ones (all pieces kept).
    Split,
}

/// Boolean between two groups of closed meshes. For `Union`, `b` may be empty and
/// all of `a` is merged. Returns one mesh per resulting closed solid.
pub fn solid_boolean(
    op: BooleanOp,
    a: &[&Mesh],
    b: &[&Mesh],
    opts: SolidOptions,
) -> Result<Vec<Mesh>, KernelError> {
    #[cfg(feature = "occt")]
    {
        occt::boolean(op, a, b, opts)
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (op, a, b, opts);
        Err(KernelError::Unavailable)
    }
}

/// Round the edges of a closed mesh nearest to each point with `radius` (`chamfer`
/// false) or bevel them at `radius` from the edge on both faces (`chamfer` true).
pub fn fillet_edges(
    solid: &Mesh,
    radius: f64,
    near: &[Point3],
    chamfer: bool,
    opts: SolidOptions,
) -> Result<Mesh, KernelError> {
    #[cfg(feature = "occt")]
    {
        occt::fillet(solid, radius, near, chamfer, opts)
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (solid, radius, near, chamfer, opts);
        Err(KernelError::Unavailable)
    }
}

/// Hollow a closed mesh: walls of `thickness` inside it, the faces nearest to the
/// points removed (open).
pub fn shell_solid(
    solid: &Mesh,
    thickness: f64,
    remove_near: &[Point3],
    opts: SolidOptions,
) -> Result<Mesh, KernelError> {
    #[cfg(feature = "occt")]
    {
        occt::shell(solid, thickness, remove_near, opts)
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (solid, thickness, remove_near, opts);
        Err(KernelError::Unavailable)
    }
}

/// Offset every face of a closed mesh by `distance` (positive grows the solid),
/// keeping sharp edges sharp.
pub fn offset_solid(solid: &Mesh, distance: f64, opts: SolidOptions) -> Result<Mesh, KernelError> {
    #[cfg(feature = "occt")]
    {
        occt::offset(solid, distance, opts)
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (solid, distance, opts);
        Err(KernelError::Unavailable)
    }
}

/// Facts about the exact solid a closed mesh becomes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidInfo {
    /// Number of closed solids.
    pub solids: usize,
    /// Number of faces after merging coplanar triangles.
    pub faces: usize,
    /// Exact volume.
    pub volume: f64,
    /// OpenCascade's validity check passed.
    pub valid: bool,
}

/// Sew a closed mesh into exact solid(s) and report on them.
pub fn solid_info(mesh: &Mesh, opts: SolidOptions) -> Result<SolidInfo, KernelError> {
    #[cfg(feature = "occt")]
    {
        occt::info(mesh, opts)
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (mesh, opts);
        Err(KernelError::Unavailable)
    }
}

/// Sew a closed mesh into exact solid(s) and triangulate them again: coplanar
/// triangles become one face, sharp edges get separate vertices.
pub fn rebuild_solid(mesh: &Mesh, opts: SolidOptions) -> Result<Mesh, KernelError> {
    #[cfg(feature = "occt")]
    {
        occt::rebuild(mesh, opts)
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (mesh, opts);
        Err(KernelError::Unavailable)
    }
}

/// Weld vertices of different faces that share a position and whose normals differ
/// by less than `crease` (radians), averaging their normals: smooth seams disappear,
/// sharp edges keep separate vertices. `face` gives the face of each vertex.
#[cfg_attr(not(feature = "occt"), allow(dead_code))]
pub(crate) fn weld_smooth_seams(mesh: &mut Mesh, face: &[u32], crease: f64) {
    use std::collections::HashMap;
    let n = mesh.positions.len();
    if n == 0 || mesh.normals.len() != n || face.len() != n {
        return;
    }
    let bb = mesh.bounding_box().expect("non-empty");
    let eps = (bb.min.distance_to(bb.max) * 1e-9).max(1e-12);
    let key = |p: Point3| {
        (
            (p.x / eps).round() as i64,
            (p.y / eps).round() as i64,
            (p.z / eps).round() as i64,
        )
    };
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let cos = crease.cos();
    let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for i in 0..n {
        cells.entry(key(mesh.positions[i])).or_default().push(i);
    }
    for group in cells.values().filter(|g| g.len() > 1) {
        for (k, &i) in group.iter().enumerate() {
            for &j in &group[k + 1..] {
                if face[i] != face[j] && mesh.normals[i].dot(mesh.normals[j]) >= cos {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[b] = a;
                }
            }
        }
    }
    let mut sum: HashMap<usize, crate::Vec3> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        let e = sum.entry(r).or_default();
        *e = *e + mesh.normals[i];
    }
    let mut remap = vec![0u32; n];
    let mut out = Mesh::default();
    let mut new_index: HashMap<usize, u32> = HashMap::new();
    for (i, slot) in remap.iter_mut().enumerate() {
        let r = find(&mut parent, i);
        *slot = *new_index.entry(r).or_insert_with(|| {
            out.positions.push(mesh.positions[r]);
            out.normals
                .push(sum[&r].normalized().unwrap_or(mesh.normals[r]));
            (out.positions.len() - 1) as u32
        });
    }
    out.triangles = mesh
        .triangles
        .iter()
        .map(|t| t.map(|v| remap[v as usize]))
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .collect();
    *mesh = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{box_mesh, cylinder_mesh, Plane};

    fn tol() -> Tolerance {
        Tolerance::default()
    }

    fn box_at(x: f64, y: f64, z: f64, u: f64, v: f64, w: f64) -> Mesh {
        box_mesh(&Plane::TOP.moved_to(Point3::new(x, y, z)), u, v, w)
    }

    fn rel(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs()
    }

    #[test]
    fn without_the_kernel_everything_says_so() {
        if KERNEL_AVAILABLE {
            return;
        }
        let b = box_at(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let e = solid_boolean(BooleanOp::Union, &[&b], &[&b], SolidOptions::default());
        assert_eq!(e, Err(KernelError::Unavailable));
        assert!(KernelError::Unavailable
            .to_string()
            .contains("--features occt"));
    }

    #[test]
    fn box_becomes_six_faces_and_comes_back_crisp() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let b = box_at(0.0, 0.0, 0.0, 10.0, 20.0, 30.0);
        let info = solid_info(&b, SolidOptions::default()).unwrap();
        assert_eq!(info.solids, 1);
        assert_eq!(info.faces, 6);
        assert!(info.valid);
        assert!(rel(info.volume, 6000.0) < 1e-9);
        let m = rebuild_solid(&b, SolidOptions::default()).unwrap();
        assert!(rel(m.volume(), 6000.0) < 1e-9);
        assert_eq!(m.boundary_edges().len(), 24); // 12 edges, both sides
    }

    #[test]
    fn faceted_cylinder_keeps_a_smooth_side() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let c = cylinder_mesh(&Plane::TOP, 5.0, 10.0, 48);
        let m = rebuild_solid(&c, SolidOptions::default()).unwrap();
        assert!(rel(m.volume(), c.volume()) < 1e-9);
        let verticals = m
            .boundary_edges()
            .iter()
            .filter(|e| {
                let (a, b) = (m.positions[e[0] as usize], m.positions[e[1] as usize]);
                (a.z - b.z).abs() > 1e-6
            })
            .count();
        assert_eq!(verticals, 0, "facet seams should be welded");
    }

    #[test]
    fn open_mesh_is_refused() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let mut b = box_at(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        b.triangles.truncate(10);
        let e = solid_info(&b, SolidOptions::default()).unwrap_err();
        assert!(e.to_string().contains("not closed"), "{e}");
    }

    #[test]
    fn union_difference_intersection_of_two_boxes() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let a = box_at(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);
        let b = box_at(5.0, 5.0, 5.0, 10.0, 10.0, 10.0);
        let o = SolidOptions::default();
        let u = solid_boolean(BooleanOp::Union, &[&a], &[&b], o).unwrap();
        assert_eq!(u.len(), 1);
        assert!(
            rel(u[0].volume(), 2000.0 - 125.0) < 1e-9,
            "{}",
            u[0].volume()
        );
        let d = solid_boolean(BooleanOp::Difference, &[&a], &[&b], o).unwrap();
        assert!(rel(d[0].volume(), 1000.0 - 125.0) < 1e-9);
        let i = solid_boolean(BooleanOp::Intersection, &[&a], &[&b], o).unwrap();
        assert!(rel(i[0].volume(), 125.0) < 1e-9);
        let s = solid_boolean(BooleanOp::Split, &[&a], &[&b], o).unwrap();
        assert_eq!(s.len(), 2);
        let total: f64 = s.iter().map(Mesh::volume).sum();
        assert!(rel(total, 1000.0) < 1e-9);
        // Union of a list (no second group).
        let u2 = solid_boolean(BooleanOp::Union, &[&a, &b], &[], o).unwrap();
        assert!(rel(u2[0].volume(), 1875.0) < 1e-9);
    }

    #[test]
    fn difference_into_two_pieces_gives_two_meshes() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let bar = box_at(0.0, 0.0, 0.0, 30.0, 10.0, 10.0);
        let cut = box_at(10.0, -1.0, -1.0, 10.0, 12.0, 12.0);
        let d = solid_boolean(
            BooleanOp::Difference,
            &[&bar],
            &[&cut],
            SolidOptions::default(),
        )
        .unwrap();
        assert_eq!(d.len(), 2);
        for m in &d {
            assert!(rel(m.volume(), 1000.0) < 1e-9);
        }
    }

    #[test]
    fn fillet_of_a_box_edge_removes_the_expected_volume() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let (l, r) = (40.0, 3.0);
        let b = box_at(0.0, 0.0, 0.0, l, 20.0, 20.0);
        // Edge along x at y = 0, z = 20.
        let f = fillet_edges(
            &b,
            r,
            &[Point3::new(l / 2.0, 0.0, 20.0)],
            false,
            SolidOptions::fine(tol()),
        )
        .unwrap();
        let removed = 16000.0 - f.volume();
        let exact = (1.0 - std::f64::consts::PI / 4.0) * r * r * l;
        assert!(rel(removed, exact) < 1e-3, "{removed} vs {exact}");
        let info = solid_info(&f, SolidOptions::default()).unwrap();
        assert_eq!(info.solids, 1);
    }

    #[test]
    fn chamfer_of_a_box_edge() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let b = box_at(0.0, 0.0, 0.0, 40.0, 20.0, 20.0);
        let c = fillet_edges(
            &b,
            2.0,
            &[Point3::new(20.0, 0.0, 20.0)],
            true,
            SolidOptions::default(),
        )
        .unwrap();
        assert!(rel(16000.0 - c.volume(), 2.0 * 2.0 / 2.0 * 40.0) < 1e-9);
    }

    #[test]
    fn fillet_needs_a_point_near_an_edge() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let b = box_at(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);
        let e = fillet_edges(
            &b,
            1.0,
            &[Point3::new(50.0, 50.0, 50.0)],
            false,
            SolidOptions::default(),
        );
        assert!(e.is_err());
    }

    #[test]
    fn shell_a_box_open_at_the_top() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let b = box_at(0.0, 0.0, 0.0, 40.0, 30.0, 20.0);
        let s = shell_solid(
            &b,
            2.0,
            &[Point3::new(20.0, 15.0, 20.0)],
            SolidOptions::default(),
        )
        .unwrap();
        let inner = 36.0 * 26.0 * 18.0;
        assert!(rel(s.volume(), 24000.0 - inner) < 1e-6, "{}", s.volume());
    }

    #[test]
    fn offset_a_box_outwards() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let b = box_at(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);
        let o = offset_solid(&b, 1.0, SolidOptions::default()).unwrap();
        assert!(rel(o.volume(), 12.0f64.powi(3)) < 1e-6, "{}", o.volume());
    }

    // Interior-design cases (Filippo's Rhino samples are not in the repo).

    #[test]
    fn wall_with_a_niche_shares_the_front_face() {
        if !KERNEL_AVAILABLE {
            return;
        }
        // Wall 300 × 20 × 270 cm; niche 80 wide, 10 deep, 120 high, flush with the front.
        let wall = box_at(0.0, 0.0, 0.0, 300.0, 20.0, 270.0);
        let niche = box_at(110.0, 0.0, 90.0, 80.0, 10.0, 120.0);
        let d = solid_boolean(
            BooleanOp::Difference,
            &[&wall],
            &[&niche],
            SolidOptions::default(),
        )
        .unwrap();
        assert_eq!(d.len(), 1);
        assert!(rel(d[0].volume(), 300.0 * 20.0 * 270.0 - 80.0 * 10.0 * 120.0) < 1e-9);
        let info = solid_info(&d[0], SolidOptions::default()).unwrap();
        assert_eq!(info.faces, 11); // 6 of the wall + 5 of the niche, front merged
        assert!(info.valid);
    }

    #[test]
    fn table_top_with_cable_holes() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let top = box_at(0.0, 0.0, 0.0, 160.0, 80.0, 3.0);
        let hole = |x: f64| {
            let p = Plane::TOP.moved_to(Point3::new(x, 60.0, -1.0));
            cylinder_mesh(&p, 4.0, 5.0, 64)
        };
        let (h1, h2) = (hole(30.0), hole(130.0));
        let d = solid_boolean(
            BooleanOp::Difference,
            &[&top],
            &[&h1, &h2],
            SolidOptions::default(),
        )
        .unwrap();
        assert_eq!(d.len(), 1);
        // The holes are 64-gons: area = ½·n·r²·sin(2π/n).
        let n = 64.0;
        let hole_area = 0.5 * n * 16.0 * (std::f64::consts::TAU / n).sin();
        let exact = 160.0 * 80.0 * 3.0 - 2.0 * hole_area * 3.0;
        assert!(
            rel(d[0].volume(), exact) < 1e-6,
            "{} vs {exact}",
            d[0].volume()
        );
    }

    #[test]
    fn two_cabinet_modules_sharing_a_face_become_one_box() {
        if !KERNEL_AVAILABLE {
            return;
        }
        let a = box_at(0.0, 0.0, 0.0, 60.0, 58.0, 72.0);
        let b = box_at(60.0, 0.0, 0.0, 60.0, 58.0, 72.0);
        let u = solid_boolean(BooleanOp::Union, &[&a, &b], &[], SolidOptions::default()).unwrap();
        assert_eq!(u.len(), 1);
        assert!(rel(u[0].volume(), 120.0 * 58.0 * 72.0) < 1e-9);
        let info = solid_info(&u[0], SolidOptions::default()).unwrap();
        assert_eq!(
            info.faces, 6,
            "shared face removed and coplanar faces merged"
        );
    }

    #[test]
    fn rounded_shelf_edge_after_a_boolean() {
        if !KERNEL_AVAILABLE {
            return;
        }
        // Shelf with a notch, then the front top edge rounded.
        let shelf = box_at(0.0, 0.0, 0.0, 100.0, 30.0, 2.0);
        let notch = box_at(-1.0, 20.0, -1.0, 11.0, 11.0, 4.0);
        let d = solid_boolean(
            BooleanOp::Difference,
            &[&shelf],
            &[&notch],
            SolidOptions::default(),
        )
        .unwrap();
        let f = fillet_edges(
            &d[0],
            0.5,
            &[Point3::new(50.0, 0.0, 2.0)],
            false,
            SolidOptions::fine(tol()),
        )
        .unwrap();
        let before = 100.0 * 30.0 * 2.0 - 10.0 * 10.0 * 2.0;
        let removed = (1.0 - std::f64::consts::PI / 4.0) * 0.25 * 100.0;
        assert!(rel(before - f.volume(), removed) < 1e-3);
    }
}
