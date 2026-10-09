//! The Forma document: objects, layers and undo/redo history.
//!
//! Every mutation goes through a [`Transaction`], which records the changes so
//! they can be undone as one step.

use forma_geom::{BoundingBox, Chain, CircleArc, LineCurve, Mesh, Point3, Seg, Vec3, Xform};
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
    /// until the solid kernel lands, see ADR 0001).
    Mesh(Mesh),
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
        }
    }

    pub fn is_curve(&self) -> bool {
        !matches!(self, Geometry::Mesh(_))
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
            _ => false,
        }
    }

    /// The curve as a chain of line/arc segments (None for meshes).
    pub fn to_chain(&self) -> Option<Chain> {
        match self {
            Geometry::Line(l) => Some(Chain::new(vec![Seg::Line(l.from, l.to)])),
            Geometry::Polyline(p) => Some(Chain::from_points(p)),
            Geometry::Arc(a) => Some(Chain::new(vec![Seg::Arc(*a)])),
            Geometry::PolyCurve(s) => Some(Chain::new(s.clone())),
            Geometry::Mesh(_) => None,
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

    /// Points of a curve as a polyline (arcs sampled). Empty for meshes.
    pub fn curve_points(&self) -> Vec<Point3> {
        match self {
            Geometry::Line(l) => vec![l.from, l.to],
            Geometry::Polyline(p) => p.clone(),
            Geometry::Arc(a) => a.points(96),
            Geometry::PolyCurve(s) => Chain::new(s.clone()).points(),
            Geometry::Mesh(_) => Vec::new(),
        }
    }

    /// Transformed copy (rigid motions, uniform scale, mirror).
    pub fn transformed(&self, x: &Xform) -> Geometry {
        match self {
            Geometry::Line(l) => Geometry::Line(LineCurve::new(x.point(l.from), x.point(l.to))),
            Geometry::Polyline(p) => Geometry::Polyline(p.iter().map(|q| x.point(*q)).collect()),
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
        if let Geometry::Arc(a) = self {
            return Some(a.plane.z);
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
            Geometry::PolyCurve(_) => {
                BoundingBox::from_points(&self.curve_points()).unwrap_or(BoundingBox {
                    min: Point3::ORIGIN,
                    max: Point3::ORIGIN,
                })
            }
            Geometry::Mesh(m) => m.bounding_box().unwrap_or(BoundingBox {
                min: Point3::ORIGIN,
                max: Point3::ORIGIN,
            }),
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
    /// Changes whenever this object's geometry or layer changes (for display caches).
    pub rev: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    pub locked: bool,
}

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

/// The modelling document.
#[derive(Debug, Clone)]
pub struct Document {
    pub units: LengthUnit,
    pub absolute_tolerance: f64,
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

    /// Bounding box of all objects on visible layers.
    pub fn visible_bounding_box(&self) -> Option<BoundingBox> {
        let pts: Vec<Point3> = self
            .objects
            .values()
            .filter(|o| self.layers[o.layer.0].visible)
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

    /// Add an object with the attributes (layer, colour) of `like`.
    pub fn add_like(&mut self, geometry: Geometry, like: &Object) -> ObjectId {
        let id = self.add_on_layer(geometry, like.layer);
        if like.color.is_some() {
            self.set_color_untracked(id, like.color);
        }
        id
    }

    fn set_color_untracked(&mut self, id: ObjectId, color: Option<[u8; 3]>) {
        if let Some(Change::Added(o)) = self.changes.last_mut() {
            if o.id == id {
                o.color = color;
                self.doc.objects.get_mut(&id).expect("just added").color = color;
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
}
