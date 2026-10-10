//! Live feedback of a running tool: measurements next to the cursor and
//! rubber-band preview segments.

use super::{Tool, ToolKind};
use forma_geom::{CircleArc, Plane, Point3, Xform};

/// A live measurement next to the cursor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Measure {
    /// A length with an optional short label (model units).
    Len(&'static str, f64),
    /// Degrees.
    Angle(f64),
    /// Scale factor.
    Factor(f64),
}

impl Tool {
    /// Live measurements for the cursor position (shown next to the cursor).
    pub fn measure(&self, cur: Point3) -> Vec<Measure> {
        use ToolKind::*;
        if self.selecting {
            return Vec::new();
        }
        let n = self.pts.len();
        let pl = |o: Point3| self.plane.moved_to(o);
        let in_plane_angle = |a: Point3, b: Point3| {
            let d = b - a;
            let (u, v) = (d.dot(self.plane.x), d.dot(self.plane.y));
            if u.hypot(v) < 1e-12 {
                0.0
            } else {
                v.atan2(u).to_degrees()
            }
        };
        let height = |from: Point3| (cur - from).dot(self.plane.z);
        match (self.kind, n) {
            (Rectangle | Box, 1) => {
                let (u, v, _) = pl(self.pts[0]).coords(cur);
                vec![Measure::Len("W", u.abs()), Measure::Len("H", v.abs())]
            }
            (Box, 2) => vec![Measure::Len("Height", height(self.pts[1]))],
            (Cylinder, 2) => vec![Measure::Len("Height", height(self.pts[0]))],
            (Extrude | ExtrudeSrf, _) => vec![Measure::Len("Height", height(self.anchor))],
            (Circle | Cylinder, 1) => {
                let r = pl(self.pts[0]).project(cur).distance_to(self.pts[0]);
                vec![Measure::Len("R", r), Measure::Len("Ø", 2.0 * r)]
            }
            (Sphere, 1) => {
                let r = cur.distance_to(self.pts[0]);
                vec![Measure::Len("R", r), Measure::Len("Ø", 2.0 * r)]
            }
            (Arc, 1) => vec![Measure::Len("R", cur.distance_to(self.pts[0]))],
            (Arc, 2) => {
                let mut a =
                    in_plane_angle(self.pts[0], cur) - in_plane_angle(self.pts[0], self.pts[1]);
                if a <= 0.0 {
                    a += 360.0;
                }
                vec![Measure::Angle(a)]
            }
            (Ellipse, 1) => vec![Measure::Len("A", cur.distance_to(self.pts[0]))],
            (Ellipse, 2) => {
                let a = self.pts[1] - self.pts[0];
                let v = cur - self.pts[0];
                let b = a
                    .normalized()
                    .map_or(v.length(), |u| (v - u * v.dot(u)).length());
                vec![Measure::Len("B", b)]
            }
            (Polygon, 1) => {
                let r = cur.distance_to(self.pts[0]);
                let k = self.count.unwrap_or(5) as f64;
                vec![
                    Measure::Len("R", r),
                    Measure::Len("Side", 2.0 * r * (std::f64::consts::PI / k).sin()),
                ]
            }
            (Rotate, 1) => match self.reference {
                Some(_) => vec![Measure::Angle(self.angle(cur))],
                None => vec![Measure::Angle(in_plane_angle(self.pts[0], cur))],
            },
            (Scale | Scale2D, 1) => match self.reference {
                Some(r) if r.distance_to(self.pts[0]) > 1e-12 => vec![Measure::Factor(
                    cur.distance_to(self.pts[0]) / r.distance_to(self.pts[0]),
                )],
                _ => vec![Measure::Len("", cur.distance_to(self.pts[0]))],
            },
            (Scale1D, 1) => match self.reference {
                Some(r) if r.distance_to(self.pts[0]) > 1e-12 => {
                    let d = r.distance_to(self.pts[0]);
                    vec![Measure::Factor(
                        (cur - self.pts[0]).dot((r - self.pts[0]) * (1.0 / d)) / d,
                    )]
                }
                _ => vec![Measure::Len("", cur.distance_to(self.pts[0]))],
            },
            (Point | Trim | Extend | Fillet | Chamfer | Sweep1 | MatchProperties, _) => Vec::new(),
            (_, k) if k >= 1 => {
                let b = self.base().unwrap_or(self.pts[k - 1]);
                vec![
                    Measure::Len("", cur.distance_to(b)),
                    Measure::Angle(in_plane_angle(b, cur)),
                ]
            }
            _ => Vec::new(),
        }
    }

    /// Rubber-band preview segments for the cursor position.
    pub fn preview(&self, cur: Point3) -> Vec<[Point3; 2]> {
        use ToolKind::*;
        let mut out: Vec<[Point3; 2]> = Vec::new();
        let n = self.pts.len();
        let poly = |out: &mut Vec<[Point3; 2]>, pts: &[Point3]| {
            out.extend(pts.windows(2).map(|w| [w[0], w[1]]));
        };
        let rect = |a: Point3, b: Point3, plane: &Plane| -> Vec<Point3> {
            let pl = plane.moved_to(a);
            let (u, v, _) = pl.coords(b);
            vec![
                pl.point_at(0.0, 0.0, 0.0),
                pl.point_at(u, 0.0, 0.0),
                pl.point_at(u, v, 0.0),
                pl.point_at(0.0, v, 0.0),
                pl.point_at(0.0, 0.0, 0.0),
            ]
        };
        let circle =
            |c: Point3, r: f64, plane: Plane| CircleArc::circle(plane.moved_to(c), r).points(72);
        let xf = |out: &mut Vec<[Point3; 2]>, x: &Xform| {
            out.extend(
                self.skeleton
                    .iter()
                    .map(|[a, b]| [x.point(*a), x.point(*b)]),
            );
        };
        if self.selecting {
            return out;
        }
        match (self.kind, n) {
            (Curve | InterpCrv, _) if n >= 1 => {
                let mut pts = self.pts.clone();
                pts.push(cur);
                let crv = if self.kind == Curve {
                    poly(&mut out, &pts);
                    forma_geom::NurbsCurve::clamped_uniform(&pts, 3)
                } else {
                    forma_geom::NurbsCurve::interpolate(&pts, 3)
                };
                if let Some(c) = crv {
                    poly(&mut out, &c.points());
                }
            }
            (Ellipse, 1) => out.push([self.pts[0], cur]),
            (Ellipse, 2) => {
                let c = self.pts[0];
                let a = self.pts[1] - c;
                let v = cur - c;
                if let Some(u) = a.normalized() {
                    let b = (v - u * v.dot(u)).length();
                    let pl = Plane::from_normal(c, self.plane.z);
                    let pl = Plane {
                        origin: c,
                        x: u,
                        y: pl.z.cross(u),
                        z: pl.z,
                    };
                    let e = forma_geom::NurbsCurve::ellipse(&pl, a.length(), b.max(1e-9));
                    poly(&mut out, &e.points());
                }
            }
            (Polygon, 1) => {
                let c = self.pts[0];
                let sides = self.count.unwrap_or(5);
                let pl = Plane::from_normal(c, self.plane.z);
                let (u0, v0, _) = pl.coords(cur);
                let (r, a0) = (u0.hypot(v0), v0.atan2(u0));
                let pts: Vec<Point3> = (0..=sides)
                    .map(|k| {
                        let a = a0 + std::f64::consts::TAU * k as f64 / sides as f64;
                        pl.point_at(r * a.cos(), r * a.sin(), 0.0)
                    })
                    .collect();
                poly(&mut out, &pts);
                out.push([c, cur]);
            }
            (Orient, 1) | (Orient, 3) | (Revolve, 1) | (Distance, 1) => {
                out.push([self.pts[n - 1], cur]);
                if self.kind == Orient && n == 3 {
                    out.push([self.pts[0], self.pts[1]]);
                    if let Some(x) =
                        Xform::orient(self.pts[0], self.pts[1], self.pts[2], cur, false)
                    {
                        xf(&mut out, &x);
                    }
                }
            }
            (Orient, 2) => {
                out.push([self.pts[0], self.pts[1]]);
                xf(&mut out, &Xform::translation(cur - self.pts[0]));
            }
            (ExtrudeSrf, _) => {
                let h = (cur - self.anchor).dot(self.plane.z);
                xf(&mut out, &Xform::translation(self.plane.z * h));
                out.push([self.anchor, self.anchor + self.plane.z * h]);
            }
            (Scale1D | Scale2D, 1) => {
                out.push([self.pts[0], cur]);
                if let Some(r) = self.reference {
                    let o = self.pts[0];
                    let d = r.distance_to(o);
                    if d > 1e-12 {
                        let f = if self.kind == Scale1D {
                            (cur - o).dot((r - o) * (1.0 / d)) / d
                        } else {
                            cur.distance_to(o) / d
                        };
                        let x = if self.kind == Scale1D {
                            let dir = (r - o) * (1.0 / d);
                            let pl = Plane::from_normal(o, dir);
                            let pl = Plane {
                                origin: o,
                                x: dir,
                                y: pl.x,
                                z: dir.cross(pl.x),
                            };
                            Xform::scale_axes(&pl, f, 1.0, 1.0)
                        } else {
                            Xform::scale_axes(&self.plane.moved_to(o), f, f, 1.0)
                        };
                        xf(&mut out, &x);
                    }
                }
            }
            (Line, 1) | (Polyline, _) if n >= 1 => {
                poly(&mut out, &self.pts);
                out.push([self.pts[n - 1], cur]);
            }
            (Rectangle, 1) => poly(&mut out, &rect(self.pts[0], cur, &self.plane)),
            (Circle, 1) | (Cylinder, 1) => {
                let c = self.pts[0];
                let r = self.plane_at(c).project(cur).distance_to(c);
                poly(&mut out, &circle(c, r, self.plane));
                out.push([c, cur]);
            }
            (Sphere, 1) => {
                let c = self.pts[0];
                let r = cur.distance_to(c);
                poly(&mut out, &circle(c, r, self.plane));
                let side = Plane::from_normal(c, self.plane.x);
                poly(&mut out, &circle(c, r, side));
                out.push([c, cur]);
            }
            (Arc, 1) => out.push([self.pts[0], cur]),
            (Arc, 2) => {
                out.push([self.pts[0], self.pts[1]]);
                if let Some(a) =
                    CircleArc::from_center_start_end(self.pts[0], self.pts[1], cur, self.plane.z)
                {
                    poly(&mut out, &a.points(72));
                }
            }
            (Box, 1) => poly(&mut out, &rect(self.pts[0], cur, &self.plane)),
            (Box, 2) => {
                let base = rect(self.pts[0], self.pts[1], &self.plane);
                let h = (cur - self.pts[1]).dot(self.plane.z);
                let top: Vec<Point3> = base.iter().map(|p| *p + self.plane.z * h).collect();
                poly(&mut out, &base);
                poly(&mut out, &top);
                for i in 0..4 {
                    out.push([base[i], top[i]]);
                }
            }
            (Cylinder, 2) => {
                let c = self.pts[0];
                let r = self.pts[1].distance_to(c);
                let h = (cur - c).dot(self.plane.z);
                let b = circle(c, r, self.plane);
                let t = circle(c + self.plane.z * h, r, self.plane);
                poly(&mut out, &b);
                poly(&mut out, &t);
                for k in 0..4 {
                    let i = k * (b.len() - 1) / 4;
                    out.push([b[i], t[i]]);
                }
            }
            (Extrude, _) => {
                let h = (cur - self.anchor).dot(self.plane.z);
                let x = Xform::translation(self.plane.z * h);
                xf(&mut out, &x);
                out.push([self.anchor, self.anchor + self.plane.z * h]);
            }
            (Move, 1) | (Copy, 1) => {
                out.push([self.pts[0], cur]);
                xf(&mut out, &Xform::translation(cur - self.pts[0]));
            }
            (Mirror, 1) => {
                out.push([self.pts[0], cur]);
                if let Some(nrm) = (cur - self.pts[0]).cross(self.plane.z).normalized() {
                    xf(&mut out, &Xform::mirror(self.pts[0], nrm));
                }
            }
            (Rotate, 1) => {
                out.push([self.pts[0], cur]);
                if let Some(r) = self.reference {
                    out.push([self.pts[0], r]);
                    xf(
                        &mut out,
                        &Xform::rotation(self.pts[0], self.plane.z, self.angle(cur).to_radians()),
                    );
                }
            }
            (Offset, _) => {
                for (c, own) in &self.curves {
                    let n = match own {
                        Some(v) if v.dot(self.plane.z) < 0.0 => -*v,
                        Some(v) => *v,
                        None => self.plane.z,
                    };
                    let plane = Plane::from_normal(c.start(), n);
                    let side = forma_geom::side_of(c, cur, &plane);
                    if let Ok(o) = forma_geom::offset(c, self.distance, side, &plane, 1e-6) {
                        poly(&mut out, &o.points());
                    }
                }
            }
            (ArrayLinear, 1) => {
                out.push([self.pts[0], cur]);
                for i in 1..self.count.unwrap_or(2) {
                    xf(
                        &mut out,
                        &Xform::translation((cur - self.pts[0]) * i as f64),
                    );
                }
            }
            (Scale, 1) => {
                out.push([self.pts[0], cur]);
                if let Some(r) = self.reference {
                    let d = r.distance_to(self.pts[0]);
                    if d > 1e-12 {
                        xf(
                            &mut out,
                            &Xform::scale(self.pts[0], cur.distance_to(self.pts[0]) / d),
                        );
                    }
                }
            }
            _ => {}
        }
        out
    }
}
