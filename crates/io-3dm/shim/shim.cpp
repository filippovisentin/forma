// Thin C ABI over openNURBS for forma-io-3dm.
// Only plain C types cross the boundary; all openNURBS objects stay on this side.

#include "opennurbs_public.h"

#include <cstring>
#include <map>
#include <string>
#include <vector>

extern "C" {

// Keep in sync with `ObjectKind` in src/lib.rs.
enum F3dmKind {
  F3DM_OTHER = 0,
  F3DM_LINE = 1,
  F3DM_POLYLINE = 2,
  F3DM_POLYCURVE = 3,
  F3DM_NURBS_CURVE = 4,
  F3DM_ARC = 5,
  F3DM_BREP = 6,
  F3DM_EXTRUSION = 7,
  F3DM_MESH = 8,
  F3DM_INSTANCE_REF = 9,
  F3DM_POINT = 10,
  F3DM_SURFACE = 11,
  F3DM_SUBD = 12,
  F3DM_ANNOTATION = 13,
};

struct F3dmObject {
  int kind;
  int layer;        // position in the layer list returned by f3dm_layer_path, -1 if unknown
  int brep_faces;   // breps only, else 0
  int is_solid;     // breps only: 1 if closed, else 0
  double bbox[6];   // min xyz, max xyz; all zero if invalid
};

struct F3dmModel {
  ONX_Model model;
  std::vector<std::string> layer_paths;
  std::vector<F3dmObject> objects;
};

static int kind_of(const ON_Geometry* g) {
  if (!g) return F3DM_OTHER;
  if (ON_LineCurve::Cast(g)) return F3DM_LINE;
  if (ON_PolylineCurve::Cast(g)) return F3DM_POLYLINE;
  if (ON_PolyCurve::Cast(g)) return F3DM_POLYCURVE;
  if (ON_ArcCurve::Cast(g)) return F3DM_ARC;
  if (ON_NurbsCurve::Cast(g)) return F3DM_NURBS_CURVE;
  if (ON_Extrusion::Cast(g)) return F3DM_EXTRUSION;
  if (ON_Brep::Cast(g)) return F3DM_BREP;
  if (ON_Mesh::Cast(g)) return F3DM_MESH;
  if (ON_InstanceRef::Cast(g)) return F3DM_INSTANCE_REF;
  if (ON_Point::Cast(g)) return F3DM_POINT;
  if (ON_SubD::Cast(g)) return F3DM_SUBD;
  if (ON_Surface::Cast(g)) return F3DM_SURFACE;
  if (ON_Annotation::Cast(g)) return F3DM_ANNOTATION;
  return F3DM_OTHER;
}

static std::string utf8(const ON_wString& w) {
  ON_String s(w);  // UTF-16/32 -> UTF-8
  return std::string(static_cast<const char*>(s));
}

// Returns nullptr on failure.
F3dmModel* f3dm_read(const char* path) {
  F3dmModel* m = new F3dmModel();
  if (!m->model.Read(path)) {
    delete m;
    return nullptr;
  }

  // Layers: collect names, ids and parents, then build "parent::child" paths.
  struct L { ON_UUID id; ON_UUID parent; std::string name; int index; };
  std::vector<L> layers;
  ONX_ModelComponentIterator lit(m->model, ON_ModelComponent::Type::Layer);
  for (const ON_ModelComponent* c = lit.FirstComponent(); c; c = lit.NextComponent()) {
    const ON_Layer* layer = ON_Layer::Cast(c);
    if (!layer) continue;
    layers.push_back({layer->Id(), layer->ParentLayerId(), utf8(layer->Name()), layer->Index()});
  }
  std::map<int, int> index_to_pos;
  for (size_t i = 0; i < layers.size(); i++) {
    std::string path = layers[i].name;
    ON_UUID parent = layers[i].parent;
    for (int guard = 0; guard < 64 && !(parent == ON_nil_uuid); guard++) {
      bool found = false;
      for (const L& p : layers) {
        if (p.id == parent) {
          path = p.name + "::" + path;
          parent = p.parent;
          found = true;
          break;
        }
      }
      if (!found) break;
    }
    index_to_pos[layers[i].index] = static_cast<int>(i);
    m->layer_paths.push_back(path);
  }

  ONX_ModelComponentIterator git(m->model, ON_ModelComponent::Type::ModelGeometry);
  for (const ON_ModelComponent* c = git.FirstComponent(); c; c = git.NextComponent()) {
    const ON_ModelGeometryComponent* mg = ON_ModelGeometryComponent::Cast(c);
    if (!mg) continue;
    const ON_Geometry* g = mg->Geometry(nullptr);
    const ON_3dmObjectAttributes* a = mg->Attributes(nullptr);
    F3dmObject o{};
    o.kind = kind_of(g);
    o.layer = -1;
    if (a) {
      auto it = index_to_pos.find(a->m_layer_index);
      if (it != index_to_pos.end()) o.layer = it->second;
    }
    if (const ON_Brep* b = ON_Brep::Cast(g)) {
      o.brep_faces = b->m_F.Count();
      o.is_solid = b->IsSolid() ? 1 : 0;
    }
    if (g) {
      ON_BoundingBox bb = g->BoundingBox();
      if (bb.IsValid()) {
        o.bbox[0] = bb.m_min.x; o.bbox[1] = bb.m_min.y; o.bbox[2] = bb.m_min.z;
        o.bbox[3] = bb.m_max.x; o.bbox[4] = bb.m_max.y; o.bbox[5] = bb.m_max.z;
      }
    }
    m->objects.push_back(o);
  }
  return m;
}

void f3dm_free(F3dmModel* m) { delete m; }

int f3dm_archive_version(const F3dmModel* m) { return m->model.m_3dm_file_version; }

// ON::LengthUnitSystem as an integer (2 = mm, 3 = cm, 4 = m, ...).
int f3dm_unit_system(const F3dmModel* m) {
  return static_cast<int>(m->model.m_settings.m_ModelUnitsAndTolerances.m_unit_system.UnitSystem());
}

double f3dm_abs_tolerance(const F3dmModel* m) {
  return m->model.m_settings.m_ModelUnitsAndTolerances.m_absolute_tolerance;
}

double f3dm_angle_tolerance_deg(const F3dmModel* m) {
  return m->model.m_settings.m_ModelUnitsAndTolerances.m_angle_tolerance * 180.0 / ON_PI;
}

int f3dm_material_count(const F3dmModel* m) {
  return static_cast<int>(m->model.ActiveComponentCount(ON_ModelComponent::Type::RenderMaterial));
}

int f3dm_block_count(const F3dmModel* m) {
  return static_cast<int>(m->model.ActiveComponentCount(ON_ModelComponent::Type::InstanceDefinition));
}

int f3dm_layer_count(const F3dmModel* m) { return static_cast<int>(m->layer_paths.size()); }

// Copies the UTF-8 layer path (NUL-terminated, truncated to cap) and returns its full length.
size_t f3dm_layer_path(const F3dmModel* m, int i, char* buf, size_t cap) {
  if (i < 0 || static_cast<size_t>(i) >= m->layer_paths.size()) return 0;
  const std::string& s = m->layer_paths[i];
  if (cap > 0) {
    size_t n = s.size() < cap - 1 ? s.size() : cap - 1;
    std::memcpy(buf, s.data(), n);
    buf[n] = 0;
  }
  return s.size();
}

int f3dm_object_count(const F3dmModel* m) { return static_cast<int>(m->objects.size()); }

int f3dm_object(const F3dmModel* m, int i, F3dmObject* out) {
  if (i < 0 || static_cast<size_t>(i) >= m->objects.size()) return 0;
  *out = m->objects[i];
  return 1;
}

// Write a new .3dm containing one line on a named layer. Returns 1 on success.
int f3dm_write_line(const char* path, const double a[3], const double b[3], const char* layer_name,
                    int unit_system, double abs_tol) {
  ONX_Model model;
  model.m_settings.m_ModelUnitsAndTolerances.m_unit_system =
      ON::LengthUnitSystemFromUnsigned(static_cast<unsigned int>(unit_system));
  model.m_settings.m_ModelUnitsAndTolerances.m_absolute_tolerance = abs_tol;
  model.m_settings.m_ModelUnitsAndTolerances.m_angle_tolerance = ON_PI / 180.0;
  model.m_settings.m_ModelUnitsAndTolerances.m_relative_tolerance = 0.01;

  ON_wString wname(layer_name);  // UTF-8 -> wide
  const int layer_index = model.AddLayer(static_cast<const wchar_t*>(wname), ON_Color::Black);

  ON_3dmObjectAttributes* attrs = new ON_3dmObjectAttributes();
  attrs->m_layer_index = layer_index;
  ON_LineCurve* line = new ON_LineCurve(ON_3dPoint(a[0], a[1], a[2]), ON_3dPoint(b[0], b[1], b[2]));
  model.AddManagedModelGeometryComponent(line, attrs);

  return model.Write(path, 0) ? 1 : 0;
}

}  // extern "C"
