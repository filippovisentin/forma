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

## Key decisions (open)

- **Geometry kernel** — curvo + truck in pure Rust vs OpenCascade bindings for booleans,
  fillets and offsets. Decided by spike S2 in M0 (see `docs/spikes/S2-kernel.md`).
- **`.3dm` I/O** — openNURBS via `cxx`/`bindgen`, built from source on Windows (MSVC).
  Decided by spike S1 in M0 (see `docs/spikes/S1-opennurbs.md`).
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
