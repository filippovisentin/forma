// Thin C ABI over OpenCascade for forma-geom (feature `occt`, ADR 0001 / 0004).
//
// Only plain C types cross the boundary: OCCT shapes are opaque `FoShape*` handles that
// the Rust side owns and frees, and every function catches OCCT exceptions and reports
// them through `fo_last_error`. Nothing OCCT-specific leaks into Forma's public API.

#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Poly_Triangulation.hxx>
#include <ShapeFix_Solid.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_Failure.hxx>
#include <TopAbs.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Pnt.hxx>
#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BSplCLib.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_SurfToAnaSurf.hxx>
#include <GeomProjLib.hxx>
#include <Geom_ElementarySurface.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <ShapeFix_Edge.hxx>
#include <ShapeFix_Shape.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColStd_Array2OfReal.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Wire.hxx>
#include <cstdlib>
#include <map>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <exception>
#include <string>
#include <vector>

struct FoShape {
  TopoDS_Shape shape;
};

struct FoTess {
  std::vector<double> positions;  // xyz
  std::vector<double> normals;    // xyz, unit
  std::vector<uint32_t> faces;    // face index per vertex
  std::vector<uint32_t> triangles;
};

static thread_local std::string g_error;

static void set_error(const std::string& s) { g_error = s; }

#define FO_TRY try {
#define FO_CATCH(ret)                                                   \
  }                                                                     \
  catch (const Standard_Failure& e) {                                   \
    set_error(std::string("OpenCascade: ") + e.DynamicType()->Name() +  \
              (e.GetMessageString() && *e.GetMessageString()            \
                   ? std::string(": ") + e.GetMessageString()           \
                   : std::string()));                                   \
    return ret;                                                         \
  }                                                                     \
  catch (const std::exception& e) {                                     \
    set_error(std::string("C++: ") + e.what());                         \
    return ret;                                                         \
  }                                                                     \
  catch (...) {                                                         \
    set_error("unknown C++ exception");                                 \
    return ret;                                                         \
  }

static FoShape* wrap(const TopoDS_Shape& s) {
  FoShape* out = new FoShape;
  out->shape = s;
  return out;
}

// Merge coplanar faces and collinear edges (a box made of 12 triangles becomes 6 faces).
static TopoDS_Shape unify(const TopoDS_Shape& s) {
  ShapeUpgrade_UnifySameDomain u(s, Standard_True, Standard_True, Standard_False);
  u.AllowInternalEdges(Standard_False);
  u.Build();
  return u.Shape();
}

static int count(const TopoDS_Shape& s, TopAbs_ShapeEnum kind) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, kind, m);
  return m.Extent();
}

static void solids_of(const TopoDS_Shape& s, TopTools_ListOfShape& out) {
  for (TopExp_Explorer ex(s, TopAbs_SOLID); ex.More(); ex.Next()) out.Append(ex.Current());
}

static TopoDS_Compound compound_of(const TopTools_ListOfShape& l) {
  TopoDS_Compound c;
  BRep_Builder b;
  b.MakeCompound(c);
  for (TopTools_ListOfShape::Iterator it(l); it.More(); it.Next()) b.Add(c, it.Value());
  return c;
}

extern "C" {

const char* fo_last_error() { return g_error.c_str(); }

void fo_shape_free(FoShape* s) { delete s; }

// Number of solids in a shape, and the i-th one (0-based) as a new handle.
int fo_solid_count(const FoShape* s) { return s ? count(s->shape, TopAbs_SOLID) : 0; }

FoShape* fo_solid_at(const FoShape* s, int i) {
  FO_TRY
  int k = 0;
  for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next(), ++k) {
    if (k == i) return wrap(ex.Current());
  }
  set_error("solid index out of range");
  return nullptr;
  FO_CATCH(nullptr)
}

int fo_face_count(const FoShape* s) { return s ? count(s->shape, TopAbs_FACE) : 0; }

double fo_volume(const FoShape* s) {
  FO_TRY
  GProp_GProps p;
  BRepGProp::VolumeProperties(s->shape, p);
  return p.Mass();
  FO_CATCH(0.0)
}

// Triangle mesh -> closed solid(s). Each closed shell found by sewing becomes a solid;
// open shells make the call fail (status -1 with an error) unless `allow_open`, in which
// case they are returned as shells. Coplanar triangles are merged into planar faces.
FoShape* fo_shape_from_mesh(const double* xyz, uint32_t nv, const uint32_t* tris,
                            uint32_t nt, double tol) {
  FO_TRY
  BRepBuilderAPI_Sewing sew(tol);
  int added = 0;
  for (uint32_t t = 0; t < nt; ++t) {
    uint32_t i[3] = {tris[3 * t], tris[3 * t + 1], tris[3 * t + 2]};
    if (i[0] >= nv || i[1] >= nv || i[2] >= nv) {
      set_error("triangle index out of range");
      return nullptr;
    }
    gp_Pnt p[3];
    for (int k = 0; k < 3; ++k) p[k] = gp_Pnt(xyz[3 * i[k]], xyz[3 * i[k] + 1], xyz[3 * i[k] + 2]);
    if (p[0].Distance(p[1]) <= tol || p[1].Distance(p[2]) <= tol ||
        p[2].Distance(p[0]) <= tol)
      continue;
    gp_Vec n = gp_Vec(p[0], p[1]).Crossed(gp_Vec(p[0], p[2]));
    if (n.Magnitude() <= tol * tol) continue;  // sliver
    BRepBuilderAPI_MakePolygon poly(p[0], p[1], p[2], Standard_True);
    if (!poly.IsDone()) continue;
    BRepBuilderAPI_MakeFace f(poly.Wire(), Standard_True);
    if (!f.IsDone()) continue;
    sew.Add(f.Face());
    ++added;
  }
  if (added < 4) {
    set_error("the mesh has too few triangles to be a solid");
    return nullptr;
  }
  sew.Perform();
  TopoDS_Shape sewn = sew.SewedShape();

  TopTools_ListOfShape solids;
  int open = 0;
  for (TopExp_Explorer ex(sewn, TopAbs_SHELL); ex.More(); ex.Next()) {
    TopoDS_Shell shell = TopoDS::Shell(ex.Current());
    // A shell is closed when every edge has two faces.
    TopTools_IndexedDataMapOfShapeListOfShape ef;
    TopExp::MapShapesAndAncestors(shell, TopAbs_EDGE, TopAbs_FACE, ef);
    bool closed = ef.Extent() > 0;
    for (int k = 1; k <= ef.Extent() && closed; ++k) {
      if (ef(k).Extent() != 2) closed = false;
    }
    if (!closed) {
      ++open;
      continue;
    }
    BRepBuilderAPI_MakeSolid ms(shell);
    if (!ms.IsDone()) {
      ++open;
      continue;
    }
    TopoDS_Solid solid = ms.Solid();
    BRepLib::OrientClosedSolid(solid);
    solids.Append(solid);
  }
  // Faces that did not end up in any shell.
  for (TopExp_Explorer ex(sewn, TopAbs_FACE, TopAbs_SHELL); ex.More(); ex.Next()) ++open;
  if (open > 0 || solids.IsEmpty()) {
    set_error("the mesh is not closed (it has open edges), so it is not a solid");
    return nullptr;
  }
  TopoDS_Shape out = solids.Extent() == 1 ? solids.First() : TopoDS_Shape(compound_of(solids));
  return wrap(unify(out));
  FO_CATCH(nullptr)
}

// op: 0 union, 1 difference (a minus b), 2 intersection, 3 split (a split by b).
FoShape* fo_boolean(int op, const FoShape* const* a, uint32_t na, const FoShape* const* b,
                    uint32_t nb, double fuzzy) {
  FO_TRY
  TopTools_ListOfShape args, tools;
  for (uint32_t i = 0; i < na; ++i) solids_of(a[i]->shape, args);
  for (uint32_t i = 0; i < nb; ++i) solids_of(b[i]->shape, tools);
  if (args.IsEmpty()) {
    set_error("no solid to operate on");
    return nullptr;
  }
  if (op == 0 && tools.IsEmpty() && args.Extent() > 1) {
    // Union of a single list: first against the rest.
    tools = args;
    args.Clear();
    args.Append(tools.First());
    tools.RemoveFirst();
  }
  if (tools.IsEmpty()) {
    set_error("no solid to operate with");
    return nullptr;
  }
  TopoDS_Shape result;
  if (op == 3) {
    BRepAlgoAPI_Splitter sp;
    sp.SetArguments(args);
    sp.SetTools(tools);
    sp.SetFuzzyValue(fuzzy);
    sp.SetNonDestructive(Standard_True);
    sp.Build();
    if (sp.HasErrors() || !sp.IsDone()) {
      set_error("the split failed");
      return nullptr;
    }
    result = sp.Shape();
  } else {
    BRepAlgoAPI_BooleanOperation* bop = nullptr;
    BRepAlgoAPI_Fuse fuse;
    BRepAlgoAPI_Cut cut;
    BRepAlgoAPI_Common common;
    switch (op) {
      case 0: bop = &fuse; break;
      case 1: bop = &cut; break;
      case 2: bop = &common; break;
      default: set_error("unknown boolean operation"); return nullptr;
    }
    bop->SetArguments(args);
    bop->SetTools(tools);
    bop->SetFuzzyValue(fuzzy);
    bop->SetNonDestructive(Standard_True);
    bop->Build();
    if (bop->HasErrors() || !bop->IsDone()) {
      set_error("the boolean operation failed");
      return nullptr;
    }
    result = bop->Shape();
  }
  if (count(result, TopAbs_SOLID) == 0) {
    set_error("the result is empty");
    return nullptr;
  }
  return wrap(unify(result));
  FO_CATCH(nullptr)
}

// Edges of `s` closest to each pick point (xyz triples), each used once; only edges
// between two faces qualify. Returns false with an error when nothing is close.
static bool pick_edges(const TopoDS_Shape& s, const double* pts, uint32_t np, double max_dist,
                       std::vector<TopoDS_Edge>& out) {
  TopTools_IndexedDataMapOfShapeListOfShape ef;
  TopExp::MapShapesAndAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  for (uint32_t k = 0; k < np; ++k) {
    TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(gp_Pnt(pts[3 * k], pts[3 * k + 1], pts[3 * k + 2]));
    double best = 1e300;
    int best_i = 0;
    for (int i = 1; i <= ef.Extent(); ++i) {
      const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
      if (BRep_Tool::Degenerated(e) || ef(i).Extent() != 2) continue;
      // A seam (a closed surface meeting itself) has the same face on both sides.
      if (ef(i).First().IsSame(ef(i).Last())) continue;
      BRepExtrema_DistShapeShape d(v, e);
      if (d.IsDone() && d.Value() < best) {
        best = d.Value();
        best_i = i;
      }
    }
    if (best_i == 0 || best > max_dist) {
      set_error("no edge near point " + std::to_string(k + 1));
      return false;
    }
    TopoDS_Edge e = TopoDS::Edge(ef.FindKey(best_i));
    bool dup = false;
    for (auto& x : out) dup = dup || x.IsSame(e);
    if (!dup) out.push_back(e);
  }
  return true;
}

// chamfer = 0: fillet with radius `r`; 1: chamfer with distance `r`.
FoShape* fo_fillet_edges(const FoShape* s, double r, const double* pts, uint32_t np,
                         double max_dist, int chamfer) {
  FO_TRY
  std::vector<TopoDS_Edge> edges;
  if (!pick_edges(s->shape, pts, np, max_dist, edges)) return nullptr;
  TopoDS_Shape result;
  if (chamfer) {
    BRepFilletAPI_MakeChamfer mk(s->shape);
    for (auto& e : edges) mk.Add(r, e);
    mk.Build();
    if (!mk.IsDone()) {
      set_error("the chamfer failed (distance too large for the edge or its faces?)");
      return nullptr;
    }
    result = mk.Shape();
  } else {
    BRepFilletAPI_MakeFillet mk(s->shape);
    for (auto& e : edges) mk.Add(r, e);
    mk.Build();
    if (!mk.IsDone()) {
      set_error("the fillet failed (radius too large for the edge or its faces?)");
      return nullptr;
    }
    result = mk.Shape();
  }
  return wrap(result);
  FO_CATCH(nullptr)
}

// Hollow a solid: wall `thickness` inwards, removing the faces nearest to the points.
FoShape* fo_shell(const FoShape* s, double thickness, const double* pts, uint32_t np,
                  double max_dist, double tol) {
  FO_TRY
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(s->shape, TopAbs_FACE, faces);
  TopTools_ListOfShape remove;
  for (uint32_t k = 0; k < np; ++k) {
    TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(gp_Pnt(pts[3 * k], pts[3 * k + 1], pts[3 * k + 2]));
    double best = 1e300;
    int best_i = 0;
    for (int i = 1; i <= faces.Extent(); ++i) {
      BRepExtrema_DistShapeShape d(v, faces(i));
      if (d.IsDone() && d.Value() < best) {
        best = d.Value();
        best_i = i;
      }
    }
    if (best_i == 0 || best > max_dist) {
      set_error("no face near point " + std::to_string(k + 1));
      return nullptr;
    }
    bool dup = false;
    for (TopTools_ListOfShape::Iterator it(remove); it.More(); it.Next())
      dup = dup || it.Value().IsSame(faces(best_i));
    if (!dup) remove.Append(faces(best_i));
  }
  BRepOffsetAPI_MakeThickSolid mk;
  mk.MakeThickSolidByJoin(s->shape, remove, -thickness, tol, BRepOffset_Skin, Standard_False,
                          Standard_False, GeomAbs_Intersection);
  if (!mk.IsDone()) {
    set_error("the shell failed (thickness too large?)");
    return nullptr;
  }
  return wrap(mk.Shape());
  FO_CATCH(nullptr)
}

// Offset every face of a closed solid by `d` (positive = outwards), sharp corners kept.
FoShape* fo_offset(const FoShape* s, double d, double tol) {
  FO_TRY
  BRepOffsetAPI_MakeOffsetShape mk;
  mk.PerformByJoin(s->shape, d, tol, BRepOffset_Skin, Standard_False, Standard_False,
                   GeomAbs_Intersection);
  if (!mk.IsDone()) {
    set_error("the offset failed");
    return nullptr;
  }
  TopoDS_Shape r = mk.Shape();
  // The offset of a solid's shell comes back as a shell: close it into a solid.
  if (count(r, TopAbs_SOLID) == 0) {
    TopTools_ListOfShape solids;
    for (TopExp_Explorer ex(r, TopAbs_SHELL); ex.More(); ex.Next()) {
      BRepBuilderAPI_MakeSolid ms(TopoDS::Shell(ex.Current()));
      if (!ms.IsDone()) continue;
      TopoDS_Solid so = ms.Solid();
      BRepLib::OrientClosedSolid(so);
      solids.Append(so);
    }
    if (solids.IsEmpty()) {
      set_error("the offset did not give a solid");
      return nullptr;
    }
    r = solids.Extent() == 1 ? solids.First() : TopoDS_Shape(compound_of(solids));
  }
  return wrap(r);
  FO_CATCH(nullptr)
}

// Triangulate. `lin` is the chordal deflection in model units, `ang` the angular one in
// radians. Vertices are per face (a face index per vertex), normals point outwards.
FoTess* fo_tessellate(const FoShape* s, double lin, double ang) {
  FO_TRY
  BRepMesh_IncrementalMesh mesher(s->shape, lin, Standard_False, ang, Standard_False);
  FoTess* out = new FoTess;
  uint32_t face_index = 0;
  for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next(), ++face_index) {
    const TopoDS_Face& f = TopoDS::Face(ex.Current());
    TopLoc_Location loc;
    Handle(Poly_Triangulation) tri = BRep_Tool::Triangulation(f, loc);
    if (tri.IsNull()) continue;
    if (!tri->HasNormals()) BRepLib_ToolTriangulatedShape::ComputeNormals(f, tri);
    const gp_Trsf tr = loc.Transformation();
    const bool reversed = f.Orientation() == TopAbs_REVERSED;
    const uint32_t base = (uint32_t)(out->positions.size() / 3);
    for (int i = 1; i <= tri->NbNodes(); ++i) {
      gp_Pnt p = tri->Node(i).Transformed(tr);
      out->positions.push_back(p.X());
      out->positions.push_back(p.Y());
      out->positions.push_back(p.Z());
      gp_Dir n = tri->Normal(i);
      gp_Vec nv(n);
      nv.Transform(tr);
      if (reversed) nv.Reverse();
      double m = nv.Magnitude();
      if (m > 1e-300) nv /= m;
      out->normals.push_back(nv.X());
      out->normals.push_back(nv.Y());
      out->normals.push_back(nv.Z());
      out->faces.push_back(face_index);
    }
    for (int i = 1; i <= tri->NbTriangles(); ++i) {
      int a, b, c;
      tri->Triangle(i).Get(a, b, c);
      if (reversed) std::swap(b, c);
      out->triangles.push_back(base + a - 1);
      out->triangles.push_back(base + b - 1);
      out->triangles.push_back(base + c - 1);
    }
  }
  return out;
  FO_CATCH(nullptr)
}

uint32_t fo_tess_vertex_count(const FoTess* t) { return (uint32_t)(t->positions.size() / 3); }
uint32_t fo_tess_triangle_count(const FoTess* t) { return (uint32_t)(t->triangles.size() / 3); }

void fo_tess_copy(const FoTess* t, double* positions, double* normals, uint32_t* faces,
                  uint32_t* triangles) {
  std::memcpy(positions, t->positions.data(), t->positions.size() * sizeof(double));
  std::memcpy(normals, t->normals.data(), t->normals.size() * sizeof(double));
  std::memcpy(faces, t->faces.data(), t->faces.size() * sizeof(uint32_t));
  std::memcpy(triangles, t->triangles.data(), t->triangles.size() * sizeof(uint32_t));
}

void fo_tess_free(FoTess* t) { delete t; }

// 1 when OCCT's checker finds the shape valid.
int fo_is_valid(const FoShape* s) {
  FO_TRY
  BRepCheck_Analyzer a(s->shape);
  return a.IsValid() ? 1 : 0;
  FO_CATCH(0)
}

}  // extern "C"

// ---------------------------------------------------------------------------
// Neutral exact-brep streams (S3): the same layout as forma-io-3dm's shim, so exact
// openNURBS breps reach the kernel and its results go back without meshing.
//   1, nv, nv x (x y z), ne, ne x (v0 v1 curve3), nf, nf x (rev surface nloops loops)
//   loop: kind ntrims trims; trim: edge(-1 singular) rev3d vertex curve2
//   curve: dim degree n knots[n+degree+1] n x (coords..., w)
//   surface: du dv nu nv knots_u knots_v (nu*nv) x (x y z w), u major

namespace {

struct Rd {
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

const int MAXN = 1 << 22;

// Full knot vector -> distinct knots and multiplicities.
bool compress(const std::vector<double>& full, std::vector<double>& k, std::vector<int>& m) {
  for (size_t i = 0; i < full.size(); i++) {
    if (i > 0 && full[i] < full[i - 1]) return false;
    if (!k.empty() && full[i] - k.back() <= 1e-12 * (1.0 + std::fabs(k.back()))) {
      m.back()++;
    } else {
      k.push_back(full[i]);
      m.push_back(1);
    }
  }
  return k.size() >= 2;
}

template <class Arr>
Arr real_array(const std::vector<double>& v) {
  Arr a(1, (int)v.size());
  for (size_t i = 0; i < v.size(); i++) a((int)i + 1) = v[i];
  return a;
}

TColStd_Array1OfInteger int_array(const std::vector<int>& v) {
  TColStd_Array1OfInteger a(1, (int)v.size());
  for (size_t i = 0; i < v.size(); i++) a((int)i + 1) = v[i];
  return a;
}

// Reads a curve of dimension `dim`; returns false on malformed data.
bool read_curve(Rd& r, int dim, int& deg, std::vector<double>& knots, std::vector<int>& mults,
                std::vector<double>& coords, std::vector<double>& weights) {
  const int d = r.count(3);
  deg = r.count(32);
  const int n = r.count(MAXN);
  if (!r.ok || d != dim || deg < 1 || n < deg + 1) return r.ok = false;
  std::vector<double> full(n + deg + 1);
  for (auto& x : full) x = r.next();
  for (int i = 0; i < n; i++) {
    for (int c = 0; c < dim; c++) coords.push_back(r.next());
    weights.push_back(r.next());
  }
  if (!r.ok || !compress(full, knots, mults)) return r.ok = false;
  for (double w : weights)
    if (!(w > 0)) return r.ok = false;
  return true;
}

Handle(Geom_BSplineCurve) read_c3(Rd& r) {
  int deg;
  std::vector<double> k, c, w;
  std::vector<int> m;
  if (!read_curve(r, 3, deg, k, m, c, w)) return nullptr;
  const int n = (int)w.size();
  TColgp_Array1OfPnt p(1, n);
  for (int i = 0; i < n; i++) p(i + 1) = gp_Pnt(c[3 * i], c[3 * i + 1], c[3 * i + 2]);
  return new Geom_BSplineCurve(p, real_array<TColStd_Array1OfReal>(w),
                               real_array<TColStd_Array1OfReal>(k), int_array(m), deg);
}

Handle(Geom2d_BSplineCurve) read_c2(Rd& r) {
  int deg;
  std::vector<double> k, c, w;
  std::vector<int> m;
  if (!read_curve(r, 2, deg, k, m, c, w)) return nullptr;
  const int n = (int)w.size();
  TColgp_Array1OfPnt2d p(1, n);
  for (int i = 0; i < n; i++) p(i + 1) = gp_Pnt2d(c[2 * i], c[2 * i + 1]);
  return new Geom2d_BSplineCurve(p, real_array<TColStd_Array1OfReal>(w),
                                 real_array<TColStd_Array1OfReal>(k), int_array(m), deg);
}

Handle(Geom_BSplineSurface) read_surface(Rd& r) {
  const int du = r.count(32), dv = r.count(32), nu = r.count(1 << 16), nv = r.count(1 << 16);
  if (!r.ok || du < 1 || dv < 1 || nu < du + 1 || nv < dv + 1 || double(nu) * nv > MAXN) {
    r.ok = false;
    return nullptr;
  }
  std::vector<double> fu(nu + du + 1), fv(nv + dv + 1);
  for (auto& x : fu) x = r.next();
  for (auto& x : fv) x = r.next();
  TColgp_Array2OfPnt p(1, nu, 1, nv);
  TColStd_Array2OfReal w(1, nu, 1, nv);
  for (int i = 1; i <= nu; i++) {
    for (int j = 1; j <= nv; j++) {
      const double x = r.next(), y = r.next(), z = r.next(), ww = r.next();
      p(i, j) = gp_Pnt(x, y, z);
      w(i, j) = ww;
      if (!(ww > 0)) r.ok = false;
    }
  }
  std::vector<double> ku, kv;
  std::vector<int> mu, mv;
  if (!r.ok || !compress(fu, ku, mu) || !compress(fv, kv, mv)) {
    r.ok = false;
    return nullptr;
  }
  return new Geom_BSplineSurface(p, w, real_array<TColStd_Array1OfReal>(ku),
                                 real_array<TColStd_Array1OfReal>(kv), int_array(mu),
                                 int_array(mv), du, dv);
}

void put_knots(const TColStd_Array1OfReal& k, const TColStd_Array1OfInteger& m,
               std::vector<double>& out) {
  for (int i = k.Lower(); i <= k.Upper(); i++)
    for (int j = 0; j < m(i); j++) out.push_back(k(i));
}

void put_c3(const Handle(Geom_BSplineCurve)& c, std::vector<double>& out) {
  out.insert(out.end(), {3.0, double(c->Degree()), double(c->NbPoles())});
  TColStd_Array1OfReal k(1, c->NbKnots());
  TColStd_Array1OfInteger m(1, c->NbKnots());
  c->Knots(k);
  c->Multiplicities(m);
  put_knots(k, m, out);
  for (int i = 1; i <= c->NbPoles(); i++) {
    const gp_Pnt p = c->Pole(i);
    out.insert(out.end(), {p.X(), p.Y(), p.Z(), c->Weight(i)});
  }
}

void put_c2(const Handle(Geom2d_BSplineCurve)& c, std::vector<double>& out) {
  out.insert(out.end(), {2.0, double(c->Degree()), double(c->NbPoles())});
  TColStd_Array1OfReal k(1, c->NbKnots());
  TColStd_Array1OfInteger m(1, c->NbKnots());
  c->Knots(k);
  c->Multiplicities(m);
  put_knots(k, m, out);
  for (int i = 1; i <= c->NbPoles(); i++) {
    const gp_Pnt2d p = c->Pole(i);
    out.insert(out.end(), {p.X(), p.Y(), c->Weight(i)});
  }
}

Handle(Geom_BSplineCurve) bspline3(const Handle(Geom_Curve)& c, double f, double l) {
  if (!c->IsPeriodic()) {  // edge ranges can overshoot the curve by rounding
    f = std::max(f, c->FirstParameter());
    l = std::min(l, c->LastParameter());
  }
  Handle(Geom_BSplineCurve) b =
      GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(c, f, l));
  if (b->IsPeriodic()) b->SetNotPeriodic();
  return b;
}

Handle(Geom2d_BSplineCurve) bspline2(const Handle(Geom2d_Curve)& c, double f, double l) {
  if (!c->IsPeriodic()) {
    f = std::max(f, c->FirstParameter());
    l = std::min(l, c->LastParameter());
  }
  Handle(Geom2d_BSplineCurve) b =
      Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(c, f, l));
  if (b->IsPeriodic()) b->SetNotPeriodic();
  return b;
}

// The curve on the face really lies under the edge (start, middle, end).
bool on_edge(const Handle(Geom2d_BSplineCurve)& c2, const TopoDS_Edge& e, const TopoDS_Face& F) {
  double f, l;
  Handle(Geom_Curve) c3 = BRep_Tool::Curve(e, f, l);
  if (c3.IsNull()) return true;  // degenerated: nothing to compare with
  Handle(Geom_Surface) s = BRep_Tool::Surface(F);
  const double tol = BRep_Tool::Tolerance(e) * 10.0 + 1e-6;
  const double t2[3] = {c2->FirstParameter(), 0.5 * (c2->FirstParameter() + c2->LastParameter()),
                        c2->LastParameter()};
  const double t3[3] = {f, 0.5 * (f + l), l};
  for (int i = 0; i < 3; i++) {
    const gp_Pnt2d uv = c2->Value(t2[i]);
    if (s->Value(uv.X(), uv.Y()).Distance(c3->Value(t3[i])) > tol) return false;
  }
  return true;
}

// The curve of edge `e` (as oriented) on face `F`, along the edge's own parameter.
// OpenCascade sometimes hands back a wrong second curve for a seam after a fillet;
// it is then rebuilt from the first one shifted by the surface's period.
Handle(Geom2d_BSplineCurve) curve_on_face(const TopoDS_Edge& e, const TopoDS_Face& F) {
  double f, l;
  Handle(Geom2d_Curve) c2 = BRep_Tool::CurveOnSurface(e, F, f, l);
  if (c2.IsNull()) return nullptr;
  Handle(Geom2d_BSplineCurve) b2 = bspline2(c2, f, l);
  if (BRep_Tool::Degenerated(e) || !BRep_Tool::IsClosed(e, F) || on_edge(b2, e, F)) return b2;
  const TopoDS_Edge other = TopoDS::Edge(e.Reversed());
  Handle(Geom2d_Curve) o2 = BRep_Tool::CurveOnSurface(other, F, f, l);
  if (o2.IsNull()) return b2;
  Handle(Geom2d_BSplineCurve) base = bspline2(o2, f, l);
  Handle(Geom_Surface) s = BRep_Tool::Surface(F);
  double u0, u1, v0, v1;
  s->Bounds(u0, u1, v0, v1);
  const gp_Vec2d shifts[4] = {gp_Vec2d(u1 - u0, 0), gp_Vec2d(u0 - u1, 0), gp_Vec2d(0, v1 - v0),
                              gp_Vec2d(0, v0 - v1)};
  const double eps = 1e-7 * (1.0 + std::fabs(u1 - u0) + std::fabs(v1 - v0));
  for (const gp_Vec2d& d : shifts) {
    if (d.Magnitude() <= eps) continue;
    Handle(Geom2d_BSplineCurve) c = Handle(Geom2d_BSplineCurve)::DownCast(base->Copy());
    c->Translate(d);
    const gp_Pnt2d p = c->StartPoint(), q = c->EndPoint();
    const bool inside = p.X() >= u0 - eps && p.X() <= u1 + eps && p.Y() >= v0 - eps &&
                        p.Y() <= v1 + eps && q.X() >= u0 - eps && q.X() <= u1 + eps &&
                        q.Y() >= v0 - eps && q.Y() <= v1 + eps;
    if (inside && on_edge(c, e, F)) return c;
  }
  return b2;
}

}  // namespace

// Exact brep from a neutral stream: shared vertices and edges, pcurves fitted to
// the edges, closed shells become solids. Faces whose NURBS surface is really a
// plane, cylinder, cone, sphere or torus get that analytic surface (offsets and
// shells need it), with pcurves projected from the 3D edges; when that does not
// give a valid solid, the NURBS surfaces are kept. Null (with an error) when the
// data is malformed or OpenCascade does not accept the result.
namespace {

struct NTrim {
  int edge;  // -1 singular
  bool rev3d;
  int vertex;
  Handle(Geom2d_BSplineCurve) c2;
};
struct NLoop {
  std::vector<NTrim> trims;
};
struct NFace {
  bool rev;
  Handle(Geom_BSplineSurface) srf;
  std::vector<NLoop> loops;
};
struct NBrep {
  std::vector<gp_Pnt> verts;
  std::vector<std::pair<int, int>> edge_verts;
  std::vector<Handle(Geom_BSplineCurve)> edges;
  std::vector<NFace> faces;
};

bool parse(const double* s, size_t n, NBrep& out) {
  Rd r{s, n};
  if (r.next() != 1.0) return false;
  const int nv = r.count(MAXN);
  for (int i = 0; i < nv && r.ok; i++) {
    const double x = r.next(), y = r.next(), z = r.next();
    out.verts.emplace_back(x, y, z);
  }
  const int ne = r.count(MAXN);
  for (int i = 0; i < ne && r.ok; i++) {
    const int v0 = r.count(nv - 1), v1 = r.count(nv - 1);
    Handle(Geom_BSplineCurve) c = read_c3(r);
    if (!r.ok) break;
    out.edge_verts.emplace_back(v0, v1);
    out.edges.push_back(c);
  }
  const int nf = r.count(MAXN);
  for (int fi = 0; fi < nf && r.ok; fi++) {
    NFace f;
    f.rev = r.next() != 0.0;
    f.srf = read_surface(r);
    const int nl = r.count(MAXN);
    for (int li = 0; li < nl && r.ok; li++) {
      r.next();  // outer / inner: OpenCascade finds the outer wire itself
      NLoop l;
      const int nt = r.count(MAXN);
      for (int ti = 0; ti < nt && r.ok; ti++) {
        NTrim t;
        const double ev = r.next();
        t.rev3d = r.next() != 0.0;
        const double vv = r.next();
        t.c2 = read_c2(r);
        if (!r.ok) break;
        if (ev < 0) {
          if (!(vv >= 0 && vv < nv)) return false;
          t.edge = -1;
          t.vertex = int(vv);
        } else {
          if (!(ev < ne)) return false;
          t.edge = int(ev);
          t.vertex = -1;
        }
        l.trims.push_back(t);
      }
      f.loops.push_back(l);
    }
    out.faces.push_back(f);
  }
  return r.ok && r.k == n;
}

// The analytic surface a NURBS face really is, oriented like it; null if none.
Handle(Geom_Surface) analytic_of(const Handle(Geom_BSplineSurface)& s, double tol) {
  GeomConvert_SurfToAnaSurf conv(s);
  Handle(Geom_Surface) a = conv.ConvertToAnalytical(tol);
  if (a.IsNull()) return a;
  Handle(Geom_ElementarySurface) el = Handle(Geom_ElementarySurface)::DownCast(a);
  if (el.IsNull()) return nullptr;
  double u0, u1, v0, v1;
  s->Bounds(u0, u1, v0, v1);
  gp_Pnt p;
  gp_Vec du, dv;
  s->D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, du, dv);
  const gp_Vec nb = du.Crossed(dv);
  GeomAPI_ProjectPointOnSurf proj(p, a);
  if (!proj.IsDone() || proj.NbPoints() == 0 || nb.Magnitude() < 1e-12) return nullptr;
  double u, v;
  proj.LowerDistanceParameters(u, v);
  gp_Vec au, av;
  gp_Pnt q;
  a->D1(u, v, q, au, av);
  if (au.Crossed(av).Dot(nb) < 0) el->UReverse();
  return a;
}

// Shift `c` by whole periods of `s` so it starts as close as possible to `at`.
void follow(Handle(Geom2d_BSplineCurve)& c, const Handle(Geom_Surface)& s, const gp_Pnt2d& at) {
  const double pu = s->IsUPeriodic() ? s->UPeriod() : 0.0;
  const double pv = s->IsVPeriodic() ? s->VPeriod() : 0.0;
  const gp_Pnt2d st = c->StartPoint();
  const double ku = pu > 0 ? std::round((at.X() - st.X()) / pu) : 0.0;
  const double kv = pv > 0 ? std::round((at.Y() - st.Y()) / pv) : 0.0;
  if (ku != 0.0 || kv != 0.0) c->Translate(gp_Vec2d(ku * pu, kv * pv));
}

// Pcurves of one loop on analytic surface `a`, along each trim; false if one
// cannot be projected.
bool project_loop(const NBrep& b, const NLoop& l, const Handle(Geom_Surface)& a, double tol,
                  std::vector<Handle(Geom2d_BSplineCurve)>& out) {
  out.assign(l.trims.size(), nullptr);
  bool have_prev = false;
  gp_Pnt2d prev;
  for (size_t i = 0; i < l.trims.size(); i++) {
    const NTrim& t = l.trims[i];
    if (t.edge < 0) continue;
    const Handle(Geom_BSplineCurve)& c3 = b.edges[t.edge];
    Handle(Geom2d_Curve) p = GeomProjLib::Curve2d(c3, c3->FirstParameter(), c3->LastParameter(), a);
    if (p.IsNull()) return false;
    Handle(Geom2d_BSplineCurve) c = bspline2(p, p->FirstParameter(), p->LastParameter());
    if (t.rev3d) c->Reverse();
    if (have_prev) follow(c, a, prev);
    out[i] = c;
    prev = c->EndPoint();
    have_prev = true;
  }
  if (!have_prev) return false;
  // Singular trims (poles): straight from the previous end to the next start.
  const size_t n = l.trims.size();
  for (size_t i = 0; i < n; i++) {
    if (!out[i].IsNull()) continue;
    size_t a0 = (i + n - 1) % n, a1 = (i + 1) % n;
    while (out[a0].IsNull() && a0 != i) a0 = (a0 + n - 1) % n;
    while (out[a1].IsNull() && a1 != i) a1 = (a1 + 1) % n;
    if (out[a0].IsNull() || out[a1].IsNull()) return false;
    const gp_Pnt2d p0 = out[a0]->EndPoint(), p1 = out[a1]->StartPoint();
    if (p0.Distance(p1) < 1e-12) return false;
    TColgp_Array1OfPnt2d poles(1, 2);
    poles(1) = p0;
    poles(2) = p1;
    TColStd_Array1OfReal k(1, 2);
    k(1) = 0.0;
    k(2) = 1.0;
    TColStd_Array1OfInteger m(1, 2);
    m(1) = m(2) = 2;
    out[i] = new Geom2d_BSplineCurve(poles, k, m, 1);
  }
  (void)tol;
  return true;
}

// A face bounded only by seams and poles is its whole closed surface (a sphere,
// an ellipsoid): with an analytic surface it is simplest made from that surface.
bool whole_surface(const NFace& f) {
  std::map<int, int> count;
  for (const NLoop& l : f.loops)
    for (const NTrim& t : l.trims)
      if (t.edge >= 0) count[t.edge]++;
  for (auto& [e, c] : count)
    if (c != 2) return false;
  return true;
}

TopoDS_Shape build(const NBrep& nb, double tol, bool analytic, const char*& stage) {
  BRep_Builder B;
  if (analytic && nb.faces.size() == 1 && whole_surface(nb.faces[0])) {
    Handle(Geom_Surface) a = analytic_of(nb.faces[0].srf, tol);
    if (!a.IsNull() && a->IsUClosed()) {
      stage = "whole surface";
      BRepBuilderAPI_MakeFace mf(a, tol);
      if (mf.IsDone()) {
        TopoDS_Face F = mf.Face();
        if (nb.faces[0].rev) F.Reverse();
        TopoDS_Shell sh;
        B.MakeShell(sh);
        B.Add(sh, F);
        sh.Closed(Standard_True);
        TopoDS_Solid so;
        B.MakeSolid(so);
        B.Add(so, sh);
        BRepLib::OrientClosedSolid(so);
        return so;
      }
    }
  }
  std::vector<TopoDS_Vertex> V(nb.verts.size());
  for (size_t i = 0; i < V.size(); i++) B.MakeVertex(V[i], nb.verts[i], tol);
  std::vector<TopoDS_Edge> E(nb.edges.size());
  stage = "edges";
  for (size_t i = 0; i < E.size(); i++) {
    const Handle(Geom_BSplineCurve)& c = nb.edges[i];
    B.MakeEdge(E[i], c, tol);
    B.Add(E[i], V[nb.edge_verts[i].first].Oriented(TopAbs_FORWARD));
    B.Add(E[i], V[nb.edge_verts[i].second].Oriented(TopAbs_REVERSED));
    B.Range(E[i], c->FirstParameter(), c->LastParameter());
  }
  std::vector<int> uses(E.size(), 0);
  TopoDS_Shell shell;
  B.MakeShell(shell);
  for (const NFace& nf : nb.faces) {
    stage = "faces";
    Handle(Geom_Surface) surface(nf.srf);
    std::vector<std::vector<Handle(Geom2d_BSplineCurve)>> projected;
    if (analytic) {
      Handle(Geom_Surface) a = analytic_of(nf.srf, tol);
      if (!a.IsNull()) {
        bool ok = true;
        for (const NLoop& l : nf.loops) {
          std::vector<Handle(Geom2d_BSplineCurve)> p;
          ok = ok && project_loop(nb, l, a, tol, p);
          projected.push_back(p);
        }
        if (ok) {
          surface = a;
        } else {
          projected.clear();
        }
      }
    }
    TopoDS_Face F;
    B.MakeFace(F, surface, tol);
    std::map<int, Handle(Geom2d_Curve)[2]> pc;
    for (size_t li = 0; li < nf.loops.size(); li++) {
      const NLoop& l = nf.loops[li];
      TopoDS_Wire W;
      B.MakeWire(W);
      for (size_t ti = 0; ti < l.trims.size(); ti++) {
        const NTrim& tr = l.trims[ti];
        Handle(Geom2d_BSplineCurve) c2 = projected.empty()
            ? Handle(Geom2d_BSplineCurve)::DownCast(tr.c2->Copy())
            : projected[li][ti];
        if (tr.edge < 0) {
          TopoDS_Edge D;
          B.MakeEdge(D);
          B.UpdateEdge(D, c2, F, tol);
          B.Degenerated(D, Standard_True);
          B.Add(D, V[tr.vertex].Oriented(TopAbs_FORWARD));
          B.Add(D, V[tr.vertex].Oriented(TopAbs_REVERSED));
          B.Range(D, c2->FirstParameter(), c2->LastParameter());
          B.Add(W, D);
          continue;
        }
        const int ei = tr.edge;
        uses[ei]++;
        if (tr.rev3d) c2->Reverse();  // now runs along the edge
        double ef, el;
        BRep_Tool::Range(E[ei], ef, el);
        TColStd_Array1OfReal k(1, c2->NbKnots());
        c2->Knots(k);
        BSplCLib::Reparametrize(ef, el, k);
        c2->SetKnots(k);
        pc[ei][tr.rev3d ? 1 : 0] = c2;
        B.Add(W, E[ei].Oriented(tr.rev3d ? TopAbs_REVERSED : TopAbs_FORWARD));
      }
      W.Closed(Standard_True);
      B.Add(F, W);
    }
    for (auto& [ei, c] : pc) {
      if (!c[0].IsNull() && !c[1].IsNull()) {
        B.UpdateEdge(E[ei], c[0], c[1], F, tol);
      } else {
        B.UpdateEdge(E[ei], c[0].IsNull() ? c[1] : c[0], F, tol);
      }
    }
    if (nf.rev) F.Reverse();
    B.Add(shell, F);
  }
  // Each edge's pcurves follow its 3D curve; edges where OpenCascade's own
  // approximation trips are left to ShapeFix below.
  stage = "same parameter";
  for (auto& e : E) {
    B.SameParameter(e, Standard_False);
    try {
      BRepLib::SameParameter(e, tol);
    } catch (const Standard_Failure&) {
      ShapeFix_Edge fe;
      fe.FixSameParameter(e, tol);
    }
  }
  bool closed = !E.empty();
  for (int u : uses) closed = closed && u == 2;
  TopoDS_Shape result = shell;
  if (closed) {
    shell.Closed(Standard_True);
    TopoDS_Solid so;
    B.MakeSolid(so);
    B.Add(so, shell);
    BRepLib::OrientClosedSolid(so);
    result = so;
  }
  stage = "shape fix";
  ShapeFix_Shape fix(result);
  fix.SetPrecision(tol);
  fix.SetMaxTolerance(tol * 1000.0);
  fix.Perform();
  return fix.Shape();
}

}  // namespace

extern "C" {

// `analytic`: try analytic surfaces first (offsets and shells need them; boolean
// and fillet results round-trip to openNURBS more reliably on NURBS surfaces).
FoShape* fo_shape_from_neutral(const double* s, size_t n, double tol, int analytic_first) {
  const char* stage = "reading";
  try {
    NBrep nb;
    if (!parse(s, n, nb)) {
      set_error("malformed exact-brep stream");
      return nullptr;
    }
    // Analytic surfaces first; plain NURBS when that does not check out.
    for (bool analytic : {true, false}) {
      if (analytic && !analytic_first) continue;
      try {
        TopoDS_Shape r = build(nb, tol, analytic, stage);
        if (BRepCheck_Analyzer(r).IsValid()) return wrap(r);
      } catch (const Standard_Failure&) {
        if (!analytic) throw;
      }
    }
    set_error("the exact solid did not pass OpenCascade's check");
    return nullptr;
  } catch (const Standard_Failure& e) {
    set_error(std::string("OpenCascade (") + stage + "): " + e.DynamicType()->Name() +
              (e.GetMessageString() && *e.GetMessageString()
                   ? std::string(": ") + e.GetMessageString()
                   : std::string()));
    return nullptr;
  } catch (...) {
    set_error(std::string("C++ exception while ") + stage);
    return nullptr;
  }
}

// A shape as a neutral stream (all geometry converted to NURBS). Null with an error
// when some face cannot be written.
double* fo_shape_to_neutral(const FoShape* sh, size_t* n) {
  *n = 0;
  FO_TRY
  BRepBuilderAPI_NurbsConvert conv(sh->shape, Standard_True);
  const TopoDS_Shape s = conv.Shape();
  TopTools_IndexedMapOfShape vmap, emap, fmap;
  TopExp::MapShapes(s, TopAbs_VERTEX, vmap);
  TopExp::MapShapes(s, TopAbs_EDGE, emap);
  TopExp::MapShapes(s, TopAbs_FACE, fmap);
  std::vector<double> out{1.0, double(vmap.Extent())};
  for (int i = 1; i <= vmap.Extent(); i++) {
    const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(vmap(i)));
    out.insert(out.end(), {p.X(), p.Y(), p.Z()});
  }
  std::vector<int> eidx(emap.Extent() + 1, -1);
  std::vector<double> edges;
  int ne = 0;
  for (int i = 1; i <= emap.Extent(); i++) {
    const TopoDS_Edge e = TopoDS::Edge(emap(i).Oriented(TopAbs_FORWARD));
    if (BRep_Tool::Degenerated(e)) continue;
    double f, l;
    Handle(Geom_Curve) c = BRep_Tool::Curve(e, f, l);
    if (c.IsNull()) {
      set_error("an edge has no 3D curve");
      return nullptr;
    }
    const int v0 = vmap.FindIndex(TopExp::FirstVertex(e)) - 1;
    const int v1 = vmap.FindIndex(TopExp::LastVertex(e)) - 1;
    if (v0 < 0 || v1 < 0) {
      set_error("an edge has no vertices");
      return nullptr;
    }
    edges.insert(edges.end(), {double(v0), double(v1)});
    put_c3(bspline3(c, f, l), edges);
    eidx[i] = ne++;
  }
  out.push_back(double(ne));
  out.insert(out.end(), edges.begin(), edges.end());
  out.push_back(double(fmap.Extent()));
  for (int fi = 1; fi <= fmap.Extent(); fi++) {
    const TopoDS_Face fo = TopoDS::Face(fmap(fi));
    const TopoDS_Face F = TopoDS::Face(fo.Oriented(TopAbs_FORWARD));
    Handle(Geom_BSplineSurface) srf =
        GeomConvert::SurfaceToBSplineSurface(BRep_Tool::Surface(F));
    if (srf.IsNull()) {
      set_error("a face has no NURBS form");
      return nullptr;
    }
    if (srf->IsUPeriodic()) srf->SetUNotPeriodic();
    if (srf->IsVPeriodic()) srf->SetVNotPeriodic();
    out.push_back(fo.Orientation() == TopAbs_REVERSED ? 1.0 : 0.0);
    out.insert(out.end(), {double(srf->UDegree()), double(srf->VDegree()),
                           double(srf->NbUPoles()), double(srf->NbVPoles())});
    {
      TColStd_Array1OfReal k(1, srf->NbUKnots());
      TColStd_Array1OfInteger m(1, srf->NbUKnots());
      srf->UKnots(k);
      srf->UMultiplicities(m);
      put_knots(k, m, out);
    }
    {
      TColStd_Array1OfReal k(1, srf->NbVKnots());
      TColStd_Array1OfInteger m(1, srf->NbVKnots());
      srf->VKnots(k);
      srf->VMultiplicities(m);
      put_knots(k, m, out);
    }
    for (int i = 1; i <= srf->NbUPoles(); i++) {
      for (int j = 1; j <= srf->NbVPoles(); j++) {
        const gp_Pnt p = srf->Pole(i, j);
        out.insert(out.end(), {p.X(), p.Y(), p.Z(), srf->Weight(i, j)});
      }
    }
    std::vector<TopoDS_Wire> wires;
    const TopoDS_Wire outer = BRepTools::OuterWire(F);
    if (!outer.IsNull()) wires.push_back(outer);
    for (TopExp_Explorer ex(F, TopAbs_WIRE); ex.More(); ex.Next()) {
      if (outer.IsNull() || !ex.Current().IsSame(outer)) wires.push_back(TopoDS::Wire(ex.Current()));
    }
    out.push_back(double(wires.size()));
    for (size_t wi = 0; wi < wires.size(); wi++) {
      out.push_back(wi == 0 ? 0.0 : 1.0);
      const size_t count_at = out.size();
      out.push_back(0.0);
      // Order the edges by their curves on the face: each starts where the previous
      // one ends in (u, v). (BRepTools_WireExplorer stops early when a closed edge
      // and a seam share the start vertex.)
      struct Piece {
        TopoDS_Edge e;
        Handle(Geom2d_BSplineCurve) c;
        gp_Pnt2d a, b;
      };
      std::vector<Piece> pieces;
      for (TopoDS_Iterator it(wires[wi]); it.More(); it.Next()) {
        const TopoDS_Edge e = TopoDS::Edge(it.Value());
        Handle(Geom2d_BSplineCurve) b2 = curve_on_face(e, F);
        if (b2.IsNull()) {
          set_error("an edge has no curve on its face");
          return nullptr;
        }
        if (e.Orientation() == TopAbs_REVERSED) b2->Reverse();
        pieces.push_back({e, b2, b2->StartPoint(), b2->EndPoint()});
      }
      std::vector<size_t> order;
      std::vector<bool> used(pieces.size(), false);
      for (size_t step = 0; step < pieces.size(); step++) {
        size_t best = pieces.size();
        if (step == 0) {
          best = 0;
        } else {
          const gp_Pnt2d at = pieces[order.back()].b;
          double bd = 1e300;
          for (size_t i = 0; i < pieces.size(); i++) {
            if (used[i]) continue;
            const double d = at.Distance(pieces[i].a);
            if (d < bd) {
              bd = d;
              best = i;
            }
          }
        }
        used[best] = true;
        order.push_back(best);
      }
      int nt = 0;
      for (size_t idx : order) {
        const Piece& pc = pieces[idx];
        const TopoDS_Edge& e = pc.e;
        const bool reversed = e.Orientation() == TopAbs_REVERSED;
        if (BRep_Tool::Degenerated(e)) {
          const int v = vmap.FindIndex(TopExp::FirstVertex(e)) - 1;
          out.insert(out.end(), {-1.0, 0.0, double(v)});
        } else {
          const int ei = eidx[emap.FindIndex(e)];
          if (ei < 0) {
            set_error("a trim refers to an unknown edge");
            return nullptr;
          }
          out.insert(out.end(), {double(ei), reversed ? 1.0 : 0.0, -1.0});
        }
        put_c2(pc.c, out);
        nt++;
      }
      out[count_at] = double(nt);
    }
  }
  double* buf = static_cast<double*>(std::malloc(out.size() * sizeof(double)));
  if (!buf) {
    set_error("out of memory");
    return nullptr;
  }
  std::memcpy(buf, out.data(), out.size() * sizeof(double));
  *n = out.size();
  return buf;
  FO_CATCH(nullptr)
}

void fo_doubles_free(double* p) { std::free(p); }

}  // extern "C"
