# Changelog

All notable changes to Forma. Versions are the
[GitHub releases](https://github.com/filippovisentin/forma/releases); each ships
`Forma-windows.zip` (`forma.exe`, `forma-cli.exe`, `LEGGIMI.txt`, licences).
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Command line: autocomplete list while typing, Up/Down history on an empty line, clickable
  command options (`Side to offset ( Distance=10 )`), a grey hint for the command that Enter
  or right click will repeat.
- "Save changes?" guard before New, Open and closing the window; `*` in the title when there
  are unsaved changes.
- Settings remembered between sessions (osnaps, Grid Snap, Ortho, Planar, SmartTrack, Gumball,
  display modes, toolbar tab, panel widths, last folder) in `%APPDATA%\Forma\settings.txt`;
  **File → Recent Files** (last 10).
- Rich toolbar tooltips: command, description, alias and shortcut.
- Documentation for the public release: README, install guide, 10-minute tutorial (English and
  Italian), contributing guide, issue templates, screenshots, a download page.

### Changed
- Much faster interaction on large files: incremental snap / pick index with screen-space
  culling (hover on a 950-object file from 33–48 ms to about 5 ms per frame, click pick from
  16 ms to 0.2 ms), no continuous repaint while a tool is idle.
- `forma-ui` split into modules (no behaviour change).

### In progress
- **Rhino command parity**: more of the commands used in day-to-day interior work, with the
  same names, prompts and options.
- **OpenCascade kernel** behind `forma-geom` (ADR 0001): exact solids, `BooleanUnion`,
  `BooleanDifference`, `BooleanIntersection`, `FilletEdge`, starting with spike S3
  (`.3dm` brep ↔ OpenCascade conversion).

## [0.6.0] — 2026-10-10

### Added
- **Push / pull faces of solids.** Ctrl+Shift+click a flat face: it is outlined in yellow with
  an orange arrow to drag or click-and-type, with a live preview in document units.
- The gumball extrude dot on a solid pushes or pulls the face on that side.
- Commands `MoveFace #id <point> <distance>` and `PushPull <direction> <distance>`; the faces
  around the moved one stretch to follow.

## [0.5.0] — 2026-10-09

### Added
- Gumball **extrude dot** on each arrow: curves extrude (closed curves into solids), planar
  surfaces into solids.
- **Live measurements** next to the cursor (length and angle, width × height, radius, height,
  scale factor) and the distance from the last point in the status bar.
- **SmartTrack**: alignment lines through snap points you rest on, combined with Ortho.
- Object snaps steer gumball drags; a drag starts where the button went down.

### Changed
- New documents are in **centimetres**.

## [0.4.0] — 2026-10-09

### Added
- Rhino 8-like interface: light theme, command line on top, ten toolbar tabs, sidebar palette,
  redesigned icons, viewport title menus and tabs, axes icon, Osnap bar (End, Near, Point, Mid,
  Cen, Int, Perp, Tan, Quad, Knot, Vertex, Project, Disable), status bar panes (Grid Snap,
  Ortho, Planar, Osnap, Gumball, Filter), selection filter, Properties / Layers / Help panels.
- Display modes Wireframe, Shaded, Ghosted, X-Ray.
- Keyboard shortcuts Ctrl+C/X/V/G/H/L; commands wait for their arguments until Enter.
- About 50 commands since 0.3.0: NURBS curves (`Curve`, `InterpCrv`, exact `Ellipse`), `Point`,
  `Points`, `Polygon`, `Split`, `Chamfer`, `Intersect`, `Scale1D`, `Scale2D`, `Orient`, `Align`,
  `Flip`, `MatchProperties`, `BoundingBox`, `ProjectToCPlane`, `Hide`, `Show`, `Isolate`,
  `Lock`, `Unlock`, `Group`, `Ungroup`, `SelGroup`, `SelLast`, `SelCrv`, `SelMesh`, `SelPt`,
  `Invert`, `CopyToClipboard`, `Cut`, `Paste`, `Import`, `Export`, `PlanarSrf`, `ExtrudeSrf`,
  `Cap`, `Loft`, `Revolve`, `Sweep1`, `Distance`, `Length`, `Area`, `Volume`, `What`.
- `.3dm`: NURBS curves and points are written; points are read.

## [0.3.0] — 2026-10-09

### Added
- Planar curve tools: `Offset`, `Trim`, `Extend`, `Fillet`, `FilletCorners`, `Join`, `Explode`.
- `Array`, `ArrayLinear`, `ArrayPolar`.
- Colours: `SetObjectColor` (by layer or custom), `LayerColor`, `LayerVisible`, `LayerLock`.
- **Gumball**: move along an axis or in a plane, rotate, typed values.
- Rhino-style menus with submenus, toolbar tabs, Properties panel (layer, colour, size), layer
  panel with visibility, lock and colour.
- `.3dm`: polycurves and object colours are written, object colours are read.

## [0.2.0] — 2026-10-09

### Added
- Modelling UI: four viewports (Top, Perspective, Front, Right) with per-view construction
  planes and grids, orbit / pan / zoom at the cursor, maximize.
- Interactive tools with prompts and rubber-band previews; object snaps End, Mid, Cen, Quad;
  grid snap; Ortho; click, window and crossing selection.
- Commands `Rectangle`, `Circle`, `Arc`, `Box`, `Cylinder`, `Sphere`, `Extrude`, `Move`,
  `Copy`, `Rotate`, `Scale`, `Mirror`, `Delete`, `SelAll`, `SelNone`, `Select`, `Layer`,
  `ChangeLayer`, `New`, `Save`; typed coordinates (absolute, `@` relative, polar, length
  constraint).
- `.3dm` writer: nested layers, lines, polylines, arcs, meshes, units.

## [0.1.0] — 2026-10-09

### Added
- First desktop build: a viewport that **opens Rhino `.3dm` files** (breps and extrusions
  triangulated for display, native meshes, curves as polylines, layer colours and visibility),
  layers panel, command line, status bar, open dialog, drag and drop.
- `forma-cli`: `info` (summary of a `.3dm` file), `render` (headless PNG), `run` (scripts).
- Foundations: layered Cargo workspace, command engine with Rhino-style coordinates, document
  with undo / redo, `cargo xtask ci` gate on Linux and Windows, openNURBS `.3dm` I/O
  (ADR 0002), geometry-kernel spike choosing OpenCascade (ADR 0001), Windows release workflow.

[Unreleased]: https://github.com/filippovisentin/forma/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/filippovisentin/forma/releases/tag/v0.6.0
[0.5.0]: https://github.com/filippovisentin/forma/releases/tag/v0.5.0
[0.4.0]: https://github.com/filippovisentin/forma/releases/tag/v0.4.0
[0.3.0]: https://github.com/filippovisentin/forma/releases/tag/v0.3.0
[0.2.0]: https://github.com/filippovisentin/forma/releases/tag/v0.2.0
[0.1.0]: https://github.com/filippovisentin/forma/releases/tag/v0.1.0
