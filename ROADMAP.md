# Forma roadmap

Personal Rhino-style modeller for interior design work. Scope = the commands Filippo
actually uses, not full Rhino parity. No Grasshopper.

Estimates are rough agent wall-clock hours, to be corrected as we measure.

## Status

| # | Milestone | Status | Est. hours |
|---|---|---|---|
| M0 | Skeleton: workspace, command engine, document + undo, CLI, CI gates; spikes S1 (openNURBS) and S2 (kernel) | **in progress** — workspace, engine, `Line`, CLI and xtask done | 10 |
| M1 | Viewport & precision: egui app, 3D viewport (orbit/pan/zoom, top/front/right/perspective), grid, CPlane, osnaps (end, mid, cen, int, perp), ortho, typed coordinates; `Polyline`, `Curve`, `Arc`, `Circle`, `Rectangle` | todo | 25 |
| M2 | Surfaces & solids: `Extrude`, `ExtrudeCrv`, `Loft`, `Revolve`, `Sweep1`, `Box`, `Cylinder`, `PlanarSrf`, shaded display | todo | 30 |
| M3 | Transform & edit: `Move`, `Copy`, `Rotate`, `Scale`, `Mirror`, `Array`, `Join`, `Explode`, `Trim`, `Split`, gumball, layers panel | todo | 25 |
| M4 | Hard kernel: `BooleanUnion/Difference/Intersection`, `FilletEdge`, `Offset`, `OffsetSrf`, `Cap` | todo | 40 |
| M5 | Files: `.3dm` read/write (layers, materials, curves, surfaces, breps), OBJ/STL/glTF export | todo | 20 |
| M6 | Agents: MCP server (`command_line`, `execute`, `inspect`, `render`), offscreen render to PNG | todo | 12 |

**Total remaining ≈ 160 hours**, compressible with parallel agents on separate crates.

## Command scope

To be replaced with Filippo's real command list (from Rhino command history).
Commands not on that list are out of scope until requested.

## M0 checklist

- [x] Cargo workspace with layered crates
- [x] `xtask ci` and `xtask layering`
- [x] Command registry + command-line parser, `Line` command
- [x] Document with undo/redo
- [x] `forma-cli run --script ... --dump`
- [ ] Spike S1: openNURBS builds and reads a `.3dm` on Windows (MSVC) and Linux
- [ ] Spike S2: kernel choice — boolean + fillet on Filippo's test cases
- [ ] GitHub Actions running `cargo xtask ci` on Windows and Linux
