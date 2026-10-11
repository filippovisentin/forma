# Changelog

All notable changes to Forma. Versions are the
[GitHub releases](https://github.com/filippovisentin/forma/releases); each ships
`Forma-windows.zip` (`forma.exe`, `forma-cli.exe`, `LEGGIMI.txt`, licences).
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.2.1] — 2026-10-11

### Fixed
- Numbers and points reject NaN and infinity (`Move 0,0,0 nan,0,0` was accepted and
  could corrupt the model and the saved file).
- Ask: answers from a model server are capped at 32 MB.
- `RunPythonScript <file.py>` typed on the command line waits for the path (a space
  no longer runs it before the path is typed).

## [1.2.0] — 2026-10-11

### Added
- **Python scripts inside the app** ([docs/SCRIPTING.md](docs/SCRIPTING.md), ADR 0008):
  `RunPythonScript`, the `forma` module (`run`, `objects`, `selection`, `select`,
  `units`), every `.py` in the scripts folder is a command named after it (Tools ▸
  Scripts), one Undo per script, `forma-cli python`, and the MCP tool `forma_python`
  (in the open app too, over the agent connection). No Python installation needed.
- **Ask: AI writes commands.** Describe a command in words (`Ask`, the violet ✦
  button first in the Standard toolbar, or Tools ▸ Ask AI): a local model (Ollama,
  LM Studio, any OpenAI-style server, set by URL under *Model*) writes it as a Python
  script. Check it, press Run (one Undo reverts it), *Ask to fix* sends the error back,
  *Save* puts it in the scripts folder as a new command.
- **Free-form surfaces are exact NURBS**: Sweep1, Sweep2, EdgeSrf (Coons patch whose
  edges are the curves), SrfPt and Pipe no longer make meshes. Sweeps along polylines
  are mitred at the corners (a picture frame from a rectangle and a profile) and
  joined into one polysurface; pipes are capped closed solids.
- **Free-form curves stay exact** through Split, Trim, Join, Offset, Fillet, Extend and
  Explode: pieces are cut from the original NURBS (knot insertion) at the exact
  crossings, joins are exact (lines and arcs become part of one NURBS curve; Explode
  cuts it again at its corners), offsets are smooth NURBS curves within tolerance, and
  fillets touch the curve exactly. CurveBoolean (union, intersection, difference) cuts
  the curves at their exact crossings and joins the right pieces, so round and free-form
  edges stay exact (straight-only results are still polylines). Before, these tools
  turned NURBS into polylines.
- **Command area like Rhino**: drag its edge to resize it (a grip shows where;
  double-click it for compact / tall), dock it at the top or bottom, or float it as a
  window you can move and resize (View ▸ Command Area, or `CommandArea
  Top|Bottom|Float|Compact|Tall`). Remembered between sessions.
- **Application icon**: an original golden free-form surface with its control points,
  in the title bar, the taskbar and on `forma.exe` itself (Explorer, Desktop shortcut,
  Start menu). ADR 0009.
- **Icons for every command**: menus show each command's icon, and the commands that
  had none (about 110: dimensions, meshes, edges, CPlanes, blocks, hidden views…) got
  one. Original drawings in the style of classic CAD toolbars.

## [1.1.0] — 2026-10-11

### Added
- **Autosave and crash recovery.** While a model has unsaved changes, a copy is
  written every 2 minutes to `autosave.3dm` in the settings folder (in the
  background, the open model is not touched). After a crash or power cut the app
  offers to recover it at the next start; Save, Open, New and a normal exit delete it.
- **Two interface looks**: View ▸ Interface ▸ Classic (the original light look) or
  Graphite (new, dark chrome with a gold accent), or type `Interface Classic` /
  `Interface Graphite`. The choice is kept between runs. Viewports keep their grey
  background in both looks.
- Crashes are written to `crash.log` in the settings folder (the Windows app has no
  console), so they can be reported.

- **New model dialog**: at start-up (and for File ▸ New, Ctrl+N or a bare `New`)
  Forma asks for the units of the new model: millimetres, centimetres, metres,
  inches or feet. The last choice is preselected; "Ask at every start" can be turned
  off. `New` accepts `in` and `ft` too.

- **AI assistants can work in Forma** (MCP server, [docs/AGENTS.md](docs/AGENTS.md)):
  `forma-cli mcp` gives Claude Desktop, Claude Code and other MCP clients four tools:
  run command lines, inspect the model, look up commands, render a view. With
  `--live` they edit the model open in the app after Tools ▸ Agent Connection
  (off by default, this computer only, protected by a random key); without it they
  work on `.3dm` files, optionally limited to one folder (`--root`).

- **Exact booleans and fillets on curved solids** (roadmap spike S3): `BooleanUnion`,
  `BooleanDifference`, `BooleanIntersection`, `BooleanSplit`, `FilletEdge` and
  `ChamferEdge` on polysurfaces now work on the exact NURBS faces and give exact
  polysurfaces back: a round hole through a box or a wall, a box minus a sphere,
  crossing cylinders, rolling-ball fillets on box edges and cylinder rims. They save to
  `.3dm` as real Rhino polysurfaces. `Shell` and `OffsetSrf` stay exact as well (a
  hollowed cylinder, a grown sphere): the kernel recognises planes, cylinders, cones,
  spheres and tori in the NURBS faces. Meshes still go through triangles as before.

- The Graphite look now has dark viewports too: dark background and grid, light edges,
  and black or near-black objects (and their text and points) drawn light grey, like
  Rhino's dark mode. Classic keeps the grey viewports.
- Graphite layout (Classic stays as it was): the file name with a dot when it has
  unsaved changes, the save state ("Autosaved 2 min ago") and the units at the right
  of the menu bar; command options as clickable chips; object snaps as toggle pills;
  tool previews and the cursor drawn light on the dark viewports.

### Fixed
- The command history no longer goes blank after a few commands (it scrolled past its
  last line).
- `FilletEdge` / `ChamferEdge` no longer pick the seam of a cylinder or sphere (it cannot
  be filleted) when a pick point is near both the seam and a real edge.
- `Mesh` keeps the sharp edges of the polysurface (split at more than 30°, smooth
  across gentler bends): `DupEdge` finds the edges of a meshed box and `Explode`
  gives its faces (it used to weld everything into one smooth piece).
- `Cap` no longer "closes" a flat mesh (a disc, one face of a box) by doubling it
  into a zero-volume solid; flat meshes have nothing to cap.
- Recent files and other settings changed by the app are written to the settings
  file (they were sometimes only kept in memory until another setting changed).
- Saving never damages the previous file: `.3dm` files are written under a
  temporary name and swapped in only when complete (a crash or a full disk during
  Save used to leave a half-written model).
- A damaged `.3dm` with mesh faces pointing at missing vertices no longer crashes
  Forma: those faces are skipped on import.

## [1.0.0] — 2026-10-11

First official release: exact solids and surfaces, tested in the app.

### Added
- Exact `Loft` (smooth NURBS through the curves, or `Loft straight` with ruled faces),
  `ExtrudeCrvAlongCrv`, `ExtrudeCrvToPoint`.
- `Untrim`, `ShrinkTrimmedSrf`, `CreateSolid`, `ExtractIsocurve`, `Check`.
- Curved surfaces show isocurves (wireframe views too); Cen and Quad snaps on round
  edges of polysurfaces.
- Menus: Surface → Extract Isocurve / Untrim / Shrink; Solid → Create Solid, Merge
  Coplanar Faces; Mesh → Mesh from Surfaces / Mesh to Polysurface; Analyze → Check;
  Edit → Select → Surfaces / Polysurfaces / Meshes.

### Fixed (found by testing the app)
- Crash when moving the mouse after starting `Cone` or `TCone`.
- A command typed with its arguments now runs exactly like a script line (optional
  arguments too); the interactive tool starts only when something is missing.
- Fast typing: the last characters before Enter are no longer lost.
- At a "select objects" prompt, SelAll / SelLast / Select work; other commands cancel
  the tool and run.
- Clicking in a shaded view picks the solid in front, not curves hidden behind it.
- BooleanDifference / BooleanSplit act on two preselected solids.
- `ExtrudeCrvAlongCrv` keeps the preselected profile and asks for the path.
- The command history follows new output; selection is cleared on Open; zooming in
  close no longer hides the model; no dark spots at the poles of round surfaces;
  edges on faces are no longer dashed; `What` prints clean numbers.

## [0.9.0] — 2026-10-11

### Added
- **Exact solids and surfaces** ([ADR 0006](docs/adr/0006-exact-breps.md)). Rhino
  polysurfaces, extrusions and surfaces open as they are and are saved back unchanged,
  also after Move, Rotate, Scale (non-uniform too), Mirror. Curves open exactly (lines,
  arcs, polycurves, NURBS) instead of polylines.
- Exact creation: `Box`, `Cylinder`, `Sphere`, `Cone`, `TCone`, `Torus`, `Ellipsoid`,
  `Tube`, `Pyramid`, `Slab`, `Plane`, `Extrude` (walls with holes; open curves become
  surfaces), `PlanarSrf`, `ExtrudeSrf`, `Revolve`, `Cap` (round holes too), `BoundingBox`.
- Exact editing: `PushPull` / `MoveFace`, `Join` (joins surfaces at their edges into
  polysurfaces and solids), `Explode` (polysurface → faces), `Flip`, control points of
  flat-faced solids (F10), booleans / `Shell` / `ChamferEdge` of flat-faced solids.
- `DupEdge`, `DupBorder`, `DupFaceBorder` give the exact edge curves.
- New commands: `Mesh`, `MeshToNURB`, `MergeAllCoplanarFaces`, `MeshBox`, `MeshSphere`,
  `SelSrf`, `SelPolysrf`.
- Snapping to the corners and edge midpoints of polysurfaces.

### Changed
- Curved faces are displayed with more triangles where they bend (about one per 10°),
  and neighbouring faces share their edge points (no cracks).
- `SelMesh` selects meshes only.

### Known limits
- Loft, Sweep1/Sweep2, FilletEdge, OffsetSrf and booleans with curved faces still give
  meshes; control points of curved surfaces are not editable yet.

## [0.8.1] — 2026-10-10

### Added
- Properties panel: editable name, size and position (Box Edit), text, and circle radius.
- `HatchSolid` (solid fill) and crossed hatch; `MeshRepair`.

## [0.8.0] — 2026-10-10

### Added
- **Section cut**: View → Section (Clipping Plane) — hide everything above a height (or
  beyond a plane) in every view; `ClippingPlane <point> <up|down|north…>`, `ClippingPlane Off`.
- **Capture View to Image** (File menu, `ViewCaptureToFile`): PNG of the active view with texts
  and dimensions.
- **Named views** (View → Named Views) and **layer states** (Layers panel → States).
- Curves: `CircleTTR` (circle tangent to two curves), `ArcDirection`, `BlendCrv` (tangency or
  curvature), `SimplifyCrv`, `Smooth`, `Silhouette`, Line from midpoint / vertical /
  perpendicular from a curve / angle bisector; `Fillet` now works between arcs, circles,
  polylines and lines.
- Surfaces and meshes: `Sweep2`, `ReduceMesh` (fewer triangles on heavy imported meshes).
- Annotation: `DimRotated`. Document: `Units` (File → Units…, with or without scaling).

### Changed
- **Faster commands on big models**: only the changed part of the scene is rebuilt after a
  command (about 30 times less work on a 950-object model).
- `Extrude` on curves inside one another makes one solid with holes (walls from their two
  outlines, like Rhino).
- OBJ export writes a `.mtl` with layer colours, normals, and Y up (Blender, D5 Render,
  SketchUp…); STL export is in millimetres for slicers.
- Typing a command with only some of its values (`BooleanDifference #3`) starts it with them;
  spaces typed in a text prompt stay in the text.

## [0.7.2] — 2026-10-10

### Added
- Dialog windows instead of typed prompts: File → Import and Export Selected (file dialogs;
  Export writes .3dm, .obj or .stl by file type), Array, Box Edit, Annotation Style, select by
  layer / colour / name / size, copy to layer, one layer on / off, rename / delete layer,
  object name.
- Layer panel right click: Show Only This Layer, New Sub-layer, Rename…, Delete….

### Fixed
- Saved `.3dm` files keep groups, hidden and locked objects; texts are real Rhino text objects
  (dots stay dots, dimensions are lines + text in a group); all of these come back on Open.
- Rhino files: hidden and locked objects and groups are read; texts and dots are shown.
- Layer names with spaces can be renamed (`RenameLayer "old name" "new name"`).

## [0.7.1] — 2026-10-10

### Added
- `Make2D [top|front|right|back|left|bottom] [hidden]`: hidden-line plans and elevations of
  meshes and curves (Dimension menu).
- Window and taskbar icon.
- `Crea collegamento sul Desktop.bat` in the zip: Desktop and Start-menu shortcut.

## [0.7.0] — 2026-10-10

### Added
- **Solid kernel (OpenCascade)** in the Windows build: `BooleanUnion`, `BooleanDifference`,
  `BooleanIntersection`, `BooleanSplit`, `FilletEdge`, `ChamferEdge`, `Shell`, `OffsetSrf` on
  closed meshes (Solid menu and toolbar). Object-first tools take a single preselected object.
- **Control points** (F10 / F11, Edit → Control Points): click, window-select and drag curve
  control points and mesh vertices with snaps, grid and Ortho (`MoveGrips`, undoable).
- **Custom construction planes** (View → Set CPlane): `CPlane World`, origin, 3 points, on a
  face of a solid; each view has its own, with its grid. Typed coordinates are CPlane coordinates.
- **Blocks**: Rhino block instances now open in place, one named group per instance (block
  definitions are no longer shown at their own position); `Block`, `Insert`, `ExplodeBlock`.
- Rhino commands: `Lines`, `Stretch`, `SelPrev`, `ShowSelected`, `UnlockSelected`, `ClosestPt`,
  `List`, `DupFaceBorder`, `UnifyMeshNormals`, `SelBoundary`, `EditText`, `IncrementalSave`,
  views Bottom, Back, Left; Zoom Window (`ZW`).
- `Export` writes `.obj` (meshes and curves) and binary `.stl` besides `.3dm`.
- [docs/commands.md](docs/commands.md): 279 Rhino commands compared one by one.
- Command line: commands with only optional arguments (`New [cm|mm|m]`) wait for Enter.
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
- Display modes reduced to Wireframe and Shaded.
- A typed bare `Save` asks for a new name like Ctrl+S, so an original Rhino file is never
  overwritten.
- Window selection starts exactly where the mouse button went down.
- Much faster interaction on large files: incremental snap / pick index with screen-space
  culling (hover on a 950-object file from 33–48 ms to about 5 ms per frame, click pick from
  16 ms to 0.2 ms), no continuous repaint while a tool is idle.
- `forma-ui` split into modules (no behaviour change).

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

[Unreleased]: https://github.com/filippovisentin/forma/compare/v1.2.1...HEAD
[1.2.1]: https://github.com/filippovisentin/forma/releases/tag/v1.2.1
[1.2.0]: https://github.com/filippovisentin/forma/releases/tag/v1.2.0
[0.6.0]: https://github.com/filippovisentin/forma/releases/tag/v0.6.0
[0.5.0]: https://github.com/filippovisentin/forma/releases/tag/v0.5.0
[0.4.0]: https://github.com/filippovisentin/forma/releases/tag/v0.4.0
[0.3.0]: https://github.com/filippovisentin/forma/releases/tag/v0.3.0
[0.2.0]: https://github.com/filippovisentin/forma/releases/tag/v0.2.0
[0.1.0]: https://github.com/filippovisentin/forma/releases/tag/v0.1.0
