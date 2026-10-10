//! The Forma document: objects, layers and undo/redo history.
//!
//! Every mutation goes through a [`Transaction`], which records the changes so
//! they can be undone as one step.

use forma_geom::{
    BoundingBox, Chain, CircleArc, Dimension, Label, LineCurve, Mesh, NurbsCurve, Point3, Seg,
    Text, Vec3, Xform,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Stable identifier of an object inside a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

/// Index of a layer in [`Document::layers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub usize);

/// Geometry an object can carry. Grows with each milestone (curves, surfaces, breps).
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    Line(LineCurve),
    /// Open or closed polyline (also used to display imported curves).
    Polyline(Vec<Point3>),
    /// Circle or arc.
    Arc(CircleArc),
    /// Chain of lines and arcs (result of Join, Fillet, Offset of mixed curves).
    PolyCurve(Vec<Seg>),
    /// Triangle mesh (also the display form of imported breps and extrusions
    /// until the solid kernel lands, see ADR 0001). "Surfaces" made by PlanarSrf,
    /// Loft, Revolve, Sweep1… are meshes too until NURBS surfaces exist.
    Mesh(Mesh),
    /// A point object (not a curve).
    Point(Point3),
    /// NURBS curve (free-form curves, ellipses, arcs after a non-uniform scale).
    Nurbs(NurbsCurve),
    /// Text or text dot (annotation, not a curve).
    Text(Text),
    /// Dimension (annotation, not a curve).
    Dimension(Dimension),
}

impl Geometry {
    pub fn kind(&self) -> &'static str {
        match self {
            Geometry::Line(_) => "line",
            Geometry::Polyline(_) => "polyline",
            Geometry::Arc(a) if a.is_closed() => "circle",
            Geometry::Arc(_) => "arc",
            Geometry::PolyCurve(_) => "polycurve",
            Geometry::Mesh(_) => "mesh",
            Geometry::Point(_) => "point",
            Geometry::Nurbs(_) => "nurbs",
            Geometry::Text(t) if t.dot => "textdot",
            Geometry::Text(_) => "text",
            Geometry::Dimension(_) => "dimension",
        }
    }

    pub fn is_curve(&self) -> bool {
        !matches!(
            self,
            Geometry::Mesh(_) | Geometry::Point(_) | Geometry::Text(_) | Geometry::Dimension(_)
        )
    }

    /// True for text, text dots and dimensions.
    pub fn is_annotation(&self) -> bool {
        matches!(self, Geometry::Text(_) | Geometry::Dimension(_))
    }

    /// Line segments to draw for an annotation (dimension, extension and arrow
    /// lines); empty for text and for every other geometry.
    pub fn annotation_lines(&self) -> Vec<[Point3; 2]> {
        match self {
            Geometry::Dimension(d) => d.lines(),
            _ => Vec::new(),
        }
    }

    /// Text label of an annotation with its placement (anchor, reading and up
    /// directions). See [`Label`] for the anchor convention.
    pub fn annotation_label(&self) -> Option<Label> {
        match self {
            Geometry::Text(t) => Some(t.label()),
            Geometry::Dimension(d) => Some(d.label()),
            _ => None,
        }
    }

    /// Text of an annotation: `(position, text, height)`. For dimensions the
    /// position is the bottom centre of the text, for text objects the bottom-left
    /// corner, for dots the dot point.
    pub fn label(&self) -> Option<(Point3, String, f64)> {
        self.annotation_label()
            .map(|l| (l.position, l.text, l.height))
    }

    /// True for closed curves (closed polylines and circles).
    pub fn is_closed_curve(&self) -> bool {
        match self {
            Geometry::Polyline(p) => {
                p.len() > 3 && p[0].distance_to(*p.last().expect("len")) < 1e-9
            }
            Geometry::Arc(a) => a.is_closed(),
            Geometry::PolyCurve(s) => {
                s.len() > 1 && s[0].start().distance_to(s[s.len() - 1].end()) < 1e-9
            }
            Geometry::Nurbs(n) => n.is_closed(),
            _ => false,
        }
    }

    /// The curve as a chain of line/arc segments (None for meshes and points).
    /// NURBS curves become their display polyline (an approximation).
    pub fn to_chain(&self) -> Option<Chain> {
        match self {
            Geometry::Line(l) => Some(Chain::new(vec![Seg::Line(l.from, l.to)])),
            Geometry::Polyline(p) => Some(Chain::from_points(p)),
            Geometry::Arc(a) => Some(Chain::new(vec![Seg::Arc(*a)])),
            Geometry::PolyCurve(s) => Some(Chain::new(s.clone())),
            Geometry::Nurbs(n) => Some(Chain::from_points(&n.points())),
            Geometry::Mesh(_) | Geometry::Point(_) | Geometry::Text(_) | Geometry::Dimension(_) => {
                None
            }
        }
    }

    /// The simplest geometry for a chain: line, arc, polyline or polycurve.
    pub fn from_chain(c: Chain) -> Option<Geometry> {
        let segs = c.segs;
        match segs.as_slice() {
            [] => None,
            [Seg::Line(a, b)] => Some(Geometry::Line(LineCurve::new(*a, *b))),
            [Seg::Arc(a)] => Some(Geometry::Arc(*a)),
            s if s.iter().all(|x| matches!(x, Seg::Line(..))) => {
                let mut pts = vec![s[0].start()];
                pts.extend(s.iter().map(Seg::end));
                Some(Geometry::Polyline(pts))
            }
            _ => Some(Geometry::PolyCurve(segs)),
        }
    }

    /// Points of a curve as a polyline (arcs and NURBS sampled). Empty for meshes
    /// and points. For annotations: an outline used for picking and bounding boxes
    /// (text box, dimension line path), not something to draw.
    pub fn curve_points(&self) -> Vec<Point3> {
        match self {
            Geometry::Line(l) => vec![l.from, l.to],
            Geometry::Polyline(p) => p.clone(),
            Geometry::Arc(a) => a.points(96),
            Geometry::PolyCurve(s) => Chain::new(s.clone()).points(),
            Geometry::Nurbs(n) => n.points(),
            Geometry::Text(t) => t.outline(),
            Geometry::Dimension(d) => d.outline(),
            Geometry::Mesh(_) | Geometry::Point(_) => Vec::new(),
        }
    }

    /// Length of a curve (`None` for meshes and points).
    pub fn length(&self) -> Option<f64> {
        match self {
            Geometry::Line(l) => Some(l.length()),
            Geometry::Polyline(p) => Some(p.windows(2).map(|w| w[0].distance_to(w[1])).sum()),
            Geometry::Arc(a) => Some(a.radius * a.sweep),
            Geometry::PolyCurve(s) => Some(s.iter().map(Seg::length).sum()),
            Geometry::Nurbs(n) => Some(n.length()),
            Geometry::Mesh(_) | Geometry::Point(_) | Geometry::Text(_) | Geometry::Dimension(_) => {
                None
            }
        }
    }

    /// Same curve running the other way; meshes get flipped normals and winding;
    /// points are unchanged.
    pub fn reversed(&self) -> Geometry {
        match self {
            Geometry::Line(l) => Geometry::Line(LineCurve::new(l.to, l.from)),
            Geometry::Polyline(p) => Geometry::Polyline(p.iter().rev().copied().collect()),
            Geometry::Arc(a) => match Seg::Arc(*a).reversed() {
                Seg::Arc(r) => Geometry::Arc(r),
                Seg::Line(..) => unreachable!("arc reverses to an arc"),
            },
            Geometry::PolyCurve(s) => Geometry::PolyCurve(Chain::new(s.clone()).reversed().segs),
            Geometry::Nurbs(n) => Geometry::Nurbs(n.reversed()),
            Geometry::Mesh(m) => Geometry::Mesh(m.flipped()),
            Geometry::Point(p) => Geometry::Point(*p),
            Geometry::Text(_) | Geometry::Dimension(_) => self.clone(),
        }
    }

    /// Transformed copy. Any affine map works: under a non-uniform map (or a
    /// projection) circles and arcs become exact rational NURBS curves, and so do
    /// polycurves that contain arcs.
    pub fn transformed(&self, x: &Xform) -> Geometry {
        let similar = x.is_similarity();
        match self {
            Geometry::Line(l) => Geometry::Line(LineCurve::new(x.point(l.from), x.point(l.to))),
            Geometry::Polyline(p) => Geometry::Polyline(p.iter().map(|q| x.point(*q)).collect()),
            Geometry::Point(p) => Geometry::Point(x.point(*p)),
            Geometry::Nurbs(n) => Geometry::Nurbs(n.transformed(x)),
            Geometry::Text(t) => Geometry::Text(t.transformed(x)),
            Geometry::Dimension(d) => Geometry::Dimension(d.transformed(x)),
            Geometry::Arc(a) if !similar => Geometry::Nurbs(NurbsCurve::from_arc(a).transformed(x)),
            Geometry::PolyCurve(s) if !similar && s.iter().any(|g| matches!(g, Seg::Arc(_))) => {
                match NurbsCurve::from_segs(s) {
                    Some(n) => Geometry::Nurbs(n.transformed(x)),
                    None => Geometry::PolyCurve(Vec::new()),
                }
            }
            Geometry::Arc(a) => Geometry::Arc(a.transformed(x)),
            Geometry::PolyCurve(s) => Geometry::PolyCurve(
                s.iter()
                    .map(|g| match g {
                        Seg::Line(a, b) => Seg::Line(x.point(*a), x.point(*b)),
                        Seg::Arc(a) => {
                            let t = a.transformed(x);
                            // A mirror re-bases the arc so it runs the other way.
                            if x.flips() {
                                Seg::Arc(t).reversed()
                            } else {
                                Seg::Arc(t)
                            }
                        }
                    })
                    .collect(),
            ),
            Geometry::Mesh(m) => {
                let flip = x.flips();
                Geometry::Mesh(Mesh {
                    positions: m.positions.iter().map(|p| x.point(*p)).collect(),
                    normals: m.normals.iter().map(|n| x.normal(*n)).collect(),
                    triangles: if flip {
                        m.triangles.iter().map(|t| [t[0], t[2], t[1]]).collect()
                    } else {
                        m.triangles.clone()
                    },
                })
            }
        }
    }

    /// Normal of a planar curve (Newell), if it has one.
    pub fn curve_normal(&self) -> Option<Vec3> {
        match self {
            Geometry::Arc(a) => return Some(a.plane.z),
            Geometry::Text(t) => return Some(t.plane.z),
            Geometry::Dimension(d) => return Some(d.plane.z),
            _ => {}
        }
        let p = self.curve_points();
        if p.len() < 3 {
            return None;
        }
        let mut v = Vec3::new(0.0, 0.0, 0.0);
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            v = v + Vec3::new(
                (a.y - b.y) * (a.z + b.z),
                (a.z - b.z) * (a.x + b.x),
                (a.x - b.x) * (a.y + b.y),
            );
        }
        v.normalized()
    }

    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Geometry::Line(l) => l.bounding_box(),
            Geometry::Polyline(p) => BoundingBox::from_points(p).unwrap_or(BoundingBox {
                min: Point3::ORIGIN,
                max: Point3::ORIGIN,
            }),
            Geometry::Arc(a) => a.bounding_box(),
            Geometry::PolyCurve(_) | Geometry::Text(_) | Geometry::Dimension(_) => {
                let mut pts = self.curve_points();
                if let Some(l) = self.annotation_label() {
                    // Include the text box (approximate width).
                    let w = forma_geom::text_width(&l.text, l.height);
                    let left = if l.centered { -w / 2.0 } else { 0.0 };
                    pts.push(l.position + l.x * left);
                    pts.push(l.position + l.x * (left + w) + l.y * l.height);
                }
                BoundingBox::from_points(&pts).unwrap_or(BoundingBox {
                    min: Point3::ORIGIN,
                    max: Point3::ORIGIN,
                })
            }
            Geometry::Mesh(m) => m.bounding_box().unwrap_or(BoundingBox {
                min: Point3::ORIGIN,
                max: Point3::ORIGIN,
            }),
            Geometry::Point(p) => BoundingBox { min: *p, max: *p },
            Geometry::Nurbs(n) => n.bounding_box(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: ObjectId,
    pub layer: LayerId,
    pub geometry: Geometry,
    /// Display colour; `None` means "by layer".
    pub color: Option<[u8; 3]>,
    /// Changes whenever this object's geometry or attributes change (for display
    /// caches).
    pub rev: u64,
    /// Hidden objects are neither drawn nor selectable (Hide / Show).
    pub hidden: bool,
    /// Locked objects are drawn but cannot be selected (Lock / Unlock).
    pub locked: bool,
    /// Group the object belongs to (Group / Ungroup), if any.
    pub group: Option<u32>,
    /// Object name (SetObjectName), written to `.3dm` attributes.
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    pub locked: bool,
}

// `Modified` holds two objects; history entries are few, so boxing is not worth it.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum Change {
    Added(Object),
    Removed(Object),
    Modified { before: Object, after: Object },
}

/// Length unit of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Millimeters,
    Centimeters,
    Meters,
    Inches,
    Feet,
}

impl LengthUnit {
    pub fn abbreviation(self) -> &'static str {
        match self {
            LengthUnit::Millimeters => "mm",
            LengthUnit::Centimeters => "cm",
            LengthUnit::Meters => "m",
            LengthUnit::Inches => "in",
            LengthUnit::Feet => "ft",
        }
    }
}

/// Defaults for new annotations (Rhino's annotation style, reduced).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimStyle {
    /// Text height of new text and dimensions, in model units.
    pub text_height: f64,
    /// Decimal places of measured values (trailing zeros are dropped).
    pub decimals: usize,
}

impl DimStyle {
    /// A readable default for a document in `units` (10 cm, 100 mm, 0.1 m…).
    pub fn for_units(units: LengthUnit) -> DimStyle {
        let text_height = match units {
            LengthUnit::Millimeters => 100.0,
            LengthUnit::Centimeters => 10.0,
            LengthUnit::Meters => 0.1,
            LengthUnit::Inches => 4.0,
            LengthUnit::Feet => 0.33,
        };
        DimStyle {
            text_height,
            decimals: 1,
        }
    }
}

/// The modelling document.
#[derive(Debug, Clone)]
pub struct Document {
    pub units: LengthUnit,
    pub absolute_tolerance: f64,
    /// Defaults for new text and dimensions.
    pub dim_style: DimStyle,
    /// Where the document was opened from, if anywhere.
    pub path: Option<String>,
    objects: BTreeMap<ObjectId, Object>,
    pub layers: Vec<Layer>,
    pub current_layer: LayerId,
    next_id: u64,
    next_rev: u64,
    /// Incremented on every change, undo and redo.
    version: u64,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            units: LengthUnit::Millimeters,
            absolute_tolerance: 0.001,
            dim_style: DimStyle::for_units(LengthUnit::Millimeters),
            path: None,
            objects: BTreeMap::new(),
            layers: vec![Layer {
                name: "Default".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            current_layer: LayerId(0),
            next_id: 1,
            next_rev: 1,
            version: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    /// Changes on every edit, undo and redo; cheap change detection for the UI.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Index of a layer by full path.
    pub fn find_layer(&self, name: &str) -> Option<LayerId> {
        self.layers.iter().position(|l| l.name == name).map(LayerId)
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.objects.values()
    }

    /// Add a layer (full path, e.g. `muri::colonne`) and return its id.
    pub fn add_layer(&mut self, name: &str, color: [u8; 3], visible: bool) -> LayerId {
        self.layers.push(Layer {
            name: name.to_string(),
            color,
            visible,
            locked: false,
        });
        LayerId(self.layers.len() - 1)
    }

    pub fn layer(&self, id: LayerId) -> &Layer {
        &self.layers[id.0]
    }

    /// Rename a layer and its sub-layers (`old::child` → `new::child`). Not undoable,
    /// like the other layer edits.
    pub fn rename_layer(&mut self, id: LayerId, new_name: &str) {
        let old = self.layers[id.0].name.clone();
        let prefix = format!("{old}::");
        for l in &mut self.layers {
            if l.name == old {
                l.name = new_name.to_string();
            } else if let Some(rest) = l.name.strip_prefix(&prefix) {
                l.name = format!("{new_name}::{rest}");
            }
        }
        self.version += 1;
    }

    /// Remove empty layers. Objects keep their layers (indices are remapped, also
    /// in the undo history); history entries that refer to a removed layer move to
    /// the current layer. The current layer cannot be removed. Returns the names
    /// of the removed layers.
    pub fn remove_layers(&mut self, ids: &[LayerId]) -> Result<Vec<String>, String> {
        for id in ids {
            if id.0 >= self.layers.len() {
                return Err(format!("no layer {}", id.0));
            }
            if *id == self.current_layer {
                return Err(format!(
                    "cannot remove the current layer {}",
                    self.layers[id.0].name
                ));
            }
            if self.objects.values().any(|o| o.layer == *id) {
                return Err(format!("layer {} is not empty", self.layers[id.0].name));
            }
        }
        let remove: std::collections::BTreeSet<usize> = ids.iter().map(|l| l.0).collect();
        let mut map = vec![0usize; self.layers.len()];
        let mut next = 0;
        for (i, m) in map.iter_mut().enumerate() {
            if !remove.contains(&i) {
                *m = next;
                next += 1;
            }
        }
        let current = map[self.current_layer.0];
        let remap = |l: &mut LayerId| {
            l.0 = if remove.contains(&l.0) {
                current
            } else {
                map[l.0]
            };
        };
        for o in self.objects.values_mut() {
            remap(&mut o.layer);
        }
        for step in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            for c in step {
                match c {
                    Change::Added(o) | Change::Removed(o) => remap(&mut o.layer),
                    Change::Modified { before, after } => {
                        remap(&mut before.layer);
                        remap(&mut after.layer);
                    }
                }
            }
        }
        let mut names = Vec::new();
        let mut keep = Vec::new();
        for (i, l) in self.layers.drain(..).enumerate() {
            if remove.contains(&i) {
                names.push(l.name);
            } else {
                keep.push(l);
            }
        }
        self.layers = keep;
        self.current_layer = LayerId(current);
        self.version += 1;
        Ok(names)
    }

    /// Change a layer's colour, visibility or lock (not undoable, like Rhino's
    /// layer panel) and notify the display.
    pub fn edit_layer(&mut self, id: LayerId, f: impl FnOnce(&mut Layer)) {
        f(&mut self.layers[id.0]);
        self.version += 1;
    }

    /// Colour an object is displayed with: its own or its layer's.
    pub fn display_color(&self, o: &Object) -> [u8; 3] {
        o.color.unwrap_or(self.layers[o.layer.0].color)
    }

    /// True when the object is drawn: its layer is visible and it is not hidden.
    pub fn is_visible(&self, o: &Object) -> bool {
        !o.hidden && self.layers[o.layer.0].visible
    }

    /// True when the object can be selected: visible, not locked, layer unlocked.
    pub fn is_selectable(&self, o: &Object) -> bool {
        self.is_visible(o) && !o.locked && !self.layers[o.layer.0].locked
    }

    /// Bounding box of all visible objects.
    pub fn visible_bounding_box(&self) -> Option<BoundingBox> {
        let pts: Vec<Point3> = self
            .objects
            .values()
            .filter(|o| self.is_visible(o))
            .flat_map(|o| {
                let b = o.geometry.bounding_box();
                [b.min, b.max]
            })
            .collect();
        BoundingBox::from_points(&pts)
    }

    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Start an undoable group of changes. Dropping the transaction without
    /// calling [`Transaction::commit`] rolls the changes back.
    pub fn begin(&mut self) -> Transaction<'_> {
        Transaction {
            doc: self,
            changes: Vec::new(),
            committed: false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forget all undo/redo steps (e.g. after opening a file).
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Undo the last committed transaction. Returns false if there was nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(changes) = self.undo.pop() else {
            return false;
        };
        for c in changes.iter().rev() {
            self.revert(c);
        }
        self.redo.push(changes);
        true
    }

    /// Redo the last undone transaction. Returns false if there was nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(changes) = self.redo.pop() else {
            return false;
        };
        for c in &changes {
            self.apply(c);
        }
        self.undo.push(changes);
        true
    }

    /// Human- and test-readable dump of the document contents.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for o in self.objects.values() {
            let layer = &self.layers[o.layer.0].name;
            match &o.geometry {
                Geometry::Line(l) => {
                    let _ = writeln!(
                        out,
                        "#{} {} [{}] {} -> {}",
                        o.id.0,
                        o.geometry.kind(),
                        layer,
                        l.from,
                        l.to
                    );
                }
                Geometry::Polyline(p) => {
                    let _ = writeln!(out, "#{} polyline [{}] {} points", o.id.0, layer, p.len());
                }
                Geometry::PolyCurve(s) => {
                    let _ = writeln!(
                        out,
                        "#{} polycurve [{}] {} segments {} -> {}",
                        o.id.0,
                        layer,
                        s.len(),
                        s[0].start(),
                        s[s.len() - 1].end()
                    );
                }
                Geometry::Arc(a) => {
                    let _ = writeln!(
                        out,
                        "#{} {} [{}] center {} r {}",
                        o.id.0,
                        o.geometry.kind(),
                        layer,
                        a.center(),
                        a.radius
                    );
                }
                Geometry::Mesh(m) => {
                    let _ = writeln!(
                        out,
                        "#{} mesh [{}] {} triangles",
                        o.id.0,
                        layer,
                        m.triangles.len()
                    );
                }
                Geometry::Point(p) => {
                    let _ = writeln!(out, "#{} point [{}] {}", o.id.0, layer, p);
                }
                Geometry::Text(t) => {
                    let _ = writeln!(
                        out,
                        "#{} {} [{}] {:?} h {} at {}",
                        o.id.0,
                        o.geometry.kind(),
                        layer,
                        t.text,
                        t.height,
                        t.plane.origin
                    );
                }
                Geometry::Dimension(d) => {
                    let _ = writeln!(
                        out,
                        "#{} dimension [{}] {:?} {:?}",
                        o.id.0,
                        layer,
                        d.kind,
                        d.text()
                    );
                }
                Geometry::Nurbs(n) => {
                    let _ = writeln!(
                        out,
                        "#{} nurbs [{}] degree {} {} points{}",
                        o.id.0,
                        layer,
                        n.degree,
                        n.points.len(),
                        if n.is_closed() { " closed" } else { "" }
                    );
                }
            }
        }
        out
    }

    fn apply(&mut self, c: &Change) {
        self.version += 1;
        match c {
            Change::Added(o) | Change::Modified { after: o, .. } => {
                self.objects.insert(o.id, o.clone());
            }
            Change::Removed(o) => {
                self.objects.remove(&o.id);
            }
        }
    }

    fn revert(&mut self, c: &Change) {
        self.version += 1;
        match c {
            Change::Added(o) => {
                self.objects.remove(&o.id);
            }
            Change::Removed(o) | Change::Modified { before: o, .. } => {
                self.objects.insert(o.id, o.clone());
            }
        }
    }

    fn take_rev(&mut self) -> u64 {
        self.next_rev += 1;
        self.next_rev
    }
}

/// An open group of document changes. See [`Document::begin`].
pub struct Transaction<'a> {
    doc: &'a mut Document,
    changes: Vec<Change>,
    committed: bool,
}

impl Transaction<'_> {
    /// Add geometry on the current layer and return its id.
    pub fn add(&mut self, geometry: Geometry) -> ObjectId {
        let layer = self.doc.current_layer;
        self.add_on_layer(geometry, layer)
    }

    /// Add geometry on a specific layer and return its id.
    pub fn add_on_layer(&mut self, geometry: Geometry, layer: LayerId) -> ObjectId {
        let id = ObjectId(self.doc.next_id);
        self.doc.next_id += 1;
        let rev = self.doc.take_rev();
        let change = Change::Added(Object {
            id,
            layer,
            geometry,
            color: None,
            rev,
            hidden: false,
            locked: false,
            group: None,
            name: None,
        });
        self.doc.apply(&change);
        self.changes.push(change);
        id
    }

    /// Remove an object. Returns false if it did not exist.
    pub fn remove(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.doc.objects.get(&id).cloned() else {
            return false;
        };
        let change = Change::Removed(obj);
        self.doc.apply(&change);
        self.changes.push(change);
        true
    }

    /// Replace an object's geometry, keeping its id and layer.
    pub fn replace(&mut self, id: ObjectId, geometry: Geometry) -> bool {
        let Some(before) = self.doc.objects.get(&id).cloned() else {
            return false;
        };
        let rev = self.doc.take_rev();
        let after = Object {
            geometry,
            rev,
            ..before.clone()
        };
        let change = Change::Modified { before, after };
        self.doc.apply(&change);
        self.changes.push(change);
        true
    }

    /// Move an object to another layer.
    pub fn set_layer(&mut self, id: ObjectId, layer: LayerId) -> bool {
        let Some(before) = self.doc.objects.get(&id).cloned() else {
            return false;
        };
        let rev = self.doc.take_rev();
        let after = Object {
            layer,
            rev,
            ..before.clone()
        };
        let change = Change::Modified { before, after };
        self.doc.apply(&change);
        self.changes.push(change);
        true
    }

    /// Set an object's display colour (`None` = by layer).
    pub fn set_color(&mut self, id: ObjectId, color: Option<[u8; 3]>) -> bool {
        let Some(before) = self.doc.objects.get(&id).cloned() else {
            return false;
        };
        let rev = self.doc.take_rev();
        let after = Object {
            color,
            rev,
            ..before.clone()
        };
        let change = Change::Modified { before, after };
        self.doc.apply(&change);
        self.changes.push(change);
        true
    }

    /// Change an object's attributes through `f` (undoable, bumps the revision).
    fn modify(&mut self, id: ObjectId, f: impl FnOnce(&mut Object)) -> bool {
        let Some(before) = self.doc.objects.get(&id).cloned() else {
            return false;
        };
        let mut after = before.clone();
        f(&mut after);
        after.rev = self.doc.take_rev();
        let change = Change::Modified { before, after };
        self.doc.apply(&change);
        self.changes.push(change);
        true
    }

    /// Hide or show an object.
    pub fn set_hidden(&mut self, id: ObjectId, hidden: bool) -> bool {
        self.modify(id, |o| o.hidden = hidden)
    }

    /// Lock or unlock an object.
    pub fn set_locked(&mut self, id: ObjectId, locked: bool) -> bool {
        self.modify(id, |o| o.locked = locked)
    }

    /// Set or clear an object's name.
    pub fn set_name(&mut self, id: ObjectId, name: Option<String>) -> bool {
        self.modify(id, |o| o.name = name)
    }

    /// Put an object in a group (`None` = no group).
    pub fn set_group(&mut self, id: ObjectId, group: Option<u32>) -> bool {
        self.modify(id, |o| o.group = group)
    }

    /// Add an object with the attributes (layer, colour, name) of `like`.
    pub fn add_like(&mut self, geometry: Geometry, like: &Object) -> ObjectId {
        let id = self.add_on_layer(geometry, like.layer);
        if like.color.is_some() || like.name.is_some() {
            self.set_attrs_untracked(id, like.color, like.name.clone());
        }
        id
    }

    fn set_attrs_untracked(&mut self, id: ObjectId, color: Option<[u8; 3]>, name: Option<String>) {
        if let Some(Change::Added(o)) = self.changes.last_mut() {
            if o.id == id {
                o.color = color;
                o.name.clone_from(&name);
                let obj = self.doc.objects.get_mut(&id).expect("just added");
                obj.color = color;
                obj.name = name;
            }
        }
    }

    /// Read access to the document while the transaction is open.
    pub fn doc(&self) -> &Document {
        self.doc
    }

    /// Record the changes as one undo step.
    pub fn commit(mut self) {
        self.committed = true;
        if !self.changes.is_empty() {
            let changes = std::mem::take(&mut self.changes);
            self.doc.undo.push(changes);
            self.doc.redo.clear();
        }
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            for c in self.changes.iter().rev() {
                self.doc.revert(c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geom::Point3;

    fn line() -> Geometry {
        Geometry::Line(LineCurve::new(Point3::ORIGIN, Point3::new(1.0, 0.0, 0.0)))
    }

    #[test]
    fn add_undo_redo() {
        let mut doc = Document::new();
        let mut t = doc.begin();
        t.add(line());
        t.add(line());
        t.commit();
        assert_eq!(doc.len(), 2);
        assert!(doc.undo());
        assert_eq!(doc.len(), 0);
        assert!(doc.redo());
        assert_eq!(doc.len(), 2);
    }

    #[test]
    fn dropped_transaction_rolls_back() {
        let mut doc = Document::new();
        {
            let mut t = doc.begin();
            t.add(line());
        }
        assert!(doc.is_empty());
        assert!(!doc.can_undo());
    }

    #[test]
    fn replace_keeps_id_and_undoes() {
        let mut doc = Document::new();
        let mut t = doc.begin();
        let id = t.add(line());
        t.commit();
        let rev0 = doc.object(id).unwrap().rev;
        let moved = doc
            .object(id)
            .unwrap()
            .geometry
            .transformed(&Xform::translation(Vec3::new(0.0, 5.0, 0.0)));
        let mut t = doc.begin();
        t.replace(id, moved.clone());
        t.commit();
        assert_eq!(doc.object(id).unwrap().geometry, moved);
        assert_ne!(doc.object(id).unwrap().rev, rev0);
        doc.undo();
        assert_eq!(doc.object(id).unwrap().geometry, line());
        assert_eq!(doc.object(id).unwrap().rev, rev0);
    }

    #[test]
    fn hide_lock_group_are_undoable_attributes() {
        let mut doc = Document::new();
        let mut t = doc.begin();
        let id = t.add(line());
        t.commit();
        let o = doc.object(id).unwrap().clone();
        assert!(doc.is_visible(&o) && doc.is_selectable(&o));
        let mut t = doc.begin();
        t.set_hidden(id, true);
        t.set_group(id, Some(3));
        t.commit();
        let o = doc.object(id).unwrap().clone();
        assert!(!doc.is_visible(&o) && !doc.is_selectable(&o));
        assert_eq!(o.group, Some(3));
        assert!(doc.visible_bounding_box().is_none());
        let mut t = doc.begin();
        t.set_hidden(id, false);
        t.set_locked(id, true);
        t.commit();
        let o = doc.object(id).unwrap().clone();
        assert!(doc.is_visible(&o) && !doc.is_selectable(&o));
        doc.undo();
        doc.undo();
        let o = doc.object(id).unwrap();
        assert!(!o.hidden && !o.locked && o.group.is_none());
    }

    #[test]
    fn non_uniform_scale_turns_circles_into_nurbs() {
        let c = Geometry::Arc(CircleArc::circle(forma_geom::Plane::TOP, 10.0));
        let x = Xform::scale_axes(&forma_geom::Plane::TOP, 2.0, 1.0, 1.0);
        let Geometry::Nurbs(n) = c.transformed(&x) else {
            panic!("expected nurbs")
        };
        assert!(n.is_closed());
        let b = Geometry::Nurbs(n).bounding_box();
        assert!(
            (b.max.x - 20.0).abs() < 1e-6 && (b.max.y - 10.0).abs() < 1e-6,
            "{b:?}"
        );
        // Uniform scale keeps the circle.
        assert!(matches!(
            c.transformed(&Xform::scale(Point3::ORIGIN, 2.0)),
            Geometry::Arc(_)
        ));
        // A polycurve with an arc too.
        let pc = Geometry::PolyCurve(vec![
            Seg::Line(Point3::new(10.0, 0.0, 0.0), Point3::new(20.0, 0.0, 0.0)),
            Seg::Arc(
                CircleArc::from_center_start_end(
                    Point3::new(10.0, 0.0, 0.0),
                    Point3::new(20.0, 0.0, 0.0),
                    Point3::new(10.0, 10.0, 0.0),
                    Vec3::Z,
                )
                .unwrap(),
            ),
        ]);
        let g = pc.transformed(&x);
        assert_eq!(g.kind(), "nurbs");
        let pts = g.curve_points();
        assert!(pts[0].distance_to(Point3::new(20.0, 0.0, 0.0)) < 1e-9);
        assert!(
            pts.last()
                .unwrap()
                .distance_to(Point3::new(20.0, 10.0, 0.0))
                < 1e-9
        );
    }

    #[test]
    fn point_and_nurbs_basics() {
        let p = Geometry::Point(Point3::new(1.0, 2.0, 3.0));
        assert!(!p.is_curve());
        assert!(p.to_chain().is_none());
        assert_eq!(p.reversed(), p);
        let n = Geometry::Nurbs(
            NurbsCurve::clamped_uniform(
                &[
                    Point3::ORIGIN,
                    Point3::new(1.0, 1.0, 0.0),
                    Point3::new(2.0, 0.0, 0.0),
                ],
                3,
            )
            .unwrap(),
        );
        assert!(n.is_curve() && !n.is_closed_curve());
        assert!(n.to_chain().unwrap().segs.len() > 2);
        let r = n.reversed();
        assert!(r.curve_points()[0].distance_to(Point3::new(2.0, 0.0, 0.0)) < 1e-12);
        let mut doc = Document::new();
        let mut t = doc.begin();
        t.add(p);
        t.add(n);
        t.commit();
        let d = doc.dump();
        assert!(d.contains("#1 point [Default] 1,2,3"), "{d}");
        assert!(d.contains("#2 nurbs [Default] degree 2 3 points"), "{d}");
    }

    #[test]
    fn remove_is_undoable() {
        let mut doc = Document::new();
        let mut t = doc.begin();
        let id = t.add(line());
        t.commit();
        let mut t = doc.begin();
        assert!(t.remove(id));
        t.commit();
        assert!(doc.is_empty());
        doc.undo();
        assert!(doc.object(id).is_some());
    }

    #[test]
    fn annotations_are_not_curves_and_transform() {
        let d = Geometry::Dimension(Dimension::linear(
            &forma_geom::Plane::TOP,
            Point3::ORIGIN,
            Point3::new(120.0, 0.0, 0.0),
            Point3::new(60.0, -20.0, 0.0),
            5.0,
            1,
        ));
        assert!(!d.is_curve() && d.is_annotation());
        assert!(d.to_chain().is_none() && d.length().is_none());
        assert_eq!(d.annotation_lines().len(), 7);
        let (pos, text, h) = d.label().unwrap();
        assert_eq!(text, "120");
        assert!((h - 5.0).abs() < 1e-12);
        assert!(pos.distance_to(Point3::new(60.0, -18.0, 0.0)) < 1e-9);
        let b = d.bounding_box();
        assert!(b.min.y < -19.0 && b.max.x > 119.0);
        let moved = d.transformed(&Xform::translation(Vec3::new(0.0, 0.0, 5.0)));
        assert!((moved.label().unwrap().0.z - 5.0).abs() < 1e-9);
        let t = Geometry::Text(Text::new(forma_geom::Plane::TOP, "Bagno", 10.0, false));
        assert_eq!(t.kind(), "text");
        assert!(t.annotation_lines().is_empty());
        let mut doc = Document::new();
        let mut tr = doc.begin();
        tr.add(d);
        tr.add(t);
        tr.commit();
        let dump = doc.dump();
        assert!(
            dump.contains("#1 dimension [Default] Linear \"120\""),
            "{dump}"
        );
        assert!(
            dump.contains("#2 text [Default] \"Bagno\" h 10 at 0,0,0"),
            "{dump}"
        );
    }

    #[test]
    fn names_follow_copies() {
        let mut doc = Document::new();
        let mut t = doc.begin();
        let id = t.add(line());
        t.set_name(id, Some("tavolo".into()));
        t.commit();
        let o = doc.object(id).unwrap().clone();
        let mut t = doc.begin();
        let c = t.add_like(line(), &o);
        t.commit();
        assert_eq!(doc.object(c).unwrap().name.as_deref(), Some("tavolo"));
        doc.undo();
        doc.undo();
        assert!(doc.is_empty());
    }

    #[test]
    fn remove_and_rename_layers_remaps_history() {
        let mut doc = Document::new();
        let a = doc.add_layer("muri", [1, 2, 3], true);
        let b = doc.add_layer("arredi", [1, 2, 3], true);
        doc.add_layer("arredi::sedie", [1, 2, 3], true);
        let mut t = doc.begin();
        let id = t.add_on_layer(line(), b);
        t.commit();
        // A removed object that lived on "muri" stays in the history.
        let mut t = doc.begin();
        let gone = t.add_on_layer(line(), a);
        t.commit();
        let mut t = doc.begin();
        t.remove(gone);
        t.commit();
        assert!(doc.remove_layers(&[b]).is_err()); // not empty
        assert!(doc.remove_layers(&[LayerId(0)]).is_err()); // current
        let names = doc.remove_layers(&[a]).unwrap();
        assert_eq!(names, vec!["muri".to_string()]);
        assert_eq!(doc.layer(doc.object(id).unwrap().layer).name, "arredi");
        doc.undo(); // the removed object comes back on the current layer
        assert_eq!(doc.layer(doc.object(gone).unwrap().layer).name, "Default");
        doc.rename_layer(doc.find_layer("arredi").unwrap(), "mobili");
        assert!(doc.find_layer("mobili::sedie").is_some());
        assert_eq!(doc.layer(doc.object(id).unwrap().layer).name, "mobili");
    }
}
