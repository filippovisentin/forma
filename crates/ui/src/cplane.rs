//! Custom construction planes (Rhino's CPlane) for the active viewport: points
//! picked or typed land on it, and its grid is drawn. A view setting, not a
//! model change, so it lives in the UI:
//!
//! - `CPlane World` — back to the view's standard plane;
//! - `CPlane <origin>` — move the plane, keeping its orientation;
//! - `CPlane <origin> <x point> [y point]` — plane through three points;
//! - `CPlane Face #id <point>` — on the flat face of a solid under the point;
//! - `CPlane` alone starts the three-point tool.

use crate::{tools, FormaApp, LogKind};
use forma_doc::{Geometry, ObjectId};
use forma_engine::parse_point;
use forma_geom::{Plane, Point3, Vec3};
use forma_render::glam::DVec3;

/// Plane through `o` with x towards `x` and y on the side of `y` (when given,
/// otherwise the old plane's normal is kept as much as possible).
fn three_points(o: Point3, x: Point3, y: Option<Point3>, old: &Plane) -> Option<Plane> {
    let xa = (x - o).normalized()?;
    let z = match y {
        Some(y) => xa.cross(y - o).normalized()?,
        None => {
            // Keep the old normal, made perpendicular to the new x axis.
            let n = old.z - xa * old.z.dot(xa);
            n.normalized().or_else(|| xa.cross(old.y).normalized())?
        }
    };
    let ya = z.cross(xa).normalized()?;
    Some(Plane {
        origin: o,
        x: xa,
        y: ya,
        z,
    })
}

fn d(v: Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}

impl FormaApp {
    /// Handle a `CPlane …` line.
    pub(crate) fn cplane_command(&mut self, line: &str) {
        let toks: Vec<&str> = line.split_whitespace().skip(1).collect();
        let vi = self.active;
        let old = self.viewports[vi].cplane();
        let last = self.engine.ctx.last_point;
        let result: Result<Option<Plane>, String> = match toks.as_slice() {
            [] => {
                self.start_tool(tools::ToolKind::Seq(&tools::seq::CPLANE));
                return;
            }
            [w] if w.eq_ignore_ascii_case("world") || w.eq_ignore_ascii_case("w") => Ok(None),
            [f, id, p] if f.eq_ignore_ascii_case("face") => self.face_plane(id, p),
            [o] => parse_point(o, last)
                .map(|o| Some(old.moved_to(o)))
                .map_err(|e| e.to_string()),
            [o, x] | [o, x, _] => {
                let parse = |t: &str, base| parse_point(t, base).map_err(|e| e.to_string());
                (|| {
                    let o = parse(o, last)?;
                    let x = parse(x, Some(o))?;
                    let y = toks.get(2).map(|t| parse(t, Some(o))).transpose()?;
                    three_points(o, x, y, &old)
                        .map(Some)
                        .ok_or_else(|| "the points are in a line".to_string())
                })()
            }
            _ => Err("CPlane World | <origin> [x point] [y point] | Face #id <point>".into()),
        };
        self.log(crate::LogKind::Command, format!("Command: {line}"));
        match result {
            Ok(p) => {
                self.viewports[vi].custom_cplane = p;
                self.cplane_dirty = true;
                self.dirty_all();
                let name = self.viewports[vi].name();
                match p {
                    Some(p) => self.log(
                        LogKind::Normal,
                        format!(
                            "{name}: construction plane at {}, normal {}",
                            tools::fmt_p(p.origin),
                            tools::fmt_v(p.z)
                        ),
                    ),
                    None => self.log(
                        LogKind::Normal,
                        format!("{name}: standard construction plane"),
                    ),
                }
            }
            Err(e) => self.log(LogKind::Error, e),
        }
    }

    /// Plane on the flat face of a mesh under a point: origin at the point,
    /// normal of the face.
    fn face_plane(&self, id: &str, p: &str) -> Result<Option<Plane>, String> {
        let id: u64 = id
            .trim_start_matches('#')
            .parse()
            .map_err(|_| format!("not an object id: {id}"))?;
        let p = parse_point(p, None).map_err(|e| e.to_string())?;
        let doc = self.engine.doc();
        let Some(Geometry::Mesh(m)) = doc.object(ObjectId(id)).map(|o| &o.geometry) else {
            return Err(format!("#{id} is not a solid or mesh"));
        };
        let tol = self.engine.ctx.tolerance.absolute;
        let (t, q, _) = m.closest_triangle(p).ok_or("empty mesh")?;
        let face = m.planar_face(t, tol).ok_or("no flat face there")?;
        Ok(Some(Plane::from_normal(q, face.normal)))
    }

    /// Upload the grids of custom construction planes after a change.
    pub(crate) fn sync_cplanes(&mut self, frame: &eframe::Frame) {
        if !self.cplane_dirty {
            return;
        }
        let (Some(rs), Some(r)) = (frame.wgpu_render_state(), self.renderer.as_mut()) else {
            return;
        };
        for (i, vp) in self.viewports.iter().enumerate() {
            let plane = vp
                .custom_cplane
                .map(|p| (d(p.origin.to_vec()), d(p.x), d(p.y)));
            r.set_custom_grid(&rs.device, i, plane);
        }
        self.cplane_dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_from_three_points() {
        let p = three_points(
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Some(Point3::new(1.0, 0.0, 5.0)),
            &Plane::TOP,
        )
        .unwrap();
        assert!((p.z.y + 1.0).abs() < 1e-12, "{p:?}");
        assert!((p.y.z - 1.0).abs() < 1e-12);
        // Two points keep the old normal.
        let q = three_points(
            Point3::ORIGIN,
            Point3::new(1.0, 1.0, 0.0),
            None,
            &Plane::TOP,
        )
        .unwrap();
        assert!((q.z.z - 1.0).abs() < 1e-12);
        assert!(three_points(Point3::ORIGIN, Point3::ORIGIN, None, &Plane::TOP).is_none());
    }
}
