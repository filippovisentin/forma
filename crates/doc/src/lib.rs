//! The Forma document: objects, layers and undo/redo history.
//!
//! Every mutation goes through a [`Transaction`], which records the changes so
//! they can be undone as one step.

use forma_geom::{BoundingBox, LineCurve};
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
}

impl Geometry {
    pub fn kind(&self) -> &'static str {
        match self {
            Geometry::Line(_) => "line",
        }
    }

    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Geometry::Line(l) => l.bounding_box(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: ObjectId,
    pub layer: LayerId,
    pub geometry: Geometry,
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
}

/// The modelling document.
#[derive(Debug, Clone)]
pub struct Document {
    objects: BTreeMap<ObjectId, Object>,
    pub layers: Vec<Layer>,
    pub current_layer: LayerId,
    next_id: u64,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            layers: vec![Layer {
                name: "Default".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            current_layer: LayerId(0),
            next_id: 1,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.objects.values()
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
            }
        }
        out
    }

    fn apply(&mut self, c: &Change) {
        match c {
            Change::Added(o) => {
                self.objects.insert(o.id, o.clone());
            }
            Change::Removed(o) => {
                self.objects.remove(&o.id);
            }
        }
    }

    fn revert(&mut self, c: &Change) {
        match c {
            Change::Added(o) => {
                self.objects.remove(&o.id);
            }
            Change::Removed(o) => {
                self.objects.insert(o.id, o.clone());
            }
        }
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
        let id = ObjectId(self.doc.next_id);
        self.doc.next_id += 1;
        let obj = Object {
            id,
            layer: self.doc.current_layer,
            geometry,
        };
        let change = Change::Added(obj);
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
