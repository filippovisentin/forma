//! The tool kinds: names, aliases, shortcuts, tooltips and how they start.

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
