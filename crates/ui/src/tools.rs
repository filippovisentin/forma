//! Interactive tools: Rhino-style prompts that collect points, numbers and a
//! selection, then emit ordinary engine command lines (world coordinates), so
//! everything still goes through `forma-engine`.

use forma_geom::{Chain, CircleArc, Plane, Point3, Vec3, Xform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Point,
    Line,
    Polyline,
    Curve,
    InterpCrv,
    Rectangle,
    Circle,
    Arc,
    Ellipse,
    Polygon,
    Box,
    Cylinder,
    Sphere,
    Extrude,
    ExtrudeSrf,
    Revolve,
    Sweep1,
    Move,
    Copy,
    Rotate,
    Scale,
    Scale1D,
    Scale2D,
    Mirror,
    Orient,
    Offset,
    Trim,
    Split,
    Extend,
    Fillet,
    Chamfer,
    FilletCorners,
    Join,
    Explode,
    ArrayLinear,
    ArrayPolar,
    Distance,
    MatchProperties,
    /// An engine command that acts on the selection (asks for one if empty).
    OnSel(&'static str),
}

/// Commands that only need a selection, then run as typed.
pub const SELECTION_COMMANDS: [&str; 22] = [
    "Hide",
    "Isolate",
    "Lock",
    "Group",
    "Ungroup",
    "SelGroup",
    "Flip",
    "ProjectToCPlane",
    "Intersect",
    "PlanarSrf",
    "Loft",
    "Cap",
    "Length",
    "Area",
    "Volume",
    "BoundingBox",
    "What",
    "CopyToClipboard",
    "Cut",
    "Delete",
    "Join",
    "Explode",
];

impl ToolKind {
    pub const CURVES: [ToolKind; 10] = [
        ToolKind::Point,
        ToolKind::Line,
        ToolKind::Polyline,
        ToolKind::Curve,
        ToolKind::InterpCrv,
        ToolKind::Rectangle,
        ToolKind::Circle,
        ToolKind::Arc,
        ToolKind::Ellipse,
        ToolKind::Polygon,
    ];
    pub const CURVE_TOOLS: [ToolKind; 12] = [
        ToolKind::Offset,
        ToolKind::Trim,
        ToolKind::Split,
        ToolKind::Extend,
        ToolKind::Fillet,
        ToolKind::Chamfer,
        ToolKind::FilletCorners,
        ToolKind::Join,
        ToolKind::Explode,
        ToolKind::OnSel("Flip"),
        ToolKind::OnSel("ProjectToCPlane"),
        ToolKind::OnSel("Intersect"),
    ];
    pub const SURFACES: [ToolKind; 7] = [
        ToolKind::OnSel("PlanarSrf"),
        ToolKind::Extrude,
        ToolKind::OnSel("Loft"),
        ToolKind::Revolve,
        ToolKind::Sweep1,
        ToolKind::ExtrudeSrf,
        ToolKind::OnSel("Cap"),
    ];
    pub const SOLIDS: [ToolKind; 6] = [
        ToolKind::Box,
        ToolKind::Cylinder,
        ToolKind::Sphere,
        ToolKind::Extrude,
        ToolKind::ExtrudeSrf,
        ToolKind::OnSel("Cap"),
    ];
    pub const TRANSFORMS: [ToolKind; 10] = [
        ToolKind::Move,
        ToolKind::Copy,
        ToolKind::Rotate,
        ToolKind::Scale,
        ToolKind::Scale1D,
        ToolKind::Scale2D,
        ToolKind::Mirror,
        ToolKind::Orient,
        ToolKind::ArrayLinear,
        ToolKind::ArrayPolar,
    ];
    pub const VISIBILITY: [ToolKind; 5] = [
        ToolKind::OnSel("Hide"),
        ToolKind::OnSel("Isolate"),
        ToolKind::OnSel("Lock"),
        ToolKind::OnSel("Group"),
        ToolKind::OnSel("Ungroup"),
    ];
    pub const ANALYZE: [ToolKind; 6] = [
        ToolKind::Distance,
        ToolKind::OnSel("Length"),
        ToolKind::OnSel("Area"),
        ToolKind::OnSel("Volume"),
        ToolKind::OnSel("BoundingBox"),
        ToolKind::OnSel("What"),
    ];
    pub const EDIT: [ToolKind; 3] = [ToolKind::Join, ToolKind::Explode, ToolKind::MatchProperties];

    pub fn name(self) -> &'static str {
        use ToolKind::*;
        match self {
            Point => "Point",
            Line => "Line",
            Polyline => "Polyline",
            Curve => "Curve",
            InterpCrv => "InterpCrv",
            Rectangle => "Rectangle",
            Circle => "Circle",
            Arc => "Arc",
            Ellipse => "Ellipse",
            Polygon => "Polygon",
            Box => "Box",
            Cylinder => "Cylinder",
            Sphere => "Sphere",
            Extrude => "ExtrudeCrv",
            ExtrudeSrf => "ExtrudeSrf",
            Revolve => "Revolve",
            Sweep1 => "Sweep1",
            Move => "Move",
            Copy => "Copy",
            Rotate => "Rotate",
            Scale => "Scale",
            Scale1D => "Scale1D",
            Scale2D => "Scale2D",
            Mirror => "Mirror",
            Orient => "Orient",
            Offset => "Offset",
            Trim => "Trim",
            Split => "Split",
            Extend => "Extend",
            Fillet => "Fillet",
            Chamfer => "Chamfer",
            FilletCorners => "FilletCorners",
            Join => "Join",
            Explode => "Explode",
            ArrayLinear => "ArrayLinear",
            ArrayPolar => "ArrayPolar",
            Distance => "Distance",
            MatchProperties => "MatchProperties",
            OnSel(n) => n,
        }
    }

    pub fn tooltip(self) -> &'static str {
        use ToolKind::*;
        match self {
            Point => "Point — single points (Enter to finish)",
            Line => "Line — single segment",
            Polyline => "Polyline — connected segments",
            Curve => "Curve — control-point curve (Enter to finish)",
            InterpCrv => "InterpCrv — curve through points (Enter to finish)",
            Rectangle => "Rectangle — two corners",
            Circle => "Circle — center, radius",
            Arc => "Arc — center, start, end",
            Ellipse => "Ellipse — center, end of first axis, second axis",
            Polygon => "Polygon — center, corner (NumSides option)",
            Box => "Box — two corners and height",
            Cylinder => "Cylinder — center, radius, height",
            Sphere => "Sphere — center, radius",
            Extrude => "ExtrudeCrv — extrude curves (closed → solid)",
            ExtrudeSrf => "ExtrudeSrf — extrude a planar surface into a solid",
            Revolve => "Revolve — curves around an axis",
            Sweep1 => "Sweep1 — profiles along one rail",
            Move => "Move — from, to",
            Copy => "Copy — from, to (repeat, Enter to finish)",
            Rotate => "Rotate — center, angle or two reference points",
            Scale => "Scale — base point, factor or two reference points",
            Scale1D => "Scale1D — scale in one direction",
            Scale2D => "Scale2D — scale in the construction plane",
            Mirror => "Mirror — two points of the mirror line (copies)",
            Orient => "Orient — two reference points to two target points",
            Offset => "Offset — parallel copy of curves at a distance",
            Trim => "Trim — cut curves with cutting objects",
            Split => "Split — divide curves at cutting objects",
            Extend => "Extend — lengthen curves to boundaries",
            Fillet => "Fillet — round the corner between two lines",
            Chamfer => "Chamfer — bevel the corner between two lines",
            FilletCorners => "FilletCorners — round all corners of polylines",
            Join => "Join — join curves end to end / meshes into one",
            Explode => "Explode — split into segments or faces",
            ArrayLinear => "ArrayLinear — copies along a direction",
            ArrayPolar => "ArrayPolar — copies around a centre",
            Distance => "Distance — between two points",
            MatchProperties => "MatchProperties — copy layer and colour from an object",
            OnSel("Hide") => "Hide — hide selected objects",
            OnSel("Isolate") => "Isolate — hide everything else",
            OnSel("Lock") => "Lock — selected objects can be seen and snapped to, not selected",
            OnSel("Group") => "Group — select together",
            OnSel("Ungroup") => "Ungroup",
            OnSel("SelGroup") => "SelGroup — select the whole groups",
            OnSel("Flip") => "Flip — reverse curve direction / surface normals",
            OnSel("ProjectToCPlane") => "ProjectToCPlane — flatten onto the construction plane",
            OnSel("Intersect") => "Intersect — points where curves cross",
            OnSel("PlanarSrf") => "PlanarSrf — surface from closed planar curves",
            OnSel("Loft") => "Loft — surface through curves",
            OnSel("Cap") => "Cap — close planar holes",
            OnSel("Length") => "Length — of curves",
            OnSel("Area") => "Area — of closed curves and surfaces",
            OnSel("Volume") => "Volume — of closed solids",
            OnSel("BoundingBox") => "BoundingBox — box around the selection",
            OnSel("What") => "What — describe the selection",
            OnSel("CopyToClipboard") => "Copy to clipboard (Ctrl+C)",
            OnSel("Cut") => "Cut (Ctrl+X)",
            OnSel("Delete") => "Delete (Del)",
            OnSel(_) => "",
        }
    }

    pub fn from_name(s: &str) -> Option<ToolKind> {
        use ToolKind::*;
        let l = s.to_lowercase();
        Some(match l.as_str() {
            "point" | "pt" => Point,
            "line" | "l" => Line,
            "polyline" | "pl" => Polyline,
            "curve" | "crv" => Curve,
            "interpcrv" | "interp" => InterpCrv,
            "rectangle" | "rec" => Rectangle,
            "circle" | "c" => Circle,
            "arc" => Arc,
            "ellipse" | "el" => Ellipse,
            "polygon" | "pol" => Polygon,
            "box" => Box,
            "cylinder" => Cylinder,
            "sphere" => Sphere,
            "extrude" | "extrudecrv" | "ext" => Extrude,
            "extrudesrf" => ExtrudeSrf,
            "revolve" | "rev" => Revolve,
            "sweep1" => Sweep1,
            "move" | "m" => Move,
            "copy" | "co" | "cp" => Copy,
            "rotate" | "ro" => Rotate,
            "scale" | "sc" => Scale,
            "scale1d" | "s1" => Scale1D,
            "scale2d" | "s2" => Scale2D,
            "mirror" | "mi" => Mirror,
            "orient" | "or" => Orient,
            "offset" | "o" => Offset,
            "trim" | "tr" => Trim,
            "split" => Split,
            "extend" | "ex" => Extend,
            "fillet" | "f" => Fillet,
            "chamfer" | "cha" => Chamfer,
            "filletcorners" | "fc" => FilletCorners,
            "join" | "j" => Join,
            "explode" | "x" => Explode,
            "arraylinear" | "al" => ArrayLinear,
            "arraypolar" | "ap" => ArrayPolar,
            "distance" | "dist" => Distance,
            "matchproperties" | "matchprop" | "ma" => MatchProperties,
            _ => return None,
        })
    }

    /// The short alias typed in the command line (Rhino-style), if any.
    pub fn alias(self) -> Option<&'static str> {
        use ToolKind::*;
        Some(match self {
            Point => "Pt",
            Line => "L",
            Polyline => "PL",
            Curve => "Crv",
            InterpCrv => "Interp",
            Rectangle => "Rec",
            Circle => "C",
            Ellipse => "El",
            Polygon => "Pol",
            Extrude => "Ext",
            Revolve => "Rev",
            Move => "M",
            Copy => "Co",
            Rotate => "Ro",
            Scale => "Sc",
            Scale1D => "S1",
            Scale2D => "S2",
            Mirror => "Mi",
            Orient => "Or",
            Offset => "O",
            Trim => "Tr",
            Extend => "Ex",
            Fillet => "F",
            Chamfer => "Cha",
            FilletCorners => "FC",
            Join => "J",
            Explode => "X",
            ArrayLinear => "AL",
            ArrayPolar => "AP",
            Distance => "Dist",
            MatchProperties => "Ma",
            OnSel("Delete") => "Del",
            OnSel("Flip") => "Dir",
            OnSel("BoundingBox") => "BBox",
            _ => return None,
        })
    }

    /// Keyboard shortcut of the command, if any.
    pub fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            ToolKind::Join => "Ctrl+J",
            ToolKind::OnSel("Hide") => "Ctrl+H",
            ToolKind::OnSel("Lock") => "Ctrl+L",
            ToolKind::OnSel("Group") => "Ctrl+G",
            ToolKind::OnSel("Ungroup") => "Ctrl+Shift+G",
            ToolKind::OnSel("Delete") => "Del",
            ToolKind::OnSel("CopyToClipboard") => "Ctrl+C",
            ToolKind::OnSel("Cut") => "Ctrl+X",
            _ => return None,
        })
    }

    /// The tooltip without the leading "Name — ".
    pub fn description(self) -> &'static str {
        let t = self.tooltip();
        t.split_once(" — ").map_or(t, |(_, d)| d)
    }

    /// A selection command typed by name (exact names only).
    pub fn selection_command(s: &str) -> Option<ToolKind> {
        let l = s.to_lowercase();
        let l = match l.as_str() {
            "dir" => "flip",
            "del" | "erase" => "delete",
            "copyclip" => "copytoclipboard",
            "bbox" => "boundingbox",
            "flatten" => "projecttocplane",
            other => other,
        }
        .to_string();
        SELECTION_COMMANDS
            .iter()
            .find(|n| n.to_lowercase() == l)
            .map(|n| ToolKind::OnSel(n))
    }

    pub fn needs_selection(self) -> bool {
        use ToolKind::*;
        matches!(
            self,
            Extrude
                | ExtrudeSrf
                | Revolve
                | Sweep1
                | Move
                | Copy
                | Rotate
                | Scale
                | Scale1D
                | Scale2D
                | Mirror
                | Orient
                | Offset
                | Trim
                | Split
                | Extend
                | FilletCorners
                | Join
                | Explode
                | ArrayLinear
                | ArrayPolar
                | MatchProperties
                | OnSel(_)
        )
    }

    /// Tools that are a single engine command once objects are selected.
    pub fn instant(self) -> bool {
        matches!(
            self,
            ToolKind::Join | ToolKind::Explode | ToolKind::OnSel(_)
        )
    }
}

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
            phase: 0,
            stash: Vec::new(),
            factor: None,
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
            (OnSel(_), _) => "press Enter",
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
            k => k.name().to_string(),
        }
    }

    /// An object was clicked (rail, property source).
    pub fn feed_object(&mut self, id: u64) -> Step {
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

    #[test]
    fn new_curve_tools_emit_commands() {
        let mut p = Tool::new(ToolKind::Polygon, Plane::TOP, false, Point3::ORIGIN, vec![]);
        assert!(matches!(p.feed_number(6.0), Step::Continue));
        p.feed_point(Point3::ORIGIN);
        let Step::Done(c) = p.feed_point(Point3::new(10.0, 0.0, 0.0)) else {
            panic!()
        };
        assert_eq!(c, vec!["Polygon 0,0,0 10,0,0 6 0,0,1"]);
        let mut e = Tool::new(ToolKind::Ellipse, Plane::TOP, false, Point3::ORIGIN, vec![]);
        e.feed_point(Point3::ORIGIN);
        e.feed_point(Point3::new(30.0, 0.0, 0.0));
        let Step::Done(c) = e.feed_point(Point3::new(5.0, 10.0, 0.0)) else {
            panic!()
        };
        assert_eq!(c, vec!["Ellipse 0,0,0 30,0,0 10 0,0,1"]);
        let mut k = Tool::new(ToolKind::Curve, Plane::TOP, false, Point3::ORIGIN, vec![]);
        for x in [0.0, 1.0, 2.0] {
            k.feed_point(Point3::new(x, x, 0.0));
        }
        let Step::Done(c) = k.enter(false) else {
            panic!()
        };
        assert_eq!(c, vec!["Curve 0,0,0 1,1,0 2,2,0"]);
    }

    #[test]
    fn scale1d_by_reference_and_selection_commands() {
        let mut t = Tool::new(ToolKind::Scale1D, Plane::TOP, true, Point3::ORIGIN, vec![]);
        t.feed_point(Point3::ORIGIN);
        t.feed_point(Point3::new(10.0, 0.0, 0.0));
        let Step::Done(c) = t.feed_point(Point3::new(25.0, 3.0, 0.0)) else {
            panic!()
        };
        assert_eq!(c, vec!["Scale1D 0,0,0 2.5 10,0,0"]);
        assert_eq!(
            ToolKind::selection_command("hide"),
            Some(ToolKind::OnSel("Hide"))
        );
        assert_eq!(
            ToolKind::selection_command("dir"),
            Some(ToolKind::OnSel("Flip"))
        );
        let mut h = Tool::new(
            ToolKind::OnSel("Hide"),
            Plane::TOP,
            false,
            Point3::ORIGIN,
            vec![],
        );
        assert_eq!(h.want(), Want::Selection);
        let Step::Done(c) = h.enter(true) else {
            panic!()
        };
        assert_eq!(c, vec!["Hide"]);
        let f = Tool::new(
            ToolKind::OnSel("ProjectToCPlane"),
            Plane::FRONT,
            true,
            Point3::ORIGIN,
            vec![],
        );
        assert_eq!(f.instant_line(), "ProjectToCPlane 0,-1,0 0,0,0");
    }

    #[test]
    fn prompt_options_follow_the_step() {
        let mut t = Tool::new(
            ToolKind::Polyline,
            Plane::TOP,
            false,
            Point3::ORIGIN,
            vec![],
        );
        assert!(t.options().is_empty());
        t.feed_point(Point3::ORIGIN);
        let labels: Vec<String> = t.options().into_iter().map(|o| o.label).collect();
        assert_eq!(labels, vec!["Undo"]);
        t.feed_point(Point3::new(10.0, 0.0, 0.0));
        t.feed_point(Point3::new(10.0, 10.0, 0.0));
        let opts = t.options();
        assert_eq!(opts[0].action, OptAction::Word("Close"));
        assert_eq!(opts[1].action, OptAction::Word("Undo"));

        let mut o = Tool::new(ToolKind::Offset, Plane::TOP, true, Point3::ORIGIN, vec![]);
        o.distance = 10.0;
        assert_eq!(o.options()[0].label, "Distance=10");
        assert_eq!(o.option_value("distance"), Some(10.0));
        o.set_option("Distance", 2.5).expect("valid");
        assert_eq!(o.options()[0].label, "Distance=2.5");
        assert!(o.set_option("Distance", -1.0).is_err());

        let mut p = Tool::new(ToolKind::Polygon, Plane::TOP, false, Point3::ORIGIN, vec![]);
        assert_eq!(p.options()[0].label, "NumSides=5");
        p.set_option("NumSides", 8.0).expect("valid");
        assert_eq!(p.option_value("NumSides"), Some(8.0));
        assert!(p.set_option("NumSides", 2.0).is_err());
        let f = Tool::new(ToolKind::Fillet, Plane::TOP, false, Point3::ORIGIN, vec![]);
        assert_eq!(f.options()[0].action, OptAction::Value("Radius"));
    }

    #[test]
    fn aliases_round_trip() {
        let kinds = ToolKind::CURVES
            .iter()
            .chain(&ToolKind::CURVE_TOOLS)
            .chain(&ToolKind::SURFACES)
            .chain(&ToolKind::TRANSFORMS)
            .chain(&ToolKind::ANALYZE)
            .chain(&ToolKind::EDIT);
        for k in kinds {
            if let (Some(a), ToolKind::OnSel(_)) = (k.alias(), k) {
                assert_eq!(ToolKind::selection_command(a), Some(*k), "{a}");
            } else if let Some(a) = k.alias() {
                assert_eq!(ToolKind::from_name(a), Some(*k), "{a}");
            }
            assert!(!k.description().is_empty(), "{}", k.name());
        }
    }
}
