//! CPU-side scene: document geometry flattened into vertex arrays, with a per-object
//! cache so edits only re-tessellate what changed.

use bytemuck::{Pod, Zeroable};
use forma_doc::{Document, Geometry, Object, ObjectId};
use forma_geom::{Mesh, Point3, Vec3};
use glam::DVec3;
use std::collections::{BTreeSet, HashMap};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

/// Colour of visible solid/surface edges (linear RGB).
pub const EDGE_COLOR: [f32; 4] = [0.02, 0.02, 0.025, 1.0];
/// Selection highlight (linear RGB), Rhino-like yellow.
pub const HIGHLIGHT: [f32; 4] = [1.0, 0.78, 0.0, 1.0];

/// Everything the GPU needs to draw some objects.
#[derive(Debug, Default, Clone)]
pub struct Scene {
    /// Scene coordinates = model coordinates − origin (keeps f32 precise for large models).
    pub origin: DVec3,
    pub mesh_vertices: Vec<MeshVertex>,
    pub mesh_indices: Vec<u32>,
    /// Curves and edges, as a line list.
    pub line_vertices: Vec<LineVertex>,
    /// Bounds in scene coordinates (zero when empty).
    pub min: DVec3,
    pub max: DVec3,
}

impl Scene {
    pub fn is_empty(&self) -> bool {
        self.mesh_indices.is_empty() && self.line_vertices.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.mesh_indices.len() / 3
    }

    /// Build a scene without a persistent cache (screenshots, tests).
    pub fn from_document(doc: &Document) -> Scene {
        let mut cache = SceneCache::default();
        cache.reset(model_center(doc));
        cache.scene(doc)
    }
}

/// Centre of the visible model, a good scene origin.
pub fn model_center(doc: &Document) -> DVec3 {
    doc.visible_bounding_box().map_or(DVec3::ZERO, |b| {
        let c = b.center();
        DVec3::new(c.x, c.y, c.z)
    })
}

pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Shaded surface colour for a layer colour: lightened so black layers read as grey.
fn surface_color(rgb: [u8; 3]) -> [f32; 4] {
    let l = rgb.map(srgb_to_linear);
    let mix = 0.45;
    [
        l[0] + (0.8 - l[0]) * mix,
        l[1] + (0.8 - l[1]) * mix,
        l[2] + (0.8 - l[2]) * mix,
        1.0,
    ]
}

fn wire_color(rgb: [u8; 3]) -> [f32; 4] {
    let l = rgb.map(srgb_to_linear);
    [l[0], l[1], l[2], 1.0]
}

/// Tessellated form of one object, positions relative to the scene origin.
#[derive(Debug, Default, Clone)]
struct Entry {
    rev: u64,
    color: [u8; 3],
    mesh_vertices: Vec<MeshVertex>,
    mesh_indices: Vec<u32>,
    lines: Vec<LineVertex>,
    min: DVec3,
    max: DVec3,
}

/// Per-object tessellation cache. Keep one per document; call [`SceneCache::reset`]
/// when a different document is loaded.
#[derive(Debug, Default)]
pub struct SceneCache {
    origin: DVec3,
    entries: HashMap<ObjectId, Entry>,
}

impl SceneCache {
    /// Forget everything and use a new origin (use the centre of the model).
    pub fn reset(&mut self, origin: DVec3) {
        self.origin = origin;
        self.entries.clear();
    }

    pub fn origin(&self) -> DVec3 {
        self.origin
    }

    fn entry(&mut self, o: &Object, color: [u8; 3]) -> &Entry {
        let stale = self
            .entries
            .get(&o.id)
            .is_none_or(|e| e.rev != o.rev || e.color != color);
        if stale {
            let e = build_entry(o, color, self.origin);
            self.entries.insert(o.id, e);
        }
        &self.entries[&o.id]
    }

    /// Build the scene of all visible objects (visible layer, not hidden).
    pub fn scene(&mut self, doc: &Document) -> Scene {
        self.entries.retain(|id, _| doc.object(*id).is_some());
        let mut s = Scene {
            origin: self.origin,
            ..Default::default()
        };
        let mut first = true;
        for o in doc.objects() {
            if !doc.is_visible(o) {
                continue;
            }
            let e = self.entry(o, doc.display_color(o));
            append(&mut s, e, None, &mut first);
        }
        s
    }

    /// Build a highlight scene for the given objects (drawn over the scene in yellow).
    pub fn highlight(&mut self, doc: &Document, ids: &BTreeSet<ObjectId>) -> Scene {
        let mut s = Scene {
            origin: self.origin,
            ..Default::default()
        };
        let mut first = true;
        for id in ids {
            let Some(o) = doc.object(*id) else { continue };
            if !doc.is_visible(o) {
                continue;
            }
            let e = self.entry(o, doc.display_color(o));
            append(&mut s, e, Some(HIGHLIGHT), &mut first);
        }
        s
    }
}

fn append(s: &mut Scene, e: &Entry, highlight: Option<[f32; 4]>, first: &mut bool) {
    let base = s.mesh_vertices.len() as u32;
    match highlight {
        None => {
            s.mesh_vertices.extend_from_slice(&e.mesh_vertices);
            s.line_vertices.extend_from_slice(&e.lines);
        }
        Some(h) => {
            s.mesh_vertices.extend(e.mesh_vertices.iter().map(|v| {
                let c = v.color;
                MeshVertex {
                    color: [
                        c[0] * 0.72 + h[0] * 0.28,
                        c[1] * 0.72 + h[1] * 0.28,
                        c[2] * 0.72 + h[2] * 0.28,
                        1.0,
                    ],
                    ..*v
                }
            }));
            s.line_vertices
                .extend(e.lines.iter().map(|v| LineVertex { color: h, ..*v }));
        }
    }
    s.mesh_indices
        .extend(e.mesh_indices.iter().map(|i| i + base));
    if e.mesh_vertices.is_empty() && e.lines.is_empty() {
        return;
    }
    if *first {
        s.min = e.min;
        s.max = e.max;
        *first = false;
    } else {
        s.min = s.min.min(e.min);
        s.max = s.max.max(e.max);
    }
}

fn build_entry(o: &Object, color: [u8; 3], origin: DVec3) -> Entry {
    let local = |p: &Point3| -> [f32; 3] {
        [
            (p.x - origin.x) as f32,
            (p.y - origin.y) as f32,
            (p.z - origin.z) as f32,
        ]
    };
    let mut e = Entry {
        rev: o.rev,
        color,
        ..Default::default()
    };
    let b = o.geometry.bounding_box();
    e.min = DVec3::new(b.min.x, b.min.y, b.min.z) - origin;
    e.max = DVec3::new(b.max.x, b.max.y, b.max.z) - origin;
    match &o.geometry {
        Geometry::Mesh(m) => {
            let c = surface_color(color);
            let normals = vertex_normals(m);
            for (p, n) in m.positions.iter().zip(&normals) {
                e.mesh_vertices.push(MeshVertex {
                    pos: local(p),
                    normal: [n.x as f32, n.y as f32, n.z as f32],
                    color: c,
                });
            }
            let n = m.positions.len() as u32;
            for t in &m.triangles {
                if t.iter().all(|&i| i < n) {
                    e.mesh_indices.extend_from_slice(t);
                }
            }
            for [a, b] in m.boundary_edges() {
                e.lines.push(LineVertex {
                    pos: local(&m.positions[a as usize]),
                    color: EDGE_COLOR,
                });
                e.lines.push(LineVertex {
                    pos: local(&m.positions[b as usize]),
                    color: EDGE_COLOR,
                });
            }
        }
        Geometry::Point(p) => {
            // Small 3D cross until the display draws screen-space point markers.
            let c = wire_color(color);
            let r = 2.0;
            for d in [Vec3::X, Vec3::Y, Vec3::Z] {
                for q in [*p - d * r, *p + d * r] {
                    e.lines.push(LineVertex {
                        pos: local(&q),
                        color: c,
                    });
                }
            }
        }
        g => {
            let c = wire_color(color);
            let pts = g.curve_points();
            for w in pts.windows(2) {
                e.lines.push(LineVertex {
                    pos: local(&w[0]),
                    color: c,
                });
                e.lines.push(LineVertex {
                    pos: local(&w[1]),
                    color: c,
                });
            }
        }
    }
    e
}

/// Per-vertex normals: the mesh's own where valid, otherwise area-weighted face normals.
fn vertex_normals(m: &Mesh) -> Vec<Vec3> {
    let n = m.positions.len();
    if m.normals.len() == n && m.normals.iter().all(|v| v.length() > 0.5) {
        return m.normals.clone();
    }
    let mut acc = vec![Vec3::new(0.0, 0.0, 0.0); n];
    for t in &m.triangles {
        if t.iter().any(|&i| i as usize >= n) {
            continue;
        }
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        let f = (b - a).cross(c - a);
        for &i in t {
            acc[i as usize] = acc[i as usize] + f;
        }
    }
    (0..n)
        .map(|i| {
            m.normals
                .get(i)
                .and_then(|v| v.normalized())
                .or_else(|| acc[i].normalized())
                .unwrap_or(Vec3::new(0.0, 0.0, 0.0))
        })
        .collect()
}

/// Construction plane a grid is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridPlane {
    /// World XY (Top and Perspective views).
    XY,
    /// World XZ (Front view).
    XZ,
    /// World YZ (Right view).
    YZ,
}

impl GridPlane {
    pub const ALL: [GridPlane; 3] = [GridPlane::XY, GridPlane::XZ, GridPlane::YZ];

    pub fn index(self) -> usize {
        match self {
            GridPlane::XY => 0,
            GridPlane::XZ => 1,
            GridPlane::YZ => 2,
        }
    }

    /// World axes (u, v) of the plane.
    pub fn axes(self) -> (DVec3, DVec3) {
        match self {
            GridPlane::XY => (DVec3::X, DVec3::Y),
            GridPlane::XZ => (DVec3::X, DVec3::Z),
            GridPlane::YZ => (DVec3::Y, DVec3::Z),
        }
    }
}

/// A "nice" grid spacing (1, 2 or 5 × 10^k) giving about 30 lines across `extent`.
pub fn grid_spacing(extent: f64) -> f64 {
    let raw = (extent.max(1e-6) * 2.0) / 30.0;
    let mag = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw * 0.999)
        .unwrap_or(10.0 * mag)
}

/// Grid lines on a world plane covering the scene, with the world axes in colour.
/// `min_extent` keeps the grid useful for empty or tiny documents. Returns the lines
/// and the spacing used.
pub fn grid_lines(scene: &Scene, plane: GridPlane, min_extent: f64) -> (Vec<LineVertex>, f64) {
    let o = scene.origin;
    let (u, v) = plane.axes();
    // Rhino-like: the grid is centred on the world origin and large enough to cover
    // the model with margin; far-away models get a grid around themselves instead.
    let wmin = scene.min + o;
    let wmax = scene.max + o;
    let reach = [wmin, wmax]
        .iter()
        .flat_map(|c| [c.dot(u).abs(), c.dot(v).abs()])
        .fold(0.0f64, f64::max);
    let size = (scene.max - scene.min).length();
    let (center, extent) = if reach <= size.max(min_extent) * 4.0 {
        (DVec3::ZERO, (reach * 1.5).max(size).max(min_extent))
    } else {
        (
            o + (scene.min + scene.max) / 2.0,
            (size * 1.5).max(min_extent),
        )
    };
    let spacing = grid_spacing(extent);
    let half = (extent / spacing).ceil() as i64;
    let cu = (center.dot(u) / spacing).round() as i64;
    let cv = (center.dot(v) / spacing).round() as i64;
    let minor = [0.50, 0.50, 0.52, 1.0];
    let major = [0.36, 0.36, 0.38, 1.0];
    let red = [0.60, 0.04, 0.04, 1.0];
    let green = [0.04, 0.45, 0.04, 1.0];
    let blue = [0.08, 0.20, 0.70, 1.0];
    // Colour of the line where the u coordinate is 0 (the v axis) and vice versa.
    let (u_axis, v_axis) = match plane {
        GridPlane::XY => (red, green),
        GridPlane::XZ => (red, blue),
        GridPlane::YZ => (green, blue),
    };
    let world = |a: f64, b: f64| -> [f32; 3] {
        let p = u * a + v * b - o;
        [p.x as f32, p.y as f32, p.z as f32]
    };
    let (u0, u1) = ((cu - half) as f64 * spacing, (cu + half) as f64 * spacing);
    let (v0, v1) = ((cv - half) as f64 * spacing, (cv + half) as f64 * spacing);
    // Each line is split into one-cell segments: very long lines that cross the
    // camera's near plane are rasterised badly by some drivers.
    let n = (2 * half) as usize;
    let mut out = Vec::with_capacity((n + 1) * 2 * n * 2);
    let color = |i: i64, axis: [f32; 4]| {
        if i == 0 {
            axis
        } else if i % 5 == 0 {
            major
        } else {
            minor
        }
    };
    for k in -half..=half {
        let iu = cu + k;
        let a = iu as f64 * spacing;
        let c = color(iu, v_axis);
        for j in 0..n {
            let b0 = v0 + (v1 - v0) * j as f64 / n as f64;
            let b1 = v0 + (v1 - v0) * (j + 1) as f64 / n as f64;
            out.push(LineVertex {
                pos: world(a, b0),
                color: c,
            });
            out.push(LineVertex {
                pos: world(a, b1),
                color: c,
            });
        }
        let iv = cv + k;
        let b = iv as f64 * spacing;
        let c = color(iv, u_axis);
        for j in 0..n {
            let a0 = u0 + (u1 - u0) * j as f64 / n as f64;
            let a1 = u0 + (u1 - u0) * (j + 1) as f64 / n as f64;
            out.push(LineVertex {
                pos: world(a0, b),
                color: c,
            });
            out.push(LineVertex {
                pos: world(a1, b),
                color: c,
            });
        }
    }
    (out, spacing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with_box() -> Document {
        let mut d = Document::new();
        let mut t = d.begin();
        t.add(Geometry::Mesh(forma_geom::box_mesh(
            &forma_geom::Plane::TOP,
            1.0,
            1.0,
            1.0,
        )));
        t.commit();
        d
    }

    #[test]
    fn cache_follows_document_changes() {
        let mut d = doc_with_box();
        let mut cache = SceneCache::default();
        let s1 = cache.scene(&d);
        assert_eq!(s1.triangle_count(), 12);
        assert_eq!(s1.line_vertices.len(), 24 * 2); // box edges
        let ids: Vec<_> = d.objects().map(|o| o.id).collect();
        let hl = cache.highlight(&d, &ids.iter().copied().collect());
        assert_eq!(hl.triangle_count(), 12);
        assert_eq!(hl.line_vertices[0].color, HIGHLIGHT);
        let mut t = d.begin();
        for id in ids {
            t.remove(id);
        }
        t.commit();
        assert!(cache.scene(&d).is_empty());
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn spacing_is_nice() {
        assert_eq!(grid_spacing(1500.0), 100.0);
        assert_eq!(grid_spacing(75.0), 5.0);
    }
}
