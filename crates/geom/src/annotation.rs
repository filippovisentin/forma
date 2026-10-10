//! Annotations: text, text dots and dimensions.
//!
//! They are drawn as plain line segments ([`Dimension::lines`]) plus one text
//! label ([`Label`]) that the UI renders. Measured values are recomputed from the
//! stored points, so a transformed dimension shows its new length.

use crate::{Plane, Point3, Vec3, Xform};
use std::f64::consts::TAU;

/// Text in a plane, or a text dot (a label that always faces the viewer).
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    /// Insertion plane: the origin is the bottom-left corner of the text, x runs
    /// along the text, y points up.
    pub plane: Plane,
    pub text: String,
    /// Text height in model units (for dots: a nominal height, drawn screen-sized).
    pub height: f64,
    /// True for a text dot.
    pub dot: bool,
}

/// Kind of a dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimKind {
    /// Horizontal or vertical in its plane (the plane's x axis is the measuring
    /// direction).
    Linear,
    /// Parallel to the two measured points.
    Aligned,
    Radius,
    Diameter,
    Angle,
}

/// A dimension. Point meaning by kind:
/// - `Linear`, `Aligned`: `[first point, second point, point on dimension line]`,
///   measured along `plane.x`;
/// - `Radius`, `Diameter`: `[centre, point on the circle, leader end]`;
/// - `Angle`: `[vertex, point on first ray, point on second ray, point on arc]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    pub kind: DimKind,
    pub plane: Plane,
    pub points: Vec<Point3>,
    /// Text override; `<>` inside it is replaced by the measured value.
    pub text: Option<String>,
    pub height: f64,
    /// Decimal places of the measured value (trailing zeros are dropped).
    pub decimals: usize,
}

/// Where and what to write for an annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// Anchor point. For dimensions it is the bottom centre of the text; for text
    /// objects the bottom-left corner; for dots the dot itself.
    pub position: Point3,
    pub text: String,
    pub height: f64,
    /// Reading direction and up direction of the text (in the annotation plane).
    pub x: Vec3,
    pub y: Vec3,
    /// True when `position` is the bottom centre (dimensions), false when it is the
    /// bottom-left corner (text) or the dot point.
    pub centered: bool,
}

/// Approximate width of a single-line text (no font metrics in the geometry crate).
pub fn text_width(text: &str, height: f64) -> f64 {
    0.6 * height * text.chars().count() as f64
}

/// `value` with at most `decimals` places, trailing zeros removed ("120", "12.5").
pub fn format_value(value: f64, decimals: usize) -> String {
    let s = format!("{value:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// Transform a plane, keeping it orthonormal (x follows the transformed x axis).
fn transform_plane(p: &Plane, x: &Xform) -> Plane {
    let o = x.point(p.origin);
    let xa = x.vector(p.x).normalized().unwrap_or(p.x);
    let ya = x.vector(p.y);
    let z = xa.cross(ya).normalized().unwrap_or(p.z);
    Plane {
        origin: o,
        x: xa,
        y: z.cross(xa),
        z,
    }
}

impl Text {
    pub fn new(plane: Plane, text: &str, height: f64, dot: bool) -> Text {
        Text {
            plane,
            text: text.to_string(),
            height,
            dot,
        }
    }

    /// Outline used for picking and bounding boxes: the text box, or a zero-length
    /// segment at a dot.
    pub fn outline(&self) -> Vec<Point3> {
        if self.dot {
            return vec![self.plane.origin, self.plane.origin];
        }
        let w = text_width(&self.text, self.height);
        let h = self.height;
        let p = &self.plane;
        vec![
            p.point_at(0.0, 0.0, 0.0),
            p.point_at(w, 0.0, 0.0),
            p.point_at(w, h, 0.0),
            p.point_at(0.0, h, 0.0),
            p.point_at(0.0, 0.0, 0.0),
        ]
    }

    pub fn label(&self) -> Label {
        Label {
            position: self.plane.origin,
            text: self.text.clone(),
            height: self.height,
            x: self.plane.x,
            y: self.plane.y,
            centered: false,
        }
    }

    pub fn transformed(&self, x: &Xform) -> Text {
        let scale = if self.dot {
            1.0
        } else {
            x.vector(self.plane.y).length()
        };
        Text {
            plane: transform_plane(&self.plane, x),
            text: self.text.clone(),
            height: self.height * scale,
            dot: self.dot,
        }
    }
}

impl Dimension {
    /// Linear dimension between `p1` and `p2` with its line through `line_point`,
    /// all in `plane`. Like Rhino, it measures horizontally (plane x) when the line
    /// point is above or below the two points, vertically (plane y) when it is to
    /// their side.
    pub fn linear(
        plane: &Plane,
        p1: Point3,
        p2: Point3,
        line_point: Point3,
        height: f64,
        decimals: usize,
    ) -> Dimension {
        let (u1, v1, _) = plane.coords(p1);
        let (u2, v2, _) = plane.coords(p2);
        let (ul, vl, _) = plane.coords(line_point);
        let outside_u = ul < u1.min(u2) || ul > u1.max(u2);
        let outside_v = vl < v1.min(v2) || vl > v1.max(v2);
        let vertical = if outside_u != outside_v {
            outside_u
        } else {
            // Inside or diagonal: measure the larger extent.
            (v2 - v1).abs() > (u2 - u1).abs()
        };
        let frame = if vertical {
            Plane {
                origin: plane.project(p1),
                x: plane.y,
                y: -plane.x,
                z: plane.z,
            }
        } else {
            plane.moved_to(plane.project(p1))
        };
        Dimension {
            kind: DimKind::Linear,
            plane: frame,
            points: vec![p1, p2, line_point],
            text: None,
            height,
            decimals,
        }
    }

    /// Aligned dimension: measured along `p1 → p2` in the plane with normal `normal`.
    pub fn aligned(
        normal: Vec3,
        p1: Point3,
        p2: Point3,
        line_point: Point3,
        height: f64,
        decimals: usize,
    ) -> Option<Dimension> {
        let z = normal.normalized()?;
        let d = p2 - p1;
        let x = (d - z * d.dot(z)).normalized()?;
        Some(Dimension {
            kind: DimKind::Aligned,
            plane: Plane {
                origin: p1,
                x,
                y: z.cross(x),
                z,
            },
            points: vec![p1, p2, line_point],
            text: None,
            height,
            decimals,
        })
    }

    /// The measured value: a length, or degrees for angles.
    pub fn value(&self) -> f64 {
        let p = &self.points;
        match self.kind {
            DimKind::Linear | DimKind::Aligned => (p[1] - p[0]).dot(self.plane.x).abs(),
            DimKind::Radius => p[0].distance_to(p[1]),
            DimKind::Diameter => 2.0 * p[0].distance_to(p[1]),
            DimKind::Angle => self.angle_arc().map_or(0.0, |(_, s, _)| s.to_degrees()),
        }
    }

    /// Displayed text: the override (with `<>` replaced) or the formatted value
    /// with its prefix (R, Ø, °).
    pub fn text(&self) -> String {
        let v = format_value(self.value(), self.decimals);
        let measured = match self.kind {
            DimKind::Radius => format!("R{v}"),
            DimKind::Diameter => format!("Ø{v}"),
            DimKind::Angle => format!("{v}°"),
            _ => v,
        };
        match &self.text {
            Some(t) => t.replace("<>", &measured),
            None => measured,
        }
    }

    /// Start angle, sweep and radius of the angle arc in plane coordinates.
    fn angle_arc(&self) -> Option<(f64, f64, f64)> {
        let p = &self.points;
        let (c, a, b, m) = (p[0], p[1], p[2], p[3]);
        let ang = |q: Point3| {
            let (u, v, _) = self.plane.coords(q);
            let (cu, cv, _) = self.plane.coords(c);
            (v - cv).atan2(u - cu)
        };
        let r = self.plane.project(m).distance_to(self.plane.project(c));
        if r < 1e-12 || a.distance_to(c) < 1e-12 || b.distance_to(c) < 1e-12 {
            return None;
        }
        let (a1, a2, am) = (ang(a), ang(b), ang(m));
        let norm = |x: f64| x.rem_euclid(TAU);
        let s = norm(a2 - a1);
        if norm(am - a1) <= s {
            Some((a1, s, r))
        } else {
            Some((a2, TAU - s, r))
        }
    }

    /// Dimension line, extension lines and arrowheads as segments.
    pub fn lines(&self) -> Vec<[Point3; 2]> {
        let mut out = Vec::new();
        let h = self.height;
        let arrow = |out: &mut Vec<[Point3; 2]>, tip: Point3, back: Vec3, side: Vec3| {
            let a = h * 0.8;
            out.push([tip, tip + back * a + side * (a * 0.3)]);
            out.push([tip, tip + back * a - side * (a * 0.3)]);
        };
        let p = &self.points;
        let pl = &self.plane;
        match self.kind {
            DimKind::Linear | DimKind::Aligned => {
                let (d1, d2, up) = self.dim_line();
                // Extension lines: from a small gap off the point to just past the
                // dimension line.
                for (q, d) in [(p[0], d1), (p[1], d2)] {
                    let q = pl.project(q);
                    if let Some(dir) = (d - q).normalized() {
                        let len = d.distance_to(q);
                        let gap = (h * 0.3).min(len * 0.5);
                        out.push([q + dir * gap, d + dir * (h * 0.4)]);
                    }
                }
                out.push([d1, d2]);
                if let Some(dir) = (d2 - d1).normalized() {
                    arrow(&mut out, d1, dir, up);
                    arrow(&mut out, d2, -dir, up);
                }
            }
            DimKind::Radius | DimKind::Diameter => {
                let (c, q, l) = (p[0], p[1], p[2]);
                let r = c.distance_to(q);
                if let Some(dir) = (q - c).normalized() {
                    let side = pl.z.cross(dir);
                    let start = if self.kind == DimKind::Diameter {
                        c - dir * r
                    } else {
                        c
                    };
                    out.push([start, q]);
                    arrow(&mut out, q, -dir, side);
                    if self.kind == DimKind::Diameter {
                        arrow(&mut out, start, dir, side);
                    }
                    if l.distance_to(q) > 1e-9 {
                        out.push([q, l]);
                    }
                    // Centre mark.
                    let s = h * 0.4;
                    out.push([c - pl.x * s, c + pl.x * s]);
                    out.push([c - pl.y * s, c + pl.y * s]);
                }
            }
            DimKind::Angle => {
                let Some((a0, sweep, r)) = self.angle_arc() else {
                    return out;
                };
                let c = pl.project(p[0]);
                let (cu, cv, _) = pl.coords(c);
                let at = |a: f64, rr: f64| pl.point_at(cu + rr * a.cos(), cv + rr * a.sin(), 0.0);
                let n = ((sweep / TAU * 96.0).ceil() as usize).max(4);
                let arc: Vec<Point3> = (0..=n)
                    .map(|i| at(a0 + sweep * i as f64 / n as f64, r))
                    .collect();
                for w in arc.windows(2) {
                    out.push([w[0], w[1]]);
                }
                // Extension lines along the rays when the arc lies beyond the points.
                for (q, a) in [(p[1], a0), (p[2], a0 + sweep)] {
                    let dq = pl.project(q).distance_to(c);
                    if r > dq + 1e-9 {
                        out.push([at(a, dq + h * 0.3), at(a, r + h * 0.4)]);
                    }
                }
                let tangent = |a: f64| pl.x * -a.sin() + pl.y * a.cos();
                let radial = |a: f64| pl.x * a.cos() + pl.y * a.sin();
                arrow(&mut out, arc[0], tangent(a0), radial(a0));
                let a1 = a0 + sweep;
                arrow(&mut out, arc[n], -tangent(a1), radial(a1));
            }
        }
        out
    }

    /// Ends of the dimension line and the text "up" direction (linear, aligned).
    fn dim_line(&self) -> (Point3, Point3, Vec3) {
        let pl = &self.plane;
        let (u1, _, _) = pl.coords(self.points[0]);
        let (u2, _, _) = pl.coords(self.points[1]);
        let (_, vl, _) = pl.coords(self.points[2]);
        (pl.point_at(u1, vl, 0.0), pl.point_at(u2, vl, 0.0), pl.y)
    }

    /// Continuous outline used for picking and bounding boxes.
    pub fn outline(&self) -> Vec<Point3> {
        let p = &self.points;
        let pl = &self.plane;
        match self.kind {
            DimKind::Linear | DimKind::Aligned => {
                let (d1, d2, _) = self.dim_line();
                vec![pl.project(p[0]), d1, d2, pl.project(p[1])]
            }
            DimKind::Radius => vec![p[0], p[1], p[2]],
            DimKind::Diameter => vec![p[0] - (p[1] - p[0]), p[1], p[2]],
            DimKind::Angle => {
                let mut v = vec![pl.project(p[1]), pl.project(p[0]), pl.project(p[2])];
                v.extend(self.lines().iter().map(|l| l[0]));
                v
            }
        }
    }

    /// Text label: centred above the dimension line (or at the leader end, or
    /// outside the arc middle for angles).
    pub fn label(&self) -> Label {
        let pl = &self.plane;
        let h = self.height;
        let (position, x, y) = match self.kind {
            DimKind::Linear | DimKind::Aligned => {
                let (d1, d2, up) = self.dim_line();
                let x = (d2 - d1).normalized().unwrap_or(pl.x);
                // Keep the text readable: run left to right in the plane.
                let (x, y) =
                    if x.dot(pl.x) < -1e-9 || (x.dot(pl.x).abs() <= 1e-9 && x.dot(pl.y) < 0.0) {
                        (-x, -up)
                    } else {
                        (x, up)
                    };
                let y = if y.dot(up) < 0.0 { up } else { y };
                (d1.mid(d2) + up * (h * 0.4), x, y)
            }
            DimKind::Radius | DimKind::Diameter => (self.points[2], pl.x, pl.y),
            DimKind::Angle => match self.angle_arc() {
                Some((a0, s, r)) => {
                    let c = pl.project(self.points[0]);
                    let am = a0 + s / 2.0;
                    let dir = pl.x * am.cos() + pl.y * am.sin();
                    (c + dir * (r + h * 0.4), pl.x, pl.y)
                }
                None => (self.points[0], pl.x, pl.y),
            },
        };
        Label {
            position,
            text: self.text(),
            height: h,
            x,
            y,
            centered: true,
        }
    }

    pub fn transformed(&self, x: &Xform) -> Dimension {
        Dimension {
            kind: self.kind,
            plane: transform_plane(&self.plane, x),
            points: self.points.iter().map(|p| x.point(*p)).collect(),
            text: self.text.clone(),
            height: self.height * x.vector(self.plane.y).length(),
            decimals: self.decimals,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-9;

    fn p(x: f64, y: f64) -> Point3 {
        Point3::new(x, y, 0.0)
    }

    #[test]
    fn values_are_formatted_without_trailing_zeros() {
        assert_eq!(format_value(120.0, 2), "120");
        assert_eq!(format_value(12.5, 2), "12.5");
        assert_eq!(format_value(12.345, 1), "12.3");
        assert_eq!(format_value(-0.0001, 1), "0");
    }

    #[test]
    fn linear_dimension_picks_direction_from_line_point() {
        let top = Plane::TOP;
        let h = Dimension::linear(&top, p(0.0, 0.0), p(120.0, 30.0), p(50.0, -20.0), 5.0, 1);
        assert!((h.value() - 120.0).abs() < TOL);
        assert_eq!(h.text(), "120");
        let v = Dimension::linear(&top, p(0.0, 0.0), p(120.0, 30.0), p(150.0, 10.0), 5.0, 1);
        assert!((v.value() - 30.0).abs() < TOL);
        // Dimension line ends lie on the line through the line point.
        let (d1, d2, _) = h.dim_line();
        assert!((d1.y + 20.0).abs() < TOL && (d2.y + 20.0).abs() < TOL);
        assert!((d2.x - 120.0).abs() < TOL);
        // 2 extension lines + dimension line + 2 arrows of 2 lines.
        assert_eq!(h.lines().len(), 7);
        let l = h.label();
        assert!(l.centered);
        assert!(
            l.position.distance_to(p(60.0, -18.0)) < TOL,
            "{}",
            l.position
        );
    }

    #[test]
    fn aligned_radius_diameter_angle() {
        let a =
            Dimension::aligned(Vec3::Z, p(0.0, 0.0), p(30.0, 40.0), p(0.0, 10.0), 2.0, 2).unwrap();
        assert!((a.value() - 50.0).abs() < TOL);
        let mut r = Dimension {
            kind: DimKind::Radius,
            plane: Plane::TOP,
            points: vec![p(0.0, 0.0), p(25.0, 0.0), p(40.0, 10.0)],
            text: None,
            height: 2.0,
            decimals: 1,
        };
        assert_eq!(r.text(), "R25");
        r.kind = DimKind::Diameter;
        assert_eq!(r.text(), "Ø50");
        r.text = Some("<> foro".into());
        assert_eq!(r.text(), "Ø50 foro");
        let ang = Dimension {
            kind: DimKind::Angle,
            plane: Plane::TOP,
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), p(7.0, 7.0)],
            text: None,
            height: 1.0,
            decimals: 1,
        };
        assert!((ang.value() - 90.0).abs() < 1e-9);
        assert_eq!(ang.text(), "90°");
        // Arc point on the other side: the reflex angle.
        let reflex = Dimension {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), p(-7.0, -7.0)],
            ..ang.clone()
        };
        assert!((reflex.value() - 270.0).abs() < 1e-9);
        assert!(!ang.lines().is_empty());
    }

    #[test]
    fn transformed_dimension_measures_the_new_length() {
        let d = Dimension::linear(
            &Plane::TOP,
            p(0.0, 0.0),
            p(100.0, 0.0),
            p(50.0, 10.0),
            5.0,
            0,
        );
        let s = d.transformed(&Xform::scale(Point3::ORIGIN, 2.0));
        assert!((s.value() - 200.0).abs() < 1e-9);
        assert!((s.height - 10.0).abs() < 1e-9);
        let r = d.transformed(&Xform::rotation(Point3::ORIGIN, Vec3::Z, 1.0));
        assert!((r.value() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn text_outline_and_label() {
        let t = Text::new(Plane::TOP.moved_to(p(10.0, 5.0)), "Cucina", 2.0, false);
        let o = t.outline();
        assert_eq!(o.len(), 5);
        assert!((o[1].x - (10.0 + 0.6 * 2.0 * 6.0)).abs() < TOL);
        assert_eq!(t.label().text, "Cucina");
        assert!(!t.label().centered);
        let m = t.transformed(&Xform::translation(Vec3::new(1.0, 0.0, 0.0)));
        assert!(m.plane.origin.distance_to(p(11.0, 5.0)) < TOL);
        assert!((m.height - 2.0).abs() < TOL);
    }
}
