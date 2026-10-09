# ADR 0003 — Viewport stack and display meshes for `.3dm` files

- Status: accepted
- Date: 2026-10-09

## Context
M1 needs a window that opens Filippo's `.3dm` files and lets him orbit them, delivered
as a ready-to-run Windows `.exe` (he does not compile locally). Two findings shaped it:

- `gggg.3dm` stores **no render meshes** (630 breps/extrusions, 0 cached meshes), and
  public openNURBS declares `ON_Brep::CreateMesh` but does not implement it.
- egui's wgpu backend wants native textures as `Rgba8Unorm` holding gamma-encoded values.

## Decision
- **UI:** `eframe`/`egui` 0.36 with the wgpu renderer; `rfd` 0.17 for the open dialog.
- **Rendering:** `forma-render` uses `wgpu` 30 directly (plus `glam`, `bytemuck`,
  `pollster`) and renders into its **own offscreen texture** (sRGB, 4× MSAA, depth).
  egui shows it through a second `Rgba8Unorm` view of the same texture. The same code
  produces headless screenshots (`forma-cli render`, tests, future MCP `render`).
- **Display meshes:** each brep face is triangulated in Forma: openNURBS samples the trim
  loops in (u, v) and evaluates points/normals; `spade` 2.15 builds a constrained Delaunay
  triangulation of the loops plus an interior grid; triangles outside the trimmed region
  are dropped. Extrusions go through `ON_Extrusion::BrepForm`. Native meshes are used as is.
- **Opening files is a command** (`Open`) in `forma-engine`, so UI, CLI and agents share it.

## Verification
- `gggg.3dm`: all 262 breps, 368 extrusions and 11 meshes produce triangles; 309 curves
  become polylines; only the 3 block instances are skipped. Import ≈ 2.2 s (release).
- Headless render test (skipped without a GPU) and manual checks on a virtual display with
  Mesa's software Vulkan driver: layer toggles, orbit, zoom, standard views, commands.

## Consequences
- Display quality is good for viewing, not for modelling; OCCT (ADR 0001) will provide
  exact tessellation and solids later.
- Block instances are not shown yet (M5).
- Release builds for Windows come from `.github/workflows/release.yml`; the binary is
  unsigned, so SmartScreen asks for confirmation on first run.
