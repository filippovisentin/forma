//! Editable points ("grips") of objects, as Rhino shows them with PointsOn:
//! curve control points (line ends, polyline vertices, NURBS control points),
//! mesh vertices, point objects, text and dimension anchor points.

use crate::Geometry;
use forma_geom::{LineCurve, Mesh, NurbsCurve, Point3, Seg, Vec3};
use std::collections::HashMap;

/// Meshes with more distinct vertices than this show no grips (too many to edit
/// by hand, and too slow to draw).
pub const MAX_MESH_GRIPS: usize = 20_000;

/// Grid key of a mesh position: coincident vertices (meshes built face by face)
/// are one grip.
fn key(p: Point3) -> (i64, i64, i64) {
    const Q: f64 = 1e-6;
    (
        (p.x / Q).round() as i64,
        (p.y / Q).round() as i64,
        (p.z / Q).round() as i64,
    )
}

/// Distinct positions of a mesh in first-use order, and for each position the
/// grip it belongs to.
fn mesh_grips(m: &Mesh) -> (Vec<Point3>, Vec<usize>) {
    let mut seen: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut grips = Vec::new();
    let map = m
        .positions
        .iter()
        .map(|p| {
            *seen.entry(key(*p)).or_insert_with(|| {
                grips.push(*p);
                grips.len() - 1
            })
        })
        .collect();
    (grips, map)
}

/// Polyline vertices without the closing duplicate.
fn open_points(p: &[Point3]) -> &[Point3] {
    if p.len() > 3 && p[0].distance_to(p[p.len() - 1]) < 1e-9 {
        &p[..p.len() - 1]
    } else {
        p
    }
}

/// Moved copy of `pts`; a closed list (last == first) keeps its seam closed.
fn moved_closed(pts: &[Point3], moves: &[(usize, Vec3)]) -> Vec<Point3> {
    let closed = pts.len() > 3 && pts[0].distance_to(pts[pts.len() - 1]) < 1e-9;
    let n = if closed { pts.len() - 1 } else { pts.len() };
    let mut out = pts.to_vec();
    for (i, v) in moves {
        if *i < n {
            out[*i] = out[*i] + *v;
        }
    }
    if closed {
        out[n] = out[0];
    }
    out
}

/// All lines and no arcs: the corners of the chain.
fn line_vertices(segs: &[Seg]) -> Option<Vec<Point3>> {
    let mut pts = Vec::with_capacity(segs.len() + 1);
    for s in segs {
        match s {
            Seg::Line(a, b) => {
                if pts.is_empty() {
                    pts.push(*a);
                }
                pts.push(*b);
            }
            Seg::Arc(_) => return None,
        }
    }
    Some(pts)
}

impl Geometry {
    /// The object's grips; empty when it has none (or too many, for meshes).
    pub fn grips(&self) -> Vec<Point3> {
        match self {
            Geometry::Line(l) => vec![l.from, l.to],
            Geometry::Polyline(p) => open_points(p).to_vec(),
            Geometry::Point(p) => vec![*p],
            Geometry::Nurbs(n) => open_points(&n.points).to_vec(),
            Geometry::Arc(a) => open_points(&NurbsCurve::from_arc(a).points).to_vec(),
            Geometry::PolyCurve(s) => match line_vertices(s) {
                Some(p) => open_points(&p).to_vec(),
                None => NurbsCurve::from_segs(s)
                    .map(|n| open_points(&n.points).to_vec())
                    .unwrap_or_default(),
            },
            Geometry::Mesh(m) => {
                let (g, _) = mesh_grips(m);
                if g.len() > MAX_MESH_GRIPS {
                    Vec::new()
                } else {
                    g
                }
            }
            Geometry::Text(t) => vec![t.plane.origin],
            Geometry::Dimension(d) => d.points.clone(),
        }
    }

    /// Copy with grips moved (indices as in [`Geometry::grips`]). Arcs and arc
    /// polycurves become NURBS curves, as in Rhino once their points are edited.
    pub fn with_grips_moved(&self, moves: &[(usize, Vec3)]) -> Geometry {
        match self {
            Geometry::Line(l) => {
                let mut p = [l.from, l.to];
                for (i, v) in moves {
                    if let Some(q) = p.get_mut(*i) {
                        *q = *q + *v;
                    }
                }
                Geometry::Line(LineCurve::new(p[0], p[1]))
            }
            Geometry::Polyline(p) => Geometry::Polyline(moved_closed(p, moves)),
            Geometry::Point(p) => {
                Geometry::Point(moves.iter().filter(|m| m.0 == 0).fold(*p, |q, m| q + m.1))
            }
            Geometry::Nurbs(n) => Geometry::Nurbs(NurbsCurve {
                points: moved_closed(&n.points, moves),
                ..n.clone()
            }),
            Geometry::Arc(a) => {
                let n = NurbsCurve::from_arc(a);
                Geometry::Nurbs(NurbsCurve {
                    points: moved_closed(&n.points, moves),
                    ..n
                })
            }
            Geometry::PolyCurve(s) => match line_vertices(s) {
                Some(p) => {
                    let p = moved_closed(&p, moves);
                    Geometry::PolyCurve(p.windows(2).map(|w| Seg::Line(w[0], w[1])).collect())
                }
                None => match NurbsCurve::from_segs(s) {
                    Some(n) => Geometry::Nurbs(NurbsCurve {
                        points: moved_closed(&n.points, moves),
                        ..n
                    }),
                    None => self.clone(),
                },
            },
            Geometry::Mesh(m) => {
                let (grips, map) = mesh_grips(m);
                let mut delta = vec![Vec3::new(0.0, 0.0, 0.0); grips.len()];
                for (i, v) in moves {
                    if let Some(d) = delta.get_mut(*i) {
                        *d = *d + *v;
                    }
                }
                Geometry::Mesh(Mesh {
                    positions: m
                        .positions
                        .iter()
                        .zip(&map)
                        .map(|(p, g)| *p + delta[*g])
                        .collect(),
                    normals: Vec::new(),
                    triangles: m.triangles.clone(),
                })
            }
            Geometry::Text(_) => {
                let v = moves
                    .iter()
                    .filter(|m| m.0 == 0)
                    .fold(Vec3::new(0.0, 0.0, 0.0), |a, m| a + m.1);
                self.transformed(&forma_geom::Xform::translation(v))
            }
            Geometry::Dimension(d) => {
                let mut d = d.clone();
                for (i, v) in moves {
                    if let Some(q) = d.points.get_mut(*i) {
                        *q = *q + *v;
                    }
                }
                Geometry::Dimension(d)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geom::{CircleArc, Plane};

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(x, y, z)
    }

    #[test]
    fn closed_polyline_moves_its_seam() {
        let sq = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(0.0, 0.0, 0.0),
        ];
        let g = Geometry::Polyline(sq);
        assert_eq!(g.grips().len(), 4);
        let m = g.with_grips_moved(&[(0, v(-1.0, -1.0, 0.0))]);
        let Geometry::Polyline(p) = m else { panic!() };
        assert!(p[0].distance_to(Point3::new(-1.0, -1.0, 0.0)) < 1e-12);
        assert!(p[4].distance_to(p[0]) < 1e-12);
    }

    #[test]
    fn circle_becomes_nurbs_when_edited() {
        let c = Geometry::Arc(CircleArc::circle(Plane::TOP, 5.0));
        let g = c.grips();
        assert_eq!(g.len(), 8, "{g:?}");
        let e = c.with_grips_moved(&[(1, v(0.0, 0.0, 3.0))]);
        assert!(matches!(e, Geometry::Nurbs(_)));
        assert!(e.is_closed_curve());
    }

    #[test]
    fn box_mesh_has_eight_grips() {
        let m = forma_geom::box_mesh(&Plane::TOP, 1.0, 1.0, 1.0);
        let g = Geometry::Mesh(m);
        assert_eq!(g.grips().len(), 8);
        let top: Vec<usize> = g
            .grips()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.z > 0.5)
            .map(|(i, _)| i)
            .collect();
        let moves: Vec<(usize, Vec3)> = top.iter().map(|i| (*i, v(0.0, 0.0, 1.0))).collect();
        let b = g.with_grips_moved(&moves).bounding_box();
        assert!((b.max.z - 2.0).abs() < 1e-12);
    }
}
