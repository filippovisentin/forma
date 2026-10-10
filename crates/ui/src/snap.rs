//! Object snap kinds and settings, grid snap, Ortho and SmartTrack. The snap
//! search itself lives in [`crate::index`].

use crate::viewport::Viewport;
use eframe::egui::Pos2;
use forma_geom::{Plane, Point3};
use forma_render::glam::DVec3;

/// Rhino's object snaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    End,
    Near,
    Point,
    Mid,
    Cen,
    Int,
    Perp,
    Tan,
    Quad,
    Knot,
    Vertex,
}

impl SnapKind {
    /// In the order of Rhino's Osnap bar.
    pub const ALL: [SnapKind; 11] = [
        SnapKind::End,
        SnapKind::Near,
        SnapKind::Point,
        SnapKind::Mid,
        SnapKind::Cen,
        SnapKind::Int,
        SnapKind::Perp,
        SnapKind::Tan,
        SnapKind::Quad,
        SnapKind::Knot,
        SnapKind::Vertex,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SnapKind::End => "End",
            SnapKind::Near => "Near",
            SnapKind::Point => "Point",
            SnapKind::Mid => "Mid",
            SnapKind::Cen => "Cen",
            SnapKind::Int => "Int",
            SnapKind::Perp => "Perp",
            SnapKind::Tan => "Tan",
            SnapKind::Quad => "Quad",
            SnapKind::Knot => "Knot",
            SnapKind::Vertex => "Vertex",
        }
    }

    fn index(self) -> usize {
        SnapKind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// Lower wins when two snaps are about as close.
    pub fn priority(self) -> u8 {
        match self {
            SnapKind::End | SnapKind::Point => 0,
            SnapKind::Int => 1,
            SnapKind::Cen => 2,
            SnapKind::Quad | SnapKind::Knot | SnapKind::Vertex => 3,
            SnapKind::Mid => 4,
            SnapKind::Perp | SnapKind::Tan => 5,
            SnapKind::Near => 9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapSettings {
    /// One flag per [`SnapKind::ALL`].
    pub on: [bool; 11],
    /// Project snapped points onto the construction plane.
    pub project: bool,
    /// All object snaps off (Rhino's "Disable").
    pub disabled: bool,
    pub grid: bool,
    pub ortho: bool,
    /// New points stay on the plane of the first point of the command.
    pub planar: bool,
    /// SmartTrack: horizontal / vertical alignment with recent snap points.
    pub smart: bool,
    /// Grid snap step in model units.
    pub step: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        let mut on = [false; 11];
        for k in [
            SnapKind::End,
            SnapKind::Point,
            SnapKind::Mid,
            SnapKind::Cen,
            SnapKind::Int,
            SnapKind::Quad,
        ] {
            on[k.index()] = true;
        }
        SnapSettings {
            on,
            project: false,
            disabled: false,
            grid: true,
            ortho: false,
            planar: true,
            smart: true,
            step: 10.0,
        }
    }
}

impl SnapSettings {
    pub fn enabled(&self, k: SnapKind) -> bool {
        !self.disabled && self.on[k.index()]
    }

    /// The user's own flag for `k`, regardless of "Disable".
    pub fn is_on(&self, k: SnapKind) -> bool {
        self.on[k.index()]
    }

    pub fn flag(&mut self, k: SnapKind) -> &mut bool {
        &mut self.on[k.index()]
    }
}

/// SmartTrack: align `p` (in `plane`) with the construction-plane axes through
/// the tracking points, when the cursor is within a few points of such a line.
/// `lock` keeps one coordinate (Ortho from a base point: `true` = moving along
/// the plane's x). Returns the aligned point and the tracking lines to draw.
pub fn smart_track(
    vp: &Viewport,
    pos: Pos2,
    origin: DVec3,
    plane: &Plane,
    p: Point3,
    tracks: &[Point3],
    lock: Option<(Point3, bool)>,
) -> Option<(Point3, Vec<[Point3; 2]>)> {
    const RADIUS: f32 = 8.0;
    let (pu, pv, pw) = plane.coords(p);
    let pr = vp.projector(origin);
    let screen_d = |u: f64, v: f64| {
        pr.to_screen(plane.point_at(u, v, pw))
            .map_or(f32::INFINITY, |s| s.distance(pos))
    };
    // Best horizontal line (fixes v) and vertical line (fixes u).
    let mut best_h: Option<(f32, f64, Point3)> = None;
    let mut best_v: Option<(f32, f64, Point3)> = None;
    for t in tracks {
        let (tu, tv, _) = plane.coords(*t);
        if (tv - pv).abs() > 1e-9 || (tu - pu).abs() > 1e-9 {
            let dh = screen_d(pu, tv);
            if dh < RADIUS && best_h.is_none_or(|b| dh < b.0) {
                best_h = Some((dh, tv, *t));
            }
            let dv = screen_d(tu, pv);
            if dv < RADIUS && best_v.is_none_or(|b| dv < b.0) {
                best_v = Some((dv, tu, *t));
            }
        }
    }
    let (u, v) = match lock {
        Some((b, true)) => {
            let (_, bv, _) = plane.coords(b);
            (best_v?.1, bv)
        }
        Some((b, false)) => {
            let (bu, _, _) = plane.coords(b);
            (bu, best_h?.1)
        }
        None => match (best_h, best_v) {
            (Some(h), Some(vv)) => (vv.1, h.1),
            (Some(h), None) => (pu, h.1),
            (None, Some(vv)) => (vv.1, pv),
            (None, None) => return None,
        },
    };
    let q = plane.point_at(u, v, pw);
    let mut lines = Vec::new();
    for (b, fixed_v) in [(best_h, true), (best_v, false)] {
        if let Some((_, c, t)) = b {
            let on = if fixed_v {
                (v - c).abs() < 1e-9
            } else {
                (u - c).abs() < 1e-9
            };
            if on {
                lines.push([t, q]);
            }
        }
    }
    Some((q, lines))
}

/// Round a point to the grid of `plane` (in-plane coordinates only).
pub fn grid_snap(p: Point3, plane: &Plane, step: f64) -> Point3 {
    if step <= 0.0 {
        return p;
    }
    let (u, v, w) = plane.coords(p);
    plane.point_at((u / step).round() * step, (v / step).round() * step, w)
}

/// Keep only the dominant in-plane direction from `base` (Rhino's Ortho).
pub fn ortho(p: Point3, base: Point3, plane: &Plane) -> Point3 {
    let d = p - base;
    let (u, v) = (d.dot(plane.x), d.dot(plane.y));
    if u.abs() >= v.abs() {
        base + plane.x * u
    } else {
        base + plane.y * v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_ortho() {
        let p = grid_snap(Point3::new(12.4, 7.6, 3.0), &Plane::TOP, 5.0);
        assert_eq!(p, Point3::new(10.0, 10.0, 3.0));
        let q = ortho(Point3::new(10.0, 3.0, 0.0), Point3::ORIGIN, &Plane::TOP);
        assert_eq!(q, Point3::new(10.0, 0.0, 0.0));
        let f = grid_snap(Point3::new(12.0, 0.0, 26.0), &Plane::FRONT, 10.0);
        assert_eq!(f, Point3::new(10.0, 0.0, 30.0));
    }
}
