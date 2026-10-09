//! Interactive tools: Rhino-style prompts that collect points, numbers and a
//! selection, then emit ordinary engine command lines (world coordinates), so
//! everything still goes through `forma-engine`.

use forma_geom::{Chain, CircleArc, Plane, Point3, Vec3, Xform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Line,
    Polyline,
    Rectangle,
    Circle,
    Arc,
    Box,
    Cylinder,
    Sphere,
    Extrude,
    Move,
    Copy,
    Rotate,
    Scale,
    Mirror,
    Offset,
    Trim,
    Extend,
    Fillet,
    FilletCorners,
    Join,
    Explode,
    ArrayLinear,
    ArrayPolar,
}

impl ToolKind {
    pub const CURVES: [ToolKind; 5] = [
        ToolKind::Line,
        ToolKind::Polyline,
        ToolKind::Rectangle,
        ToolKind::Circle,
        ToolKind::Arc,
    ];
    pub const SOLIDS: [ToolKind; 4] = [
        ToolKind::Box,
        ToolKind::Cylinder,
        ToolKind::Sphere,
        ToolKind::Extrude,
    ];
    pub const TRANSFORMS: [ToolKind; 7] = [
        ToolKind::Move,
        ToolKind::Copy,
        ToolKind::Rotate,
        ToolKind::Scale,
        ToolKind::Mirror,
        ToolKind::ArrayLinear,
        ToolKind::ArrayPolar,
    ];
    pub const CURVE_TOOLS: [ToolKind; 5] = [
        ToolKind::Offset,
        ToolKind::Trim,
        ToolKind::Extend,
        ToolKind::Fillet,
        ToolKind::FilletCorners,
    ];
    pub const EDIT: [ToolKind; 2] = [ToolKind::Join, ToolKind::Explode];

    pub fn name(self) -> &'static str {
        match self {
            ToolKind::Line => "Line",
            ToolKind::Polyline => "Polyline",
            ToolKind::Rectangle => "Rectangle",
            ToolKind::Circle => "Circle",
            ToolKind::Arc => "Arc",
            ToolKind::Box => "Box",
            ToolKind::Cylinder => "Cylinder",
            ToolKind::Sphere => "Sphere",
            ToolKind::Extrude => "ExtrudeCrv",
            ToolKind::Move => "Move",
            ToolKind::Copy => "Copy",
            ToolKind::Rotate => "Rotate",
            ToolKind::Scale => "Scale",
            ToolKind::Mirror => "Mirror",
            ToolKind::Offset => "Offset",
            ToolKind::Trim => "Trim",
            ToolKind::Extend => "Extend",
            ToolKind::Fillet => "Fillet",
            ToolKind::FilletCorners => "FilletCorners",
            ToolKind::Join => "Join",
            ToolKind::Explode => "Explode",
            ToolKind::ArrayLinear => "ArrayLinear",
            ToolKind::ArrayPolar => "ArrayPolar",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            ToolKind::Line => "Line — single segment",
            ToolKind::Polyline => "Polyline — connected segments (C closes)",
            ToolKind::Rectangle => "Rectangle — two corners",
            ToolKind::Circle => "Circle — center, radius",
            ToolKind::Arc => "Arc — center, start, end",
            ToolKind::Box => "Box — two corners and height",
            ToolKind::Cylinder => "Cylinder — center, radius, height",
            ToolKind::Sphere => "Sphere — center, radius",
            ToolKind::Extrude => "ExtrudeCrv — extrude curves (closed → solid)",
            ToolKind::Move => "Move — from, to",
            ToolKind::Copy => "Copy — from, to (repeat, Enter to finish)",
            ToolKind::Rotate => "Rotate — center, angle or two reference points",
            ToolKind::Scale => "Scale — base point, factor or two reference points",
            ToolKind::Mirror => "Mirror — two points of the mirror line (copies)",
            ToolKind::Offset => "Offset — parallel copy of curves at a distance",
            ToolKind::Trim => "Trim — cut curves with cutting objects",
            ToolKind::Extend => "Extend — lengthen curves to boundaries",
            ToolKind::Fillet => "Fillet — round the corner between two lines",
            ToolKind::FilletCorners => "FilletCorners — round all corners of polylines",
            ToolKind::Join => "Join — join curves end to end / meshes into one",
            ToolKind::Explode => "Explode — split into segments or faces",
            ToolKind::ArrayLinear => "ArrayLinear — copies along a direction",
            ToolKind::ArrayPolar => "ArrayPolar — copies around a centre",
        }
    }

    pub fn from_name(s: &str) -> Option<ToolKind> {
        let l = s.to_lowercase();
        Some(match l.as_str() {
            "line" | "l" => ToolKind::Line,
            "polyline" | "pl" => ToolKind::Polyline,
            "rectangle" | "rec" => ToolKind::Rectangle,
            "circle" | "c" => ToolKind::Circle,
            "arc" => ToolKind::Arc,
            "box" => ToolKind::Box,
            "cylinder" => ToolKind::Cylinder,
            "sphere" => ToolKind::Sphere,
            "extrude" | "extrudecrv" | "ext" => ToolKind::Extrude,
            "move" | "m" => ToolKind::Move,
            "copy" | "co" | "cp" => ToolKind::Copy,
            "rotate" | "ro" => ToolKind::Rotate,
            "scale" | "sc" => ToolKind::Scale,
            "mirror" | "mi" => ToolKind::Mirror,
            "offset" | "o" => ToolKind::Offset,
            "trim" | "tr" => ToolKind::Trim,
            "extend" | "ex" => ToolKind::Extend,
            "fillet" | "f" => ToolKind::Fillet,
            "filletcorners" | "fc" => ToolKind::FilletCorners,
            "join" | "j" => ToolKind::Join,
            "explode" | "x" => ToolKind::Explode,
            "arraylinear" | "al" => ToolKind::ArrayLinear,
            "arraypolar" | "ap" => ToolKind::ArrayPolar,
            _ => return None,
        })
    }

    pub fn needs_selection(self) -> bool {
        matches!(
            self,
            ToolKind::Extrude
                | ToolKind::Move
                | ToolKind::Copy
                | ToolKind::Rotate
                | ToolKind::Scale
                | ToolKind::Mirror
                | ToolKind::Offset
                | ToolKind::Trim
                | ToolKind::Extend
                | ToolKind::FilletCorners
                | ToolKind::Join
                | ToolKind::Explode
                | ToolKind::ArrayLinear
                | ToolKind::ArrayPolar
        )
    }

    /// Tools that are a single engine command once objects are selected.
    pub fn instant(self) -> bool {
        matches!(self, ToolKind::Join | ToolKind::Explode)
    }
}

/// What the current step accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Want {
    /// Select objects; Enter continues.
    Selection,
    Point,
    /// A point or a typed number (radius, angle, factor).
    PointOrNumber,
    /// A typed number only (counts, radii).
    Number,
    /// Click on a curve (no snaps); a typed number changes the tool's distance.
    Pick,
    /// A distance along a line (heights): picked on the line or typed.
    Height {
        from: Point3,
        dir: Vec3,
    },
}

pub enum Step {
    Continue,
    /// Run these command lines and finish.
    Done(Vec<String>),
    /// Run these command lines and keep going (Copy).
    Emit(Vec<String>),
    Cancel(String),
}

pub struct Tool {
    pub kind: ToolKind,
    pub plane: Plane,
    pub pts: Vec<Point3>,
    pub selecting: bool,
    /// Reference point for selection-based heights (Extrude).
    pub anchor: Point3,
    /// Wireframe of the selection, for transform previews.
    pub skeleton: Vec<[Point3; 2]>,
    reference: Option<Point3>,
    /// Offset distance or fillet radius (remembered between uses by the app).
    pub distance: f64,
    /// Number of elements (arrays).
    count: Option<usize>,
    /// Selected curves with their own plane normal, for the Offset preview.
    pub curves: Vec<(Chain, Option<Vec3>)>,
}

fn fmt_p(p: Point3) -> String {
    format!("{},{},{}", round(p.x), round(p.y), round(p.z))
}

fn fmt_v(v: Vec3) -> String {
    format!("{},{},{}", round(v.x), round(v.y), round(v.z))
}

/// Trim float noise (1e-12) from emitted numbers so command history stays readable.
fn round(x: f64) -> f64 {
    let r = (x * 1e9).round() / 1e9;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

impl Tool {
    pub fn new(
        kind: ToolKind,
        plane: Plane,
        has_selection: bool,
        anchor: Point3,
        skeleton: Vec<[Point3; 2]>,
    ) -> Tool {
        Tool {
            kind,
            plane,
            pts: Vec::new(),
            selecting: kind.needs_selection() && !has_selection,
            anchor,
            skeleton,
            reference: None,
            distance: 1.0,
            count: None,
            curves: Vec::new(),
        }
    }

    pub fn prompt(&self) -> String {
        use ToolKind::*;
        if self.selecting {
            let what = match self.kind {
                Trim => "select cutting objects",
                Extend => "select boundary objects",
                Offset | FilletCorners => "select curves",
                _ => "select objects",
            };
            return format!("{} — {what}, press Enter when done", self.kind.name());
        }
        let d = round(self.distance);
        let n = self.pts.len();
        let p = match (self.kind, n) {
            (Line, 0) => "Start of line",
            (Line, _) => "End of line",
            (Polyline, 0) => "Start of polyline",
            (Polyline, 1 | 2) => "Next point (Enter to finish)",
            (Polyline, _) => "Next point (Enter to finish, C to close)",
            (Rectangle, 0) => "First corner of rectangle",
            (Rectangle, _) => "Other corner (or length)",
            (Circle, 0) => "Center of circle",
            (Circle, _) => "Radius",
            (Arc, 0) => "Center of arc",
            (Arc, 1) => "Start of arc",
            (Arc, _) => "End point (counter-clockwise)",
            (Box, 0) => "First corner of base",
            (Box, 1) => "Other corner of base",
            (Box, _) => "Height",
            (Cylinder, 0) => "Center of base",
            (Cylinder, 1) => "Radius",
            (Cylinder, _) => "End of cylinder (height)",
            (Sphere, 0) => "Center of sphere",
            (Sphere, _) => "Radius",
            (Extrude, _) => "Extrusion distance",
            (Move, 0) => "Point to move from",
            (Move, _) => "Point to move to",
            (Copy, 0) => "Point to copy from",
            (Copy, _) => "Point to copy to (Enter to finish)",
            (Rotate, 0) => "Center of rotation",
            (Rotate, 1) if self.reference.is_none() => "Angle or first reference point",
            (Rotate, _) => "Second reference point",
            (Scale, 0) => "Origin point",
            (Scale, 1) if self.reference.is_none() => "Scale factor or first reference point",
            (Scale, _) => "Second reference point",
            (Mirror, 0) => "Start of mirror plane",
            (Mirror, _) => "End of mirror plane",
            (Offset, _) => {
                return format!("Offset — side to offset (distance {d}; type a number to change)")
            }
            (Trim, _) => "Click the part of a curve to cut away (Enter to finish)",
            (Extend, _) => "Click near the end of a curve to extend (Enter to finish)",
            (Fillet, 0) => {
                return format!("Fillet — first line (radius {d}; type a number to change)")
            }
            (Fillet, _) => "Second line",
            (FilletCorners, _) => "Fillet radius",
            (Join | Explode, _) => "press Enter",
            (ArrayLinear, _) if self.count.is_none() => "Number of elements",
            (ArrayLinear, 0) => "First reference point",
            (ArrayLinear, _) => "Second reference point (spacing and direction)",
            (ArrayPolar, 0) => "Centre of polar array",
            (ArrayPolar, _) => "Number of elements (full turn)",
        };
        format!("{} — {p}", self.kind.name())
    }

    pub fn want(&self) -> Want {
        use ToolKind::*;
        if self.selecting {
            return Want::Selection;
        }
        let n = self.pts.len();
        match (self.kind, n) {
            (Circle, 1) | (Sphere, 1) | (Cylinder, 1) => Want::PointOrNumber,
            (Rotate, 1) | (Scale, 1) if self.reference.is_none() => Want::PointOrNumber,
            (Offset, _) => Want::PointOrNumber,
            (Trim | Extend | Fillet, _) => Want::Pick,
            (FilletCorners, _) => Want::Number,
            (ArrayLinear, _) if self.count.is_none() => Want::Number,
            (ArrayPolar, 1) => Want::Number,
            (Box, 2) => Want::Height {
                from: self.pts[1],
                dir: self.plane.z,
            },
            (Cylinder, 2) => Want::Height {
                from: self.pts[0],
                dir: self.plane.z,
            },
            (Extrude, _) => Want::Height {
                from: self.anchor,
                dir: self.plane.z,
            },
            _ => Want::Point,
        }
    }

    /// Last point entered, for relative input, ortho and length constraints.
    pub fn base(&self) -> Option<Point3> {
        if self.kind == ToolKind::Copy && !self.pts.is_empty() {
            return Some(self.pts[0]);
        }
        self.pts.last().copied()
    }

    fn n(&self) -> String {
        fmt_v(self.plane.z)
    }

    fn plane_at(&self, p: Point3) -> Plane {
        self.plane.moved_to(p)
    }

    pub fn feed_point(&mut self, p: Point3) -> Step {
        use ToolKind::*;
        if self.selecting {
            return Step::Continue;
        }
        if let Want::Height { from, dir } = self.want() {
            let h = (p - from).dot(dir.normalized().unwrap_or(Vec3::Z));
            return self.feed_number(h);
        }
        let n = self.pts.len();
        match (self.kind, n) {
            (Line, 1) => Step::Done(vec![format!("Line {} {}", fmt_p(self.pts[0]), fmt_p(p))]),
            (Polyline, k) if k >= 3 && p.distance_to(self.pts[0]) < 1e-9 => {
                self.pts.push(p);
                self.finish_polyline(false)
            }
            (Rectangle, 1) => Step::Done(vec![format!(
                "Rectangle {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(p),
                self.n()
            )]),
            (Circle, 1) | (Sphere, 1) | (Cylinder, 1) => {
                let r = self
                    .plane_at(self.pts[0])
                    .project(p)
                    .distance_to(self.pts[0]);
                let r = if self.kind == Sphere {
                    p.distance_to(self.pts[0])
                } else {
                    r
                };
                self.feed_number(r)
            }
            (Arc, 2) => Step::Done(vec![format!(
                "Arc {} {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(self.pts[1]),
                fmt_p(p),
                self.n()
            )]),
            (Move, 1) => Step::Done(vec![format!("Move {} {}", fmt_p(self.pts[0]), fmt_p(p))]),
            (Copy, 1) => Step::Emit(vec![format!("Copy {} {}", fmt_p(self.pts[0]), fmt_p(p))]),
            (Mirror, 1) => Step::Done(vec![format!(
                "Mirror {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(p),
                self.n()
            )]),
            (Rotate, 1) | (Scale, 1) if self.reference.is_none() => {
                self.reference = Some(p);
                Step::Continue
            }
            (Rotate, 1) => {
                let a = self.angle(p);
                Step::Done(vec![format!(
                    "Rotate {} {} {}",
                    fmt_p(self.pts[0]),
                    round(a),
                    self.n()
                )])
            }
            (Offset, _) => Step::Done(vec![format!(
                "Offset {} {} {}",
                round(self.distance),
                fmt_p(p),
                self.n()
            )]),
            (Trim, _) => Step::Emit(vec![format!("Trim {} {}", fmt_p(p), self.n())]),
            (Extend, _) => Step::Emit(vec![format!("Extend {} {}", fmt_p(p), self.n())]),
            (Fillet, 1) => Step::Done(vec![format!(
                "Fillet {} {} {} {}",
                round(self.distance),
                fmt_p(self.pts[0]),
                fmt_p(p),
                self.n()
            )]),
            (ArrayLinear, 1) => Step::Done(vec![format!(
                "ArrayLinear {} {} {}",
                self.count.unwrap_or(2),
                fmt_p(self.pts[0]),
                fmt_p(p)
            )]),
            (Scale, 1) => {
                let r = self.reference.expect("reference").distance_to(self.pts[0]);
                if r < 1e-12 {
                    return Step::Cancel("reference point is on the origin".into());
                }
                let f = p.distance_to(self.pts[0]) / r;
                Step::Done(vec![format!("Scale {} {}", fmt_p(self.pts[0]), round(f))])
            }
            _ => {
                self.pts.push(p);
                Step::Continue
            }
        }
    }

    fn angle(&self, p: Point3) -> f64 {
        let c = self.pts[0];
        let r = self.reference.unwrap_or(c);
        let (a0, b0) = (self.plane.x.dot(r - c), self.plane.y.dot(r - c));
        let (a1, b1) = (self.plane.x.dot(p - c), self.plane.y.dot(p - c));
        (b1.atan2(a1) - b0.atan2(a0)).to_degrees()
    }

    pub fn feed_number(&mut self, x: f64) -> Step {
        use ToolKind::*;
        if self.selecting {
            return Step::Continue;
        }
        let n = self.pts.len();
        match (self.kind, n) {
            (Circle, 1) => Step::Done(vec![format!(
                "Circle {} {} {}",
                fmt_p(self.pts[0]),
                round(x.abs()),
                self.n()
            )]),
            (Sphere, 1) => Step::Done(vec![format!(
                "Sphere {} {}",
                fmt_p(self.pts[0]),
                round(x.abs())
            )]),
            (Cylinder, 1) => {
                // Remember the radius as a point on the plane.
                let p = self.plane_at(self.pts[0]).point_at(x.abs(), 0.0, 0.0);
                self.pts.push(p);
                Step::Continue
            }
            (Cylinder, 2) => {
                let r = self.pts[1].distance_to(self.pts[0]);
                Step::Done(vec![format!(
                    "Cylinder {} {} {} {}",
                    fmt_p(self.pts[0]),
                    round(r),
                    round(x),
                    self.n()
                )])
            }
            (Box, 2) => Step::Done(vec![format!(
                "Box {} {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(self.pts[1]),
                round(x),
                self.n()
            )]),
            (Extrude, _) => Step::Done(vec![format!("Extrude {} {}", round(x), self.n())]),
            (Rotate, 1) => Step::Done(vec![format!(
                "Rotate {} {} {}",
                fmt_p(self.pts[0]),
                round(x),
                self.n()
            )]),
            (Scale, 1) => Step::Done(vec![format!("Scale {} {}", fmt_p(self.pts[0]), round(x))]),
            (Offset | Fillet | Trim | Extend, _) => {
                if x > 0.0 || (self.kind == Fillet && x >= 0.0) {
                    self.distance = x;
                }
                Step::Continue
            }
            (FilletCorners, _) => Step::Done(vec![format!("FilletCorners {}", round(x.abs()))]),
            (ArrayLinear, _) if self.count.is_none() => {
                if x >= 2.0 {
                    self.count = Some(x.round() as usize);
                    Step::Continue
                } else {
                    Step::Cancel("at least 2 elements".into())
                }
            }
            (ArrayPolar, 1) => {
                if x < 2.0 {
                    return Step::Cancel("at least 2 elements".into());
                }
                Step::Done(vec![format!(
                    "ArrayPolar {} {} 360 {}",
                    fmt_p(self.pts[0]),
                    x.round(),
                    self.n()
                )])
            }
            _ => Step::Cancel("a point is expected here".into()),
        }
    }

    fn finish_polyline(&mut self, close: bool) -> Step {
        let mut pts: Vec<String> = self.pts.iter().map(|p| fmt_p(*p)).collect();
        if close {
            pts.push("c".into());
        }
        Step::Done(vec![format!("Polyline {}", pts.join(" "))])
    }

    /// Enter / right click.
    pub fn enter(&mut self, has_selection: bool) -> Step {
        if self.selecting {
            if has_selection {
                if self.kind.instant() {
                    return Step::Done(vec![self.kind.name().to_string()]);
                }
                self.selecting = false;
                return Step::Continue;
            }
            return Step::Cancel("nothing selected".into());
        }
        match self.kind {
            ToolKind::Polyline if self.pts.len() >= 2 => self.finish_polyline(false),
            ToolKind::Copy if !self.pts.is_empty() => Step::Done(Vec::new()),
            ToolKind::Trim | ToolKind::Extend => Step::Done(Vec::new()),
            _ => Step::Cancel("cancelled".into()),
        }
    }

    /// Typed options: `C` closes a polyline.
    pub fn option(&mut self, word: &str) -> Option<Step> {
        let w = word.to_lowercase();
        if self.kind == ToolKind::Polyline && (w == "c" || w == "close") && self.pts.len() >= 3 {
            return Some(self.finish_polyline(true));
        }
        if self.kind == ToolKind::Polyline && (w == "u" || w == "undo") && !self.pts.is_empty() {
            self.pts.pop();
            return Some(Step::Continue);
        }
        None
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

/// Parse a typed point in construction-plane coordinates, Rhino style:
/// `x,y[,z]`, `@dx,dy[,dz]`, `@d<angle`, or a plain number = length from the base
/// point towards the cursor.
pub fn parse_typed_point(
    tok: &str,
    plane: &Plane,
    base: Option<Point3>,
    toward: Option<Point3>,
) -> Option<Point3> {
    let (rel, body) = match tok.strip_prefix('@') {
        Some(r) => (true, r),
        None => (false, tok),
    };
    let num = |s: &str| s.trim().parse::<f64>().ok();
    if rel {
        let b = base?;
        if let Some((d, a)) = body.split_once('<') {
            let (d, a) = (num(d)?, num(a)?.to_radians());
            return Some(b + plane.x * (d * a.cos()) + plane.y * (d * a.sin()));
        }
    }
    let parts: Vec<&str> = body.split(',').collect();
    let (u, v, w) = match parts.as_slice() {
        [u, v] => (num(u)?, num(v)?, 0.0),
        [u, v, w] => (num(u)?, num(v)?, num(w)?),
        [d] if !rel => {
            // Length constraint along the rubber band.
            let d = num(d)?;
            let b = base?;
            let dir = toward.and_then(|t| (t - b).normalized()).unwrap_or(plane.x);
            return Some(b + dir * d);
        }
        _ => return None,
    };
    if rel {
        Some(base? + plane.x * u + plane.y * v + plane.z * w)
    } else {
        Some(plane.point_at(u, v, w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(tool: &mut Tool, pts: &[Point3]) -> Vec<String> {
        for p in pts {
            match tool.feed_point(*p) {
                Step::Done(c) | Step::Emit(c) => return c,
                Step::Cancel(e) => panic!("{e}"),
                Step::Continue => {}
            }
        }
        panic!("tool did not finish");
    }

    #[test]
    fn line_and_box_emit_commands() {
        let mut t = Tool::new(ToolKind::Line, Plane::TOP, false, Point3::ORIGIN, vec![]);
        let c = run(&mut t, &[Point3::ORIGIN, Point3::new(100.0, 0.0, 0.0)]);
        assert_eq!(c, vec!["Line 0,0,0 100,0,0"]);
        let mut b = Tool::new(ToolKind::Box, Plane::TOP, false, Point3::ORIGIN, vec![]);
        b.feed_point(Point3::ORIGIN);
        b.feed_point(Point3::new(600.0, 400.0, 0.0));
        let Step::Done(c) = b.feed_number(18.0) else {
            panic!()
        };
        assert_eq!(c, vec!["Box 0,0,0 600,400,0 18 0,0,1"]);
    }

    #[test]
    fn transform_tools_need_selection() {
        let mut t = Tool::new(ToolKind::Move, Plane::TOP, false, Point3::ORIGIN, vec![]);
        assert_eq!(t.want(), Want::Selection);
        assert!(matches!(t.enter(false), Step::Cancel(_)));
        let mut t = Tool::new(ToolKind::Move, Plane::TOP, false, Point3::ORIGIN, vec![]);
        assert!(matches!(t.enter(true), Step::Continue));
        let c = run(&mut t, &[Point3::ORIGIN, Point3::new(0.0, 50.0, 0.0)]);
        assert_eq!(c, vec!["Move 0,0,0 0,50,0"]);
    }

    #[test]
    fn polyline_closes_with_c() {
        let mut t = Tool::new(
            ToolKind::Polyline,
            Plane::TOP,
            false,
            Point3::ORIGIN,
            vec![],
        );
        for p in [
            Point3::ORIGIN,
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
        ] {
            t.feed_point(p);
        }
        let Some(Step::Done(c)) = t.option("C") else {
            panic!()
        };
        assert_eq!(c, vec!["Polyline 0,0,0 1,0,0 1,1,0 c"]);
    }

    #[test]
    fn rotate_by_reference_points() {
        let mut t = Tool::new(ToolKind::Rotate, Plane::TOP, true, Point3::ORIGIN, vec![]);
        t.feed_point(Point3::ORIGIN);
        t.feed_point(Point3::new(1.0, 0.0, 0.0));
        let Step::Done(c) = t.feed_point(Point3::new(0.0, 2.0, 0.0)) else {
            panic!()
        };
        assert_eq!(c, vec!["Rotate 0,0,0 90 0,0,1"]);
    }

    #[test]
    fn typed_points_in_cplane() {
        let front = Plane::FRONT;
        let p = parse_typed_point("100,50", &front, None, None).unwrap();
        assert_eq!(p, Point3::new(100.0, 0.0, 50.0));
        let base = Some(Point3::new(10.0, 0.0, 0.0));
        let q = parse_typed_point("@0,20", &front, base, None).unwrap();
        assert_eq!(q, Point3::new(10.0, 0.0, 20.0));
        let r =
            parse_typed_point("30", &Plane::TOP, base, Some(Point3::new(10.0, 5.0, 0.0))).unwrap();
        assert!(r.distance_to(Point3::new(10.0, 30.0, 0.0)) < 1e-9);
        let s = parse_typed_point("@10<90", &Plane::TOP, base, None).unwrap();
        assert!(s.distance_to(Point3::new(10.0, 10.0, 0.0)) < 1e-9);
    }
}
