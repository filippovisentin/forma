# Forma architecture

## Layers

A crate may depend only on crates in a strictly lower layer. `cargo xtask layering`
checks this on every CI run.

| Layer | Crate | Responsibility |
|---|---|---|
| 0 | `forma-geom` | Math and geometry: points, vectors, planes, tolerances, NURBS curves and surfaces, tessellation. Hides the kernel choice (curvo / truck / OpenCascade) behind our own types. |
| 1 | `forma-doc` | The document: objects, layers, materials, attributes, selection state, undo/redo history. |
| 2 | `forma-io-3dm` | Read/write Rhino `.3dm` through openNURBS (FFI). |
| 2 | `forma-io-mesh` | Export OBJ, STL, glTF. |
| 2 | `forma-render` | Turn document geometry into GPU-ready meshes and lines; viewports, camera, display modes, picking. wgpu, no windowing. |
| 3 | `forma-engine` | Command registry, command-line parser and prompts, object snaps, construction planes, sessions. The single entry point for every action. |
| 4 | `forma-ui` | egui front end: viewports, command line widget, layer panel, properties. Replaceable. |
| 4 | `forma-mcp` | MCP server exposing `command_line`, `execute`, `inspect`, `render`. |
| 5 | `apps/forma` | Desktop binary (eframe). |
| 5 | `apps/forma-cli` | Headless CLI: run scripts, convert files, start MCP. |
| — | `xtask` | Dev automation (`ci`, `layering`). Not part of the product. |

## Key decisions

- **Geometry kernel** — OpenCascade for all solid modelling (booleans, fillets, lofts,
  offsets), hidden inside `forma-geom` behind a cargo feature `occt`; plain curve math in
  Rust. truck failed 4/6 interior-design cases in spike S2. See ADR 0001.
  Built by `occt-sys` (static OCCT 7.8.1) and called through a small C ABI shim in
  `crates/geom/occt/shim.cpp`; `forma-geom` exposes only mesh-in / mesh-out functions. See ADR 0004.
- **`.3dm` I/O** — openNURBS linked through a small C ABI shim, built with CMake from a
  git submodule. See ADR 0002.
- **UI** — egui + wgpu via eframe, same stack as ArtCraft's Crafting Apps.

## Data flow

```
input (UI / command line / MCP / CLI script)
        │  text or JSON
        ▼
forma-engine ── parses → Command ── prompts for points/objects (snaps, CPlane)
        │                      │
        │                      ▼
        │               forma-doc (mutates through a Transaction → undo entry)
        ▼                      │
forma-render  ◄────────────────┘  (re-tessellates changed objects only)
```

## Testing strategy

- Unit tests per crate (geometry with explicit tolerances).
- Command tests through `Engine::run_line` asserting on the document.
- Golden tests: script → document dump + offscreen PNG, compared to references.
- Black-box comparison with Rhino on Filippo's own `.3dm` files (behaviour only).
