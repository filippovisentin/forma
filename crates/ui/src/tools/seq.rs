//! Sequence tools: a command described as a list of inputs (points, numbers,
//! text, an object, a choice) and a function that turns the collected values
//! into one engine command line. Most commands added after v0.6 use this
//! instead of a hand-written state machine; the engine stays the only place
//! where modelling happens.

use super::{fmt_p, fmt_v, round};
use forma_geom::{
    arc_3pt, circle_3pt, CircleArc, Deform, DimKind, Dimension, Plane, Point3, Vec3, Xform,
};

/// One input of a sequence tool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum In {
    /// A picked or typed point.
    Point(&'static str),
    /// A distance: a typed number, or a point measured from the point of input
    /// `from` (in the construction plane).
    Dist(&'static str, usize),
    /// A height along the construction-plane normal from the point of input `from`.
    Height(&'static str, usize),
    /// A typed number; Enter takes the default when there is one.
    Num(&'static str, Option<f64>),
    /// A line of text (typed in the command line).
    Text(&'static str),
    /// A clicked object (or a typed `#id`).
    Object(&'static str),
    /// Points until Enter, at least this many.
    Points(&'static str, usize),
    /// One point clicked on the surface of a mesh or solid.
    SurfacePoint(&'static str),
    /// Points clicked on the surface of a solid (near its edges or on its
    /// faces) until Enter, at least this many.
    SurfacePoints(&'static str, usize),
    /// One of a few words, clickable in the prompt.
    Choice(&'static str, &'static [&'static str]),
}

/// A collected value.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    P(Point3),
    N(f64),
    T(String),
    O(u64),
    Ps(Vec<Point3>),
    W(&'static str),
}

impl Val {
    pub fn p(&self) -> Point3 {
        match self {
            Val::P(p) => *p,
            Val::Ps(v) => *v.last().unwrap_or(&Point3::ORIGIN),
            _ => Point3::ORIGIN,
        }
    }
    pub fn n(&self) -> f64 {
        match self {
            Val::N(x) => *x,
            _ => 0.0,
        }
    }
    fn t(&self) -> &str {
        match self {
            Val::T(s) => s,
            Val::W(w) => w,
            _ => "",
        }
    }
    fn o(&self) -> u64 {
        match self {
            Val::O(i) => *i,
            _ => 0,
        }
    }
}

/// What a preview function sees: the values so far, the cursor, the plane and
/// the wireframe of the selection.
pub struct View<'a> {
    pub vals: &'a [Val],
    pub cur: Point3,
    pub plane: &'a Plane,
    pub skeleton: &'a [[Point3; 2]],
}

impl View<'_> {
    fn p(&self, i: usize) -> Point3 {
        self.vals.get(i).map_or(self.cur, Val::p)
    }
    fn xf(&self, x: &Xform) -> Vec<[Point3; 2]> {
        self.skeleton
            .iter()
            .map(|[a, b]| [x.point(*a), x.point(*b)])
            .collect()
    }
}

type Emit = fn(&[Val], &Plane) -> String;
type Preview = fn(&View) -> Vec<[Point3; 2]>;

/// A sequence tool.
pub struct Seq {
    pub name: &'static str,
    /// `Name — what it does`.
    pub tip: &'static str,
    /// Asks for a selection first.
    pub sel: bool,
    pub steps: &'static [In],
    pub emit: Emit,
    pub preview: Preview,
}

impl PartialEq for Seq {
    fn eq(&self, o: &Seq) -> bool {
        self.name == o.name
    }
}
impl Eq for Seq {}
impl std::fmt::Debug for Seq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Seq({})", self.name)
    }
}

/// Rubber band from the last point to the cursor.
fn band(v: &View) -> Vec<[Point3; 2]> {
    let pts: Vec<Point3> = v
        .vals
        .iter()
        .filter_map(|x| match x {
            Val::P(p) => Some(*p),
            _ => None,
        })
        .collect();
    pts.last().map_or(Vec::new(), |p| vec![[*p, v.cur]])
}

fn none(_: &View) -> Vec<[Point3; 2]> {
    Vec::new()
}

fn poly(pts: &[Point3]) -> Vec<[Point3; 2]> {
    pts.windows(2).map(|w| [w[0], w[1]]).collect()
}

fn n(pl: &Plane) -> String {
    fmt_v(pl.z)
}

fn quad(c: [Point3; 4]) -> Vec<[Point3; 2]> {
    poly(&[c[0], c[1], c[2], c[3], c[0]])
}

/// The selection command without inputs.
fn just_name(name: &'static str) -> String {
    name.to_string()
}

macro_rules! sel_only {
    ($id:ident, $name:literal, $tip:literal) => {
        pub static $id: Seq = Seq {
            name: $name,
            tip: $tip,
            sel: true,
            steps: &[],
            emit: |_, _| just_name($name),
            preview: none,
        };
    };
}

// ------------------------------------------------------------------ curves

pub static CIRCLE3PT: Seq = Seq {
    name: "Circle3Pt",
    tip: "Circle3Pt — circle through three points",
    sel: false,
    steps: &[
        In::Point("First point on circle"),
        In::Point("Second point on circle"),
        In::Point("Third point on circle"),
    ],
    emit: |v, _| {
        format!(
            "Circle3Pt {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        2 => circle_3pt(v.p(0), v.p(1), v.cur).map_or_else(|| band(v), |c| poly(&c.points(72))),
        _ => band(v),
    },
};

pub static CIRCLE2PT: Seq = Seq {
    name: "Circle2Pt",
    tip: "Circle2Pt — circle from the two ends of a diameter",
    sel: false,
    steps: &[In::Point("Start of diameter"), In::Point("End of diameter")],
    emit: |v, pl| {
        format!(
            "Circle2Pt {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        1 => {
            let (a, b) = (v.p(0), v.cur);
            let mut out = vec![[a, b]];
            let r = a.distance_to(b) / 2.0;
            if r > 1e-12 {
                out.extend(poly(
                    &CircleArc::circle(v.plane.moved_to(a.mid(b)), r).points(72),
                ));
            }
            out
        }
        _ => Vec::new(),
    },
};

pub static ARC3PT: Seq = Seq {
    name: "Arc3Pt",
    tip: "Arc3Pt — arc from its start and end through a point",
    sel: false,
    steps: &[
        In::Point("Start of arc"),
        In::Point("End of arc"),
        In::Point("Point on arc"),
    ],
    emit: |v, _| {
        format!(
            "Arc3Pt {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        2 => arc_3pt(v.p(0), v.p(1), v.cur).map_or_else(|| band(v), |a| poly(&a.points(72))),
        _ => band(v),
    },
};

/// Rectangle with edge a–b and width towards c (same rule as the engine).
fn rect3(a: Point3, b: Point3, c: Point3) -> Option<[Point3; 4]> {
    let x = (b - a).normalized()?;
    let d = c - b;
    let off = d - x * d.dot(x);
    Some([a, b, b + off, a + off])
}

pub static RECTANGLE3PT: Seq = Seq {
    name: "Rectangle3Pt",
    tip: "Rectangle3Pt — rectangle from one edge and its width",
    sel: false,
    steps: &[
        In::Point("Start of edge"),
        In::Point("End of edge"),
        In::Point("Width"),
    ],
    emit: |v, _| {
        format!(
            "Rectangle3Pt {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        2 => rect3(v.p(0), v.p(1), v.cur).map_or_else(|| band(v), quad),
        _ => band(v),
    },
};

fn rect_center(c: Point3, k: Point3, plane: &Plane) -> [Point3; 4] {
    let pl = plane.moved_to(c);
    let (u, w, _) = pl.coords(k);
    [
        pl.point_at(-u, -w, 0.0),
        pl.point_at(u, -w, 0.0),
        pl.point_at(u, w, 0.0),
        pl.point_at(-u, w, 0.0),
    ]
}

pub static RECTANGLE_CENTER: Seq = Seq {
    name: "RectangleCenter",
    tip: "RectangleCenter — rectangle from its centre and a corner",
    sel: false,
    steps: &[In::Point("Center of rectangle"), In::Point("Corner")],
    emit: |v, pl| {
        format!(
            "RectangleCenter {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        1 => quad(rect_center(v.p(0), v.cur, v.plane)),
        _ => Vec::new(),
    },
};

fn rect2(a: Point3, b: Point3, plane: &Plane) -> [Point3; 4] {
    let pl = plane.moved_to(a);
    let (u, w, _) = pl.coords(b);
    [
        pl.point_at(0.0, 0.0, 0.0),
        pl.point_at(u, 0.0, 0.0),
        pl.point_at(u, w, 0.0),
        pl.point_at(0.0, w, 0.0),
    ]
}

pub static ROUNDED_RECTANGLE: Seq = Seq {
    name: "RoundedRectangle",
    tip: "RoundedRectangle — rectangle with rounded corners",
    sel: false,
    steps: &[
        In::Point("First corner"),
        In::Point("Other corner"),
        In::Num("Corner radius", None),
    ],
    emit: |v, pl| {
        format!(
            "RoundedRectangle {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        1 => quad(rect2(v.p(0), v.cur, v.plane)),
        2 => quad(rect2(v.p(0), v.p(1), v.plane)),
        _ => Vec::new(),
    },
};

pub static SLOT: Seq = Seq {
    name: "Slot",
    tip: "Slot — two half circles joined by straight lines",
    sel: false,
    steps: &[
        In::Point("First centre"),
        In::Point("Second centre"),
        In::Dist("Slot width (or a point on the side)", 1),
    ],
    emit: |v, pl| {
        format!(
            "Slot {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(2.0 * v[2].n().abs()),
            n(pl)
        )
    },
    preview: band,
};

pub static HELIX: Seq = Seq {
    name: "Helix",
    tip: "Helix — around an axis, with radius and number of turns",
    sel: false,
    steps: &[
        In::Point("Start of axis"),
        In::Point("End of axis"),
        In::Dist("Radius", 0),
        In::Num("Number of turns", Some(10.0)),
    ],
    emit: |v, _| {
        format!(
            "Helix {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n().abs()),
            round(v[3].n())
        )
    },
    preview: band,
};

pub static SPIRAL: Seq = Seq {
    name: "Spiral",
    tip: "Spiral — flat spiral from a start to an end radius",
    sel: false,
    steps: &[
        In::Point("Center of spiral"),
        In::Dist("Start radius", 0),
        In::Dist("End radius", 0),
        In::Num("Number of turns", Some(5.0)),
    ],
    emit: |v, pl| {
        format!(
            "Spiral {} {} {} {} 0 {}",
            fmt_p(v[0].p()),
            round(v[1].n().abs()),
            round(v[2].n().abs()),
            round(v[3].n()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        1 | 2 => {
            let r = v.plane.moved_to(v.p(0)).project(v.cur).distance_to(v.p(0));
            poly(&CircleArc::circle(v.plane.moved_to(v.p(0)), r.max(1e-9)).points(72))
        }
        _ => Vec::new(),
    },
};

sel_only!(
    CONVERT,
    "Convert",
    "Convert — curves to polylines within a tolerance"
);
sel_only!(
    CLOSECRV,
    "CloseCrv",
    "CloseCrv — close open curves with a line"
);
sel_only!(
    EXTRACTPT,
    "ExtractPt",
    "ExtractPt — point objects at curve vertices / mesh vertices"
);
sel_only!(
    DUPEDGE,
    "DupEdge",
    "DupEdge — curves along the visible edges of meshes"
);
sel_only!(
    DUPBORDER,
    "DupBorder",
    "DupBorder — curves along the open borders of meshes"
);

pub static DIVIDE: Seq = Seq {
    name: "Divide",
    tip: "Divide — points dividing curves into equal segments",
    sel: true,
    steps: &[In::Num("Number of segments", Some(10.0))],
    emit: |v, _| format!("Divide {}", v[0].n().round()),
    preview: none,
};

pub static DIVIDE_LENGTH: Seq = Seq {
    name: "DivideByLength",
    tip: "DivideByLength — points along curves at a fixed distance",
    sel: true,
    steps: &[In::Num("Segment length", None)],
    emit: |v, _| format!("DivideByLength {}", round(v[0].n())),
    preview: none,
};

pub static REBUILD: Seq = Seq {
    name: "Rebuild",
    tip: "Rebuild — smooth curves with a given number of control points",
    sel: true,
    steps: &[
        In::Num("Point count", Some(10.0)),
        In::Num("Degree", Some(3.0)),
    ],
    emit: |v, _| format!("Rebuild {} {}", v[0].n().round(), v[1].n().round()),
    preview: none,
};

pub static CURVE_BOOLEAN: Seq = Seq {
    name: "CurveBoolean",
    tip: "CurveBoolean — union, intersection or difference of closed planar curves",
    sel: true,
    steps: &[In::Choice(
        "Operation",
        &["Union", "Intersection", "Difference"],
    )],
    emit: |v, _| format!("CurveBoolean {}", v[0].t().to_lowercase()),
    preview: none,
};

/// Contour planes: base point and direction.
pub static CONTOUR: Seq = Seq {
    name: "Contour",
    tip: "Contour — sections of meshes by parallel planes",
    sel: true,
    steps: &[
        In::Point("Contour plane base point"),
        In::Point("Direction perpendicular to the contour planes"),
        In::Num("Distance between contours", None),
    ],
    emit: |v, _| {
        format!(
            "Contour {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n())
        )
    },
    preview: band,
};

pub static SECTION: Seq = Seq {
    name: "Section",
    tip: "Section — section of meshes by a vertical plane through a line",
    sel: true,
    steps: &[In::Point("Start of section"), In::Point("End of section")],
    emit: |v, pl| format!("Section {} {} {}", fmt_p(v[0].p()), fmt_p(v[1].p()), n(pl)),
    preview: band,
};

pub static PROJECT: Seq = Seq {
    name: "Project",
    tip: "Project — curves and points onto meshes, along the construction-plane normal",
    sel: true,
    steps: &[],
    emit: |_, pl| format!("Project {}", fmt_v(-pl.z)),
    preview: none,
};

sel_only!(
    PULL,
    "Pull",
    "Pull — curves and points to the closest points of meshes"
);

// ------------------------------------------------------------- surfaces / solids

pub static PLANE: Seq = Seq {
    name: "Plane",
    tip: "Plane — rectangular planar surface from two corners",
    sel: false,
    steps: &[
        In::Point("First corner of plane"),
        In::Point("Other corner"),
    ],
    emit: |v, pl| format!("Plane {} {} {}", fmt_p(v[0].p()), fmt_p(v[1].p()), n(pl)),
    preview: |v| match v.vals.len() {
        1 => quad(rect2(v.p(0), v.cur, v.plane)),
        _ => Vec::new(),
    },
};

pub static SRFPT: Seq = Seq {
    name: "SrfPt",
    tip: "SrfPt — surface from four corner points",
    sel: false,
    steps: &[
        In::Point("First corner of surface"),
        In::Point("Second corner"),
        In::Point("Third corner"),
        In::Point("Fourth corner"),
    ],
    emit: |v, _| {
        format!(
            "SrfPt {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p()),
            fmt_p(v[3].p())
        )
    },
    preview: |v| {
        let mut pts: Vec<Point3> = v.vals.iter().map(Val::p).collect();
        pts.push(v.cur);
        if pts.len() >= 3 {
            pts.push(pts[0]);
        }
        poly(&pts)
    },
};

sel_only!(
    EDGESRF,
    "EdgeSrf",
    "EdgeSrf — surface from 2, 3 or 4 edge curves"
);

fn circle_at(c: Point3, r: f64, pl: &Plane) -> Vec<[Point3; 2]> {
    poly(&CircleArc::circle(pl.moved_to(c), r.abs().max(1e-9)).points(72))
}

/// Base circle while the radius is picked; base and top while the height is.
fn round_solid(v: &View, top_radius: Option<usize>) -> Vec<[Point3; 2]> {
    let c = v.p(0);
    match v.vals.len() {
        1 => {
            let r = v.plane.moved_to(c).project(v.cur).distance_to(c);
            let mut out = circle_at(c, r, v.plane);
            out.push([c, v.cur]);
            out
        }
        k => {
            let r = v.vals[1].n();
            let mut out = circle_at(c, r, v.plane);
            let h = (v.cur - c).dot(v.plane.z);
            let rt = top_radius.and_then(|i| v.vals.get(i)).map_or(0.0, Val::n);
            if k >= 2 && top_radius.is_none_or(|i| k > i) {
                let tc = c + v.plane.z * h;
                if rt > 1e-9 {
                    out.extend(circle_at(tc, rt, v.plane));
                }
                out.push([c, tc]);
                for a in [0.0, std::f64::consts::PI] {
                    let d = v.plane.x * a.cos() + v.plane.y * a.sin();
                    out.push([c + d * r, tc + d * rt]);
                }
            }
            out
        }
    }
}

pub static CONE: Seq = Seq {
    name: "Cone",
    tip: "Cone — base centre, radius and height",
    sel: false,
    steps: &[
        In::Point("Center of base"),
        In::Dist("Radius", 0),
        In::Height("Height (apex)", 0),
    ],
    emit: |v, pl| {
        format!(
            "Cone {} {} {} {}",
            fmt_p(v[0].p()),
            round(v[1].n().abs()),
            round(v[2].n()),
            n(pl)
        )
    },
    preview: |v| round_solid(v, None),
};

pub static TCONE: Seq = Seq {
    name: "TCone",
    tip: "TCone — truncated cone: base radius, top radius and height",
    sel: false,
    steps: &[
        In::Point("Center of base"),
        In::Dist("Base radius", 0),
        In::Dist("Top radius", 0),
        In::Height("Height", 0),
    ],
    emit: |v, pl| {
        format!(
            "TCone {} {} {} {} {}",
            fmt_p(v[0].p()),
            round(v[1].n().abs()),
            round(v[2].n().abs()),
            round(v[3].n()),
            n(pl)
        )
    },
    preview: |v| round_solid(v, Some(2)),
};

pub static TUBE: Seq = Seq {
    name: "Tube",
    tip: "Tube — hollow cylinder: outer radius, inner radius and height",
    sel: false,
    steps: &[
        In::Point("Center of base"),
        In::Dist("Outer radius", 0),
        In::Dist("Inner radius", 0),
        In::Height("Height", 0),
    ],
    emit: |v, pl| {
        format!(
            "Tube {} {} {} {} {}",
            fmt_p(v[0].p()),
            round(v[1].n().abs()),
            round(v[2].n().abs()),
            round(v[3].n()),
            n(pl)
        )
    },
    preview: |v| round_solid(v, Some(1)),
};

pub static TORUS: Seq = Seq {
    name: "Torus",
    tip: "Torus — centre, major radius and minor radius",
    sel: false,
    steps: &[
        In::Point("Center of torus"),
        In::Dist("Radius", 0),
        In::Num("Second radius", None),
    ],
    emit: |v, pl| {
        format!(
            "Torus {} {} {} {}",
            fmt_p(v[0].p()),
            round(v[1].n().abs()),
            round(v[2].n().abs()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        1 => round_solid(v, None),
        _ => Vec::new(),
    },
};

pub static ELLIPSOID: Seq = Seq {
    name: "Ellipsoid",
    tip: "Ellipsoid — centre and the three radii",
    sel: false,
    steps: &[
        In::Point("Center of ellipsoid"),
        In::Dist("Radius along x", 0),
        In::Dist("Radius along y", 0),
        In::Num("Radius along z", None),
    ],
    emit: |v, pl| {
        format!(
            "Ellipsoid {} {} {} {} {}",
            fmt_p(v[0].p()),
            round(v[1].n().abs()),
            round(v[2].n().abs()),
            round(v[3].n().abs()),
            n(pl)
        )
    },
    preview: band,
};

pub static PYRAMID: Seq = Seq {
    name: "Pyramid",
    tip: "Pyramid — regular polygon base and height",
    sel: false,
    steps: &[
        In::Num("Number of sides", Some(4.0)),
        In::Point("Center of base"),
        In::Point("Corner of base"),
        In::Height("Height", 1),
    ],
    emit: |v, pl| {
        format!(
            "Pyramid {} {} {} {} {}",
            fmt_p(v[1].p()),
            fmt_p(v[2].p()),
            v[0].n().round(),
            round(v[3].n()),
            n(pl)
        )
    },
    preview: band,
};

pub static PIPE: Seq = Seq {
    name: "Pipe",
    tip: "Pipe — round pipe along curves",
    sel: true,
    steps: &[In::Num("Pipe radius", None)],
    emit: |v, _| format!("Pipe {}", round(v[0].n().abs())),
    preview: none,
};

pub static SLAB: Seq = Seq {
    name: "Slab",
    tip: "Slab — walls / slabs from planar curves: thickness and height",
    sel: true,
    steps: &[
        In::Num("Thickness", None),
        In::Num("Height", None),
        In::Choice("Curve at", &["Center", "Left", "Right"]),
    ],
    emit: |v, _| {
        format!(
            "Slab {} {} {}",
            round(v[0].n()),
            round(v[1].n()),
            v[2].t().to_lowercase()
        )
    },
    preview: none,
};

pub static EXTRUDE_TAPERED: Seq = Seq {
    name: "ExtrudeCrvTapered",
    tip: "ExtrudeCrvTapered — extrude closed curves with a draft angle",
    sel: true,
    steps: &[
        In::Num("Extrusion distance", None),
        In::Num("Draft angle", Some(5.0)),
    ],
    emit: |v, _| format!("ExtrudeCrvTapered {} {}", round(v[0].n()), round(v[1].n())),
    preview: none,
};

pub static EXTRUDE_TO_POINT: Seq = Seq {
    name: "ExtrudeCrvToPoint",
    tip: "ExtrudeCrvToPoint — extrude curves to a point (cones, pyramids)",
    sel: true,
    steps: &[In::Point("Point to extrude to")],
    emit: |v, _| format!("ExtrudeCrvToPoint {}", fmt_p(v[0].p())),
    preview: |v| {
        v.skeleton
            .iter()
            .flat_map(|[a, b]| [[*a, v.cur], [*b, v.cur]])
            .collect()
    },
};

pub static EXTRUDE_ALONG: Seq = Seq {
    name: "ExtrudeCrvAlongCrv",
    tip: "ExtrudeCrvAlongCrv — extrude curves along a path curve",
    sel: true,
    steps: &[In::Object("Select the path curve")],
    emit: |v, _| format!("ExtrudeCrvAlongCrv #{}", v[0].o()),
    preview: none,
};

sel_only!(
    WELD,
    "Weld",
    "Weld — smooth mesh shading (hide edges between faces)"
);
sel_only!(
    UNWELD,
    "Unweld",
    "Unweld — flat mesh shading (show edges between faces)"
);

// --------------------------------------------------------------- transforms

pub static ROTATE3D: Seq = Seq {
    name: "Rotate3D",
    tip: "Rotate3D — rotate around an axis through two points",
    sel: true,
    steps: &[
        In::Point("Start of rotation axis"),
        In::Point("End of rotation axis"),
        In::Num("Angle", None),
    ],
    emit: |v, _| {
        format!(
            "Rotate3D {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n())
        )
    },
    preview: band,
};

pub static MIRROR3PT: Seq = Seq {
    name: "Mirror3Pt",
    tip: "Mirror3Pt — mirrored copies across a plane through three points",
    sel: true,
    steps: &[
        In::Point("First point of mirror plane"),
        In::Point("Second point"),
        In::Point("Third point"),
    ],
    emit: |v, _| {
        format!(
            "Mirror3Pt {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        2 => {
            let (a, b) = (v.p(0), v.p(1));
            let mut out = vec![[a, b], [b, v.cur], [v.cur, a]];
            if let Some(nrm) = (b - a).cross(v.cur - a).normalized() {
                out.extend(v.xf(&Xform::mirror(a, nrm)));
            }
            out
        }
        _ => band(v),
    },
};

pub static SHEAR: Seq = Seq {
    name: "Shear",
    tip: "Shear — slant the selection by an angle",
    sel: true,
    steps: &[
        In::Point("Origin point"),
        In::Point("Reference point"),
        In::Num("Shear angle", None),
    ],
    emit: |v, pl| {
        format!(
            "Shear {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n()),
            n(pl)
        )
    },
    preview: band,
};

pub static SCALENU: Seq = Seq {
    name: "ScaleNU",
    tip: "ScaleNU — different scale factors along x, y and z",
    sel: true,
    steps: &[
        In::Point("Origin point"),
        In::Num("Scale factor x", Some(1.0)),
        In::Num("Scale factor y", Some(1.0)),
        In::Num("Scale factor z", Some(1.0)),
    ],
    emit: |v, _| {
        format!(
            "ScaleNU {} {} {} {}",
            fmt_p(v[0].p()),
            round(v[1].n()),
            round(v[2].n()),
            round(v[3].n())
        )
    },
    preview: none,
};

pub static SETPT: Seq = Seq {
    name: "SetPt",
    tip: "SetPt — line the selection up on chosen coordinates of a point",
    sel: true,
    steps: &[
        In::Choice("Set", &["X", "Y", "Z", "XY", "XYZ"]),
        In::Point("Location"),
    ],
    emit: |v, _| {
        let axes: Vec<String> = v[0]
            .t()
            .chars()
            .map(|c| c.to_ascii_lowercase().to_string())
            .collect();
        format!("SetPt {} {}", fmt_p(v[1].p()), axes.join(" "))
    },
    preview: none,
};

pub static ORIENT3PT: Seq = Seq {
    name: "Orient3Pt",
    tip: "Orient3Pt — three reference points onto three target points",
    sel: true,
    steps: &[
        In::Point("Reference point 1"),
        In::Point("Reference point 2"),
        In::Point("Reference point 3"),
        In::Point("Target point 1"),
        In::Point("Target point 2"),
        In::Point("Target point 3"),
    ],
    emit: |v, _| {
        let p: Vec<String> = v.iter().map(|x| fmt_p(x.p())).collect();
        format!("Orient3Pt {}", p.join(" "))
    },
    preview: |v| {
        let k = v.vals.len();
        let mut out = band(v);
        if k >= 3 {
            out.push([v.p(0), v.p(1)]);
            out.push([v.p(1), v.p(2)]);
        }
        if k == 5 {
            if let Some(x) = Xform::orient3([v.p(0), v.p(1), v.p(2)], [v.p(3), v.p(4), v.cur]) {
                out.extend(v.xf(&x));
            }
        }
        out
    },
};

pub static ARRAYCRV: Seq = Seq {
    name: "ArrayCrv",
    tip: "ArrayCrv — copies spaced along a curve",
    sel: true,
    steps: &[
        In::Object("Select the path curve"),
        In::Num("Number of items", Some(5.0)),
    ],
    emit: |v, _| format!("ArrayCrv #{} {}", v[0].o(), v[1].n().round()),
    preview: none,
};

pub static DISTRIBUTE: Seq = Seq {
    name: "Distribute",
    tip: "Distribute — space objects evenly along an axis",
    sel: true,
    steps: &[In::Choice("Axis", &["X", "Y", "Z"])],
    emit: |v, _| format!("Distribute {}", v[0].t().to_lowercase()),
    preview: none,
};

fn deform_preview(v: &View, d: Deform) -> Vec<[Point3; 2]> {
    v.skeleton
        .iter()
        .flat_map(|[a, b]| {
            let pts: Vec<Point3> = forma_geom::densify(&[*a, *b], a.distance_to(*b) / 8.0)
                .into_iter()
                .map(|p| d.point(p))
                .collect();
            poly(&pts)
        })
        .collect()
}

pub static BEND: Seq = Seq {
    name: "Bend",
    tip: "Bend — bend the selection along a spine",
    sel: true,
    steps: &[
        In::Point("Start of spine"),
        In::Point("End of spine"),
        In::Point("Point to bend through"),
    ],
    emit: |v, _| {
        format!(
            "Bend {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        2 => {
            let mut out = vec![[v.p(0), v.p(1)]];
            out.extend(deform_preview(
                v,
                Deform::Bend {
                    start: v.p(0),
                    end: v.p(1),
                    through: v.cur,
                },
            ));
            out
        }
        _ => band(v),
    },
};

pub static TWIST: Seq = Seq {
    name: "Twist",
    tip: "Twist — twist the selection around an axis",
    sel: true,
    steps: &[
        In::Point("Start of twist axis"),
        In::Point("End of twist axis"),
        In::Num("Twist angle", None),
    ],
    emit: |v, _| {
        format!(
            "Twist {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n())
        )
    },
    preview: band,
};

pub static TAPER: Seq = Seq {
    name: "Taper",
    tip: "Taper — scale gradually along an axis",
    sel: true,
    steps: &[
        In::Point("Start of taper axis"),
        In::Point("End of taper axis"),
        In::Num("Start factor", Some(1.0)),
        In::Num("End factor", None),
    ],
    emit: |v, _| {
        format!(
            "Taper {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            round(v[2].n()),
            round(v[3].n())
        )
    },
    preview: band,
};

// --------------------------------------------------------------- annotation

pub static TEXT: Seq = Seq {
    name: "Text",
    tip: "Text — single-line text in the construction plane",
    sel: false,
    steps: &[In::Point("Text location"), In::Text("Text")],
    emit: |v, pl| format!("Text {} * {} {}", fmt_p(v[0].p()), n(pl), v[1].t()),
    preview: none,
};

pub static DOT: Seq = Seq {
    name: "Dot",
    tip: "Dot — text dot: a label at a point that always faces you",
    sel: false,
    steps: &[In::Point("Dot location"), In::Text("Dot text")],
    emit: |v, _| format!("Dot {} {}", fmt_p(v[0].p()), v[1].t()),
    preview: none,
};

/// Preview dimension text height: a tenth of the measured size.
fn dim_h(len: f64) -> f64 {
    (len / 10.0).max(1e-6)
}

pub static DIM: Seq = Seq {
    name: "Dim",
    tip: "Dim — horizontal or vertical dimension",
    sel: false,
    steps: &[
        In::Point("First dimension point"),
        In::Point("Second dimension point"),
        In::Point("Dimension line location"),
    ],
    emit: |v, pl| {
        format!(
            "Dim {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        2 => {
            let (a, b) = (v.p(0), v.p(1));
            let pl = v.plane.moved_to(a);
            Dimension::linear(&pl, a, b, v.cur, dim_h(a.distance_to(b)), 0).lines()
        }
        _ => band(v),
    },
};

pub static DIM_ALIGNED: Seq = Seq {
    name: "DimAligned",
    tip: "DimAligned — dimension parallel to two points",
    sel: false,
    steps: &[
        In::Point("First dimension point"),
        In::Point("Second dimension point"),
        In::Point("Dimension line location"),
    ],
    emit: |v, pl| {
        format!(
            "DimAligned {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p()),
            n(pl)
        )
    },
    preview: |v| match v.vals.len() {
        2 => {
            let (a, b) = (v.p(0), v.p(1));
            Dimension::aligned(v.plane.z, a, b, v.cur, dim_h(a.distance_to(b)), 0)
                .map_or_else(|| band(v), |d| d.lines())
        }
        _ => band(v),
    },
};

pub static DIM_ANGLE: Seq = Seq {
    name: "DimAngle",
    tip: "DimAngle — angle between two lines from a vertex",
    sel: false,
    steps: &[
        In::Point("Vertex of angle"),
        In::Point("Point on first line"),
        In::Point("Point on second line"),
        In::Point("Dimension arc location"),
    ],
    emit: |v, _| {
        format!(
            "DimAngle {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p()),
            fmt_p(v[3].p())
        )
    },
    preview: |v| match v.vals.len() {
        1 => vec![[v.p(0), v.cur]],
        2 => vec![[v.p(0), v.p(1)], [v.p(0), v.cur]],
        3 => {
            let (c, a, b) = (v.p(0), v.p(1), v.p(2));
            let nrm = (a - c).cross(b - c).normalized().map_or(Vec3::Z, |n| {
                if n.dot(Vec3::Z) < 0.0 {
                    -n
                } else {
                    n
                }
            });
            let d = Dimension {
                kind: DimKind::Angle,
                plane: Plane::from_normal(c, nrm),
                points: vec![c, a, b, v.cur],
                text: None,
                height: dim_h(c.distance_to(v.cur)),
                decimals: 0,
            };
            let mut out = vec![[c, a], [c, b]];
            out.extend(d.lines());
            out
        }
        _ => Vec::new(),
    },
};

pub static DIM_RADIUS: Seq = Seq {
    name: "DimRadius",
    tip: "DimRadius — radius of an arc or circle",
    sel: false,
    steps: &[
        In::Object("Select an arc or circle"),
        In::Point("Dimension location"),
    ],
    emit: |v, _| format!("DimRadius {} #{}", fmt_p(v[1].p()), v[0].o()),
    preview: none,
};

pub static DIM_DIAMETER: Seq = Seq {
    name: "DimDiameter",
    tip: "DimDiameter — diameter of an arc or circle",
    sel: false,
    steps: &[
        In::Object("Select an arc or circle"),
        In::Point("Dimension location"),
    ],
    emit: |v, _| format!("DimDiameter {} #{}", fmt_p(v[1].p()), v[0].o()),
    preview: none,
};

pub static LEADER: Seq = Seq {
    name: "Leader",
    tip: "Leader — arrow with a text",
    sel: false,
    steps: &[
        In::Points("Point of the leader (Enter when done)", 2),
        In::Text("Leader text"),
    ],
    emit: |v, _| {
        let pts: Vec<String> = match &v[0] {
            Val::Ps(p) => p.iter().map(|q| fmt_p(*q)).collect(),
            _ => Vec::new(),
        };
        format!("Leader {} {}", pts.join(" "), v[1].t())
    },
    preview: none,
};

pub static HATCH: Seq = Seq {
    name: "Hatch",
    tip: "Hatch — parallel lines inside closed planar curves",
    sel: true,
    steps: &[
        In::Num("Hatch spacing", None),
        In::Num("Hatch angle", Some(45.0)),
    ],
    emit: |v, _| format!("Hatch {} {}", round(v[0].n()), round(v[1].n())),
    preview: none,
};

// ----------------------------------------------------------------- analysis

pub static ANGLE: Seq = Seq {
    name: "Angle",
    tip: "Angle — between two directions from a vertex",
    sel: false,
    steps: &[
        In::Point("Vertex"),
        In::Point("Point on first line"),
        In::Point("Point on second line"),
    ],
    emit: |v, _| {
        format!(
            "Angle {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        2 => vec![[v.p(0), v.p(1)], [v.p(0), v.cur]],
        _ => band(v),
    },
};

pub static EVALUATE_PT: Seq = Seq {
    name: "EvaluatePt",
    tip: "EvaluatePt — coordinates of a point",
    sel: false,
    steps: &[In::Point("Point to evaluate")],
    emit: |v, _| format!("EvaluatePt {}", fmt_p(v[0].p())),
    preview: none,
};

sel_only!(RADIUS, "Radius", "Radius — of arcs and circles");
sel_only!(
    AREA_CENTROID,
    "AreaCentroid",
    "AreaCentroid — point at the area centroid"
);
sel_only!(
    VOLUME_CENTROID,
    "VolumeCentroid",
    "VolumeCentroid — point at the volume centroid"
);
sel_only!(
    CHANGE_TO_CURRENT,
    "ChangeToCurrentLayer",
    "ChangeToCurrentLayer — move objects to the current layer"
);

// ------------------------------------------------------------------ solid kernel

fn pts_list(v: &Val) -> String {
    match v {
        Val::Ps(p) => p.iter().map(|q| fmt_p(*q)).collect::<Vec<_>>().join(" "),
        other => fmt_p(other.p()),
    }
}

sel_only!(
    BOOLEAN_UNION,
    "BooleanUnion",
    "BooleanUnion — merge the selected solids into one"
);
sel_only!(
    BOOLEAN_INTERSECTION,
    "BooleanIntersection",
    "BooleanIntersection — keep only the common part of the selected solids"
);

pub static BOOLEAN_DIFFERENCE: Seq = Seq {
    name: "BooleanDifference",
    tip: "BooleanDifference — subtract solids from a base solid",
    sel: false,
    steps: &[
        In::Object("Select the solid to subtract from"),
        In::Object("Select the solid to subtract with"),
    ],
    emit: |v, _| format!("BooleanDifference #{} #{}", v[0].o(), v[1].o()),
    preview: none,
};

pub static BOOLEAN_SPLIT: Seq = Seq {
    name: "BooleanSplit",
    tip: "BooleanSplit — cut a solid in the parts inside and outside another",
    sel: false,
    steps: &[
        In::Object("Select the solid to split"),
        In::Object("Select the cutting solid"),
    ],
    emit: |v, _| format!("BooleanSplit #{} #{}", v[0].o(), v[1].o()),
    preview: none,
};

pub static FILLET_EDGE: Seq = Seq {
    name: "FilletEdge",
    tip: "FilletEdge — round edges of a solid",
    sel: false,
    steps: &[
        In::Object("Select the solid"),
        In::Num("Fillet radius", Some(1.0)),
        In::SurfacePoints("Click near the edges to fillet (Enter when done)", 1),
    ],
    emit: |v, _| {
        format!(
            "FilletEdge #{} {} {}",
            v[0].o(),
            round(v[1].n()),
            pts_list(&v[2])
        )
    },
    preview: none,
};

pub static CHAMFER_EDGE: Seq = Seq {
    name: "ChamferEdge",
    tip: "ChamferEdge — bevel edges of a solid",
    sel: false,
    steps: &[
        In::Object("Select the solid"),
        In::Num("Chamfer distance", Some(1.0)),
        In::SurfacePoints("Click near the edges to chamfer (Enter when done)", 1),
    ],
    emit: |v, _| {
        format!(
            "ChamferEdge #{} {} {}",
            v[0].o(),
            round(v[1].n()),
            pts_list(&v[2])
        )
    },
    preview: none,
};

pub static SHELL: Seq = Seq {
    name: "Shell",
    tip: "Shell — hollow a solid, removing the clicked faces",
    sel: false,
    steps: &[
        In::Object("Select the solid"),
        In::Num("Wall thickness", Some(1.0)),
        In::SurfacePoints("Click the faces to remove (Enter when done)", 1),
    ],
    emit: |v, _| {
        format!(
            "Shell #{} {} {}",
            v[0].o(),
            round(v[1].n()),
            pts_list(&v[2])
        )
    },
    preview: none,
};

pub static OFFSET_SRF: Seq = Seq {
    name: "OffsetSrf",
    tip: "OffsetSrf — grow or shrink the selected solids (negative = inwards)",
    sel: true,
    steps: &[In::Num("Offset distance (negative = inwards)", Some(1.0))],
    emit: |v, _| format!("OffsetSrf {}", round(v[0].n())),
    preview: none,
};

pub static LINES: Seq = Seq {
    name: "Lines",
    tip: "Lines — chain of separate line segments",
    sel: false,
    steps: &[In::Points("Next point (Enter when done)", 2)],
    emit: |v, _| format!("Lines {}", pts_list(&v[0])),
    preview: |v| {
        let mut pts = match v.vals.first() {
            Some(Val::Ps(p)) => p.clone(),
            _ => Vec::new(),
        };
        if !pts.is_empty() {
            pts.push(v.cur);
        }
        poly(&pts)
    },
};

pub static STRETCH: Seq = Seq {
    name: "Stretch",
    tip: "Stretch — move the points inside a window (selection, or everything visible)",
    sel: false,
    steps: &[
        In::Point("First corner of the stretch window"),
        In::Point("Opposite corner"),
        In::Point("Point to stretch from"),
        In::Point("Point to stretch to"),
    ],
    emit: |v, _| {
        format!(
            "Stretch {} {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p()),
            fmt_p(v[3].p())
        )
    },
    preview: |v| {
        let rect = |a: Point3, b: Point3, pl: &Plane| {
            let (u, w) = (pl.x, pl.y);
            let d = b - a;
            let du = u * d.dot(u);
            let dw = w * d.dot(w);
            quad([a, a + du, a + du + dw, a + dw])
        };
        match v.vals.len() {
            0 => Vec::new(),
            1 => rect(v.p(0), v.cur, v.plane),
            2 => rect(v.p(0), v.p(1), v.plane),
            _ => {
                let mut out = rect(v.p(0), v.p(1), v.plane);
                out.push([v.p(2), v.cur]);
                out
            }
        }
    },
};

pub static CLOSEST_PT: Seq = Seq {
    name: "ClosestPt",
    tip: "ClosestPt — point on the selected objects nearest to a picked point",
    sel: true,
    steps: &[In::Point("Point to measure from")],
    emit: |v, _| format!("ClosestPt {}", fmt_p(v[0].p())),
    preview: none,
};

pub static DUP_FACE_BORDER: Seq = Seq {
    name: "DupFaceBorder",
    tip: "DupFaceBorder — copy the outline of a flat face of a solid",
    sel: false,
    steps: &[
        In::Object("Solid or mesh"),
        In::SurfacePoint("Click the face"),
    ],
    emit: |v, _| format!("DupFaceBorder #{} {}", v[0].o(), fmt_p(v[1].p())),
    preview: none,
};

sel_only!(
    UNIFY_MESH_NORMALS,
    "UnifyMeshNormals",
    "UnifyMeshNormals — make all faces of the selected meshes point the same way"
);

pub static BLOCK: Seq = Seq {
    name: "Block",
    tip: "Block — group the selected objects as a named block",
    sel: true,
    steps: &[In::Text("Block name")],
    emit: |v, _| format!("Block {}", v[0].t()),
    preview: none,
};

pub static INSERT: Seq = Seq {
    name: "Insert",
    tip: "Insert — place a copy of a block",
    sel: false,
    steps: &[
        In::Text("Block name"),
        In::Point("Insertion point"),
        In::Num("Scale", Some(1.0)),
        In::Num("Rotation angle", Some(0.0)),
    ],
    emit: |v, _| {
        format!(
            "Insert {} {} {} {}",
            v[0].t(),
            fmt_p(v[1].p()),
            round(v[2].n()),
            round(v[3].n())
        )
    },
    preview: none,
};

pub static EDIT_TEXT: Seq = Seq {
    name: "EditText",
    tip: "EditText — change the text of a text, dot or dimension",
    sel: false,
    steps: &[In::Object("Text, dot or dimension"), In::Text("New text")],
    emit: |v, _| format!("EditText #{} {}", v[0].o(), v[1].t()),
    preview: none,
};

pub static SEL_BOUNDARY: Seq = Seq {
    name: "SelBoundary",
    tip: "SelBoundary — select what lies inside a closed curve",
    sel: false,
    steps: &[In::Object("Closed boundary curve")],
    emit: |v, _| format!("SelBoundary #{}", v[0].o()),
    preview: none,
};

pub static CPLANE: Seq = Seq {
    name: "CPlane",
    tip: "CPlane — construction plane through three points",
    sel: false,
    steps: &[
        In::Point("CPlane origin"),
        In::Point("X axis direction"),
        In::Point("Point on the plane (Y side)"),
    ],
    emit: |v, _| {
        format!(
            "CPlane {} {} {}",
            fmt_p(v[0].p()),
            fmt_p(v[1].p()),
            fmt_p(v[2].p())
        )
    },
    preview: |v| match v.vals.len() {
        1 => vec![[v.p(0), v.cur]],
        2 => vec![[v.p(0), v.p(1)], [v.p(0), v.cur]],
        _ => Vec::new(),
    },
};

pub static CPLANE_ORIGIN: Seq = Seq {
    name: "CPlaneOrigin",
    tip: "CPlaneOrigin — move the construction plane",
    sel: false,
    steps: &[In::Point("New CPlane origin")],
    emit: |v, _| format!("CPlane {}", fmt_p(v[0].p())),
    preview: none,
};

pub static CPLANE_FACE: Seq = Seq {
    name: "CPlaneFace",
    tip: "CPlaneFace — construction plane on a flat face of a solid",
    sel: false,
    steps: &[
        In::Object("Solid or mesh"),
        In::SurfacePoint("Click the face"),
    ],
    emit: |v, _| format!("CPlane Face #{} {}", v[0].o(), fmt_p(v[1].p())),
    preview: none,
};

pub static MAKE2D: Seq = Seq {
    name: "Make2D",
    tip: "Make2D — flat hidden-line drawing (plan or elevation) of the selection",
    sel: true,
    steps: &[
        In::Choice("View", &["Top", "Front", "Right", "Back", "Left", "Bottom"]),
        In::Choice("Hidden lines", &["No", "Yes"]),
    ],
    emit: |v, _| {
        let hidden = if v[1].t() == "Yes" { " hidden" } else { "" };
        format!("Make2D {}{hidden}", v[0].t().to_lowercase())
    },
    preview: none,
};

/// Every sequence tool (for typed names, help and tests).
pub static ALL: &[&Seq] = &[
    &MAKE2D,
    &CPLANE,
    &CPLANE_ORIGIN,
    &CPLANE_FACE,
    &EDIT_TEXT,
    &SEL_BOUNDARY,
    &BLOCK,
    &INSERT,
    &LINES,
    &STRETCH,
    &CLOSEST_PT,
    &DUP_FACE_BORDER,
    &UNIFY_MESH_NORMALS,
    &BOOLEAN_UNION,
    &BOOLEAN_DIFFERENCE,
    &BOOLEAN_INTERSECTION,
    &BOOLEAN_SPLIT,
    &FILLET_EDGE,
    &CHAMFER_EDGE,
    &SHELL,
    &OFFSET_SRF,
    &CIRCLE3PT,
    &CIRCLE2PT,
    &ARC3PT,
    &RECTANGLE3PT,
    &RECTANGLE_CENTER,
    &ROUNDED_RECTANGLE,
    &SLOT,
    &HELIX,
    &SPIRAL,
    &CONVERT,
    &CLOSECRV,
    &EXTRACTPT,
    &DUPEDGE,
    &DUPBORDER,
    &DIVIDE,
    &DIVIDE_LENGTH,
    &REBUILD,
    &CURVE_BOOLEAN,
    &CONTOUR,
    &SECTION,
    &PROJECT,
    &PULL,
    &PLANE,
    &SRFPT,
    &EDGESRF,
    &CONE,
    &TCONE,
    &TUBE,
    &TORUS,
    &ELLIPSOID,
    &PYRAMID,
    &PIPE,
    &SLAB,
    &EXTRUDE_TAPERED,
    &EXTRUDE_ALONG,
    &EXTRUDE_TO_POINT,
    &WELD,
    &UNWELD,
    &ROTATE3D,
    &MIRROR3PT,
    &SHEAR,
    &SCALENU,
    &SETPT,
    &ARRAYCRV,
    &ORIENT3PT,
    &DISTRIBUTE,
    &BEND,
    &TWIST,
    &TAPER,
    &TEXT,
    &DOT,
    &DIM,
    &DIM_ALIGNED,
    &DIM_ANGLE,
    &DIM_RADIUS,
    &DIM_DIAMETER,
    &LEADER,
    &HATCH,
    &ANGLE,
    &EVALUATE_PT,
    &RADIUS,
    &AREA_CENTROID,
    &VOLUME_CENTROID,
    &CHANGE_TO_CURRENT,
];

/// The sequence tool with this name (case-insensitive).
pub fn by_name(name: &str) -> Option<&'static Seq> {
    ALL.iter()
        .copied()
        .find(|s| s.name.eq_ignore_ascii_case(name))
}
