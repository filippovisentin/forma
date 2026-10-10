# ADR 0006 — Exact surfaces and solids kept as openNURBS objects

- Status: accepted
- Date: 2026-10-11
- Follows: ADR 0002 (openNURBS for `.3dm`), ADR 0003 (display meshes), ADR 0004 (OCCT)

## Context
Until v0.8 every surface and solid in a Forma document was a triangle mesh. Opening a
Rhino file and saving it again turned its polysurfaces into meshes, fillets were faceted,
and nothing Forma made could be edited as a NURBS object in Rhino. Forma needs to keep
exact geometry through the whole round trip, and to create it, without waiting for the
full OCCT ⇄ brep bridge (spike S3).

## Options
1. **Our own brep data structure in `forma-geom`** (faces, loops, trims, NURBS
   surfaces): the cleanest model, but months of work before the first file round-trips.
2. **OCCT shapes in the document**: exact, but OCCT is an optional feature (not in the
   default build or the default CI), and every `.3dm` read/write would need a brep ⇄ OCCT
   conversion that does not exist yet.
3. **Keep the native openNURBS object as opaque bytes** (`ON_Brep`, `ON_Extrusion`, a
   surface), serialized with openNURBS' own archive format, plus a display mesh, a
   pending transform and a few facts (face count, closed, all faces flat).

## Decision
Option 3: `forma_geom::Brep { data, xform, mesh, faces, solid, planar, … }` and
`Geometry::Brep` in `forma-doc`.

- `forma-geom` and `forma-doc` never interpret the bytes. Transforms only accumulate in
  `xform` (and move the display mesh); `forma-io-3dm` applies them to the exact object
  when it is written or operated on. Shears and non-uniform scales first convert the
  faces to NURBS (exact for planes, surfaces of revolution with straight trims, …);
  mirrors flip the faces back so solids keep facing outwards.
- All exact work lives in `forma-io-3dm::exact` (C ABI in `shim.cpp`): constructors
  (box, cylinder, cone, sphere, torus, planar regions and their extrusions, extruded
  curves, surfaces of revolution, polyhedra), and operations (faces, edges, face loops,
  flip, extrude a surface, cap planar holes, join naked edges).
- Display meshes are made per face from the trims, with the edge samples shared by
  neighbouring faces, so a closed brep gives a closed (welded) mesh; tools that still
  work on triangles (kernel booleans, deforms) use that mesh.
- Mesh results of flat-faced inputs (push / pull, booleans of boxes and walls, Join) are
  rebuilt as exact polysurfaces of trimmed planes (`Mesh::polyhedron` + the shim's
  polyhedron builder); curved results stay meshes until S3.
- No new dependency: openNURBS is already vendored (ADR 0002).

## Consequences
- Rhino polysurfaces, extrusions, surfaces and curves survive Open → edit → Save exactly.
- Fillets, Loft/Sweep and booleans involving curved faces are still meshes; the next
  step is S3 (brep ⇄ OCCT) on top of these bytes, which keeps the document model.
- Control points of curved surfaces are not editable yet (flat-faced polysurfaces expose
  their corners).
