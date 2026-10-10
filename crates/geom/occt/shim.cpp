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

#include <cstdint>
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
