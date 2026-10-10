//! Mesh reduction (Rhino's ReduceMesh): quadric-error edge collapses
//! (Garland & Heckbert) until the triangle count is reached. Borders are kept
//! in place by extra planes along them, and a collapse that would flip a
//! triangle is skipped.

use crate::{Mesh, Point3, Vec3};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Symmetric 4×4 quadric, upper triangle: a² ab ac ad b² bc bd c² cd d².
#[derive(Clone, Copy, Default)]
struct Quadric([f64; 10]);

impl Quadric {
    fn plane(n: Vec3, d: f64, w: f64) -> Quadric {
        let (a, b, c) = (n.x, n.y, n.z);
        Quadric([
            a * a * w,
            a * b * w,
            a * c * w,
            a * d * w,
            b * b * w,
            b * c * w,
            b * d * w,
            c * c * w,
            c * d * w,
            d * d * w,
        ])
    }
    fn add(&self, o: &Quadric) -> Quadric {
        let mut r = self.0;
        for (x, y) in r.iter_mut().zip(o.0) {
            *x += y;
        }
        Quadric(r)
    }
    fn error(&self, p: Point3) -> f64 {
        let q = &self.0;
        let (x, y, z) = (p.x, p.y, p.z);
        q[0] * x * x
            + 2.0 * q[1] * x * y
            + 2.0 * q[2] * x * z
            + 2.0 * q[3] * x
            + q[4] * y * y
            + 2.0 * q[5] * y * z
            + 2.0 * q[6] * y
            + q[7] * z * z
            + 2.0 * q[8] * z
            + q[9]
    }
}

struct Candidate {
    cost: f64,
    a: u32,
    b: u32,
    pos: Point3,
    /// Versions of both vertices when the candidate was made (stale otherwise).
    va: u32,
    vb: u32,
}

impl PartialEq for Candidate {
    fn eq(&self, o: &Self) -> bool {
        self.cost == o.cost
    }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Candidate {
    fn cmp(&self, o: &Self) -> Ordering {
        // Min-heap on cost.
        o.cost.total_cmp(&self.cost)
    }
}

fn normal(p: [Point3; 3]) -> Option<Vec3> {
    (p[1] - p[0]).cross(p[2] - p[0]).normalized()
}

/// A copy of `mesh` with about `target` triangles (never fewer than 4 when it
/// had more). Vertices closer than `tol` are welded first; normals are dropped.
pub fn reduce_mesh(mesh: &Mesh, target: usize, tol: f64) -> Mesh {
    let m = mesh.welded(tol);
    let target = target.max(4);
    if m.triangles.len() <= target {
        return m;
    }
    let mut pos: Vec<Point3> = m.positions.clone();
    let mut tris: Vec<[u32; 3]> = m.triangles.clone();
    let mut alive = vec![true; tris.len()];
    let mut vfaces: Vec<Vec<usize>> = vec![Vec::new(); pos.len()];
    for (f, t) in tris.iter().enumerate() {
        for &v in t {
            vfaces[v as usize].push(f);
        }
    }
    // Quadrics from the faces, plus border constraints.
    let mut q = vec![Quadric::default(); pos.len()];
    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (f, t) in tris.iter().enumerate() {
        let p = t.map(|i| pos[i as usize]);
        let Some(n) = normal(p) else { continue };
        let area = (p[1] - p[0]).cross(p[2] - p[0]).length() * 0.5;
        let pq = Quadric::plane(n, -n.dot(p[0].to_vec()), area.max(1e-12));
        for &v in t {
            q[v as usize] = q[v as usize].add(&pq);
        }
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            edge_faces.entry((a.min(b), a.max(b))).or_default().push(f);
        }
    }
    let scale = {
        let b = mesh.bounding_box();
        b.map_or(1.0, |b| b.min.distance_to(b.max)).max(1e-9)
    };
    for (&(a, b), fs) in &edge_faces {
        if fs.len() != 1 {
            continue;
        }
        let t = tris[fs[0]];
        let p = t.map(|i| pos[i as usize]);
        let (Some(n), Some(e)) = (normal(p), (pos[b as usize] - pos[a as usize]).normalized())
        else {
            continue;
        };
        // A plane through the border, perpendicular to the face: heavy weight.
        if let Some(bn) = e.cross(n).normalized() {
            let w = scale * scale * 1e3;
            let bq = Quadric::plane(bn, -bn.dot(pos[a as usize].to_vec()), w);
            q[a as usize] = q[a as usize].add(&bq);
            q[b as usize] = q[b as usize].add(&bq);
        }
    }
    let mut version = vec![0u32; pos.len()];
    let candidate = |a: u32, b: u32, pos: &[Point3], q: &[Quadric], version: &[u32]| {
        let qq = q[a as usize].add(&q[b as usize]);
        let (pa, pb) = (pos[a as usize], pos[b as usize]);
        let mid = pa + (pb - pa) * 0.5;
        let (cost, p) = [pa, pb, mid]
            .into_iter()
            .map(|p| (qq.error(p), p))
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .expect("three candidates");
        Candidate {
            cost,
            a,
            b,
            pos: p,
            va: version[a as usize],
            vb: version[b as usize],
        }
    };
    let mut heap: BinaryHeap<Candidate> = edge_faces
        .keys()
        .map(|&(a, b)| candidate(a, b, &pos, &q, &version))
        .collect();
    let mut faces = tris.len();
    while faces > target {
        let Some(c) = heap.pop() else { break };
        let (a, b) = (c.a as usize, c.b as usize);
        if version[a] != c.va || version[b] != c.vb || a == b {
            continue;
        }
        // Faces around a and b after the collapse must not flip or degenerate.
        let mut ok = true;
        for &f in vfaces[a].iter().chain(&vfaces[b]) {
            if !alive[f] {
                continue;
            }
            let t = tris[f];
            if t.contains(&c.a) && t.contains(&c.b) {
                continue; // removed by the collapse
            }
            let before = t.map(|i| pos[i as usize]);
            let after = t.map(|i| {
                if i == c.a || i == c.b {
                    c.pos
                } else {
                    pos[i as usize]
                }
            });
            match (normal(before), normal(after)) {
                (Some(n0), Some(n1)) if n0.dot(n1) > 0.2 => {}
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        // Collapse b into a.
        pos[a] = c.pos;
        q[a] = q[a].add(&q[b]);
        version[a] += 1;
        version[b] += 1;
        let moved: Vec<usize> = std::mem::take(&mut vfaces[b]);
        for f in moved {
            if !alive[f] {
                continue;
            }
            let t = &mut tris[f];
            if t.contains(&c.a) {
                alive[f] = false;
                faces -= 1;
                continue;
            }
            for v in t.iter_mut() {
                if *v == c.b {
                    *v = c.a;
                }
            }
            vfaces[a].push(f);
        }
        vfaces[a].retain(|f| alive[*f]);
        vfaces[a].sort_unstable();
        vfaces[a].dedup();
        // New candidates for the edges around a.
        let mut nbrs: Vec<u32> = vfaces[a]
            .iter()
            .flat_map(|f| tris[*f])
            .filter(|v| *v != c.a)
            .collect();
        nbrs.sort_unstable();
        nbrs.dedup();
        for n in nbrs {
            heap.push(candidate(c.a, n, &pos, &q, &version));
        }
    }
    // Compact.
    let mut map = vec![u32::MAX; pos.len()];
    let mut out = Mesh::default();
    for (f, t) in tris.iter().enumerate() {
        if !alive[f] {
            continue;
        }
        let tri = t.map(|v| {
            if map[v as usize] == u32::MAX {
                map[v as usize] = out.positions.len() as u32;
                out.positions.push(pos[v as usize]);
            }
            map[v as usize]
        });
        out.triangles.push(tri);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere_mesh;

    #[test]
    fn reduce_a_sphere_keeps_its_shape() {
        let s = sphere_mesh(Point3::ORIGIN, 10.0, 48);
        let n = s.triangles.len();
        let r = reduce_mesh(&s, n / 4, 1e-6);
        assert!(
            r.triangles.len() <= n / 4 + 2,
            "{} of {n}",
            r.triangles.len()
        );
        assert!(r.triangles.len() >= n / 4 - 2);
        // Still a closed sphere of about the same size.
        assert!(r.is_closed(1e-6));
        for p in &r.positions {
            let d = p.to_vec().length();
            assert!((d - 10.0).abs() < 1.5, "{d}");
        }
    }

    #[test]
    fn reduce_a_flat_grid_keeps_its_border() {
        let rows: Vec<Vec<Point3>> = (0..=20)
            .map(|j| {
                (0..=20)
                    .map(|i| Point3::new(i as f64 * 0.5, j as f64 * 0.5, 0.0))
                    .collect()
            })
            .collect();
        let g = crate::grid_mesh(&rows, false);
        let r = reduce_mesh(&g, 50, 1e-6);
        assert!(r.triangles.len() <= 52);
        let b = r.bounding_box().unwrap();
        assert!(
            b.min.distance_to(Point3::new(0.0, 0.0, 0.0)) < 1e-6,
            "{b:?}"
        );
        assert!(
            b.max.distance_to(Point3::new(10.0, 10.0, 0.0)) < 1e-6,
            "{b:?}"
        );
        let area: f64 = r
            .triangles
            .iter()
            .map(|t| {
                let p = t.map(|i| r.positions[i as usize]);
                (p[1] - p[0]).cross(p[2] - p[0]).length() / 2.0
            })
            .sum();
        assert!((area - 100.0).abs() < 1e-6, "{area}");
    }
}
