//! Interactive tools: Rhino-style prompts that collect points, numbers and a
//! selection, then emit ordinary engine command lines (world coordinates), so
//! everything still goes through `forma-engine`.

mod kind;
mod preview;
pub mod seq;

pub use kind::{ToolKind, SELECTION_COMMANDS};
pub use preview::Measure;
use seq::{In, Val};

use forma_geom::{Chain, Plane, Point3, Vec3};

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
    /// Click on any object (rails, property sources).
    PickObject,
    /// A distance along a line (heights): picked on the line or typed.
    Height {
        from: Point3,
        dir: Vec3,
    },
    /// A line of text typed in the command line (spaces allowed).
    Text,
    /// One of the words offered in the prompt.
    Choice,
    /// A point on the surface of a solid under the cursor (edges, faces).
    SurfacePoint,
}

pub enum Step {
    Continue,
    /// Run these command lines and finish.
    Done(Vec<String>),
    /// Run these command lines and keep going (Copy).
    Emit(Vec<String>),
    Cancel(String),
}

/// What clicking (or typing) an option does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptAction {
    /// Runs at once, like typing the word (`Close`, `Undo`).
    Word(&'static str),
    /// Asks for a new number (`Distance`, `Radius`, `NumSides`).
    Value(&'static str),
}

/// A command option offered in the prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOption {
    pub label: String,
    pub action: OptAction,
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
    /// Second phase of a two-selection tool (Split: cutting objects).
    pub phase: u8,
    /// Ids (as numbers) of the first selection of a two-selection tool.
    pub stash: Vec<u64>,
    /// Typed factor waiting for a direction (Scale1D).
    factor: Option<f64>,
    /// Values collected by a sequence tool.
    pub vals: Vec<Val>,
    /// Points of a sequence tool's open point list (Leader).
    pub multi: Vec<Point3>,
}

pub(crate) fn fmt_p(p: Point3) -> String {
    format!("{},{},{}", round(p.x), round(p.y), round(p.z))
}

pub(crate) fn fmt_v(v: Vec3) -> String {
    format!("{},{},{}", round(v.x), round(v.y), round(v.z))
}

/// Trim float noise (1e-12) from emitted numbers so command history stays readable.
pub(crate) fn round(x: f64) -> f64 {
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
            phase: 0,
            stash: Vec::new(),
            factor: None,
            vals: Vec::new(),
            multi: Vec::new(),
        }
    }

    /// The sequence tool and its current input, if this is one.
    pub fn seq_step(&self) -> Option<(&'static seq::Seq, In)> {
        match self.kind {
            ToolKind::Seq(s) => s.steps.get(self.vals.len()).map(|i| (s, *i)),
            _ => None,
        }
    }

    /// Store a value of a sequence tool; emit the command after the last one.
    fn seq_push(&mut self, v: Val) -> Step {
        let ToolKind::Seq(s) = self.kind else {
            return Step::Continue;
        };
        match &v {
            Val::P(p) => self.pts.push(*p),
            Val::Ps(_) => self.multi.clear(),
            _ => {}
        }
        self.vals.push(v);
        if self.vals.len() >= s.steps.len() {
            Step::Done(vec![(s.emit)(&self.vals, &self.plane)])
        } else {
            Step::Continue
        }
    }

    /// Typed text (Text, Dot, Leader).
    pub fn feed_text(&mut self, text: &str) -> Step {
        match self.seq_step() {
            Some((_, In::Text(_))) if !text.trim().is_empty() => {
                self.seq_push(Val::T(text.trim().to_string()))
            }
            Some((_, In::Text(_))) => Step::Continue,
            _ => Step::Continue,
        }
    }

    pub fn prompt(&self) -> String {
        use ToolKind::*;
        if self.selecting {
            let what = match self.kind {
                Trim => "select cutting objects",
                Extend => "select boundary objects",
                Split if self.phase == 0 => "select objects to split",
                Split => "select cutting objects (none: the curves cut each other)",
                Offset | FilletCorners => "select curves",
                Revolve | Sweep1 => "select curves to revolve / sweep",
                Join => "select objects to join",
                MatchProperties => "select objects to change",
                _ => "select objects",
            };
            return format!("{} — {what}, press Enter when done", self.kind.name());
        }
        if let Some((s, step)) = self.seq_step() {
            let p = match step {
                In::Point(p) | In::Dist(p, _) | In::Height(p, _) | In::Text(p) => p.to_string(),
                In::Object(p) | In::Choice(p, _) => p.to_string(),
                In::Num(p, Some(d)) => format!("{p} <{}>", round(d)),
                In::Num(p, None) => p.to_string(),
                In::Points(p, min) | In::SurfacePoints(p, min) if self.multi.len() < min => {
                    p.split(" (").next().unwrap_or(p).to_string()
                }
                In::Points(p, _) | In::SurfacePoints(p, _) => p.to_string(),
            };
            return format!("{} — {p}", s.name);
        }
        let n = self.pts.len();
        let p = match (self.kind, n) {
            (Line, 0) => "Start of line",
            (Line, _) => "End of line",
            (Polyline, 0) => "Start of polyline",
            (Polyline, _) => "Next point (Enter to finish)",
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
            (Offset, _) => "Side to offset",
            (Trim, _) => "Click the part of a curve to cut away (Enter to finish)",
            (Extend, _) => "Click near the end of a curve to extend (Enter to finish)",
            (Fillet, 0) => "First line to fillet",
            (Fillet, _) => "Second line",
            (FilletCorners, _) => "Fillet radius",
            (Join | Explode, _) => "press Enter",
            (ArrayLinear, _) if self.count.is_none() => "Number of elements",
            (ArrayLinear, 0) => "First reference point",
            (ArrayLinear, _) => "Second reference point (spacing and direction)",
            (ArrayPolar, 0) => "Centre of polar array",
            (ArrayPolar, _) => "Number of elements (full turn)",
            (Point, _) => "Location of point (Enter to finish)",
            (Curve | InterpCrv, 0) => "Start of curve",
            (Curve | InterpCrv, _) => "Next point (Enter to finish)",
            (Ellipse, 0) => "Ellipse center",
            (Ellipse, 1) => "End of first axis",
            (Ellipse, _) => "End of second axis (or radius)",
            (Polygon, 0) => "Center of polygon",
            (Polygon, _) => "Corner of polygon",
            (Split, _) => "press Enter",
            (Chamfer, 0) => "First line to chamfer",
            (Chamfer, _) => "Second line",
            (Scale1D | Scale2D, 0) => "Origin point",
            (Scale1D, 1) if self.factor.is_some() => "Direction of scaling",
            (Scale1D | Scale2D, 1) if self.reference.is_none() => {
                "Scale factor or first reference point"
            }
            (Scale1D | Scale2D, _) => "Second reference point",
            (Orient, 0) => "Reference point 1",
            (Orient, 1) => "Reference point 2",
            (Orient, 2) => "Target point 1",
            (Orient, _) => "Target point 2",
            (Revolve, 0) => "Start of revolve axis",
            (Revolve, _) => "End of revolve axis",
            (Sweep1, _) => "Select the rail curve",
            (ExtrudeSrf, _) => "Extrusion distance",
            (Distance, 0) => "First point for distance",
            (Distance, _) => "Second point for distance",
            (MatchProperties, _) => "Select the object to match",
            (OnSel(_) | Seq(_), _) => "press Enter",
        };
        format!("{} — {p}", self.kind.name())
    }

    pub fn want(&self) -> Want {
        use ToolKind::*;
        if self.selecting {
            return Want::Selection;
        }
        if let Some((_, step)) = self.seq_step() {
            return match step {
                In::Point(_) | In::Points(..) => Want::Point,
                In::SurfacePoints(..) => Want::SurfacePoint,
                In::Dist(..) => Want::PointOrNumber,
                In::Height(_, from) => Want::Height {
                    from: self.vals.get(from).map_or(self.anchor, Val::p),
                    dir: self.plane.z,
                },
                In::Num(..) => Want::Number,
                In::Text(_) => Want::Text,
                In::Object(_) => Want::PickObject,
                In::Choice(..) => Want::Choice,
            };
        }
        let n = self.pts.len();
        match (self.kind, n) {
            (Circle, 1) | (Sphere, 1) | (Cylinder, 1) => Want::PointOrNumber,
            (Rotate, 1) | (Scale, 1) if self.reference.is_none() => Want::PointOrNumber,
            (Offset, _) => Want::PointOrNumber,
            (Trim | Extend | Fillet | Chamfer, _) => Want::Pick,
            (Sweep1 | MatchProperties, _) => Want::PickObject,
            (Ellipse, 2) | (Polygon, 0) => Want::PointOrNumber,
            (Scale1D | Scale2D, 1) if self.reference.is_none() && self.factor.is_none() => {
                Want::PointOrNumber
            }
            (ExtrudeSrf, _) => Want::Height {
                from: self.anchor,
                dir: self.plane.z,
            },
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
        if let Some(p) = self.multi.last() {
            return Some(*p);
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
        if let Some((_, step)) = self.seq_step() {
            return match step {
                In::Point(_) => self.seq_push(Val::P(p)),
                In::Points(..) | In::SurfacePoints(..) => {
                    if self.multi.last().is_none_or(|q| q.distance_to(p) > 1e-9) {
                        self.multi.push(p);
                    }
                    Step::Continue
                }
                In::Dist(_, from) => {
                    let o = self.vals.get(from).map_or(p, Val::p);
                    let d = self.plane.moved_to(o).project(p).distance_to(o);
                    self.seq_push(Val::N(d))
                }
                _ => Step::Continue,
            };
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
            (Point, _) => Step::Emit(vec![format!("Point {}", fmt_p(p))]),
            (Ellipse, 2) => {
                let c = self.pts[0];
                let a = self.pts[1] - c;
                let v = p - c;
                let b = match a.normalized() {
                    Some(u) => (v - u * v.dot(u)).length(),
                    None => v.length(),
                };
                self.feed_number(b)
            }
            (Polygon, 1) => Step::Done(vec![format!(
                "Polygon {} {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(p),
                self.count.unwrap_or(5),
                self.n()
            )]),
            (Chamfer, 1) => Step::Done(vec![format!(
                "Chamfer {} {} {} {}",
                round(self.distance),
                fmt_p(self.pts[0]),
                fmt_p(p),
                self.n()
            )]),
            (Scale1D, 1) if self.factor.is_some() => Step::Done(vec![format!(
                "Scale1D {} {} {}",
                fmt_p(self.pts[0]),
                round(self.factor.unwrap_or(1.0)),
                fmt_p(p)
            )]),
            (Scale1D | Scale2D, 1) if self.reference.is_none() => {
                self.reference = Some(p);
                Step::Continue
            }
            (Scale1D, 1) => {
                let o = self.pts[0];
                let r = self.reference.expect("reference");
                let Some(dir) = (r - o).normalized() else {
                    return Step::Cancel("reference point is on the origin".into());
                };
                let f = (p - o).dot(dir) / (r - o).length();
                Step::Done(vec![format!(
                    "Scale1D {} {} {}",
                    fmt_p(o),
                    round(f),
                    fmt_p(r)
                )])
            }
            (Scale2D, 1) => {
                let o = self.pts[0];
                let r = self.reference.expect("reference").distance_to(o);
                if r < 1e-12 {
                    return Step::Cancel("reference point is on the origin".into());
                }
                Step::Done(vec![format!(
                    "Scale2D {} {} {}",
                    fmt_p(o),
                    round(p.distance_to(o) / r),
                    self.n()
                )])
            }
            (Orient, 3) => Step::Done(vec![format!(
                "Orient {} {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(self.pts[1]),
                fmt_p(self.pts[2]),
                fmt_p(p)
            )]),
            (Revolve, 1) => {
                Step::Done(vec![format!("Revolve {} {}", fmt_p(self.pts[0]), fmt_p(p))])
            }
            (Distance, 1) => Step::Done(vec![format!(
                "Distance {} {}",
                fmt_p(self.pts[0]),
                fmt_p(p)
            )]),
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
        if let Some((_, step)) = self.seq_step() {
            return match step {
                In::Dist(..) | In::Height(..) | In::Num(..) => self.seq_push(Val::N(x)),
                _ => Step::Cancel("a point is expected here".into()),
            };
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
            (Chamfer, _) => {
                if x > 0.0 {
                    self.distance = x;
                }
                Step::Continue
            }
            (Ellipse, 2) => Step::Done(vec![format!(
                "Ellipse {} {} {} {}",
                fmt_p(self.pts[0]),
                fmt_p(self.pts[1]),
                round(x.abs()),
                self.n()
            )]),
            (Polygon, 0) => {
                if (3.0..=1000.0).contains(&x) {
                    self.count = Some(x.round() as usize);
                    Step::Continue
                } else {
                    Step::Cancel("a polygon needs 3 or more sides".into())
                }
            }
            (Scale1D, 1) => {
                self.factor = Some(x);
                Step::Continue
            }
            (Scale2D, 1) => Step::Done(vec![format!(
                "Scale2D {} {} {}",
                fmt_p(self.pts[0]),
                round(x),
                self.n()
            )]),
            (ExtrudeSrf, _) => Step::Done(vec![format!("ExtrudeSrf {} {}", round(x), self.n())]),
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

    /// Engine line for an instant tool (selection commands).
    pub fn instant_line(&self) -> String {
        match self.kind {
            ToolKind::OnSel("ProjectToCPlane") => {
                format!("ProjectToCPlane {} {}", self.n(), fmt_p(self.plane.origin))
            }
            ToolKind::Seq(s) => (s.emit)(&[], &self.plane),
            k => k.name().to_string(),
        }
    }

    /// An object was clicked (rail, property source).
    pub fn feed_object(&mut self, id: u64) -> Step {
        if let Some((_, In::Object(_))) = self.seq_step() {
            return self.seq_push(Val::O(id));
        }
        match self.kind {
            ToolKind::Sweep1 => Step::Done(vec![format!("Sweep1 #{id}")]),
            ToolKind::MatchProperties => Step::Done(vec![format!("MatchProperties #{id}")]),
            _ => Step::Continue,
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
                    return Step::Done(vec![self.instant_line()]);
                }
                self.selecting = false;
                return Step::Continue;
            }
            return Step::Cancel("nothing selected".into());
        }
        if let Some((_, step)) = self.seq_step() {
            return match step {
                In::Num(_, Some(d)) => self.seq_push(Val::N(d)),
                In::Points(_, min) | In::SurfacePoints(_, min) if self.multi.len() >= min => {
                    let pts = self.multi.clone();
                    self.seq_push(Val::Ps(pts))
                }
                _ => Step::Cancel("cancelled".into()),
            };
        }
        match self.kind {
            ToolKind::Polyline if self.pts.len() >= 2 => self.finish_polyline(false),
            ToolKind::Copy if !self.pts.is_empty() => Step::Done(Vec::new()),
            ToolKind::Trim | ToolKind::Extend | ToolKind::Point => Step::Done(Vec::new()),
            ToolKind::Curve | ToolKind::InterpCrv if self.pts.len() >= 2 => {
                let pts: Vec<String> = self.pts.iter().map(|p| fmt_p(*p)).collect();
                Step::Done(vec![format!("{} {}", self.kind.name(), pts.join(" "))])
            }
            _ => Step::Cancel("cancelled".into()),
        }
    }

    /// Typed options: `C` / `Close` closes a polyline, `U` / `Undo` removes the
    /// last point.
    pub fn option(&mut self, word: &str) -> Option<Step> {
        let w = word.to_lowercase();
        match self.seq_step() {
            Some((_, In::Choice(_, words))) => {
                let found = words.iter().find(|c| c.eq_ignore_ascii_case(word))?;
                return Some(self.seq_push(Val::W(found)));
            }
            Some((_, In::Points(..) | In::SurfacePoints(..)))
                if (w == "u" || w == "undo") && !self.multi.is_empty() =>
            {
                self.multi.pop();
                return Some(Step::Continue);
            }
            _ => {}
        }
        if self.kind == ToolKind::Polyline && (w == "c" || w == "close") && self.pts.len() >= 3 {
            return Some(self.finish_polyline(true));
        }
        let undoable = matches!(
            self.kind,
            ToolKind::Polyline | ToolKind::Curve | ToolKind::InterpCrv
        );
        if undoable && (w == "u" || w == "undo") && !self.pts.is_empty() {
            self.pts.pop();
            return Some(Step::Continue);
        }
        None
    }

    /// Options of the current step, shown as clickable words in the prompt
    /// (Rhino: `Next point ( Close Undo )`, `Side to offset ( Distance=10 )`).
    pub fn options(&self) -> Vec<ToolOption> {
        use ToolKind::*;
        let mut v = Vec::new();
        if self.selecting {
            return v;
        }
        let n = self.pts.len();
        let value = |name: &'static str, x: f64| ToolOption {
            label: format!("{name}={}", round(x)),
            action: OptAction::Value(name),
        };
        let word = |name: &'static str| ToolOption {
            label: name.to_string(),
            action: OptAction::Word(name),
        };
        match self.seq_step() {
            Some((_, In::Choice(_, words))) => {
                v.extend(words.iter().map(|w| word(w)));
                return v;
            }
            Some((_, In::Points(..) | In::SurfacePoints(..))) if !self.multi.is_empty() => {
                v.push(word("Undo"));
                return v;
            }
            _ => {}
        }
        match self.kind {
            Polyline if n >= 3 => v.extend([word("Close"), word("Undo")]),
            Polyline | Curve | InterpCrv if n >= 1 => v.push(word("Undo")),
            Offset => v.push(value("Distance", self.distance)),
            Chamfer => v.push(value("Distance", self.distance)),
            Fillet => v.push(value("Radius", self.distance)),
            Polygon if n == 0 => v.push(value("NumSides", self.count.unwrap_or(5) as f64)),
            _ => {}
        }
        v
    }

    /// Current value of a numeric option.
    pub fn option_value(&self, name: &str) -> Option<f64> {
        self.options().iter().find_map(|o| match o.action {
            OptAction::Value(n) if n.eq_ignore_ascii_case(name) => Some(if n == "NumSides" {
                self.count.unwrap_or(5) as f64
            } else {
                self.distance
            }),
            _ => None,
        })
    }

    /// Set a numeric option (`Distance`, `Radius`, `NumSides`).
    pub fn set_option(&mut self, name: &str, x: f64) -> Result<(), String> {
        match name.to_ascii_lowercase().as_str() {
            "distance" | "radius" if x > 0.0 || (self.kind == ToolKind::Fillet && x >= 0.0) => {
                self.distance = x;
                Ok(())
            }
            "numsides" if (3.0..=1000.0).contains(&x) => {
                self.count = Some(x.round() as usize);
                Ok(())
            }
            "numsides" => Err("a polygon needs 3 or more sides".into()),
            _ => Err(format!("invalid value for {name}: {x}")),
        }
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
mod tests;
