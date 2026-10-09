//! CPU-side scene: document geometry flattened into vertex arrays.

use bytemuck::{Pod, Zeroable};
use forma_doc::{Document, Geometry};
use forma_geom::{Mesh, Point3, Vec3};
use glam::DVec3;

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

/// Everything the GPU needs to draw a document.
#[derive(Debug, Default)]
pub struct Scene {
    /// Scene coordinates = model coordinates − origin (keeps f32 precise for large models).
    pub origin: DVec3,
    pub mesh_vertices: Vec<MeshVertex>,
    pub mesh_indices: Vec<u32>,
    pub line_vertices: Vec<LineVertex>,
    /// Bounds in scene coordinates.
    pub min: DVec3,
    pub max: DVec3,
}

fn srgb_to_linear(c: u8) -> f32 {
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

impl Scene {
    pub fn is_empty(&self) -> bool {
        self.mesh_indices.is_empty() && self.line_vertices.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.mesh_indices.len() / 3
    }

    /// Build from the visible layers of a document.
    pub fn from_document(doc: &Document) -> Scene {
        let bb = doc.visible_bounding_box();
        let origin = bb.map_or(DVec3::ZERO, |b| {
            DVec3::new(
                (b.min.x + b.max.x) / 2.0,
                (b.min.y + b.max.y) / 2.0,
                (b.min.z + b.max.z) / 2.0,
            )
        });
        let mut s = Scene {
            origin,
            ..Default::default()
        };
        if let Some(b) = bb {
            s.min = DVec3::new(b.min.x, b.min.y, b.min.z) - origin;
            s.max = DVec3::new(b.max.x, b.max.y, b.max.z) - origin;
        }
        let local = |p: &Point3| -> [f32; 3] {
            [
                (p.x - origin.x) as f32,
                (p.y - origin.y) as f32,
                (p.z - origin.z) as f32,
            ]
        };
        for o in doc.objects() {
            let layer = doc.layer(o.layer);
            if !layer.visible {
                continue;
            }
            match &o.geometry {
                Geometry::Mesh(m) => s.push_mesh(m, surface_color(layer.color), &local),
                Geometry::Polyline(p) => {
                    let c = wire_color(layer.color);
                    for w in p.windows(2) {
                        s.line_vertices.push(LineVertex {
                            pos: local(&w[0]),
                            color: c,
                        });
                        s.line_vertices.push(LineVertex {
                            pos: local(&w[1]),
                            color: c,
                        });
                    }
                }
                Geometry::Line(l) => {
                    let c = wire_color(layer.color);
                    s.line_vertices.push(LineVertex {
                        pos: local(&l.from),
                        color: c,
                    });
                    s.line_vertices.push(LineVertex {
                        pos: local(&l.to),
                        color: c,
                    });
                }
            }
        }
        s
    }

    fn push_mesh(&mut self, m: &Mesh, color: [f32; 4], local: &dyn Fn(&Point3) -> [f32; 3]) {
        let base = self.mesh_vertices.len() as u32;
        let normals = vertex_normals(m);
        for (p, n) in m.positions.iter().zip(&normals) {
            self.mesh_vertices.push(MeshVertex {
                pos: local(p),
                normal: [n.x as f32, n.y as f32, n.z as f32],
                color,
            });
        }
        let n = m.positions.len() as u32;
        for t in &m.triangles {
            if t.iter().all(|&i| i < n) {
                self.mesh_indices.extend(t.iter().map(|i| i + base));
            }
        }
    }
}

/// Per-vertex normals: the mesh's own where valid, otherwise area-weighted face normals.
fn vertex_normals(m: &Mesh) -> Vec<Vec3> {
    let n = m.positions.len();
    let mut acc = vec![Vec3::new(0.0, 0.0, 0.0); n];
    for t in &m.triangles {
        if t.iter().any(|&i| i as usize >= n) {
            continue;
        }
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        let f = (b - a).cross(c - a);
        for &i in t {
            let v = &mut acc[i as usize];
            *v = Vec3::new(v.x + f.x, v.y + f.y, v.z + f.z);
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

/// Grid lines on the XY plane (Rhino's construction plane) covering the scene, plus
/// red/green X/Y axes. Returns vertices in scene coordinates.
pub fn grid_lines(scene: &Scene) -> Vec<LineVertex> {
    let extent = ((scene.max - scene.min).length() * 0.75).max(10.0);
    // Spacing: a "nice" 1/2/5×10^k value giving roughly 30 lines across.
    let raw = extent * 2.0 / 30.0;
    let mag = 10f64.powf(raw.log10().floor());
    let spacing = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let half = (extent / spacing).ceil() as i64;
    let o = scene.origin;
    // Grid lines sit at model coordinates that are multiples of `spacing`.
    let cx = (o.x / spacing).round() as i64;
    let cy = (o.y / spacing).round() as i64;
    let z = (0.0 - o.z) as f32;
    let minor = [0.55, 0.55, 0.57, 1.0];
    let major = [0.42, 0.42, 0.45, 1.0];
    let mut v = Vec::new();
    for k in -half..=half {
        let gx = (cx + k) as f64 * spacing;
        let gy = (cy + k) as f64 * spacing;
        let y0 = ((cy - half) as f64 * spacing - o.y) as f32;
        let y1 = ((cy + half) as f64 * spacing - o.y) as f32;
        let x0 = ((cx - half) as f64 * spacing - o.x) as f32;
        let x1 = ((cx + half) as f64 * spacing - o.x) as f32;
        let cxk = if (cx + k) % 5 == 0 { major } else { minor };
        let cyk = if (cy + k) % 5 == 0 { major } else { minor };
        let x = (gx - o.x) as f32;
        let y = (gy - o.y) as f32;
        if cx + k == 0 {
            // Y axis (green)
            v.push(LineVertex {
                pos: [x, y0, z],
                color: [0.05, 0.45, 0.05, 1.0],
            });
            v.push(LineVertex {
                pos: [x, y1, z],
                color: [0.05, 0.45, 0.05, 1.0],
            });
        } else {
            v.push(LineVertex {
                pos: [x, y0, z],
                color: cxk,
            });
            v.push(LineVertex {
                pos: [x, y1, z],
                color: cxk,
            });
        }
        if cy + k == 0 {
            // X axis (red)
            v.push(LineVertex {
                pos: [x0, y, z],
                color: [0.55, 0.05, 0.05, 1.0],
            });
            v.push(LineVertex {
                pos: [x1, y, z],
                color: [0.55, 0.05, 0.05, 1.0],
            });
        } else {
            v.push(LineVertex {
                pos: [x0, y, z],
                color: cyk,
            });
            v.push(LineVertex {
                pos: [x1, y, z],
                color: cyk,
            });
        }
    }
    v
}
