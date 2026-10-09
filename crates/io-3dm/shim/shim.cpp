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

struct F3dmLayer {
  unsigned char rgb[3];
  int visible;
};

struct F3dmModel {
  ONX_Model model;
  std::vector<std::string> layer_paths;
  std::vector<F3dmLayer> layers;
  std::vector<F3dmObject> objects;
  // Geometry of each object (owned by `model`), and a brep for breps/extrusions
  // (extrusions are converted on demand and owned by `owned_breps`).
  std::vector<const ON_Geometry*> geometry;
  std::vector<const ON_Brep*> breps;
  std::vector<ON_Brep*> owned_breps;
  // Scratch buffers for the two-call "size, then copy" pattern.
  std::vector<std::vector<double>> scratch_loops;  // uv pairs per loop
  std::vector<int> scratch_loop_outer;
  std::vector<double> scratch_points;              // xyz triples
  std::vector<double> scratch_mesh_v;               // xyz triples
  std::vector<unsigned int> scratch_mesh_t;         // triangle indices

  ~F3dmModel() {
    for (ON_Brep* b : owned_breps) delete b;
  }
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
    const ON_Color col = layer->Color();
    F3dmLayer info{};
    info.rgb[0] = static_cast<unsigned char>(col.Red());
    info.rgb[1] = static_cast<unsigned char>(col.Green());
    info.rgb[2] = static_cast<unsigned char>(col.Blue());
    info.visible = layer->IsVisible() ? 1 : 0;
    m->layers.push_back(info);
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
    m->geometry.push_back(g);
    const ON_Brep* brep = ON_Brep::Cast(g);
    if (!brep) {
      if (const ON_Extrusion* ex = ON_Extrusion::Cast(g)) {
        ON_Brep* converted = ex->BrepForm(nullptr);
        if (converted) {
          m->owned_breps.push_back(converted);
          brep = converted;
        }
      }
    }
    m->breps.push_back(brep);
  }
  return m;
}

// ---------------------------------------------------------------------------
// Display data. Rhino files often carry no render meshes, and public openNURBS
// cannot mesh breps, so Rust triangulates faces from these samples.

void f3dm_layer_display(const F3dmModel* m, int i, unsigned char rgb[3], int* visible) {
  if (i < 0 || static_cast<size_t>(i) >= m->layers.size()) return;
  rgb[0] = m->layers[i].rgb[0];
  rgb[1] = m->layers[i].rgb[1];
  rgb[2] = m->layers[i].rgb[2];
  *visible = m->layers[i].visible;
}

static const ON_Brep* brep_of(const F3dmModel* m, int obj) {
  if (obj < 0 || static_cast<size_t>(obj) >= m->breps.size()) return nullptr;
  return m->breps[obj];
}

// Number of faces of a brep or extrusion (0 for anything else).
int f3dm_face_count(const F3dmModel* m, int obj) {
  const ON_Brep* b = brep_of(m, obj);
  return b ? b->m_F.Count() : 0;
}

// Face parameter domain, span counts and degrees, and orientation flag.
int f3dm_face_info(const F3dmModel* m, int obj, int face, double domain[4], int spans[2],
                   int degree[2], int* reversed) {
  const ON_Brep* b = brep_of(m, obj);
  if (!b || face < 0 || face >= b->m_F.Count()) return 0;
  const ON_BrepFace& f = b->m_F[face];
  for (int dir = 0; dir < 2; dir++) {
    ON_Interval d = f.Domain(dir);
    domain[2 * dir] = d.Min();
    domain[2 * dir + 1] = d.Max();
    spans[dir] = f.SpanCount(dir);
    degree[dir] = f.Degree(dir);
  }
  *reversed = f.m_bRev ? 1 : 0;
  return 1;
}

// Sample a parameter-space or 3D curve into points (excluding the end point when
// `skip_last`), appending (x, y, z) triples.
static void sample_curve(const ON_Curve* c, bool skip_last, std::vector<double>& out, int per_span) {
  if (!c) return;
  ON_SimpleArray<ON_3dPoint> pline;
  if (c->IsPolyline(&pline) && pline.Count() >= 2) {
    const int n = pline.Count() - (skip_last ? 1 : 0);
    for (int i = 0; i < n; i++) {
      out.push_back(pline[i].x);
      out.push_back(pline[i].y);
      out.push_back(pline[i].z);
    }
    return;
  }
  const int spans = c->SpanCount();
  std::vector<double> knots(spans + 1);
  c->GetSpanVector(knots.data());
  for (int s = 0; s < spans; s++) {
    for (int k = 0; k < per_span; k++) {
      const double t = knots[s] + (knots[s + 1] - knots[s]) * k / per_span;
      ON_3dPoint p = c->PointAt(t);
      out.push_back(p.x);
      out.push_back(p.y);
      out.push_back(p.z);
    }
  }
  if (!skip_last) {
    ON_3dPoint p = c->PointAtEnd();
    out.push_back(p.x);
    out.push_back(p.y);
    out.push_back(p.z);
  }
}

// Tessellate the trimming loops of a face in (u, v). Returns the number of loops;
// fetch each with f3dm_loop_points.
int f3dm_face_loops(F3dmModel* m, int obj, int face) {
  m->scratch_loops.clear();
  m->scratch_loop_outer.clear();
  const ON_Brep* b = brep_of(m, obj);
  if (!b || face < 0 || face >= b->m_F.Count()) return 0;
  const ON_BrepFace& f = b->m_F[face];
  for (int li = 0; li < f.m_li.Count(); li++) {
    const ON_BrepLoop& loop = b->m_L[f.m_li[li]];
    if (loop.m_type != ON_BrepLoop::outer && loop.m_type != ON_BrepLoop::inner) continue;
    std::vector<double> xyz;
    for (int ti = 0; ti < loop.m_ti.Count(); ti++) {
      sample_curve(&b->m_T[loop.m_ti[ti]], true, xyz, 12);
    }
    std::vector<double> uv;
    for (size_t k = 0; k + 2 < xyz.size(); k += 3) {
      uv.push_back(xyz[k]);
      uv.push_back(xyz[k + 1]);
    }
    m->scratch_loops.push_back(uv);
    m->scratch_loop_outer.push_back(loop.m_type == ON_BrepLoop::outer ? 1 : 0);
  }
  return static_cast<int>(m->scratch_loops.size());
}

// Copy loop `k` from the last f3dm_face_loops call: (u, v) pairs. Returns the point count.
int f3dm_loop_points(const F3dmModel* m, int k, double* uv, int cap_points, int* is_outer) {
  if (k < 0 || static_cast<size_t>(k) >= m->scratch_loops.size()) return 0;
  const std::vector<double>& l = m->scratch_loops[k];
  const int n = static_cast<int>(l.size() / 2);
  *is_outer = m->scratch_loop_outer[k];
  if (uv && cap_points >= n) std::memcpy(uv, l.data(), l.size() * sizeof(double));
  return n;
}

// Evaluate `n` (u, v) samples of a face into points and unit normals (normals already
// account for the face orientation flag).
int f3dm_face_eval(const F3dmModel* m, int obj, int face, int n, const double* uv, double* xyz,
                   double* nrm) {
  const ON_Brep* b = brep_of(m, obj);
  if (!b || face < 0 || face >= b->m_F.Count()) return 0;
  const ON_BrepFace& f = b->m_F[face];
  const double sign = f.m_bRev ? -1.0 : 1.0;
  for (int i = 0; i < n; i++) {
    ON_3dPoint p;
    ON_3dVector v;
    if (!f.EvNormal(uv[2 * i], uv[2 * i + 1], p, v)) {
      p = f.PointAt(uv[2 * i], uv[2 * i + 1]);
      v = ON_3dVector::ZeroVector;
    }
    xyz[3 * i] = p.x; xyz[3 * i + 1] = p.y; xyz[3 * i + 2] = p.z;
    nrm[3 * i] = sign * v.x; nrm[3 * i + 1] = sign * v.y; nrm[3 * i + 2] = sign * v.z;
  }
  return 1;
}

// Sample a curve object into a polyline. Returns the point count; fetch with
// f3dm_points_copy.
int f3dm_curve_points(F3dmModel* m, int obj) {
  m->scratch_points.clear();
  if (obj < 0 || static_cast<size_t>(obj) >= m->geometry.size()) return 0;
  const ON_Curve* c = ON_Curve::Cast(m->geometry[obj]);
  if (!c) return 0;
  sample_curve(c, false, m->scratch_points, 16);
  return static_cast<int>(m->scratch_points.size() / 3);
}

int f3dm_points_copy(const F3dmModel* m, double* xyz, int cap_points) {
  const int n = static_cast<int>(m->scratch_points.size() / 3);
  if (xyz && cap_points >= n) std::memcpy(xyz, m->scratch_points.data(), m->scratch_points.size() * sizeof(double));
  return n;
}

// Triangles of a mesh object (quads split). Returns 1 if the object is a mesh;
// sizes are written to *nv and *nt; fetch with f3dm_mesh_copy.
int f3dm_mesh_data(F3dmModel* m, int obj, int* nv, int* nt) {
  m->scratch_mesh_v.clear();
  m->scratch_mesh_t.clear();
  *nv = 0;
  *nt = 0;
  if (obj < 0 || static_cast<size_t>(obj) >= m->geometry.size()) return 0;
  const ON_Mesh* mesh = ON_Mesh::Cast(m->geometry[obj]);
  if (!mesh) return 0;
  for (int i = 0; i < mesh->m_V.Count(); i++) {
    ON_3dPoint p = mesh->Vertex(i);
    m->scratch_mesh_v.push_back(p.x);
    m->scratch_mesh_v.push_back(p.y);
    m->scratch_mesh_v.push_back(p.z);
  }
  for (int i = 0; i < mesh->m_F.Count(); i++) {
    const ON_MeshFace& f = mesh->m_F[i];
    m->scratch_mesh_t.insert(m->scratch_mesh_t.end(), {(unsigned)f.vi[0], (unsigned)f.vi[1], (unsigned)f.vi[2]});
    if (f.IsQuad()) {
      m->scratch_mesh_t.insert(m->scratch_mesh_t.end(), {(unsigned)f.vi[0], (unsigned)f.vi[2], (unsigned)f.vi[3]});
    }
  }
  *nv = static_cast<int>(m->scratch_mesh_v.size() / 3);
  *nt = static_cast<int>(m->scratch_mesh_t.size() / 3);
  return 1;
}

void f3dm_mesh_copy(const F3dmModel* m, double* xyz, unsigned int* tri) {
  std::memcpy(xyz, m->scratch_mesh_v.data(), m->scratch_mesh_v.size() * sizeof(double));
  std::memcpy(tri, m->scratch_mesh_t.data(), m->scratch_mesh_t.size() * sizeof(unsigned int));
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

// ---------------------------------------------------------------------------
// Writing. A writer collects layers and objects, then saves one .3dm file.

extern "C" {

struct F3dmWriter {
  ONX_Model model;
  std::vector<ON_UUID> layer_ids;   // by writer layer index
  std::vector<int> layer_indices;   // model layer index by writer layer index
};

F3dmWriter* f3dm_writer_new(int unit_system, double abs_tol) {
  F3dmWriter* w = new F3dmWriter();
  w->model.m_settings.m_ModelUnitsAndTolerances.m_unit_system =
      ON::LengthUnitSystemFromUnsigned(static_cast<unsigned int>(unit_system));
  w->model.m_settings.m_ModelUnitsAndTolerances.m_absolute_tolerance = abs_tol;
  w->model.m_settings.m_ModelUnitsAndTolerances.m_angle_tolerance = ON_PI / 180.0;
  w->model.m_settings.m_ModelUnitsAndTolerances.m_relative_tolerance = 0.01;
  return w;
}

void f3dm_writer_free(F3dmWriter* w) { delete w; }

// Adds a layer (short name, parent = writer index or -1). Returns the writer index.
int f3dm_writer_layer(F3dmWriter* w, const char* name, int parent, const unsigned char rgb[3],
                      int visible) {
  ON_Layer layer;
  ON_wString wname(name);
  layer.SetName(static_cast<const wchar_t*>(wname));
  layer.SetColor(ON_Color(rgb[0], rgb[1], rgb[2]));
  layer.SetVisible(visible != 0);
  if (parent >= 0 && static_cast<size_t>(parent) < w->layer_ids.size()) {
    layer.SetParentLayerId(w->layer_ids[parent]);
  }
  ON_ModelComponentReference ref = w->model.AddModelComponent(layer, true);
  const ON_ModelComponent* c = ref.ModelComponent();
  if (!c) return -1;
  w->layer_ids.push_back(c->Id());
  w->layer_indices.push_back(c->Index());
  return static_cast<int>(w->layer_ids.size() - 1);
}

static int add_object(F3dmWriter* w, int layer, ON_Object* geometry) {
  ON_3dmObjectAttributes* a = new ON_3dmObjectAttributes();
  if (layer >= 0 && static_cast<size_t>(layer) < w->layer_indices.size()) {
    a->m_layer_index = w->layer_indices[layer];
  }
  ON_ModelComponentReference ref = w->model.AddManagedModelGeometryComponent(geometry, a);
  return ref.IsEmpty() ? 0 : 1;
}

int f3dm_writer_line(F3dmWriter* w, int layer, const double a[3], const double b[3]) {
  return add_object(w, layer, new ON_LineCurve(ON_3dPoint(a), ON_3dPoint(b)));
}

int f3dm_writer_polyline(F3dmWriter* w, int layer, const double* xyz, int n) {
  ON_Polyline pl;
  for (int i = 0; i < n; i++) pl.Append(ON_3dPoint(xyz + 3 * i));
  return add_object(w, layer, new ON_PolylineCurve(pl));
}

int f3dm_writer_arc(F3dmWriter* w, int layer, const double center[3], const double xaxis[3],
                    const double yaxis[3], double radius, double sweep) {
  const ON_3dPoint o(center);
  const ON_3dVector vx(xaxis), vy(yaxis);
  ON_Plane plane(o, vx, vy);
  ON_Arc arc(plane, radius, sweep);
  return add_object(w, layer, new ON_ArcCurve(arc));
}

int f3dm_writer_mesh(F3dmWriter* w, int layer, const double* xyz, const double* normals, int nv,
                     const unsigned int* tri, int nt) {
  ON_Mesh* mesh = new ON_Mesh(nt, nv, normals != nullptr, false);
  for (int i = 0; i < nv; i++) {
    mesh->SetVertex(i, ON_3dPoint(xyz + 3 * i));
    if (normals) mesh->SetVertexNormal(i, ON_3dVector(normals + 3 * i));
  }
  for (int i = 0; i < nt; i++) {
    mesh->SetTriangle(i, static_cast<int>(tri[3 * i]), static_cast<int>(tri[3 * i + 1]),
                      static_cast<int>(tri[3 * i + 2]));
  }
  if (!normals) mesh->ComputeVertexNormals();
  mesh->BoundingBox();
  return add_object(w, layer, mesh);
}

int f3dm_writer_save(F3dmWriter* w, const char* path) { return w->model.Write(path, 0) ? 1 : 0; }

}  // extern "C"
