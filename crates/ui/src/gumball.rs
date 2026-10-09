//! Gumball: on-screen handles at the selection centre to move along an axis, move
//! in a plane or rotate around an axis. Drags end in ordinary `Move` / `Rotate`
//! engine commands.

use crate::viewport::{closest_on_line, Viewport};
use eframe::egui::{self, Color32, Painter, Pos2, Stroke};
use forma_geom::{Plane, Point3, Vec3, Xform};
use forma_render::glam::DVec3;

const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
const COLORS: [Color32; 3] = [
    Color32::from_rgb(225, 55, 55),
    Color32::from_rgb(60, 175, 60),
    Color32::from_rgb(60, 100, 230),
];
const HOT: Color32 = Color32::from_rgb(255, 215, 0);
/// Arrow length on screen, points.
const ARROW_PX: f32 = 85.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Move along axis 0/1/2.
    Axis(usize),
    /// Move in the plane normal to axis 0/1/2.
    Plane(usize),
    /// Rotate around axis 0/1/2.
    Rotate(usize),
    /// Extrude the selection along axis 0/1/2 (the dot on each arrow).
    Extrude(usize),
}

impl Handle {
    pub fn axis(self) -> usize {
        match self {
            Handle::Axis(i) | Handle::Plane(i) | Handle::Rotate(i) | Handle::Extrude(i) => i,
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Handle::Axis(0) => "Move along X",
            Handle::Axis(1) => "Move along Y",
            Handle::Axis(_) => "Move along Z",
            Handle::Plane(0) => "Move in YZ",
            Handle::Plane(1) => "Move in XZ",
            Handle::Plane(_) => "Move in XY",
            Handle::Rotate(0) => "Rotate around X",
            Handle::Rotate(1) => "Rotate around Y",
            Handle::Rotate(_) => "Rotate around Z",
            Handle::Extrude(0) => "Extrude along X",
            Handle::Extrude(1) => "Extrude along Y",
            Handle::Extrude(_) => "Extrude along Z",
        }
    }
}

/// Screen layout of the gumball in one viewport.
pub struct Layout {
    pub center: Point3,
    /// World length of an arrow.
    pub size: f64,
    /// Axes that are not seen end-on.
    pub visible: [bool; 3],
}

pub fn layout(vp: &Viewport, center: Point3, origin: DVec3) -> Option<Layout> {
    let c = vp.to_screen(center, origin)?;
    if !vp.rect.contains(c) {
        return None;
    }
    let mut lens = [0.0f32; 3];
    for (i, a) in AXES.iter().enumerate() {
        lens[i] = vp.to_screen(center + *a, origin)?.distance(c);
    }
    let max = lens.iter().copied().fold(0.0f32, f32::max);
    if max < 1e-9 {
        return None;
    }
    let size = f64::from(ARROW_PX / max);
    let visible = lens.map(|l| l / max > 0.18);
    Some(Layout {
        center,
        size,
        visible,
    })
}

fn arc_points(l: &Layout, axis: usize) -> Vec<Point3> {
    let (u, v) = (AXES[(axis + 1) % 3], AXES[(axis + 2) % 3]);
    let r = l.size * 0.62;
    (0..=16)
        .map(|k| {
            let a = std::f64::consts::FRAC_PI_2 * f64::from(k) / 16.0;
            l.center + u * (r * a.cos()) + v * (r * a.sin())
        })
        .collect()
}

fn plane_square(l: &Layout, axis: usize) -> [Point3; 4] {
    let (u, v) = (AXES[(axis + 1) % 3], AXES[(axis + 2) % 3]);
    let (a, b) = (l.size * 0.25, l.size * 0.4);
    let c = l.center;
    [
        c + u * a + v * a,
        c + u * b + v * a,
        c + u * b + v * b,
        c + u * a + v * b,
    ]
}

/// Where the extrude dot sits on an arrow (fraction of the arrow length).
const DOT_AT: f64 = 0.72;

fn dist_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() < 1e-6 {
        0.0
    } else {
        ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}

/// Handle under the cursor.
#[allow(clippy::needless_range_loop)]
pub fn hit(vp: &Viewport, l: &Layout, origin: DVec3, pos: Pos2) -> Option<Handle> {
    let s = |p: Point3| vp.to_screen(p, origin);
    let c = s(l.center)?;
    let mut best: Option<(f32, Handle)> = None;
    let mut consider = |d: f32, h: Handle| {
        if d < 7.0 && best.is_none_or(|b| d < b.0) {
            best = Some((d, h));
        }
    };
    for i in 0..3 {
        if l.visible[i] {
            if let Some(tip) = s(l.center + AXES[i] * l.size) {
                consider(dist_seg(pos, c, tip) + 0.5, Handle::Axis(i));
            }
            if let Some(dot) = s(l.center + AXES[i] * (l.size * DOT_AT)) {
                // The dot wins over the arrow it sits on.
                let d = dot.distance(pos);
                if d < 6.5 {
                    consider(0.0, Handle::Extrude(i));
                }
            }
        }
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        if l.visible[j] && l.visible[k] {
            let sq: Vec<Pos2> = plane_square(l, i).iter().filter_map(|p| s(*p)).collect();
            if sq.len() == 4 {
                let r = egui::Rect::from_points(&sq);
                if r.contains(pos) {
                    consider(0.0, Handle::Plane(i));
                }
            }
        }
        let arc: Vec<Pos2> = arc_points(l, i).iter().filter_map(|p| s(*p)).collect();
        // An arc seen edge-on would be a line through other handles: skip it.
        if arc.len() > 1 && egui::Rect::from_points(&arc).size().min_elem() > 6.0 {
            for w in arc.windows(2) {
                consider(dist_seg(pos, w[0], w[1]), Handle::Rotate(i));
            }
        }
    }
    best.map(|b| b.1)
}

#[allow(clippy::needless_range_loop)]
pub fn draw(p: &Painter, vp: &Viewport, l: &Layout, origin: DVec3, hot: Option<Handle>) {
    let s = |q: Point3| vp.to_screen(q, origin);
    let Some(c) = s(l.center) else { return };
    let col = |h: Handle| {
        if hot == Some(h) {
            HOT
        } else {
            COLORS[h.axis()]
        }
    };
    for i in 0..3 {
        // Rotation arcs.
        let arc: Vec<Pos2> = arc_points(l, i).iter().filter_map(|q| s(*q)).collect();
        if arc.len() > 1 && egui::Rect::from_points(&arc).size().min_elem() > 6.0 {
            p.add(egui::Shape::line(
                arc,
                Stroke::new(2.0, col(Handle::Rotate(i))),
            ));
        }
        // Plane squares.
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        if l.visible[j] && l.visible[k] {
            let sq: Vec<Pos2> = plane_square(l, i).iter().filter_map(|q| s(*q)).collect();
            if sq.len() == 4 {
                let c0 = col(Handle::Plane(i));
                p.add(egui::Shape::convex_polygon(
                    sq,
                    Color32::from_rgba_unmultiplied(c0.r(), c0.g(), c0.b(), 110),
                    Stroke::new(1.0, c0),
                ));
            }
        }
    }
    for i in 0..3 {
        if !l.visible[i] {
            continue;
        }
        let Some(tip) = s(l.center + AXES[i] * l.size) else {
            continue;
        };
        let color = col(Handle::Axis(i));
        p.line_segment([c, tip], Stroke::new(2.5, color));
        let d = (tip - c).normalized();
        let n = egui::vec2(-d.y, d.x);
        p.add(egui::Shape::convex_polygon(
            vec![tip + d * 9.0, tip + n * 4.5, tip - n * 4.5],
            color,
            Stroke::NONE,
        ));
        // Extrude dot, like Rhino's gumball.
        if let Some(dot) = s(l.center + AXES[i] * (l.size * DOT_AT)) {
            let dc = col(Handle::Extrude(i));
            p.circle_filled(dot, 4.2, dc);
            p.circle_stroke(dot, 4.2, Stroke::new(1.0, Color32::WHITE));
        }
    }
    p.circle_filled(c, 3.5, Color32::WHITE);
    p.circle_stroke(c, 3.5, Stroke::new(1.0, Color32::from_gray(40)));
}

/// Parameter of the cursor for a handle: a world point (moves) or an angle in
/// radians (rotations).
pub enum Grip {
    Point(Point3),
    Angle(f64),
}

pub fn grip(vp: &Viewport, center: Point3, h: Handle, origin: DVec3, pos: Pos2) -> Option<Grip> {
    let (o, d) = vp.ray(pos, origin);
    let i = h.axis();
    match h {
        Handle::Axis(_) | Handle::Extrude(_) => {
            Some(Grip::Point(closest_on_line(center, AXES[i], o, d)))
        }
        Handle::Plane(_) | Handle::Rotate(_) => {
            let plane = Plane {
                origin: center,
                x: AXES[(i + 1) % 3],
                y: AXES[(i + 2) % 3],
                z: AXES[i],
            };
            if d.dot(plane.z).abs() < 1e-3 {
                return None;
            }
            let q = plane.intersect_line(o, d)?;
            match h {
                Handle::Plane(_) => Some(Grip::Point(q)),
                _ => {
                    let (u, v, _) = plane.coords(q);
                    Some(Grip::Angle(v.atan2(u)))
                }
            }
        }
    }
}

/// The motion a handle drag describes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    Translate(Vec3),
    /// Degrees around an axis through the centre.
    Rotate(f64, Vec3),
    /// Extrusion distance (signed) along an axis.
    Extrude(f64, Vec3),
}

impl Motion {
    pub fn xform(self, center: Point3) -> Xform {
        match self {
            Motion::Translate(v) => Xform::translation(v),
            Motion::Rotate(deg, axis) => Xform::rotation(center, axis, deg.to_radians()),
            Motion::Extrude(d, axis) => Xform::translation(axis * d),
        }
    }

    pub fn command(self, center: Point3) -> String {
        let r = |x: f64| {
            let v = (x * 1e6).round() / 1e6;
            if v == 0.0 {
                0.0
            } else {
                v
            }
        };
        match self {
            Motion::Translate(v) => {
                let to = center + v;
                format!(
                    "Move {},{},{} {},{},{}",
                    r(center.x),
                    r(center.y),
                    r(center.z),
                    r(to.x),
                    r(to.y),
                    r(to.z)
                )
            }
            Motion::Rotate(deg, ax) => format!(
                "Rotate {},{},{} {} {},{},{}",
                r(center.x),
                r(center.y),
                r(center.z),
                r(deg),
                ax.x,
                ax.y,
                ax.z
            ),
            // Curves; the app adds ExtrudeSrf for surfaces.
            Motion::Extrude(d, ax) => format!("Extrude {} {},{},{}", r(d), ax.x, ax.y, ax.z),
        }
    }
}

/// Motion for dragging `h` from grip `a` to grip `b`; `step` > 0 rounds distances
/// (grid snap), `angle_step` rounds angles (degrees).
pub fn motion(h: Handle, a: &Grip, b: &Grip, step: f64, angle_step: f64) -> Option<Motion> {
    match (a, b) {
        (Grip::Point(p), Grip::Point(q)) => {
            let mut v = *q - *p;
            if let Handle::Axis(i) | Handle::Extrude(i) = h {
                v = AXES[i] * v.dot(AXES[i]);
            }
            if step > 0.0 {
                v = Vec3::new(
                    (v.x / step).round() * step,
                    (v.y / step).round() * step,
                    (v.z / step).round() * step,
                );
            }
            if v.length() <= 1e-9 {
                return None;
            }
            Some(match h {
                Handle::Extrude(i) => Motion::Extrude(v.dot(AXES[i]), AXES[i]),
                _ => Motion::Translate(v),
            })
        }
        (Grip::Angle(a0), Grip::Angle(a1)) => {
            let mut deg = (a1 - a0).to_degrees();
            if deg > 180.0 {
                deg -= 360.0;
            } else if deg < -180.0 {
                deg += 360.0;
            }
            if angle_step > 0.0 {
                deg = (deg / angle_step).round() * angle_step;
            }
            (deg.abs() > 1e-9).then_some(Motion::Rotate(deg, AXES[h.axis()]))
        }
        _ => None,
    }
}

/// Grip for a snapped world point: its projection on the dragged axis or plane.
pub fn project_grip(center: Point3, h: Handle, q: Point3) -> Grip {
    let a = AXES[h.axis()];
    match h {
        Handle::Plane(_) => Grip::Point(q - a * (q - center).dot(a)),
        _ => Grip::Point(center + a * (q - center).dot(a)),
    }
}

/// Motion for a typed value on a handle (distance or angle).
pub fn typed_motion(h: Handle, value: f64) -> Motion {
    match h {
        Handle::Rotate(i) => Motion::Rotate(value, AXES[i]),
        Handle::Extrude(i) => Motion::Extrude(value, AXES[i]),
        Handle::Axis(i) | Handle::Plane(i) => Motion::Translate(AXES[i] * value),
    }
}

/// Command for a typed value on a handle (distance or angle).
#[cfg(test)]
pub fn typed_command(center: Point3, h: Handle, value: f64) -> String {
    match h {
        Handle::Rotate(i) => Motion::Rotate(value, AXES[i]),
        Handle::Extrude(i) => Motion::Extrude(value, AXES[i]),
        Handle::Axis(i) | Handle::Plane(i) => Motion::Translate(AXES[i] * value),
    }
    .command(center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_commands() {
        let c = Point3::new(10.0, 0.0, 0.0);
        let m = motion(
            Handle::Axis(0),
            &Grip::Point(Point3::new(12.0, 0.0, 0.0)),
            &Grip::Point(Point3::new(37.0, 3.0, 0.0)),
            10.0,
            0.0,
        )
        .unwrap();
        assert_eq!(m.command(c), "Move 10,0,0 40,0,0");
        let r = motion(
            Handle::Rotate(2),
            &Grip::Angle(0.1),
            &Grip::Angle(0.1 + 0.5),
            0.0,
            5.0,
        )
        .unwrap();
        assert_eq!(r.command(c), "Rotate 10,0,0 30 0,0,1");
        assert_eq!(
            typed_command(c, Handle::Axis(2), -5.0),
            "Move 10,0,0 10,0,-5"
        );
    }
}
