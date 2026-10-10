# Forma roadmap

Personal Rhino-style modeller for interior design work. Scope = the commands Filippo
actually uses, not full Rhino parity. No Grasshopper.

Estimates are rough agent wall-clock hours, to be corrected as we measure.

## Status

| # | Milestone | Status | Est. hours |
|---|---|---|---|
| M0 | Skeleton: workspace, command engine, document + undo, CLI, CI gates; spikes S1 (openNURBS) and S2 (kernel) | **done** — S1: openNURBS `.3dm` I/O (ADR 0002); S2: OpenCascade chosen for solids (ADR 0001). Open: Rhino black-box check of a written file | — |
| M1 | Viewport & precision | **done (v0.2.0)** — 4 viewports (Top/Perspective/Front/Right) with per-view CPlanes and grids, orbit/pan/zoom at cursor, maximize; osnaps End/Mid/Cen/Quad, grid snap, Ortho; typed coordinates (absolute, @relative, @d<a, length constraint); Line, Polyline, Rectangle, Circle, Arc | — |
| M2 | Surfaces & solids | **partial** — Box, Cylinder, Sphere, ExtrudeCrv as display meshes with visible edges; PlanarSrf (with holes), ExtrudeSrf, Cap, Loft, Revolve, Sweep1 (rotation-minimising frames) as meshes; NURBS curves (Curve, InterpCrv, Ellipse; exact rational arcs under non-uniform scale) and point objects. push / pull of flat faces (MoveFace, PushPull, gumball extrude dot on solids, Ctrl+Shift+click face selection; v0.6.0). Todo: NURBS surfaces, exact solids via OCCT | 16 |
| M3 | Transform & edit | **done (v0.4.0)** — selection, Move, Copy, Rotate, Scale, Mirror, Delete; gumball (move along axis / in plane, rotate, typed values); Array, ArrayLinear, ArrayPolar; planar curve tools Offset, Trim, Extend, Fillet, FilletCorners; Join / Explode (curves → polycurves, meshes ↔ faces); object colour (SetObjectColor) and layer colour / visibility / lock; Properties panel; Rhino-like menus with submenus and toolbar tabs; Split, Chamfer, Intersect, Polygon; Scale1D/Scale2D, Orient (2-point), Align, Flip, MatchProperties, BoundingBox, ProjectToCPlane; Hide/Show/Isolate, Lock/Unlock, Group/Ungroup/SelGroup, SelLast/SelCrv/SelMesh/SelPt/Invert; internal clipboard (CopyToClipboard/Cut/Paste), Import/Export of selected objects; analysis (Distance, Length, Area, Volume, What). All in the UI (v0.4.0) with a Rhino 8-like interface: light theme, command line on top, 10 toolbar tabs, sidebar palette, viewport title menus with display modes (Wireframe/Shaded/Ghosted/X-Ray), viewport tabs, 11 object snaps + Project/Disable, status bar panes (Grid Snap, Ortho, Planar, Osnap, Gumball, Filter). v0.5.0: gumball extrude dots, live measurements, SmartTrack, osnaps while dragging the gumball, cm by default. UI review (v0.7): `forma-ui` split into modules; command-line autocomplete, history recall and clickable options; "Save changes?" guard and `*` in the title; settings and recent files kept between runs; incremental snap / pick index with screen-space culling (cursor hover on gggg.3dm 33–48 ms → ~5 ms per frame, click pick 16 → 0.2 ms, no continuous repaint while a tool waits). Todo: gumball scale, Record History | 1 |
| M3b | Rhino command parity without the kernel | **done (v0.7)** — parity table in `docs/commands.md` (279 Rhino commands: 152 ✓, 42 partial, 58 ✗, 27 need the kernel). Added: Circle2Pt/3Pt, Arc3Pt, Rectangle 3Pt/Center/Rounded, Slot, Helix, Spiral; Divide, DivideByLength, Contour, Section, Project, Pull, DupEdge, DupBorder, CurveBoolean, Rebuild, Convert, CloseCrv, ExtractPt; Plane, SrfPt, EdgeSrf, Cone, TCone, Torus, Ellipsoid, Pyramid, Tube, Pipe, Slab, ExtrudeCrvAlongCrv / Tapered / ToPoint, Weld, Unweld; Rotate3D, Mirror3Pt, Orient3Pt, ScaleNU, Shear, Bend, Twist, Taper, ArrayCrv, SetPt, BoxEdit, Distribute; Text, Dot, Dim, DimAligned, DimRadius, DimDiameter, DimAngle, Leader, Hatch, DimStyle; Angle, Radius, EvaluatePt, Area/VolumeCentroid; layer tools (Rename/Delete/OneLayerOn/Off, AllLayersOn, Purge, ChangeToCurrentLayer, CopyObjectsToLayer), object names, selection filters (SelLayer, SelColor, SelName, SelSmall, SelDup, SelOpen/ClosedCrv, SelClosed/OpenMesh, SelVisible…), HideSwap, LockSwap. In the UI as "sequence tools" (prompts, previews, clickable options) and new Mesh / Dimension menus; text and dimension labels drawn as a viewport overlay | — |
| M4 | Hard kernel (OpenCascade behind `forma-geom`, feature `occt`; starts with spike **S3: `.3dm` brep ↔ OCCT conversion**): `BooleanUnion/Difference/Intersection`, `FilletEdge`, `Offset`, `OffsetSrf`, `Cap` | **partial** — first kernel milestone (ADR 0004: `occt-sys` + own C ABI shim, static OCCT 7.8.1): closed meshes are sewn into exact solids (coplanar triangles → planar faces), operated on, and triangulated back (crisp edges, welded smooth seams). Commands `BooleanUnion`, `BooleanDifference`, `BooleanIntersection`, `BooleanSplit`, `FilletEdge`, `ChamferEdge`, `Shell`, `OffsetSrf` (closed solids); UI Solid menu / Solid Tools tab enabled. Built and tested in CI (`cargo xtask ci --occt`, Linux + Windows, OCCT cached) and in release builds. Todo: keep exact breps in the document (today results are stored as meshes, so curved faces become facets for the next operation), S3 (imported Rhino breps are still meshes), variable-radius fillets, `Offset`/`Cap` on breps | 40 |
| M5 | Files (`.3dm` save done in v0.2.0: curves exact, solids/imported surfaces as meshes, nested layers, units): full `.3dm` round trip (nested layers, materials, blocks, extrusions, breps, meshes, poly/NURBS curves, units), OBJ/STL/glTF export | todo | 25 |
| M6 | Agents: MCP server (`command_line`, `execute`, `inspect`, `render`), offscreen render to PNG | todo | 12 |

**Total remaining ≈ 160 hours** (+ ~8 for S3), compressible with parallel agents on separate crates.

## What Filippo's real files contain

Reference models (kept out of git in `tests/data/private/`): `binario.3dm` (10 MB) and
`gggg.3dm` (90 MB, an exhibition stand). Both Rhino 8, **centimetres, abs. tolerance
0.01, angle 1°**.

| | binario.3dm | gggg.3dm |
|---|---|---|
| Objects | 1,167 | 953 |
| Main types | 904 lines, 256 breps, 5 extrusions, 2 polycurves | 368 extrusions, 262 breps, 205 polycurves, 73 polylines, 30 NURBS curves, 11 meshes, 3 block instances |
| Closed solids | 256 / 256 breps | 261 / 262 breps |
| Faces per brep (min / median / max) | 3 / 3 / 152 | 3 / 3 / 152 |
| Layers / materials / blocks | 9 / 4 / 3 | 14 (nested, e.g. `muri::colonne`) / 25 / 3 |
| Extents | 64 × 31 × 8 m | 70 × 68 × 20 m |

Consequences for the design:

- **Units are per document.** Default stays mm, but cm documents with 0.01 tolerance must
  load and keep their units and tolerances.
- **Extrusion is a first-class object type** (Rhino's lightweight extrusion), not just a brep.
- **Layers are a tree** (`parent::child`), and **block instances** must survive a round trip.
- Most breps are small closed solids (3 faces ≈ cylinders, e.g. ropes and supports), so
  tessellation and display of hundreds of solids matters early.
- Scale: ~1,000 objects per file is the performance baseline.

## Command scope

To be replaced with Filippo's real command list (from Rhino command history).
Commands not on that list are out of scope until requested. Current coverage of
Rhino's common commands: `docs/commands.md`.

## M0 checklist

- [x] Cargo workspace with layered crates
- [x] `xtask ci` and `xtask layering`
- [x] Command registry + command-line parser, `Line` command
- [x] Document with undo/redo
- [x] `forma-cli run --script ... --dump`
- [x] Spike S1: openNURBS builds and reads Filippo's `.3dm` files on Linux (counts match rhino3dm); `forma-cli info` — ADR 0002
- [x] S1 on Windows (MSVC): builds, links and passes tests in the CI Windows job
- [ ] S1 black-box check: a file written by Forma opens correctly in Rhino
- [x] Spike S2: kernel choice — OpenCascade passes 6/6 interior-design cases, truck 2/6 (`docs/spikes/S2-results.md`, ADR 0001)
- [x] GitHub Actions running `cargo xtask ci` on Windows and Linux
