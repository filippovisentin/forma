# Forma roadmap

Personal Rhino-style modeller for interior design work. Scope = the commands Filippo
actually uses, not full Rhino parity. No Grasshopper.

Estimates are rough agent wall-clock hours, to be corrected as we measure.

## Status

| # | Milestone | Status | Est. hours |
|---|---|---|---|
| M0 | Skeleton: workspace, command engine, document + undo, CLI, CI gates; spikes S1 (openNURBS) and S2 (kernel) | **in progress** — workspace, engine, CLI, xtask and S1 (`.3dm` reading) done; S2 next | 10 |
| M1 | Viewport & precision: egui app, 3D viewport (orbit/pan/zoom, top/front/right/perspective), grid, CPlane, osnaps (end, mid, cen, int, perp), ortho, typed coordinates; `Polyline`, `Curve`, `Arc`, `Circle`, `Rectangle`. **Target demo: open `gggg.3dm` read-only and orbit it** (needs S1 + render meshes of breps/extrusions) | todo | 30 |
| M2 | Surfaces & solids: `Extrude`, `ExtrudeCrv`, `Loft`, `Revolve`, `Sweep1`, `Box`, `Cylinder`, `PlanarSrf`, shaded display | todo | 30 |
| M3 | Transform & edit: `Move`, `Copy`, `Rotate`, `Scale`, `Mirror`, `Array`, `Join`, `Explode`, `Trim`, `Split`, gumball, layers panel | todo | 25 |
| M4 | Hard kernel: `BooleanUnion/Difference/Intersection`, `FilletEdge`, `Offset`, `OffsetSrf`, `Cap` | todo | 40 |
| M5 | Files: full `.3dm` round trip (nested layers, materials, blocks, extrusions, breps, meshes, poly/NURBS curves, units), OBJ/STL/glTF export | todo | 25 |
| M6 | Agents: MCP server (`command_line`, `execute`, `inspect`, `render`), offscreen render to PNG | todo | 12 |

**Total remaining ≈ 160 hours**, compressible with parallel agents on separate crates.

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
Commands not on that list are out of scope until requested.

## M0 checklist

- [x] Cargo workspace with layered crates
- [x] `xtask ci` and `xtask layering`
- [x] Command registry + command-line parser, `Line` command
- [x] Document with undo/redo
- [x] `forma-cli run --script ... --dump`
- [x] Spike S1: openNURBS builds and reads Filippo's `.3dm` files on Linux (counts match rhino3dm); `forma-cli info` — ADR 0002
- [x] S1 on Windows (MSVC): builds, links and passes tests in the CI Windows job
- [ ] S1 black-box check: a file written by Forma opens correctly in Rhino
- [ ] Spike S2: kernel choice — boolean + fillet on Filippo's test cases
- [x] GitHub Actions running `cargo xtask ci` on Windows and Linux
