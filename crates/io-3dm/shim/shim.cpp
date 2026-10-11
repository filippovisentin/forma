// Thin C ABI over openNURBS for forma-io-3dm.
// Only plain C types cross the boundary; all openNURBS objects stay on this side.

#include "opennurbs_public.h"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <limits>
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
  F3DM_TEXTDOT = 14,
};

struct F3dmObject {
  int kind;
  int layer;        // position in the layer list returned by f3dm_layer_path, -1 if unknown
  int brep_faces;   // breps only, else 0
  int is_solid;     // breps only: 1 if closed, else 0
  int all_planar;   // breps only: 1 if every face is planar
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
  std::vector<long> object_colors;  // 0xRRGGBB, or -1 for "by layer"
  std::vector<std::string> object_names;  // UTF-8, empty = no name
  // Geometry of each object (owned by `model`), and a brep for breps/extrusions
  // (extrusions are converted on demand and owned by `owned_breps`).
  std::vector<const ON_Geometry*> geometry;
  std::vector<const ON_Brep*> breps;
  std::vector<ON_Brep*> owned_breps;
  // Transformed copies of block members.
  std::vector<ON_Geometry*> owned_geometry;
  // Block instance number of each object, -1 for plain objects.
  std::vector<int> object_instance;
  int instance_count = 0;
  // Per object: bit 0 hidden, bit 1 locked; and its (top) group index or -1.
  std::vector<int> object_state;
  std::vector<int> object_group;
  // Scratch buffers for the two-call "size, then copy" pattern.
  std::vector<std::vector<double>> scratch_loops;  // uv pairs per loop
  std::vector<std::vector<double>> scratch_loop_xyz;  // xyz per loop point (NaN = evaluate)
  std::vector<int> scratch_loop_outer;
  std::vector<double> scratch_points;              // xyz triples
  std::vector<double> scratch_mesh_v;               // xyz triples
  std::vector<unsigned int> scratch_mesh_t;         // triangle indices

  ~F3dmModel() {
    for (ON_Brep* b : owned_breps) delete b;
    for (ON_Geometry* g : owned_geometry) delete g;
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
  if (ON_TextDot::Cast(g)) return F3DM_TEXTDOT;
  return F3DM_OTHER;
}

static std::string utf8(const ON_wString& w) {
  ON_String s(w);  // UTF-16/32 -> UTF-8
  return std::string(static_cast<const char*>(s));
}

// Adds one object (geometry owned by the model or by `m->owned_geometry`).
// `instance` >= 0 marks members of one block instance; `parent_rgb` is the
// instance's own colour (for members coloured "by parent"); `block_name`
// names members that have no name of their own.
static void read_object(F3dmModel* m, const std::map<int, int>& index_to_pos,
                       const ON_Geometry* g, const ON_3dmObjectAttributes* a, int instance,
                       long parent_rgb, const ON_wString* block_name) {
  F3dmObject o{};
  o.kind = kind_of(g);
  o.layer = -1;
  if (a) {
    auto it = index_to_pos.find(a->m_layer_index);
    if (it != index_to_pos.end()) o.layer = it->second;
  }
  if (g) {
    ON_BoundingBox bb = g->BoundingBox();
    if (bb.IsValid()) {
      o.bbox[0] = bb.m_min.x; o.bbox[1] = bb.m_min.y; o.bbox[2] = bb.m_min.z;
      o.bbox[3] = bb.m_max.x; o.bbox[4] = bb.m_max.y; o.bbox[5] = bb.m_max.z;
    }
  }
  m->objects.push_back(o);
  long rgb = -1;
  if (a && a->ColorSource() == ON::color_from_object) {
    const ON_Color col = a->m_color;
    rgb = (static_cast<long>(col.Red()) << 16) | (col.Green() << 8) | col.Blue();
  } else if (a && a->ColorSource() == ON::color_from_parent) {
    rgb = parent_rgb;
  }
  m->object_colors.push_back(rgb);
  {
    ON_wString wn;
    if (a) wn = a->Name();
    if (wn.IsEmpty() && block_name) wn = *block_name;
    m->object_names.push_back(utf8(wn));
  }
  m->object_instance.push_back(instance);
  {
    int state = 0;
    int group = -1;
    // Block members follow their instance (only plain objects carry state).
    if (a && instance < 0) {
      if (!a->IsVisible()) state |= 1;
      if (a->Mode() == ON::locked_object) state |= 2;
      if (a->GroupCount() > 0) group = a->TopGroup();
    }
    m->object_state.push_back(state);
    m->object_group.push_back(group);
  }
  m->geometry.push_back(g);
  const ON_Brep* brep = ON_Brep::Cast(g);
  if (!brep && (ON_Extrusion::Cast(g) || ON_Surface::Cast(g))) {
    // Extrusions and single surfaces are shown (and measured) through their brep form.
    ON_Brep* converted = g->BrepForm(nullptr);
    if (converted) {
      m->owned_breps.push_back(converted);
      brep = converted;
    }
  }
  m->breps.push_back(brep);
  if (brep) {
    m->objects.back().brep_faces = brep->m_F.Count();
    m->objects.back().is_solid = brep->IsSolid() ? 1 : 0;
    int planar = brep->m_F.Count() > 0 ? 1 : 0;
    for (int i = 0; i < brep->m_F.Count() && planar; i++) {
      if (!brep->m_F[i].IsPlanar(nullptr, 1e-6)) planar = 0;
    }
    m->objects.back().all_planar = planar;
  }
}

// Adds the members of a block instance, transformed by `parent * ref.m_xform`;
// nested instances recurse (up to a sane depth).
static void expand_instance(F3dmModel* m, const std::map<int, int>& index_to_pos,
                            const ON_InstanceRef* ref, const ON_Xform& parent, int depth,
                            int instance, const ON_3dmObjectAttributes* ref_attrs,
                            long parent_rgb) {
  if (depth > 16) return;
  const ON_ModelComponentReference dref = m->model.ComponentFromId(
      ON_ModelComponent::Type::InstanceDefinition, ref->m_instance_definition_uuid);
  const ON_InstanceDefinition* idef = ON_InstanceDefinition::FromModelComponentRef(dref, nullptr);
  if (!idef) return;
  const ON_Xform x = parent * ref->m_xform;
  const ON_wString name = idef->Name();
  const ON_SimpleArray<ON_UUID>& ids = idef->InstanceGeometryIdList();
  for (int k = 0; k < ids.Count(); k++) {
    const ON_ModelComponentReference gref =
        m->model.ComponentFromId(ON_ModelComponent::Type::ModelGeometry, ids[k]);
    const ON_ModelGeometryComponent* mg =
        ON_ModelGeometryComponent::FromModelComponentRef(gref, nullptr);
    if (!mg) continue;
    const ON_Geometry* g = mg->Geometry(nullptr);
    const ON_3dmObjectAttributes* a = mg->Attributes(nullptr);
    if (!g) continue;
    long rgb = parent_rgb;
    if (a && a->ColorSource() == ON::color_from_object) {
      const ON_Color col = a->m_color;
      rgb = (static_cast<long>(col.Red()) << 16) | (col.Green() << 8) | col.Blue();
    }
    if (const ON_InstanceRef* inner = ON_InstanceRef::Cast(g)) {
      expand_instance(m, index_to_pos, inner, x, depth + 1, instance, ref_attrs, rgb);
      continue;
    }
    ON_Geometry* copy = g->Duplicate();
    if (!copy) continue;
    copy->Transform(x);
    m->owned_geometry.push_back(copy);
    // Members on layer "0"-style defaults keep their own layer; Rhino does the same.
    read_object(m, index_to_pos, copy, a ? a : ref_attrs, instance, parent_rgb, &name);
  }
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

  // Top-level objects; block instances are expanded into their (transformed)
  // member objects, which share an instance number so Forma can group them.
  ONX_ModelComponentIterator git(m->model, ON_ModelComponent::Type::ModelGeometry);
  for (const ON_ModelComponent* c = git.FirstComponent(); c; c = git.NextComponent()) {
    const ON_ModelGeometryComponent* mg = ON_ModelGeometryComponent::Cast(c);
    if (!mg) continue;
    const ON_Geometry* g = mg->Geometry(nullptr);
    const ON_3dmObjectAttributes* a = mg->Attributes(nullptr);
    if (a && a->IsInstanceDefinitionObject()) continue;  // block member, see below
    if (const ON_InstanceRef* ref = ON_InstanceRef::Cast(g)) {
      const int instance = m->instance_count++;
      long rgb = -1;
      if (a && a->ColorSource() == ON::color_from_object) {
        const ON_Color col = a->m_color;
        rgb = (static_cast<long>(col.Red()) << 16) | (col.Green() << 8) | col.Blue();
      }
      expand_instance(m, index_to_pos, ref, ON_Xform::IdentityTransformation, 0, instance, a, rgb);
      continue;
    }
    read_object(m, index_to_pos, g, a, -1, -1, nullptr);
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

// Text of a text object (returns 1) or text dot (returns 2), 0 otherwise.
// plane: origin, x axis, y axis (9 doubles); height in model units (dots: 0);
// text as UTF-8 into buf (NUL-terminated, truncated to cap), full length in *len.
int f3dm_text(const F3dmModel* m, int obj, double* plane, double* height, char* buf, int cap,
              int* len) {
  if (obj < 0 || static_cast<size_t>(obj) >= m->geometry.size()) return 0;
  const ON_Geometry* g = m->geometry[obj];
  std::string text;
  int kind = 0;
  if (const ON_Text* t = ON_Text::Cast(g)) {
    const ON_ModelComponentReference r = m->model.DimensionStyleFromId(t->DimensionStyleId());
    const ON_DimStyle* ds = ON_DimStyle::FromModelComponentRef(r, &ON_DimStyle::Default);
    const ON_Plane& p = t->Plane();
    const double v[9] = {p.origin.x, p.origin.y, p.origin.z, p.xaxis.x, p.xaxis.y,
                         p.xaxis.z,  p.yaxis.x,  p.yaxis.y,  p.yaxis.z};
    std::memcpy(plane, v, sizeof v);
    *height = t->TextHeight(ds);
    text = utf8(t->PlainText());
    kind = 1;
  } else if (const ON_TextDot* d = ON_TextDot::Cast(g)) {
    const ON_3dPoint c = d->CenterPoint();
    const double v[9] = {c.x, c.y, c.z, 1, 0, 0, 0, 1, 0};
    std::memcpy(plane, v, sizeof v);
    *height = 0.0;
    text = utf8(ON_wString(d->PrimaryText()));
    kind = 2;
  } else {
    return 0;
  }
  // One line: Forma texts are single-line.
  for (char& c : text) {
    if (c == '\r' || c == '\n') c = ' ';
  }
  *len = static_cast<int>(text.size());
  if (buf && cap > 0) {
    const size_t n = (std::min)(text.size(), static_cast<size_t>(cap - 1));
    std::memcpy(buf, text.data(), n);
    buf[n] = 0;
  }
  return kind;
}

// Hidden (bit 0) / locked (bit 1) state of an object and its group (-1 = none).
int f3dm_object_state(const F3dmModel* m, int i, int* group) {
  if (i < 0 || static_cast<size_t>(i) >= m->object_state.size()) return 0;
  *group = m->object_group[i];
  return m->object_state[i];
}

// Block instance number of an object (members of one instance share it), or -1.
int f3dm_object_instance(const F3dmModel* m, int i) {
  if (i < 0 || static_cast<size_t>(i) >= m->object_instance.size()) return -1;
  return m->object_instance[i];
}

// Object colour when set on the object (returns 1), else 0 (by layer).
int f3dm_object_color(const F3dmModel* m, int i, unsigned char rgb[3]) {
  if (i < 0 || static_cast<size_t>(i) >= m->object_colors.size()) return 0;
  const long c = m->object_colors[i];
  if (c < 0) return 0;
  rgb[0] = static_cast<unsigned char>((c >> 16) & 255);
  rgb[1] = static_cast<unsigned char>((c >> 8) & 255);
  rgb[2] = static_cast<unsigned char>(c & 255);
  return 1;
}

// Object name as UTF-8 into buf (NUL-terminated, truncated to cap). Returns the
// full name length in bytes (0 = no name).
int f3dm_object_name(const F3dmModel* m, int i, char* buf, int cap) {
  if (i < 0 || static_cast<size_t>(i) >= m->object_names.size()) return 0;
  const std::string& s = m->object_names[i];
  if (buf && cap > 0) {
    const size_t n = (std::min)(s.size(), static_cast<size_t>(cap - 1));
    std::memcpy(buf, s.data(), n);
    buf[n] = 0;
  }
  return static_cast<int>(s.size());
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

// Sample parameters of an edge curve: its span ends, and more points inside
// curved spans (one per 10 degrees of turning). Both faces of an edge use the
// same samples, so their display meshes meet without cracks.
static void edge_params(const ON_Curve& c, std::vector<double>& t) {
  t.clear();
  const ON_Interval d = c.Domain();
  if (c.IsLinear(1e-9)) {
    t.push_back(d[0]);
    t.push_back(d[1]);
    return;
  }
  const int spans = (std::max)(c.SpanCount(), 1);
  std::vector<double> k(spans + 1);
  if (!c.GetSpanVector(k.data())) {
    for (int s = 0; s <= spans; s++) k[s] = d.ParameterAt(static_cast<double>(s) / spans);
  }
  const bool curved = c.Degree() > 1;
  for (int s = 0; s < spans; s++) {
    int n = 1;
    if (curved) {
      double turn = 0.0;
      ON_3dVector prev = c.TangentAt(k[s]);
      for (int i = 1; i <= 8; i++) {
        const ON_3dVector tg = c.TangentAt(k[s] + (k[s + 1] - k[s]) * i / 8.0);
        const double dot = (std::max)(-1.0, (std::min)(1.0, ON_DotProduct(prev, tg)));
        turn += std::acos(dot);
        prev = tg;
      }
      n = (std::max)(1, (std::min)(64, static_cast<int>(std::ceil(turn / (10.0 * ON_PI / 180.0)))));
    }
    for (int i = 0; i < n; i++) t.push_back(k[s] + (k[s + 1] - k[s]) * i / n);
  }
  t.push_back(d[1]);
}

// Tessellate the trimming loops of a face: (u, v) for the triangulation and the
// 3D point of each sample (taken from the edge, NaN on singular trims). Returns
// the number of loops; fetch each with f3dm_loop_points / f3dm_loop_xyz.
int f3dm_face_loops(F3dmModel* m, int obj, int face) {
  m->scratch_loops.clear();
  m->scratch_loop_xyz.clear();
  m->scratch_loop_outer.clear();
  const ON_Brep* b = brep_of(m, obj);
  if (!b || face < 0 || face >= b->m_F.Count()) return 0;
  const ON_BrepFace& f = b->m_F[face];
  const double nan = std::numeric_limits<double>::quiet_NaN();
  std::vector<double> params;
  for (int li = 0; li < f.m_li.Count(); li++) {
    const ON_BrepLoop& loop = b->m_L[f.m_li[li]];
    if (loop.m_type != ON_BrepLoop::outer && loop.m_type != ON_BrepLoop::inner) continue;
    std::vector<double> uv, xyz;
    for (int ti = 0; ti < loop.m_ti.Count(); ti++) {
      const ON_BrepTrim& trim = b->m_T[loop.m_ti[ti]];
      const ON_BrepEdge* edge = trim.m_ei >= 0 ? &b->m_E[trim.m_ei] : nullptr;
      const ON_Interval td = trim.Domain();
      if (!edge || !edge->EdgeCurveOf()) {
        // Singular trim (pole): a few points, positions from the surface.
        std::vector<double> pts;
        sample_curve(&trim, true, pts, 4);
        for (size_t k = 0; k + 2 < pts.size(); k += 3) {
          uv.insert(uv.end(), {pts[k], pts[k + 1]});
          xyz.insert(xyz.end(), {nan, nan, nan});
        }
        continue;
      }
      edge_params(*edge, params);
      const ON_Interval ed = edge->Domain();
      // Every sample but the trim's end (the next trim starts there).
      for (size_t i = 0; i + 1 < params.size(); i++) {
        const size_t j = trim.m_bRev3d ? params.size() - 1 - i : i;
        const double x = ed.NormalizedParameterAt(params[j]);
        const ON_3dPoint q = trim.PointAt(td.ParameterAt(trim.m_bRev3d ? 1.0 - x : x));
        const ON_3dPoint p = edge->PointAt(params[j]);
        uv.insert(uv.end(), {q.x, q.y});
        xyz.insert(xyz.end(), {p.x, p.y, p.z});
      }
    }
    m->scratch_loops.push_back(uv);
    m->scratch_loop_xyz.push_back(xyz);
    m->scratch_loop_outer.push_back(loop.m_type == ON_BrepLoop::outer ? 1 : 0);
  }
  return static_cast<int>(m->scratch_loops.size());
}

// 3D points of loop `k` from the last f3dm_face_loops call (NaN = evaluate the
// surface). Returns the point count.
int f3dm_loop_xyz(const F3dmModel* m, int k, double* xyz, int cap_points) {
  if (k < 0 || static_cast<size_t>(k) >= m->scratch_loop_xyz.size()) return 0;
  const std::vector<double>& l = m->scratch_loop_xyz[k];
  const int n = static_cast<int>(l.size() / 3);
  if (xyz && cap_points >= n) std::memcpy(xyz, l.data(), l.size() * sizeof(double));
  return n;
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

// Vertices then edge midpoints of a brep (for snapping), into the points
// scratch buffer (fetch with f3dm_points_copy). Returns the total count;
// *nverts gets the number of vertices.
int f3dm_brep_points(F3dmModel* m, int obj, int* nverts) {
  m->scratch_points.clear();
  *nverts = 0;
  const ON_Brep* b = brep_of(m, obj);
  if (!b) return 0;
  for (int i = 0; i < b->m_V.Count(); i++) {
    const ON_3dPoint p = b->m_V[i].point;
    m->scratch_points.insert(m->scratch_points.end(), {p.x, p.y, p.z});
  }
  *nverts = b->m_V.Count();
  for (int i = 0; i < b->m_E.Count(); i++) {
    const ON_BrepEdge& e = b->m_E[i];
    if (!e.EdgeCurveOf()) continue;
    const ON_3dPoint p = e.PointAt(e.Domain().Mid());
    m->scratch_points.insert(m->scratch_points.end(), {p.x, p.y, p.z});
  }
  return static_cast<int>(m->scratch_points.size() / 3);
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

// Location of a point object. Returns 1 if the object is an ON_Point.
int f3dm_point(const F3dmModel* m, int obj, double* xyz) {
  if (obj < 0 || static_cast<size_t>(obj) >= m->geometry.size()) return 0;
  const ON_Point* p = ON_Point::Cast(m->geometry[obj]);
  if (!p) return 0;
  xyz[0] = p->point.x;
  xyz[1] = p->point.y;
  xyz[2] = p->point.z;
  return 1;
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
  const int vcount = mesh->m_V.Count();
  for (int i = 0; i < mesh->m_F.Count(); i++) {
    const ON_MeshFace& f = mesh->m_F[i];
    // A damaged file may hold faces that point past the vertex list.
    if (!f.IsValid(vcount)) continue;
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
  bool has_color = false;           // colour for the next objects (else by layer)
  ON_Color color;
  ON_wString name;                  // name for the next objects (empty = none)
  bool hidden = false;              // state of the next objects
  bool locked = false;
  int group = -1;                   // Forma group of the next objects, -1 = none
  std::map<int, int> groups;        // Forma group -> model group index
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
  if (w->has_color) {
    a->SetColorSource(ON::color_from_object);
    a->m_color = w->color;
  }
  if (!w->name.IsEmpty()) a->SetName(static_cast<const wchar_t*>(w->name), true);
  if (w->hidden) a->SetVisible(false);
  if (w->locked) a->SetMode(ON::locked_object);
  if (w->group >= 0) {
    auto it = w->groups.find(w->group);
    if (it == w->groups.end()) {
      ON_Group g;
      const ON_ModelComponentReference gr = w->model.AddModelComponent(g, true);
      const ON_Group* mg = ON_Group::FromModelComponentRef(gr, nullptr);
      if (mg) it = w->groups.emplace(w->group, mg->Index()).first;
    }
    if (it != w->groups.end()) a->AddToGroup(it->second);
  }
  ON_ModelComponentReference ref = w->model.AddManagedModelGeometryComponent(geometry, a);
  return ref.IsEmpty() ? 0 : 1;
}

// Single-line text in the plane (origin = bottom-left, x = reading direction,
// y = up), UTF-8, height in model units. Rhino shows it as a normal text object.
int f3dm_writer_text(F3dmWriter* w, int layer, const double origin[3], const double xaxis[3],
                     const double yaxis[3], const char* text, double height) {
  const ON_DimStyle& ds = ON_DimStyle::Default;
  const ON_wString s{ON_String(text)};
  const ON_3dPoint o(origin[0], origin[1], origin[2]);
  const ON_3dVector x(xaxis[0], xaxis[1], xaxis[2]);
  const ON_3dVector y(yaxis[0], yaxis[1], yaxis[2]);
  ON_Plane pl(o, x, y);
  if (!pl.IsValid()) pl = ON_Plane(o, ON_3dVector::ZAxis);
  ON_Text* t = new ON_Text();
  if (!t->Create(static_cast<const wchar_t*>(s), &ds, pl)) {
    delete t;
    return 0;
  }
  if (height > 0.0) t->SetTextHeight(&ds, height);
  return add_object(w, layer, t);
}

// Hidden / locked state and group (-1 = none) of the objects added next.
void f3dm_writer_state(F3dmWriter* w, int hidden, int locked, int group) {
  w->hidden = hidden != 0;
  w->locked = locked != 0;
  w->group = group;
}

// Colour for the objects added next: has = 0 means "by layer".
void f3dm_writer_color(F3dmWriter* w, int has, const unsigned char rgb[3]) {
  w->has_color = has != 0;
  if (w->has_color) w->color = ON_Color(rgb[0], rgb[1], rgb[2]);
}

// Polycurve of n segments, 12 doubles each: kind (0 line, 1 arc), then
// line: a[3] b[3]; arc: center[3] xaxis[3] yaxis[3] radius sweep.
int f3dm_writer_polycurve(F3dmWriter* w, int layer, const double* data, int n) {
  ON_PolyCurve* pc = new ON_PolyCurve(n);
  for (int i = 0; i < n; i++) {
    const double* d = data + 12 * i;
    if (d[0] < 0.5) {
      pc->Append(new ON_LineCurve(ON_3dPoint(d + 1), ON_3dPoint(d + 4)));
    } else {
      const ON_3dPoint o(d + 1);
      const ON_3dVector vx(d + 4), vy(d + 7);
      ON_Plane plane(o, vx, vy);
      ON_Arc arc(plane, d[10], d[11]);
      pc->Append(new ON_ArcCurve(arc));
    }
  }
  return add_object(w, layer, pc);
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

// NURBS curve: degree, cv_count control points as xyz triples, one weight per
// point, and cv_count + degree - 1 knots (openNURBS convention).
int f3dm_writer_nurbs(F3dmWriter* w, int layer, int degree, int cv_count, const double* xyz,
                      const double* weights, const double* knots) {
  bool rational = false;
  for (int i = 0; i < cv_count; i++) {
    if (weights[i] != 1.0) rational = true;
  }
  ON_NurbsCurve* c = ON_NurbsCurve::New(3, rational, degree + 1, cv_count);
  for (int i = 0; i < cv_count; i++) {
    const double* p = xyz + 3 * i;
    if (rational) {
      const double wt = weights[i];
      c->SetCV(i, ON_4dPoint(p[0] * wt, p[1] * wt, p[2] * wt, wt));
    } else {
      c->SetCV(i, ON_3dPoint(p));
    }
  }
  for (int i = 0; i < cv_count + degree - 1; i++) c->SetKnot(i, knots[i]);
  if (!c->IsValid()) {
    delete c;
    return 0;
  }
  return add_object(w, layer, c);
}

// Name (UTF-8) for the objects added next; null or empty = no name.
void f3dm_writer_name(F3dmWriter* w, const char* name) {
  if (name && name[0]) {
    w->name = ON_wString(ON_String(name));
  } else {
    w->name = ON_wString::EmptyString;
  }
}

// Text dot with UTF-8 text and font height in points.
int f3dm_writer_textdot(F3dmWriter* w, int layer, const double xyz[3], const char* text,
                        int height) {
  const ON_wString wtext{ON_String(text)};
  ON_TextDot* dot = new ON_TextDot(ON_3dPoint(xyz), static_cast<const wchar_t*>(wtext), nullptr);
  dot->SetHeightInPoints(height);
  return add_object(w, layer, dot);
}

int f3dm_writer_point(F3dmWriter* w, int layer, const double xyz[3]) {
  return add_object(w, layer, new ON_Point(ON_3dPoint(xyz)));
}

int f3dm_writer_save(F3dmWriter* w, const char* path) { return w->model.Write(path, 0) ? 1 : 0; }

}  // extern "C"

// ---------------------------------------------------------------------------
// Exact geometry as bytes. Breps, extrusions and surfaces are serialized with
// openNURBS' own archive format; Forma keeps the bytes opaque (with a display
// mesh) and hands them back when saving, so they reach Rhino unchanged.

#include <cstdlib>

extern "C" {

static unsigned char* to_bytes(const ON_Object* obj, size_t* len) {
  *len = 0;
  if (!obj) return nullptr;
  ON_Write3dmBufferArchive a(0, 0, 0, ON::Version());
  if (!a.WriteObject(obj)) return nullptr;
  const size_t n = a.SizeOfArchive();
  if (n == 0) return nullptr;
  unsigned char* out = static_cast<unsigned char*>(std::malloc(n));
  if (!out) return nullptr;
  std::memcpy(out, a.Buffer(), n);
  *len = n;
  return out;
}

static ON_Geometry* from_bytes(const unsigned char* data, size_t len) {
  if (!data || len == 0) return nullptr;
  ON_Read3dmBufferArchive a(len, data, false, ON_BinaryArchive::CurrentArchiveVersion(),
                            ON::Version());
  ON_Object* obj = nullptr;
  if (a.ReadObject(&obj) != 1 || !obj) {
    delete obj;
    return nullptr;
  }
  ON_Geometry* g = ON_Geometry::Cast(obj);
  if (!g) delete obj;
  return g;
}

// Turns every surface and edge curve of a brep into NURBS so a shear or a
// non-uniform scale can be applied exactly. Surfaces whose NURBS form has
// another parameterization (surfaces of revolution) are accepted when all
// their trims are straight in (u, v): the trims are mapped onto the NURBS
// parameters. Returns false when some face cannot be converted.
static bool nurbify(ON_Brep* b) {
  for (int si = 0; si < b->m_S.Count(); si++) {
    ON_Surface* s = b->m_S[si];
    if (!s || ON_NurbsSurface::Cast(s)) continue;
    ON_NurbsSurface* ns = ON_NurbsSurface::New();
    const int rc = s->GetNurbForm(*ns);
    if (rc == 0) {
      delete ns;
      return false;
    }
    std::vector<int> faces;
    for (int fi = 0; fi < b->m_F.Count(); fi++) {
      if (b->m_F[fi].m_si == si) faces.push_back(fi);
    }
    if (rc == 2) {
      // Map the straight trims onto the new parameters.
      for (int fi : faces) {
        const ON_BrepFace& f = b->m_F[fi];
        for (int li = 0; li < f.m_li.Count(); li++) {
          const ON_BrepLoop& l = b->m_L[f.m_li[li]];
          for (int ti = 0; ti < l.m_ti.Count(); ti++) {
            ON_BrepTrim& t = b->m_T[l.m_ti[ti]];
            if (!t.IsLinear(1e-9)) {
              delete ns;
              return false;
            }
            const ON_3dPoint a = t.PointAtStart(), c = t.PointAtEnd();
            double au = 0, av = 0, cu = 0, cv = 0;
            if (!s->GetNurbFormParameterFromSurfaceParameter(a.x, a.y, &au, &av) ||
                !s->GetNurbFormParameterFromSurfaceParameter(c.x, c.y, &cu, &cv)) {
              delete ns;
              return false;
            }
            ON_LineCurve* line = new ON_LineCurve(ON_2dPoint(au, av), ON_2dPoint(cu, cv));
            line->SetDomain(t.Domain()[0], t.Domain()[1]);
            const int c2 = b->AddTrimCurve(line);
            t.ChangeTrimCurve(c2);
          }
        }
      }
    }
    delete s;
    b->m_S[si] = ns;
    for (int fi : faces) b->m_F[fi].SetProxySurface(ns);
  }
  for (int ci = 0; ci < b->m_C3.Count(); ci++) {
    ON_Curve* c = b->m_C3[ci];
    if (!c || ON_NurbsCurve::Cast(c)) continue;
    ON_NurbsCurve* nc = ON_NurbsCurve::New();
    if (!c->GetNurbForm(*nc)) {
      delete nc;
      return false;
    }
    nc->SetDomain(c->Domain()[0], c->Domain()[1]);
    delete c;
    b->m_C3[ci] = nc;
    for (int ei = 0; ei < b->m_E.Count(); ei++) {
      if (b->m_E[ei].m_c3i == ci) b->m_E[ei].ChangeEdgeCurve(ci);
    }
  }
  b->Compact();
  b->SetTolerancesBoxesAndFlags(false, false, false, false, true, true, true, true);
  return true;
}

// Applies a row-major 4x4 transform (null = none). Takes ownership of `g` and
// returns the result (possibly another object), or nullptr on failure.
static ON_Geometry* apply_xform(ON_Geometry* g, const double* x16) {
  if (!g || !x16) return g;
  ON_Xform x;
  for (int i = 0; i < 4; i++)
    for (int j = 0; j < 4; j++) x.m_xform[i][j] = x16[4 * i + j];
  if (x.IsIdentity()) return g;
  const bool mirror = x.Determinant() < 0.0;
  if (x.IsSimilarity() == 0 || (mirror && ON_Extrusion::Cast(g))) {
    // Shears and non-uniform scales: extrusions and special surfaces must become
    // NURBS breps first.
    if (ON_Extrusion::Cast(g) || ON_Surface::Cast(g)) {
      ON_Brep* b = g->BrepForm(nullptr);
      if (!b) {
        delete g;
        return nullptr;
      }
      delete g;
      g = b;
    }
    if (x.IsSimilarity() == 0) {
      ON_Brep* b = ON_Brep::Cast(g);
      if (b && !nurbify(b)) {
        delete g;
        return nullptr;
      }
    }
    g->MakeDeformable();
  }
  if (!g->Transform(x)) {
    delete g;
    return nullptr;
  }
  // A mirror turns every face inside out: flip them back so solids keep
  // facing outwards (and surfaces keep their side).
  if (ON_Brep* b = ON_Brep::Cast(g)) {
    if (mirror) b->Flip();
  }
  return g;
}

void f3dm_bytes_free(unsigned char* p) { std::free(p); }

// Serialized exact geometry of a brep, extrusion or surface object (the
// object as stored in the file; block members already carry their transform).
unsigned char* f3dm_object_bytes(const F3dmModel* m, int obj, size_t* len) {
  *len = 0;
  if (obj < 0 || static_cast<size_t>(obj) >= m->geometry.size()) return nullptr;
  const ON_Geometry* g = m->geometry[obj];
  if (!ON_Brep::Cast(g) && !ON_Extrusion::Cast(g) && !ON_Surface::Cast(g)) return nullptr;
  return to_bytes(g, len);
}

// A model holding one object read from bytes (transformed by x16 unless null),
// for the display functions above (object index 0). Free with f3dm_free.
F3dmModel* f3dm_bytes_model(const unsigned char* data, size_t len, const double* x16) {
  ON_Geometry* g = apply_xform(from_bytes(data, len), x16);
  if (!g) return nullptr;
  F3dmModel* m = new F3dmModel();
  m->owned_geometry.push_back(g);
  const std::map<int, int> none;
  read_object(m, none, g, nullptr, -1, -1, nullptr);
  return m;
}

// The bytes with the transform applied (for exact operations on moved objects).
unsigned char* f3dm_bytes_transformed(const unsigned char* data, size_t len, const double* x16,
                                      size_t* out_len) {
  *out_len = 0;
  ON_Geometry* g = apply_xform(from_bytes(data, len), x16);
  if (!g) return nullptr;
  unsigned char* r = to_bytes(g, out_len);
  delete g;
  return r;
}

// The bytes of a brep with all its faces reversed (inside out).
unsigned char* f3dm_bytes_flipped(const unsigned char* data, size_t len, size_t* out_len) {
  *out_len = 0;
  ON_Geometry* g = from_bytes(data, len);
  ON_Brep* b = ON_Brep::Cast(g);
  if (!b && g) {
    b = g->BrepForm(nullptr);
    delete g;
    g = b;
  }
  if (!b) {
    delete g;
    return nullptr;
  }
  b->Flip();
  unsigned char* r = to_bytes(b, out_len);
  delete b;
  return r;
}

// Adds exact geometry from bytes (transformed by x16 unless null). Returns 0 when
// the bytes cannot be read or the transform fails (the caller writes a mesh).
int f3dm_writer_bytes(F3dmWriter* w, int layer, const unsigned char* data, size_t len,
                      const double* x16) {
  ON_Geometry* g = apply_xform(from_bytes(data, len), x16);
  if (!g) return 0;
  if (!g->IsValid()) {
    delete g;
    return 0;
  }
  return add_object(w, layer, g);
}

static unsigned char* brep_bytes(ON_Brep* b, size_t* len) {
  *len = 0;
  if (!b) return nullptr;
  unsigned char* r = nullptr;
  ON_wString log_str;
  ON_TextLog log(log_str);
  if (!b->IsValid(&log) && std::getenv("F3DM_DEBUG")) {
    ON_String s(log_str);
    fprintf(stderr, "invalid brep: %s\n", static_cast<const char*>(s));
  }
  if (b->IsValid()) {
    if (b->IsSolid() && b->SolidOrientation() == -1) b->Flip();
    r = to_bytes(b, len);
  }
  delete b;
  return r;
}

// Box from 8 corners: bottom 0-3 counter-clockwise seen from above, top 4-7 above them.
unsigned char* f3dm_make_box(const double* c, size_t* len) {
  ON_3dPoint p[8];
  for (int i = 0; i < 8; i++) p[i] = ON_3dPoint(c + 3 * i);
  return brep_bytes(ON_BrepBox(p, nullptr), len);
}

// Plane from origin, x axis and y axis (9 doubles).
static ON_Plane plane_of(const double* p) {
  return ON_Plane(ON_3dPoint(p), ON_3dVector(p + 3), ON_3dVector(p + 6));
}

// Cylinder on a base plane (origin = base centre), capped at both ends.
unsigned char* f3dm_make_cylinder(const double* plane, double radius, double height,
                                  size_t* len) {
  ON_Cylinder cyl(ON_Circle(plane_of(plane), radius), height);
  return brep_bytes(ON_BrepCylinder(cyl, true, true, nullptr), len);
}

// Cone with its base on the plane and the apex at `height` along its normal.
unsigned char* f3dm_make_cone(const double* plane, double radius, double height, size_t* len) {
  ON_Plane pl = plane_of(plane);
  // ON_Cone: apex at the plane origin, base at `height` along the normal.
  pl.origin = pl.origin + pl.zaxis * height;
  pl.Flip();
  pl.UpdateEquation();
  ON_Cone cone(pl, height, radius);
  return brep_bytes(ON_BrepCone(cone, true, nullptr), len);
}

unsigned char* f3dm_make_sphere(const double center[3], double radius, size_t* len) {
  ON_Sphere s(ON_3dPoint(center), radius);
  return brep_bytes(ON_BrepSphere(s, nullptr), len);
}

unsigned char* f3dm_make_torus(const double* plane, double major, double minor, size_t* len) {
  ON_Torus t(plane_of(plane), major, minor);
  return brep_bytes(ON_BrepTorus(t, nullptr), len);
}

// Curves of one loop from a segment stream: per segment a kind, then
// 0 line: a[3] b[3]; 1 arc: center[3] xaxis[3] yaxis[3] radius sweep;
// 2 nurbs: degree n xyz[3n] weights[n] knots[n+degree-1]. Advances *k.
static bool read_loop(const double* s, size_t n, size_t* k, ON_SimpleArray<ON_Curve*>& out) {
  if (*k >= n) return false;
  const int segs = static_cast<int>(s[(*k)++]);
  for (int i = 0; i < segs; i++) {
    if (*k >= n) return false;
    const int kind = static_cast<int>(s[(*k)++]);
    if (kind == 0) {
      if (*k + 6 > n) return false;
      out.Append(new ON_LineCurve(ON_3dPoint(s + *k), ON_3dPoint(s + *k + 3)));
      *k += 6;
    } else if (kind == 1) {
      if (*k + 11 > n) return false;
      const double* d = s + *k;
      ON_Plane plane(ON_3dPoint(d), ON_3dVector(d + 3), ON_3dVector(d + 6));
      out.Append(new ON_ArcCurve(ON_Arc(plane, d[9], d[10])));
      *k += 11;
    } else if (kind == 2) {
      if (*k + 2 > n) return false;
      const int degree = static_cast<int>(s[*k]);
      const int cvs = static_cast<int>(s[*k + 1]);
      *k += 2;
      const size_t need = static_cast<size_t>(cvs) * 4 + cvs + degree - 1;
      if (degree < 1 || cvs <= degree || *k + need > n) return false;
      const double* xyz = s + *k;
      const double* w = xyz + 3 * cvs;
      const double* knots = w + cvs;
      bool rational = false;
      for (int j = 0; j < cvs; j++) rational |= w[j] != 1.0;
      ON_NurbsCurve* c = ON_NurbsCurve::New(3, rational, degree + 1, cvs);
      for (int j = 0; j < cvs; j++) {
        const double* p = xyz + 3 * j;
        if (rational) {
          c->SetCV(j, ON_4dPoint(p[0] * w[j], p[1] * w[j], p[2] * w[j], w[j]));
        } else {
          c->SetCV(j, ON_3dPoint(p));
        }
      }
      for (int j = 0; j < cvs + degree - 1; j++) c->SetKnot(j, knots[j]);
      out.Append(c);
      *k += need;
    } else {
      return false;
    }
  }
  return out.Count() > 0;
}

// Planar face bounded by `loops` closed loops (the first is the outer boundary,
// the others holes) on `plane` (9 doubles), extruded along `dir` and capped
// when dir is not null (a solid), else just the trimmed plane.
unsigned char* f3dm_make_planar(const double* plane, const double* stream, size_t n, int loops,
                                const double* dir, size_t* len) {
  *len = 0;
  size_t k = 0;
  ON_SimpleArray<ON_Curve*> outer;
  if (!read_loop(stream, n, &k, outer)) {
    for (int i = 0; i < outer.Count(); i++) delete outer[i];
    return nullptr;
  }
  ON_Brep* b = ON_BrepTrimmedPlane(plane_of(plane), outer, false, nullptr);
  if (!b) {
    for (int i = 0; i < outer.Count(); i++) delete outer[i];
    return nullptr;
  }
  for (int l = 1; l < loops; l++) {
    ON_SimpleArray<ON_Curve*> hole;
    if (!read_loop(stream, n, &k, hole) ||
        !b->NewPlanarFaceLoop(0, ON_BrepLoop::inner, hole, false)) {
      for (int i = 0; i < hole.Count(); i++) delete hole[i];
      delete b;
      return nullptr;
    }
  }
  if (dir) {
    const ON_3dVector v(dir);
    ON_LineCurve path(ON_3dPoint::Origin, ON_3dPoint::Origin + v);
    if (!ON_BrepExtrude(*b, path, true)) {
      delete b;
      return nullptr;
    }
  }
  b->Compact();
  return brep_bytes(b, len);
}

}  // extern "C"

extern "C" {

// Polysurface of planar faces (a polyhedron). `pts`: npts xyz triples. `data`:
// face count, then per face: loop count, then per loop: vertex count and vertex
// indices (outer loop first, counter-clockwise around the face normal; holes
// clockwise). Neighbouring faces share edges, so a closed polyhedron is a solid.
unsigned char* f3dm_make_polyhedron(const double* pts, int npts, const int* data, size_t ndata,
                                    size_t* len) {
  *len = 0;
  ON_Brep* b = new ON_Brep();
  for (int i = 0; i < npts; i++) b->NewVertex(ON_3dPoint(pts + 3 * i), 0.0);
  std::map<std::pair<int, int>, int> edges;
  size_t k = 0;
  auto next = [&](int* v) {
    if (k >= ndata) return false;
    *v = data[k++];
    return true;
  };
  auto fail = [&]() -> unsigned char* {
    delete b;
    return nullptr;
  };
  int nfaces = 0;
  if (!next(&nfaces)) return fail();
  for (int f = 0; f < nfaces; f++) {
    int nloops = 0;
    if (!next(&nloops) || nloops < 1) return fail();
    std::vector<std::vector<int>> loops(nloops);
    for (auto& l : loops) {
      int n = 0;
      if (!next(&n) || n < 3) return fail();
      l.resize(n);
      for (int& v : l) {
        if (!next(&v) || v < 0 || v >= npts) return fail();
      }
    }
    // Plane: Newell normal of the outer loop, x along its first edge.
    ON_3dVector nrm(0, 0, 0);
    const std::vector<int>& outer = loops[0];
    for (size_t i = 0; i < outer.size(); i++) {
      const ON_3dPoint a = b->m_V[outer[i]].point;
      const ON_3dPoint c = b->m_V[outer[(i + 1) % outer.size()]].point;
      nrm.x += (a.y - c.y) * (a.z + c.z);
      nrm.y += (a.z - c.z) * (a.x + c.x);
      nrm.z += (a.x - c.x) * (a.y + c.y);
    }
    if (!nrm.Unitize()) return fail();
    const ON_3dPoint origin = b->m_V[outer[0]].point;
    ON_3dVector xdir = b->m_V[outer[1]].point - origin;
    xdir = xdir - (xdir * nrm) * nrm;
    if (!xdir.Unitize()) return fail();
    const ON_Plane plane(origin, xdir, ON_CrossProduct(nrm, xdir));
    ON_BoundingBox box;
    for (int v : outer) {
      double u = 0, w = 0;
      plane.ClosestPointTo(b->m_V[v].point, &u, &w);
      box.Set(ON_3dPoint(u, w, 0), true);
    }
    ON_PlaneSurface* srf = new ON_PlaneSurface(plane);
    const double pad = 0.0;  // the face fits its outline exactly (tight bounding box)
    srf->SetExtents(0, ON_Interval(box.m_min.x - pad, box.m_max.x + pad), true);
    srf->SetExtents(1, ON_Interval(box.m_min.y - pad, box.m_max.y + pad), true);
    ON_BrepFace& face = b->NewFace(b->AddSurface(srf));
    for (int li = 0; li < nloops; li++) {
      const std::vector<int>& l = loops[li];
      ON_BrepLoop& loop = b->NewLoop(li == 0 ? ON_BrepLoop::outer : ON_BrepLoop::inner, face);
      for (size_t i = 0; i < l.size(); i++) {
        const int v0 = l[i], v1 = l[(i + 1) % l.size()];
        if (v0 == v1) return fail();
        const std::pair<int, int> key((std::min)(v0, v1), (std::max)(v0, v1));
        auto it = edges.find(key);
        if (it == edges.end()) {
          const int c3 = b->AddEdgeCurve(new ON_LineCurve(b->m_V[v0].point, b->m_V[v1].point));
          ON_BrepEdge& e = b->NewEdge(b->m_V[v0], b->m_V[v1], c3);
          e.m_tolerance = 0.0;
          it = edges.emplace(key, e.m_edge_index).first;
        }
        ON_BrepEdge& e = b->m_E[it->second];
        if (e.m_ti.Count() >= 2) return fail();  // non-manifold
        const bool rev = e.m_vi[0] != v0;
        double u0, w0, u1, w1;
        plane.ClosestPointTo(b->m_V[v0].point, &u0, &w0);
        plane.ClosestPointTo(b->m_V[v1].point, &u1, &w1);
        const int c2 = b->AddTrimCurve(new ON_LineCurve(ON_2dPoint(u0, w0), ON_2dPoint(u1, w1)));
        ON_BrepTrim& t = b->NewTrim(e, rev, loop, c2);
        t.m_tolerance[0] = t.m_tolerance[1] = 0.0;
        t.m_type = ON_BrepTrim::boundary;
      }
    }
  }
  for (int i = 0; i < b->m_E.Count(); i++) {
    if (!(b->m_E[i].m_tolerance >= 0.0)) b->m_E[i].m_tolerance = 0.0;
  }
  for (int i = 0; i < b->m_V.Count(); i++) {
    if (!(b->m_V[i].m_tolerance >= 0.0)) b->m_V[i].m_tolerance = 0.0;
  }
  b->SetTolerancesBoxesAndFlags(false, false, false, false, true, true, true, true);
  return brep_bytes(b, len);
}

}  // extern "C"

extern "C" {

// Surfaces swept by the curves of one loop stream (see read_loop) moving along
// `dir`: one face per segment, joined at their side edges (closed when the
// chain is closed). Not capped.
unsigned char* f3dm_make_extruded_curve(const double* stream, size_t n, const double* dir,
                                        size_t* len) {
  *len = 0;
  size_t k = 0;
  ON_SimpleArray<ON_Curve*> curves;
  if (!read_loop(stream, n, &k, curves)) {
    for (int i = 0; i < curves.Count(); i++) delete curves[i];
    return nullptr;
  }
  const ON_3dVector v(dir);
  const int count = curves.Count();
  const bool closed =
      curves[0]->PointAtStart().DistanceTo(curves[count - 1]->PointAtEnd()) <= ON_ZERO_TOLERANCE * 1e3;
  ON_Brep* b = new ON_Brep();
  auto vertex = [&](const ON_3dPoint& p) { return b->NewVertex(p, 0.0).m_vertex_index; };
  // Bottom and top vertices of each segment start (and the chain end).
  std::vector<int> bottom, top;
  for (int i = 0; i < count; i++) {
    bottom.push_back(vertex(curves[i]->PointAtStart()));
    top.push_back(vertex(curves[i]->PointAtStart() + v));
  }
  if (closed) {
    bottom.push_back(bottom[0]);
    top.push_back(top[0]);
  } else {
    bottom.push_back(vertex(curves[count - 1]->PointAtEnd()));
    top.push_back(vertex(curves[count - 1]->PointAtEnd() + v));
  }
  // Side (vertical) edges, shared by neighbouring faces.
  std::vector<int> side;
  for (size_t i = 0; i < bottom.size(); i++) {
    if (closed && i + 1 == bottom.size()) {
      side.push_back(side[0]);
      break;
    }
    const ON_3dPoint p = b->m_V[bottom[i]].point;
    const int c3 = b->AddEdgeCurve(new ON_LineCurve(p, p + v));
    side.push_back(b->NewEdge(b->m_V[bottom[i]], b->m_V[top[i]], c3).m_edge_index);
  }
  for (int i = 0; i < count; i++) {
    ON_SumSurface* srf = new ON_SumSurface();
    if (!srf->Create(*curves[i], v)) {
      delete srf;
      for (int j = i; j < count; j++) delete curves[j];
      delete b;
      return nullptr;
    }
    const ON_Interval du = srf->Domain(0), dv = srf->Domain(1);
    ON_BrepFace& face = b->NewFace(b->AddSurface(srf));
    ON_BrepLoop& loop = b->NewLoop(ON_BrepLoop::outer, face);
    ON_Curve* top_curve = curves[i]->DuplicateCurve();
    top_curve->Translate(v);
    const int e_bottom =
        b->NewEdge(b->m_V[bottom[i]], b->m_V[bottom[i + 1]], b->AddEdgeCurve(curves[i])).m_edge_index;
    const int e_top =
        b->NewEdge(b->m_V[top[i]], b->m_V[top[i + 1]], b->AddEdgeCurve(top_curve)).m_edge_index;
    const ON_2dPoint c[4] = {ON_2dPoint(du[0], dv[0]), ON_2dPoint(du[1], dv[0]),
                             ON_2dPoint(du[1], dv[1]), ON_2dPoint(du[0], dv[1])};
    const int edges[4] = {e_bottom, side[i + 1], e_top, side[i]};
    const bool rev[4] = {false, false, true, true};
    for (int s = 0; s < 4; s++) {
      const int c2 = b->AddTrimCurve(new ON_LineCurve(c[s], c[(s + 1) % 4]));
      ON_BrepTrim& t = b->NewTrim(b->m_E[edges[s]], rev[s], loop, c2);
      t.m_tolerance[0] = t.m_tolerance[1] = 0.0;
    }
  }
  for (int i = 0; i < b->m_E.Count(); i++) {
    if (!(b->m_E[i].m_tolerance >= 0.0)) b->m_E[i].m_tolerance = 0.0;
  }
  for (int i = 0; i < b->m_V.Count(); i++) {
    if (!(b->m_V[i].m_tolerance >= 0.0)) b->m_V[i].m_tolerance = 0.0;
  }
  b->SetTolerancesBoxesAndFlags(false, false, false, false, true, true, true, true);
  return brep_bytes(b, len);
}

}  // extern "C"

extern "C" {

static ON_Brep* brep_from_bytes(const unsigned char* data, size_t len) {
  ON_Geometry* g = from_bytes(data, len);
  if (!g) return nullptr;
  ON_Brep* b = ON_Brep::Cast(g);
  if (b) return b;
  b = g->BrepForm(nullptr);
  delete g;
  return b;
}

// Face `i` of a brep as a brep of its own (Explode).
unsigned char* f3dm_bytes_face(const unsigned char* data, size_t len, int i, size_t* out_len) {
  *out_len = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  ON_Brep* f = (i >= 0 && i < b->m_F.Count()) ? b->DuplicateFace(i, false) : nullptr;
  delete b;
  if (!f) return nullptr;
  f->Compact();
  return brep_bytes(f, out_len);
}

// Appends one curve to a segment stream (see read_loop): line, arc or NURBS.
static void push_curve(const ON_Curve& c, std::vector<double>& out) {
  ON_Arc arc;
  if (c.IsLinear(1e-9)) {
    const ON_3dPoint a = c.PointAtStart(), b = c.PointAtEnd();
    out.insert(out.end(), {0.0, a.x, a.y, a.z, b.x, b.y, b.z});
    return;
  }
  if (c.IsArc(nullptr, &arc, 1e-9) && arc.Radius() > 0.0) {
    // Re-base the plane so the arc starts on its x axis.
    ON_Plane p = arc.plane;
    const double a0 = arc.DomainRadians()[0];
    p.Rotate(a0, p.zaxis);
    const ON_3dPoint o = p.origin;
    out.insert(out.end(), {1.0, o.x, o.y, o.z, p.xaxis.x, p.xaxis.y, p.xaxis.z, p.yaxis.x,
                           p.yaxis.y, p.yaxis.z, arc.Radius(), arc.AngleRadians()});
    return;
  }
  ON_NurbsCurve n;
  if (!c.GetNurbForm(n) || n.Dimension() != 3) return;
  out.insert(out.end(), {2.0, static_cast<double>(n.Degree()), static_cast<double>(n.CVCount())});
  for (int i = 0; i < n.CVCount(); i++) {
    ON_3dPoint p;
    n.GetCV(i, p);  // Euclidean point (divided by the weight)
    out.insert(out.end(), {p.x, p.y, p.z});
  }
  for (int i = 0; i < n.CVCount(); i++) out.push_back(n.Weight(i));
  for (int i = 0; i < n.KnotCount(); i++) out.push_back(n.Knot(i));
}

// Edge curves of a brep as a segment stream: count, then one segment per edge.
// `which`: 0 all edges, 1 naked (border) edges only.
static std::vector<double> edges_stream(const ON_Brep* b, int which) {
  std::vector<double> out{0.0};
  int count = 0;
  for (int i = 0; i < b->m_E.Count(); i++) {
    const ON_BrepEdge& e = b->m_E[i];
    if (e.m_edge_index < 0 || !e.EdgeCurveOf()) continue;
    if (which == 1 && e.m_ti.Count() != 1) continue;
    const size_t before = out.size();
    push_curve(e, out);
    if (out.size() > before) count++;
  }
  out[0] = count;
  return out;
}

static double* to_malloc(const std::vector<double>& out, size_t* n) {
  double* r = static_cast<double*>(std::malloc(out.size() * sizeof(double)));
  if (!r) return nullptr;
  std::memcpy(r, out.data(), out.size() * sizeof(double));
  *n = out.size();
  return r;
}

// Edge stream of a brep object of a model (see f3dm_bytes_edges).
double* f3dm_object_edges(const F3dmModel* m, int obj, size_t* n) {
  *n = 0;
  const ON_Brep* b = brep_of(m, obj);
  if (!b) return nullptr;
  return to_malloc(edges_stream(b, 0), n);
}

double* f3dm_bytes_edges(const unsigned char* data, size_t len, int which, size_t* n) {
  *n = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  std::vector<double> out = edges_stream(b, which);
  delete b;
  double* r = static_cast<double*>(std::malloc(out.size() * sizeof(double)));
  if (!r) return nullptr;
  std::memcpy(r, out.data(), out.size() * sizeof(double));
  *n = out.size();
  return r;
}

// Outline of face `i`: its loops as segment streams (loop count, then streams).
double* f3dm_bytes_face_loops(const unsigned char* data, size_t len, int i, size_t* n) {
  *n = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b || i < 0 || i >= b->m_F.Count()) {
    delete b;
    return nullptr;
  }
  const ON_BrepFace& f = b->m_F[i];
  std::vector<double> out{static_cast<double>(f.m_li.Count())};
  for (int li = 0; li < f.m_li.Count(); li++) {
    const ON_BrepLoop& l = b->m_L[f.m_li[li]];
    const size_t at = out.size();
    out.push_back(0.0);
    int count = 0;
    for (int ti = 0; ti < l.m_ti.Count(); ti++) {
      const ON_BrepTrim& t = b->m_T[l.m_ti[ti]];
      if (t.m_ei < 0) continue;  // singular trim (pole)
      ON_Curve* c = b->m_E[t.m_ei].DuplicateCurve();
      if (!c) continue;
      if (t.m_bRev3d) c->Reverse();
      const size_t before = out.size();
      push_curve(*c, out);
      delete c;
      if (out.size() > before) count++;
    }
    out[at] = count;
  }
  delete b;
  double* r = static_cast<double*>(std::malloc(out.size() * sizeof(double)));
  if (!r) return nullptr;
  std::memcpy(r, out.data(), out.size() * sizeof(double));
  *n = out.size();
  return r;
}

}  // extern "C"

extern "C" {

// The brep moved along `dir`, sweeping its faces into a solid (ExtrudeSrf).
unsigned char* f3dm_bytes_extruded(const unsigned char* data, size_t len, const double* dir,
                                   size_t* out_len) {
  *out_len = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  const ON_3dVector v(dir);
  ON_LineCurve path(ON_3dPoint::Origin, ON_3dPoint::Origin + v);
  if (!ON_BrepExtrude(*b, path, true)) {
    delete b;
    return nullptr;
  }
  b->Compact();
  return brep_bytes(b, out_len);
}

// Closes the planar holes of a brep with trimmed planes (Cap). Each hole is a
// chain of naked edges; the cap runs along them against the existing faces, so
// the result stays consistently oriented. Returns null when nothing was capped;
// *caps gets the number of caps added.
unsigned char* f3dm_bytes_capped(const unsigned char* data, size_t len, double tol, int* caps,
                                 size_t* out_len) {
  *out_len = 0;
  *caps = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  // Directed naked edges: (edge, reversed) so the cap goes against the face.
  struct Dir {
    int edge;
    bool rev;
    int from, to;
  };
  std::vector<Dir> naked;
  for (int ei = 0; ei < b->m_E.Count(); ei++) {
    const ON_BrepEdge& e = b->m_E[ei];
    if (e.m_ti.Count() != 1) continue;
    const ON_BrepTrim& t = b->m_T[e.m_ti[0]];
    const bool rev = !t.m_bRev3d;  // opposite to the face's trim
    naked.push_back({ei, rev, e.m_vi[rev ? 1 : 0], e.m_vi[rev ? 0 : 1]});
  }
  std::vector<bool> used(naked.size(), false);
  for (size_t start = 0; start < naked.size(); start++) {
    if (used[start]) continue;
    std::vector<size_t> chain{start};
    used[start] = true;
    bool closed = false;
    for (int guard = 0; guard < 100000; guard++) {
      const int at = naked[chain.back()].to;
      if (at == naked[start].from) {
        closed = true;
        break;
      }
      size_t next = naked.size();
      for (size_t j = 0; j < naked.size(); j++) {
        if (!used[j] && naked[j].from == at) {
          next = j;
          break;
        }
      }
      if (next == naked.size()) break;
      used[next] = true;
      chain.push_back(next);
    }
    if (!closed) continue;
    // Plane through the chain (Newell normal in cap direction), checked flat.
    std::vector<ON_3dPoint> pts;
    for (size_t k : chain) {
      const ON_BrepEdge& e = b->m_E[naked[k].edge];
      const ON_Interval d = e.Domain();
      for (int s = 0; s < 8; s++) {
        const double t = naked[k].rev ? d.ParameterAt(1.0 - s / 8.0) : d.ParameterAt(s / 8.0);
        pts.push_back(e.PointAt(t));
      }
    }
    ON_3dVector nrm(0, 0, 0);
    ON_3dPoint c(0, 0, 0);
    for (size_t i = 0; i < pts.size(); i++) {
      const ON_3dPoint& a = pts[i];
      const ON_3dPoint& q = pts[(i + 1) % pts.size()];
      nrm.x += (a.y - q.y) * (a.z + q.z);
      nrm.y += (a.z - q.z) * (a.x + q.x);
      nrm.z += (a.x - q.x) * (a.y + q.y);
      c = c + a;
    }
    if (!nrm.Unitize()) continue;
    c = c / static_cast<double>(pts.size());
    ON_Plane plane(c, nrm);
    bool flat = true;
    for (const ON_3dPoint& p : pts) flat &= std::fabs(plane.DistanceTo(p)) <= tol;
    if (!flat) continue;
    ON_BoundingBox box;
    for (const ON_3dPoint& p : pts) {
      double u = 0, w = 0;
      plane.ClosestPointTo(p, &u, &w);
      box.Set(ON_3dPoint(u, w, 0), true);
    }
    ON_PlaneSurface* srf = new ON_PlaneSurface(plane);
    const double pad = 0.0;  // the face fits its outline exactly (tight bounding box)
    srf->SetExtents(0, ON_Interval(box.m_min.x - pad, box.m_max.x + pad), true);
    srf->SetExtents(1, ON_Interval(box.m_min.y - pad, box.m_max.y + pad), true);
    ON_BrepFace& face = b->NewFace(b->AddSurface(srf));
    ON_BrepLoop& loop = b->NewLoop(ON_BrepLoop::outer, face);
    // Plane coordinates of a 3D point (the affine map used for the 2D trims).
    ON_Xform to_uv;
    to_uv.ChangeBasis(ON_Plane::World_xy, plane);
    for (size_t k : chain) {
      ON_BrepEdge& e = b->m_E[naked[k].edge];
      ON_NurbsCurve n;
      if (!e.GetNurbForm(n)) continue;
      if (naked[k].rev) n.Reverse();
      n.Transform(to_uv);
      n.ChangeDimension(2);
      const int c2 = b->AddTrimCurve(new ON_NurbsCurve(n));
      ON_BrepTrim& t = b->NewTrim(e, naked[k].rev, loop, c2);
      t.m_tolerance[0] = t.m_tolerance[1] = 0.0;
    }
    (*caps)++;
  }
  if (*caps == 0) {
    delete b;
    return nullptr;
  }
  b->SetTolerancesBoxesAndFlags(false, false, false, false, true, true, true, true);
  return brep_bytes(b, out_len);
}

}  // extern "C"

extern "C" {

static bool line_or_arc(const ON_Curve& c) {
  ON_Arc arc;
  return c.IsLinear(1e-9) || (c.IsArc(nullptr, &arc, 1e-9) && arc.Radius() > 0.0);
}

// Segments of a curve: lines and arcs as they are, anything else one NURBS.
static void curve_segments(const ON_Curve& c, std::vector<const ON_Curve*>& out, bool* all) {
  if (const ON_PolyCurve* pc = ON_PolyCurve::Cast(&c)) {
    for (int i = 0; i < pc->Count(); i++) {
      if (const ON_Curve* s = pc->SegmentCurve(i)) curve_segments(*s, out, all);
    }
    return;
  }
  if (!line_or_arc(c) && !ON_PolylineCurve::Cast(&c)) *all = false;
  out.push_back(&c);
}

// A curve object as a segment stream (count, then segments; see read_loop):
// lines and arcs exactly, polylines as lines, other curves as one NURBS curve.
double* f3dm_curve_exact(const F3dmModel* m, int obj, size_t* n) {
  *n = 0;
  if (obj < 0 || static_cast<size_t>(obj) >= m->geometry.size()) return nullptr;
  const ON_Curve* c = ON_Curve::Cast(m->geometry[obj]);
  if (!c) return nullptr;
  std::vector<const ON_Curve*> segs;
  bool all = true;
  curve_segments(*c, segs, &all);
  std::vector<double> out{0.0};
  int count = 0;
  if (!all) {
    push_curve(*c, out);  // not a line or an arc: its NURBS form
    count = out.size() > 1 ? 1 : 0;
  } else {
    for (const ON_Curve* s : segs) {
      if (const ON_PolylineCurve* pl = ON_PolylineCurve::Cast(s)) {
        for (int i = 0; i + 1 < pl->m_pline.Count(); i++) {
          const ON_3dPoint a = pl->m_pline[i], b = pl->m_pline[i + 1];
          if (a.DistanceTo(b) <= 0.0) continue;
          out.insert(out.end(), {0.0, a.x, a.y, a.z, b.x, b.y, b.z});
          count++;
        }
        continue;
      }
      const size_t before = out.size();
      push_curve(*s, out);
      if (out.size() > before) count++;
    }
  }
  out[0] = count;
  double* r = static_cast<double*>(std::malloc(out.size() * sizeof(double)));
  if (!r) return nullptr;
  std::memcpy(r, out.data(), out.size() * sizeof(double));
  *n = out.size();
  return r;
}

}  // extern "C"

extern "C" {

// True when the curves coincide within `tol` (b sampled densely, a checked
// against it), ignoring direction.
static bool same_place(const ON_Curve& a, const ON_Curve& b, double tol) {
  std::vector<ON_3dPoint> pts;
  const ON_Interval db = b.Domain();
  for (int i = 0; i <= 64; i++) pts.push_back(b.PointAt(db.ParameterAt(i / 64.0)));
  double chord = 0.0;
  for (size_t i = 1; i < pts.size(); i++) chord = (std::max)(chord, pts[i].DistanceTo(pts[i - 1]));
  const ON_Interval da = a.Domain();
  for (double k : {0.2, 0.5, 0.8}) {
    const ON_3dPoint p = a.PointAt(da.ParameterAt(k));
    double best = 1e300;
    for (size_t i = 1; i < pts.size(); i++) {
      const ON_Line l(pts[i - 1], pts[i]);
      double t = 0;
      l.ClosestPointTo(p, &t);
      t = (std::max)(0.0, (std::min)(1.0, t));
      best = (std::min)(best, p.DistanceTo(l.PointAt(t)));
    }
    if (best > tol + 0.02 * chord) return false;
  }
  return true;
}

// Reverses edge `ei` (a new edge with the reversed curve takes its trims).
// Returns the index of the new edge.
static int reverse_edge(ON_Brep* b, int ei) {
  ON_Curve* c = b->m_E[ei].DuplicateCurve();
  if (!c) return -1;
  c->Reverse();
  const int v0 = b->m_E[ei].m_vi[1], v1 = b->m_E[ei].m_vi[0];
  const int c3 = b->AddEdgeCurve(c);
  ON_BrepEdge& ne = b->NewEdge(b->m_V[v0], b->m_V[v1], c3);
  ne.m_tolerance = b->m_E[ei].m_tolerance;
  const int ni = ne.m_edge_index;
  ON_SimpleArray<int> trims = b->m_E[ei].m_ti;
  for (int k = 0; k < trims.Count(); k++) {
    ON_BrepTrim& t = b->m_T[trims[k]];
    t.m_ei = ni;
    t.m_bRev3d = !t.m_bRev3d;
    t.SetProxyCurve(t.ProxyCurve());  // unchanged 2D curve
    b->m_E[ni].m_ti.Append(trims[k]);
  }
  b->m_E[ei].m_ti.Empty();
  b->DeleteEdge(b->m_E[ei], false);
  return ni;
}

// Direction in which trim `ti` runs along its edge, seen from the face's
// outside (+1 / -1).
static int trim_sense(const ON_Brep* b, int ti) {
  const ON_BrepTrim& t = b->m_T[ti];
  const ON_BrepFace* f = t.Face();
  const int s = t.m_bRev3d ? -1 : 1;
  return (f && f->m_bRev) ? -s : s;
}

// Makes the faces of each connected piece face the same way: across a shared
// edge the two trims must run in opposite directions. Faces are flipped
// (breadth first from the first face of each piece) where they do not.
static void orient_faces(ON_Brep* b) {
  const int nf = b->m_F.Count();
  std::vector<int> state(nf, 0);  // 0 unvisited, 1 done
  for (int seed = 0; seed < nf; seed++) {
    if (state[seed]) continue;
    state[seed] = 1;
    std::vector<int> queue{seed};
    while (!queue.empty()) {
      const int fi = queue.back();
      queue.pop_back();
      const ON_BrepFace& f = b->m_F[fi];
      for (int li = 0; li < f.m_li.Count(); li++) {
        const ON_BrepLoop& l = b->m_L[f.m_li[li]];
        for (int k = 0; k < l.m_ti.Count(); k++) {
          const int ti = l.m_ti[k];
          const int ei = b->m_T[ti].m_ei;
          if (ei < 0 || b->m_E[ei].m_ti.Count() != 2) continue;
          const int other = b->m_E[ei].m_ti[0] == ti ? b->m_E[ei].m_ti[1] : b->m_E[ei].m_ti[0];
          const int gi = b->m_T[other].Face() ? b->m_T[other].Face()->m_face_index : -1;
          if (gi < 0 || gi == fi || state[gi]) continue;
          if (trim_sense(b, ti) == trim_sense(b, other)) b->FlipFace(b->m_F[gi]);
          state[gi] = 1;
          queue.push_back(gi);
        }
      }
    }
  }
}

// Joins coincident naked edges of a brep (Rhino's Join): coincident vertices
// are merged, then naked edges lying on top of each other become one edge.
// Returns the number of edges joined.
static int join_naked_edges(ON_Brep* b, double tol) {
  for (int i = 0; i < b->m_V.Count(); i++) {
    if (b->m_V[i].m_vertex_index < 0) continue;
    for (int j = i + 1; j < b->m_V.Count(); j++) {
      if (b->m_V[j].m_vertex_index < 0) continue;
      if (b->m_V[i].point.DistanceTo(b->m_V[j].point) <= tol) {
        b->CombineCoincidentVertices(b->m_V[i], b->m_V[j]);
      }
    }
  }
  int joined = 0;
  for (int i = 0; i < b->m_E.Count(); i++) {
    if (b->m_E[i].m_edge_index < 0 || b->m_E[i].m_ti.Count() != 1) continue;
    for (int j = i + 1; j < b->m_E.Count(); j++) {
      if (b->m_E[j].m_edge_index < 0 || b->m_E[j].m_ti.Count() != 1) continue;
      const int a0 = b->m_E[i].m_vi[0], a1 = b->m_E[i].m_vi[1];
      const int b0 = b->m_E[j].m_vi[0], b1 = b->m_E[j].m_vi[1];
      const bool same = a0 == b0 && a1 == b1;
      const bool opposite = a0 == b1 && a1 == b0;
      if (!same && !opposite) continue;
      if (!same_place(b->m_E[i], b->m_E[j], tol)) continue;
      // Direction: compare the points a quarter along each edge.
      const ON_3dPoint pi = b->m_E[i].PointAt(b->m_E[i].Domain().ParameterAt(0.25));
      const ON_3dPoint pj = b->m_E[j].PointAt(b->m_E[j].Domain().ParameterAt(0.25));
      const ON_3dPoint qj = b->m_E[j].PointAt(b->m_E[j].Domain().ParameterAt(0.75));
      const bool closed = a0 == a1;
      const bool same_dir = closed ? pi.DistanceTo(pj) <= pi.DistanceTo(qj) : same;
      int jj = j;
      if (!same_dir) {
        jj = reverse_edge(b, j);
        if (jj < 0) continue;
      }
      if (b->CombineCoincidentEdges(b->m_E[i], b->m_E[jj])) {
        joined++;
        break;
      }
    }
  }
  for (int i = 0; i < b->m_E.Count(); i++) {
    if (!(b->m_E[i].m_tolerance >= 0.0)) b->m_E[i].m_tolerance = tol;
  }
  for (int i = 0; i < b->m_V.Count(); i++) {
    if (!(b->m_V[i].m_tolerance >= 0.0)) b->m_V[i].m_tolerance = tol;
  }
  b->Compact();
  b->SetTolerancesBoxesAndFlags(false, false, false, false, true, true, true, true);
  orient_faces(b);
  if (std::getenv("F3DM_DEBUG")) {
    for (int i = 0; i < b->m_E.Count(); i++) {
      const ON_BrepEdge& e = b->m_E[i];
      if (e.m_ti.Count() == 1)
        fprintf(stderr, "naked edge %d v%d-v%d len %g\n", i, e.m_vi[0], e.m_vi[1],
                e.PointAtStart().DistanceTo(e.PointAtEnd()));
    }
  }
  return joined;
}

// Breps from bytes joined into polysurfaces where their naked edges meet.
// Output: count (u64), then per piece its length (u64) and bytes.
unsigned char* f3dm_bytes_join(const unsigned char* const* datas, const size_t* lens, int n,
                               double tol, int* joined, size_t* out_len) {
  *out_len = 0;
  *joined = 0;
  ON_Brep all;
  for (int i = 0; i < n; i++) {
    ON_Brep* b = brep_from_bytes(datas[i], lens[i]);
    if (!b) continue;
    all.Append(*b);
    delete b;
  }
  if (all.m_F.Count() == 0) return nullptr;
  *joined = join_naked_edges(&all, tol);
  ON_SimpleArray<ON_Brep*> parts;
  if (all.GetConnectedComponents(parts, false) == 0) parts.Append(all.Duplicate());
  std::vector<unsigned char> out(8, 0);
  uint64_t count = 0;
  for (int i = 0; i < parts.Count(); i++) {
    size_t len = 0;
    unsigned char* bytes = brep_bytes(parts[i], &len);  // deletes the part
    if (!bytes) continue;
    const uint64_t l = len;
    out.insert(out.end(), reinterpret_cast<const unsigned char*>(&l),
               reinterpret_cast<const unsigned char*>(&l) + 8);
    out.insert(out.end(), bytes, bytes + len);
    std::free(bytes);
    count++;
  }
  std::memcpy(out.data(), &count, 8);
  unsigned char* r = static_cast<unsigned char*>(std::malloc(out.size()));
  if (!r) return nullptr;
  std::memcpy(r, out.data(), out.size());
  *out_len = out.size();
  return r;
}

// Surface of revolution of a profile (segment stream, see read_loop) around
// the axis through `origin` along `axis`, by `angle` radians: one face per
// segment, joined (closed solid when the profile closes on the axis).
unsigned char* f3dm_make_revolved(const double* stream, size_t n, const double* origin,
                                  const double* axis, double angle, double tol, size_t* len) {
  *len = 0;
  size_t k = 0;
  ON_SimpleArray<ON_Curve*> curves;
  if (!read_loop(stream, n, &k, curves)) {
    for (int i = 0; i < curves.Count(); i++) delete curves[i];
    return nullptr;
  }
  const ON_3dPoint o(origin);
  const ON_Line ax(o, o + ON_3dVector(axis));
  ON_Brep* all = new ON_Brep();
  for (int i = 0; i < curves.Count(); i++) {
    // Segments lying on the axis sweep nothing.
    if (ax.DistanceTo(curves[i]->PointAtStart()) <= tol &&
        ax.DistanceTo(curves[i]->PointAtEnd()) <= tol &&
        ax.DistanceTo(curves[i]->PointAt(curves[i]->Domain().Mid())) <= tol) {
      delete curves[i];
      continue;
    }
    ON_RevSurface* rs = ON_RevSurface::New();
    rs->m_curve = curves[i];
    rs->m_axis = ax;
    rs->m_angle = ON_Interval(0.0, angle);
    rs->m_t = rs->m_angle;
    rs->BoundingBox();
    ON_Surface* s = rs;
    ON_Brep f;
    if (!f.Create(s)) {
      delete s;
      continue;
    }
    all->Append(f);
  }
  if (all->m_F.Count() == 0) {
    delete all;
    return nullptr;
  }
  join_naked_edges(all, tol);
  return brep_bytes(all, len);
}

}  // extern "C"

extern "C" {

// One closed or open curve from a segment stream as a single ON_Curve
// (a polycurve when it has several segments). Caller owns the result.
static ON_Curve* stream_curve(const double* s, size_t n, size_t* k) {
  ON_SimpleArray<ON_Curve*> segs;
  if (!read_loop(s, n, k, segs)) {
    for (int i = 0; i < segs.Count(); i++) delete segs[i];
    return nullptr;
  }
  if (segs.Count() == 1) return segs[0];
  ON_PolyCurve* pc = new ON_PolyCurve(segs.Count());
  for (int i = 0; i < segs.Count(); i++) pc->Append(segs[i]);
  return pc;
}

// Solves the dense system a·x = b (n×n, m right-hand sides) in place.
static bool solve(std::vector<double>& a, std::vector<double>& b, int n, int m) {
  for (int c = 0; c < n; c++) {
    int p = c;
    for (int r = c + 1; r < n; r++)
      if (std::fabs(a[r * n + c]) > std::fabs(a[p * n + c])) p = r;
    if (std::fabs(a[p * n + c]) < 1e-14) return false;
    if (p != c) {
      for (int j = 0; j < n; j++) std::swap(a[c * n + j], a[p * n + j]);
      for (int j = 0; j < m; j++) std::swap(b[c * m + j], b[p * m + j]);
    }
    for (int r = 0; r < n; r++) {
      if (r == c) continue;
      const double f = a[r * n + c] / a[c * n + c];
      if (f == 0.0) continue;
      for (int j = c; j < n; j++) a[r * n + j] -= f * a[c * n + j];
      for (int j = 0; j < m; j++) b[r * m + j] -= f * b[c * m + j];
    }
  }
  for (int r = 0; r < n; r++)
    for (int j = 0; j < m; j++) b[r * m + j] /= a[r * n + r];
  return true;
}

// Makes two NURBS curves compatible (same degree, knots and CV count) by
// building the ruled surface between them and reading back its two edges.
static bool make_compatible(ON_NurbsCurve& a, ON_NurbsCurve& b) {
  ON_NurbsSurface s;
  if (!s.CreateRuledSurface(a, b)) return false;
  const ON_Interval v = s.Domain(1);
  ON_Curve* ca = s.IsoCurve(0, v[0]);
  ON_Curve* cb = s.IsoCurve(0, v[1]);
  const ON_NurbsCurve* na = ON_NurbsCurve::Cast(ca);
  const ON_NurbsCurve* nb = ON_NurbsCurve::Cast(cb);
  const bool ok = na && nb;
  if (ok) {
    a = *na;
    b = *nb;
  }
  delete ca;
  delete cb;
  return ok;
}

// Loft through `count` curves (segment streams, one after the other). With
// `straight`, ruled faces between neighbouring curves joined into a
// polysurface; else one smooth NURBS surface through all of them (cubic
// across, chord-length parameters).
unsigned char* f3dm_make_loft(const double* stream, size_t n, int count, int straight, double tol,
                              size_t* len) {
  *len = 0;
  size_t k = 0;
  std::vector<ON_Curve*> curves;
  for (int i = 0; i < count; i++) {
    ON_Curve* c = stream_curve(stream, n, &k);
    if (!c) break;
    curves.push_back(c);
  }
  auto cleanup = [&]() {
    for (ON_Curve* c : curves) delete c;
  };
  if (static_cast<int>(curves.size()) != count || count < 2) {
    cleanup();
    return nullptr;
  }
  // Same running direction as the first curve (open curves).
  for (size_t i = 1; i < curves.size(); i++) {
    const ON_3dPoint a0 = curves[i - 1]->PointAtStart(), a1 = curves[i - 1]->PointAtEnd();
    const ON_3dPoint b0 = curves[i]->PointAtStart(), b1 = curves[i]->PointAtEnd();
    if (!curves[i]->IsClosed() && a0.DistanceTo(b1) + a1.DistanceTo(b0) < a0.DistanceTo(b0) + a1.DistanceTo(b1))
      curves[i]->Reverse();
  }
  ON_Brep* out = nullptr;
  if (straight || count == 2) {
    out = new ON_Brep();
    for (size_t i = 0; i + 1 < curves.size(); i++) {
      // One ruled face per pair of matching segments when both are polycurves
      // with the same number of segments (keeps creases as edges).
      const ON_PolyCurve* pa = ON_PolyCurve::Cast(curves[i]);
      const ON_PolyCurve* pb = ON_PolyCurve::Cast(curves[i + 1]);
      std::vector<std::pair<const ON_Curve*, const ON_Curve*>> pairs;
      if (pa && pb && pa->Count() == pb->Count()) {
        for (int s = 0; s < pa->Count(); s++) pairs.push_back({pa->SegmentCurve(s), pb->SegmentCurve(s)});
      } else {
        pairs.push_back({curves[i], curves[i + 1]});
      }
      for (auto& pr : pairs) {
        ON_NurbsSurface* ns = ON_NurbsSurface::New();
        if (!ns->CreateRuledSurface(*pr.first, *pr.second)) {
          delete ns;
          continue;
        }
        ON_Surface* s = ns;
        ON_Brep f;
        if (f.Create(s)) out->Append(f);
        else delete s;
      }
    }
    if (out->m_F.Count() == 0) {
      delete out;
      cleanup();
      return nullptr;
    }
    join_naked_edges(out, tol);
  } else {
    // Compatible NURBS forms: same degree, domain and knots.
    std::vector<ON_NurbsCurve> nc(curves.size());
    for (size_t i = 0; i < curves.size(); i++) {
      if (!curves[i]->GetNurbForm(nc[i])) {
        cleanup();
        return nullptr;
      }
      nc[i].SetDomain(0.0, 1.0);
    }
    for (int pass = 0; pass < 2; pass++)
      for (size_t i = 1; i < nc.size(); i++)
        if (!make_compatible(nc[0], nc[i])) {
          cleanup();
          return nullptr;
        }
    const int cvs = nc[0].CVCount();
    for (auto& c : nc)
      if (c.CVCount() != cvs || c.Order() != nc[0].Order()) {
        cleanup();
        return nullptr;
      }
    // Chord-length parameters across the curves (averaged over the columns).
    const int rows = count;
    std::vector<double> t(rows, 0.0);
    for (int j = 0; j < cvs; j++) {
      std::vector<double> d(rows, 0.0);
      double total = 0.0;
      for (int i = 1; i < rows; i++) {
        ON_3dPoint p, q;
        nc[i - 1].GetCV(j, p);
        nc[i].GetCV(j, q);
        d[i] = p.DistanceTo(q);
        total += d[i];
      }
      double acc = 0.0;
      for (int i = 1; i < rows; i++) {
        acc += total > 0 ? d[i] / total : 1.0 / (rows - 1);
        t[i] += acc / cvs;
      }
    }
    t[rows - 1] = 1.0;
    const int deg = (std::min)(3, rows - 1);
    // Knots by averaging (openNURBS convention: no repeated end knots).
    std::vector<double> knots(rows + deg - 1);
    for (int i = 0; i < deg; i++) {
      knots[i] = 0.0;
      knots[rows + deg - 2 - i] = 1.0;
    }
    for (int j = 1; j < rows - deg; j++) {
      double s = 0.0;
      for (int i = j; i < j + deg; i++) s += t[i];
      knots[deg - 1 + j] = s / deg;
    }
    // Basis matrix at the parameters.
    std::vector<double> a(rows * rows, 0.0);
    for (int i = 0; i < rows; i++) {
      for (int j = 0; j < rows; j++) {
        ON_NurbsCurve basis(1, false, deg + 1, rows);
        for (int q = 0; q < rows + deg - 1; q++) basis.SetKnot(q, knots[q]);
        for (int q = 0; q < rows; q++) basis.SetCV(q, ON_3dPoint(q == j ? 1.0 : 0.0, 0, 0));
        double v = 0.0;
        basis.Evaluate(t[i], 0, 1, &v);
        a[i * rows + j] = v;
      }
    }
    const bool rational = nc[0].IsRational();
    // Right-hand sides: homogeneous control points, one column per CV.
    std::vector<double> b(rows * cvs * 4, 0.0);
    for (int i = 0; i < rows; i++)
      for (int j = 0; j < cvs; j++) {
        ON_4dPoint h;
        nc[i].GetCV(j, h);
        if (!rational) h.w = 1.0;
        double* r = &b[(i * cvs + j) * 4];
        r[0] = h.x;
        r[1] = h.y;
        r[2] = h.z;
        r[3] = h.w;
      }
    if (!solve(a, b, rows, cvs * 4)) {
      cleanup();
      return nullptr;
    }
    bool any_rational = rational;
    ON_NurbsSurface* ns =
        ON_NurbsSurface::New(3, any_rational, nc[0].Order(), deg + 1, cvs, rows);
    for (int q = 0; q < nc[0].KnotCount(); q++) ns->SetKnot(0, q, nc[0].Knot(q));
    for (int q = 0; q < rows + deg - 1; q++) ns->SetKnot(1, q, knots[q]);
    for (int i = 0; i < rows; i++)
      for (int j = 0; j < cvs; j++) {
        const double* r = &b[(i * cvs + j) * 4];
        if (any_rational) ns->SetCV(j, i, ON_4dPoint(r[0], r[1], r[2], r[3]));
        else ns->SetCV(j, i, ON_3dPoint(r[0], r[1], r[2]));
      }
    ON_Surface* s = ns;
    out = new ON_Brep();
    if (!out->Create(s)) {
      delete s;
      delete out;
      cleanup();
      return nullptr;
    }
    join_naked_edges(out, tol);  // closed curves: the seam edges become one
  }
  cleanup();
  return brep_bytes(out, len);
}

// Surface made by moving a curve along a path (ExtrudeCrvAlongCrv): the sum
// surface curve(s) + path(t) - path(start), one face per profile segment.
unsigned char* f3dm_make_along(const double* stream, size_t n, double tol, size_t* len) {
  *len = 0;
  size_t k = 0;
  ON_Curve* profile = stream_curve(stream, n, &k);
  ON_Curve* path = profile ? stream_curve(stream, n, &k) : nullptr;
  if (!profile || !path) {
    delete profile;
    delete path;
    return nullptr;
  }
  ON_Brep* out = new ON_Brep();
  std::vector<const ON_Curve*> segs;
  if (const ON_PolyCurve* pc = ON_PolyCurve::Cast(profile)) {
    for (int i = 0; i < pc->Count(); i++) segs.push_back(pc->SegmentCurve(i));
  } else {
    segs.push_back(profile);
  }
  for (const ON_Curve* s : segs) {
    ON_SumSurface* ss = new ON_SumSurface();
    // ON_SumSurface: S(u,v) = A(u) + B(v) + basepoint; shift so the profile starts in place.
    ss->m_curve[0] = s->DuplicateCurve();
    ss->m_curve[1] = path->DuplicateCurve();
    ss->m_basepoint = ON_3dPoint::Origin - path->PointAtStart();
    ss->BoundingBox();
    ON_Surface* srf = ss;
    ON_Brep f;
    if (f.Create(srf)) out->Append(f);
    else delete srf;
  }
  delete profile;
  delete path;
  if (out->m_F.Count() == 0) {
    delete out;
    return nullptr;
  }
  join_naked_edges(out, tol);
  return brep_bytes(out, len);
}

// Cone from a curve to an apex (ExtrudeCrvToPoint): one face per segment,
// joined; a closed planar curve also gets its planar base (a closed solid).
unsigned char* f3dm_make_to_point(const double* stream, size_t n, const double* apex,
                                  const double* plane, double tol, size_t* len) {
  *len = 0;
  size_t k = 0;
  ON_Curve* c = stream_curve(stream, n, &k);
  if (!c) return nullptr;
  ON_Brep* out = new ON_Brep();
  std::vector<const ON_Curve*> segs;
  if (const ON_PolyCurve* pc = ON_PolyCurve::Cast(c)) {
    for (int i = 0; i < pc->Count(); i++) segs.push_back(pc->SegmentCurve(i));
  } else {
    segs.push_back(c);
  }
  for (const ON_Curve* s : segs) {
    ON_NurbsSurface* ns = ON_NurbsSurface::New();
    if (!ns->CreateConeSurface(ON_3dPoint(apex), *s)) {
      delete ns;
      continue;
    }
    ON_Surface* srf = ns;
    ON_Brep f;
    if (f.Create(srf)) out->Append(f);
    else delete srf;
  }
  if (plane && c->IsClosed()) {
    ON_Brep* base = ON_BrepTrimmedPlane(plane_of(plane), *c, nullptr);
    if (base) {
      out->Append(*base);
      delete base;
    }
  }
  delete c;
  if (out->m_F.Count() == 0) {
    delete out;
    return nullptr;
  }
  join_naked_edges(out, tol);
  return brep_bytes(out, len);
}

}  // extern "C"

extern "C" {

// Face `i` without its trims: its whole underlying surface (Untrim).
unsigned char* f3dm_bytes_untrimmed(const unsigned char* data, size_t len, int i,
                                    size_t* out_len) {
  *out_len = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b || i < 0 || i >= b->m_F.Count()) {
    delete b;
    return nullptr;
  }
  const ON_Surface* s = b->m_F[i].SurfaceOf();
  ON_Surface* copy = s ? s->DuplicateSurface() : nullptr;
  const bool rev = b->m_F[i].m_bRev;
  delete b;
  if (!copy) return nullptr;
  ON_Brep* out = new ON_Brep();
  ON_Surface* srf = copy;
  if (!out->Create(srf)) {
    delete srf;
    delete out;
    return nullptr;
  }
  if (rev) out->Flip();
  return brep_bytes(out, out_len);
}

// The brep with each face's surface shrunk to its trims (ShrinkTrimmedSrf).
unsigned char* f3dm_bytes_shrunk(const unsigned char* data, size_t len, size_t* out_len) {
  *out_len = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  b->ShrinkSurfaces();
  b->Compact();
  return brep_bytes(b, out_len);
}

// Validity report of the exact object (Check): returns 1 when valid; the
// openNURBS log (UTF-8, truncated to cap) goes into buf.
int f3dm_bytes_check(const unsigned char* data, size_t len, char* buf, int cap) {
  ON_Geometry* g = from_bytes(data, len);
  if (!g) {
    if (buf && cap > 0) {
      const char* msg = "the object could not be read";
      std::strncpy(buf, msg, cap - 1);
      buf[cap - 1] = 0;
    }
    return 0;
  }
  ON_wString text;
  ON_TextLog log(text);
  const bool ok = g->IsValid(&log);
  delete g;
  const ON_String s(text);
  if (buf && cap > 0) {
    std::strncpy(buf, static_cast<const char*>(s), cap - 1);
    buf[cap - 1] = 0;
  }
  return ok ? 1 : 0;
}

// Closest (u, v) on face `fi` to p: best of a 24×24 grid, refined by Newton
// steps on the surface. Returns the distance.
static double face_closest_uv(const ON_BrepFace& f, const ON_3dPoint& p, double* u, double* v) {
  const ON_Interval du = f.Domain(0), dv = f.Domain(1);
  double best = 1e300;
  for (int i = 0; i <= 24; i++)
    for (int j = 0; j <= 24; j++) {
      const double s = du.ParameterAt(i / 24.0), t = dv.ParameterAt(j / 24.0);
      const double d = f.PointAt(s, t).DistanceTo(p);
      if (d < best) {
        best = d;
        *u = s;
        *v = t;
      }
    }
  for (int it = 0; it < 20; it++) {
    ON_3dPoint q;
    ON_3dVector su, sv;
    if (!f.Ev1Der(*u, *v, q, su, sv)) break;
    const ON_3dVector r = p - q;
    // Solve [su·su su·sv; su·sv sv·sv] [du dv] = [r·su r·sv].
    const double a = su * su, b = su * sv, c = sv * sv, e = r * su, g = r * sv;
    const double det = a * c - b * b;
    if (std::fabs(det) < 1e-30) break;
    double nu = *u + (e * c - g * b) / det, nv = *v + (a * g - b * e) / det;
    nu = (std::max)(du.Min(), (std::min)(du.Max(), nu));
    nv = (std::max)(dv.Min(), (std::min)(dv.Max(), nv));
    const bool done = std::fabs(nu - *u) < 1e-12 * (1 + du.Length()) &&
                      std::fabs(nv - *v) < 1e-12 * (1 + dv.Length());
    *u = nu;
    *v = nv;
    if (done) break;
  }
  return f.PointAt(*u, *v).DistanceTo(p);
}

// True when (u, v) lies inside the trimmed region of the face (even-odd test
// against its trims sampled into polygons).
static bool inside_face(const ON_Brep* b, const ON_BrepFace& f, double u, double v) {
  bool in = false;
  for (int li = 0; li < f.m_li.Count(); li++) {
    const ON_BrepLoop& l = b->m_L[f.m_li[li]];
    std::vector<ON_2dPoint> poly;
    for (int ti = 0; ti < l.m_ti.Count(); ti++) {
      const ON_BrepTrim& t = b->m_T[l.m_ti[ti]];
      const ON_Interval d = t.Domain();
      for (int k = 0; k < 24; k++) {
        const ON_3dPoint q = t.PointAt(d.ParameterAt(k / 24.0));
        poly.push_back(ON_2dPoint(q.x, q.y));
      }
    }
    for (size_t i = 0, n = poly.size(); i < n; i++) {
      const ON_2dPoint a = poly[i], c = poly[(i + 1) % n];
      if ((a.y > v) != (c.y > v)) {
        const double x = a.x + (v - a.y) / (c.y - a.y) * (c.x - a.x);
        if (u < x) in = !in;
      }
    }
  }
  return in;
}

// Isocurves through the point of the brep nearest to p (ExtractIsocurve):
// dir 0 = along u, 1 = along v, 2 = both; only the parts inside the trims.
// Output: a curve stream (see f3dm_bytes_edges).
double* f3dm_bytes_isocurves(const unsigned char* data, size_t len, const double* xyz, int dir,
                             size_t* n) {
  *n = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  const ON_3dPoint p(xyz);
  int face = -1;
  double fu = 0, fv = 0, best = 1e300;
  for (int fi = 0; fi < b->m_F.Count(); fi++) {
    double u = 0, v = 0;
    const double d = face_closest_uv(b->m_F[fi], p, &u, &v);
    if (d < best) {
      best = d;
      face = fi;
      fu = u;
      fv = v;
    }
  }
  std::vector<double> out{0.0};
  int count = 0;
  if (face >= 0) {
    const ON_BrepFace& f = b->m_F[face];
    for (int d = 0; d < 2; d++) {
      if (dir != 2 && dir != d) continue;
      // An isocurve along u has constant v (ON: IsoCurve(0, v)).
      const double c = d == 0 ? fv : fu;
      ON_Curve* iso = f.SurfaceOf()->IsoCurve(d, c);
      if (!iso) continue;
      const ON_Interval dom = iso->Domain();
      // Parts inside the trims, found by sampling and bisection.
      const int samples = 200;
      auto in_at = [&](double t) {
        return d == 0 ? inside_face(b, f, t, c) : inside_face(b, f, c, t);
      };
      auto edge = [&](double a, double z) {
        const bool ia = in_at(a);
        for (int k = 0; k < 40; k++) {
          const double m = 0.5 * (a + z);
          if (in_at(m) == ia) a = m;
          else z = m;
        }
        return 0.5 * (a + z);
      };
      double start = 0;
      bool was = in_at(dom[0]);
      if (was) start = dom[0];
      for (int k = 1; k <= samples; k++) {
        const double t0 = dom.ParameterAt((k - 1.0) / samples), t1 = dom.ParameterAt(1.0 * k / samples);
        const bool now = in_at(t1);
        if (now && !was) start = edge(t0, t1);
        if (!now && was || (now && k == samples)) {
          const double end = now ? dom[1] : edge(t0, t1);
          if (end - start > 1e-9 * dom.Length()) {
            ON_Curve* piece = iso->DuplicateCurve();
            // The whole curve is kept as it is (Trim refuses the full domain).
            const bool whole = start <= dom[0] + 1e-12 * dom.Length() &&
                               end >= dom[1] - 1e-12 * dom.Length();
            if (whole || piece->Trim(ON_Interval(start, end))) {
              const size_t before = out.size();
              push_curve(*piece, out);
              if (out.size() > before) count++;
            }
            delete piece;
          }
        }
        was = now;
      }
      delete iso;
    }
  }
  delete b;
  out[0] = count;
  return to_malloc(out, n);
}

}  // extern "C"

// ---------------------------------------------------------------------------
// Neutral exact-brep stream, shared with forma-geom's OpenCascade shim (S3, see
// forma_geom::neutral). All doubles:
//   1 (version)
//   nv, nv x (x y z)                                   vertices
//   ne, ne x (v0 v1 curve3)                            edges, curve from v0 to v1
//   nf, nf x (rev surface nloops loop...)              faces
//     loop: kind (0 outer, 1 inner) ntrims trim...
//       trim: edge (-1 singular) rev3d vertex curve2   curve along the loop
//   curve: dim degree n knots[n+degree+1] n x (dim coords, weight)
//   surface: du dv nu nv knots_u[nu+du+1] knots_v[nv+dv+1] (nu*nv) x (x y z w), u major
// Knot vectors are full (OpenCascade style); openNURBS drops the first and last.

static void neutral_knots(const double* k, int count, std::vector<double>& out) {
  out.push_back(k[0]);
  for (int i = 0; i < count; i++) out.push_back(k[i]);
  out.push_back(k[count - 1]);
}

static bool neutral_curve(const ON_Curve& c, int dim, std::vector<double>& out) {
  ON_NurbsCurve n;
  if (!c.GetNurbForm(n) || n.CVCount() < 2) return false;
  out.insert(out.end(), {double(dim), double(n.Degree()), double(n.CVCount())});
  neutral_knots(n.m_knot, n.KnotCount(), out);
  for (int i = 0; i < n.CVCount(); i++) {
    ON_3dPoint p;
    n.GetCV(i, p);
    out.push_back(p.x);
    out.push_back(p.y);
    if (dim == 3) out.push_back(p.z);
    out.push_back(n.Weight(i));
  }
  return true;
}

static bool neutral_surface(const ON_Surface& s, std::vector<double>& out) {
  ON_NurbsSurface n;
  if (s.GetNurbForm(n) != 1) return false;  // 2 = other parameters than the trims use
  out.insert(out.end(), {double(n.Degree(0)), double(n.Degree(1)), double(n.CVCount(0)),
                         double(n.CVCount(1))});
  neutral_knots(n.m_knot[0], n.KnotCount(0), out);
  neutral_knots(n.m_knot[1], n.KnotCount(1), out);
  for (int i = 0; i < n.CVCount(0); i++) {
    for (int j = 0; j < n.CVCount(1); j++) {
      ON_3dPoint p;
      n.GetCV(i, j, p);
      out.insert(out.end(), {p.x, p.y, p.z, n.Weight(i, j)});
    }
  }
  return true;
}

// Reader over a neutral stream; every read is bounds-checked.
struct NeutralReader {
  const double* s;
  size_t n, k = 0;
  bool ok = true;
  double next() {
    if (k >= n) {
      ok = false;
      return 0.0;
    }
    return s[k++];
  }
  int count(int max) {
    const double v = next();
    if (!(v >= 0 && v <= max)) {
      ok = false;
      return 0;
    }
    return int(v);
  }
};

static const int NEUTRAL_MAX = 1 << 22;

static void read_knots(NeutralReader& r, int full, ON_NurbsCurve& c) {
  r.next();  // superfluous first knot
  for (int i = 0; i < full - 2; i++) c.m_knot[i] = r.next();
  r.next();
}

static ON_NurbsCurve* read_neutral_curve(NeutralReader& r, int want_dim) {
  const int dim = r.count(3), deg = r.count(32), n = r.count(NEUTRAL_MAX);
  if (!r.ok || dim != want_dim || deg < 1 || n < deg + 1) {
    r.ok = false;
    return nullptr;
  }
  ON_NurbsCurve* c = ON_NurbsCurve::New(dim, true, deg + 1, n);
  read_knots(r, n + deg + 1, *c);
  for (int i = 0; i < n; i++) {
    double p[3] = {0, 0, 0};
    for (int d = 0; d < dim; d++) p[d] = r.next();
    const double w = r.next();
    c->SetCV(i, ON_4dPoint(p[0] * w, p[1] * w, p[2] * w, w));
  }
  if (!r.ok || !c->IsValid()) {
    delete c;
    r.ok = false;
    return nullptr;
  }
  return c;
}

static ON_NurbsSurface* read_neutral_surface(NeutralReader& r) {
  const int du = r.count(32), dv = r.count(32), nu = r.count(1 << 16), nv = r.count(1 << 16);
  if (!r.ok || du < 1 || dv < 1 || nu < du + 1 || nv < dv + 1 || double(nu) * nv > NEUTRAL_MAX) {
    r.ok = false;
    return nullptr;
  }
  ON_NurbsSurface* s = ON_NurbsSurface::New(3, true, du + 1, dv + 1, nu, nv);
  for (int dir = 0; dir < 2; dir++) {
    const int full = (dir ? nv + dv : nu + du) + 1;
    r.next();
    for (int i = 0; i < full - 2; i++) s->m_knot[dir][i] = r.next();
    r.next();
  }
  for (int i = 0; i < nu; i++) {
    for (int j = 0; j < nv; j++) {
      const double x = r.next(), y = r.next(), z = r.next(), w = r.next();
      s->SetCV(i, j, ON_4dPoint(x * w, y * w, z * w, w));
    }
  }
  if (!r.ok || !s->IsValid()) {
    delete s;
    r.ok = false;
    return nullptr;
  }
  return s;
}

extern "C" {

// A brep (or extrusion / surface) as a neutral stream; null when some surface has
// no NURBS form matching its trims.
double* f3dm_bytes_to_neutral(const unsigned char* data, size_t len, size_t* n) {
  *n = 0;
  ON_Brep* b = brep_from_bytes(data, len);
  if (!b) return nullptr;
  if (!nurbify(b)) {
    delete b;
    return nullptr;
  }
  std::vector<double> out{1.0, double(b->m_V.Count())};
  for (int i = 0; i < b->m_V.Count(); i++) {
    const ON_3dPoint p = b->m_V[i].point;
    out.insert(out.end(), {p.x, p.y, p.z});
  }
  out.push_back(double(b->m_E.Count()));
  bool ok = true;
  for (int i = 0; i < b->m_E.Count() && ok; i++) {
    const ON_BrepEdge& e = b->m_E[i];
    out.insert(out.end(), {double(e.m_vi[0]), double(e.m_vi[1])});
    ok = neutral_curve(e, 3, out);
  }
  out.push_back(double(b->m_F.Count()));
  for (int fi = 0; fi < b->m_F.Count() && ok; fi++) {
    const ON_BrepFace& f = b->m_F[fi];
    const ON_Surface* s = f.SurfaceOf();
    out.push_back(f.m_bRev ? 1.0 : 0.0);
    ok = s && neutral_surface(*s, out);
    out.push_back(double(f.m_li.Count()));
    for (int li = 0; li < f.m_li.Count() && ok; li++) {
      const ON_BrepLoop& l = b->m_L[f.m_li[li]];
      out.push_back(l.m_type == ON_BrepLoop::outer ? 0.0 : 1.0);
      out.push_back(double(l.m_ti.Count()));
      for (int ti = 0; ti < l.m_ti.Count() && ok; ti++) {
        const ON_BrepTrim& t = b->m_T[l.m_ti[ti]];
        const bool singular = t.m_type == ON_BrepTrim::singular || t.m_ei < 0;
        out.insert(out.end(), {singular ? -1.0 : double(t.m_ei), t.m_bRev3d ? 1.0 : 0.0,
                               double(t.m_vi[0])});
        ok = neutral_curve(t, 2, out);
      }
    }
  }
  delete b;
  if (!ok) return nullptr;
  return to_malloc(out, n);
}

// A neutral stream back to brep bytes (null when it does not make a valid brep).
unsigned char* f3dm_neutral_to_bytes(const double* s, size_t n, double tol, size_t* out_len) {
  *out_len = 0;
  NeutralReader r{s, n};
  if (r.next() != 1.0) return nullptr;
  ON_Brep* b = new ON_Brep();
  const int nv = r.count(NEUTRAL_MAX);
  for (int i = 0; i < nv && r.ok; i++) {
    const double x = r.next(), y = r.next(), z = r.next();
    b->NewVertex(ON_3dPoint(x, y, z), tol);
  }
  const int ne = r.count(NEUTRAL_MAX);
  for (int i = 0; i < ne && r.ok; i++) {
    const int v0 = r.count(nv - 1), v1 = r.count(nv - 1);
    ON_NurbsCurve* c = read_neutral_curve(r, 3);
    if (!r.ok) break;
    const int ci = b->AddEdgeCurve(c);
    ON_BrepEdge& e = b->NewEdge(b->m_V[v0], b->m_V[v1], ci);
    e.m_tolerance = tol;
  }
  const int nf = r.count(NEUTRAL_MAX);
  for (int fi = 0; fi < nf && r.ok; fi++) {
    const bool rev = r.next() != 0.0;
    ON_NurbsSurface* srf = read_neutral_surface(r);
    if (!r.ok) break;
    ON_BrepFace& f = b->NewFace(b->AddSurface(srf));
    f.m_bRev = rev;
    const int fidx = f.m_face_index;
    const int nl = r.count(NEUTRAL_MAX);
    for (int li = 0; li < nl && r.ok; li++) {
      const bool outer = r.next() == 0.0;
      ON_BrepLoop& l = b->NewLoop(outer ? ON_BrepLoop::outer : ON_BrepLoop::inner, b->m_F[fidx]);
      const int lidx = l.m_loop_index;
      const int nt = r.count(NEUTRAL_MAX);
      struct T {
        double ev, vv;
        bool rev3d;
        ON_NurbsCurve* c2;
      };
      std::vector<T> trims;
      for (int ti = 0; ti < nt && r.ok; ti++) {
        T t;
        t.ev = r.next();
        t.rev3d = r.next() != 0.0;
        t.vv = r.next();
        t.c2 = read_neutral_curve(r, 2);
        if (r.ok) trims.push_back(t);
      }
      if (!r.ok) {
        for (auto& t : trims) delete t.c2;
        break;
      }
      // Close the tiny gaps another kernel leaves between consecutive trims
      // (openNURBS checks loop continuity much more tightly).
      for (size_t i = 0; i < trims.size(); i++) {
        ON_NurbsCurve* a = trims[i].c2;
        ON_NurbsCurve* c = trims[(i + 1) % trims.size()].c2;
        const ON_3dPoint end = a->PointAtEnd(), start = c->PointAtStart();
        // Gaps well below the model tolerance are rounding, not holes.
        if (end.DistanceTo(start) > 0.0 && end.DistanceTo(start) < (std::max)(1e-6, 0.5 * tol)) {
          // Moving the start needs a curve clamped at its start.
          if (!c->SetStartPoint(end) && c->ClampEnd(0)) c->SetStartPoint(end);
          if (std::getenv("F3DM_DEBUG") && c->PointAtStart() != end)
            fprintf(stderr, "trim gap not closed: %g\n", c->PointAtStart().DistanceTo(end));
        }
      }
      for (const T& tr : trims) {
        const double ev = tr.ev, vv = tr.vv;
        const bool rev3d = tr.rev3d;
        ON_NurbsCurve* c2 = tr.c2;
        const int c2i = b->AddTrimCurve(c2);
        if (ev < 0) {
          if (!(vv >= 0 && vv < nv)) {
            r.ok = false;
            break;
          }
          ON_BrepTrim& t = b->NewSingularTrim(b->m_V[int(vv)], b->m_L[lidx], ON_Surface::not_iso, c2i);
          t.m_tolerance[0] = t.m_tolerance[1] = 0.0;
        } else {
          if (!(ev < ne)) {
            r.ok = false;
            break;
          }
          ON_BrepTrim& t = b->NewTrim(b->m_E[int(ev)], rev3d, b->m_L[lidx], c2i);
          t.m_tolerance[0] = t.m_tolerance[1] = tol;
        }
      }
    }
  }
  if (!r.ok || r.k != n) {
    delete b;
    return nullptr;
  }
  b->SetTrimIsoFlags();
  b->SetTrimTypeFlags();
  b->SetTolerancesBoxesAndFlags(true, true, true, true, true, true, true, true);
  return brep_bytes(b, out_len);
}

}  // extern "C"
