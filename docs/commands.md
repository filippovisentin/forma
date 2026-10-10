# Rhino command parity

Which of Rhino's commonly used commands Forma has. Rhino is the behavioural
reference only: names are listed to compare, nothing is copied from its
documentation. Statuses: **✓** works like the Rhino command for everyday use;
**partial** exists with the limits noted; **✗** not yet, possible without a
solid kernel; **needs kernel** needs NURBS surfaces from OpenCascade (milestone M4; booleans, fillets and shells already use it on closed meshes).

Surfaces and solids are display meshes until the kernel lands, so surface
commands that produce them are *partial* by definition. Forma command syntax
is in `Help` and `forma-cli commands`.

## Summary

279 Rhino commands: **168 ✓**, **55 partial**, **38 ✗**, **18 needs kernel**.

## Drawing: points and lines

| Rhino command | Forma | Notes |
|---|---|---|
| Point | ✓ | `Point` (alias Pt) |
| Points | ✓ | `Points` — several points in one go |
| Line | partial | `Line` single segment; Bisector / Perpendicular / Tangent / FromMidpoint options not yet |
| Lines | ✓ | `Lines p1 p2 …` separate segments |
| Polyline | ✓ | `Polyline` (PL), Close / Undo options |
| Curve | ✓ | `Curve` — degree-3 control-point curve |
| InterpCrv | ✓ | `InterpCrv` — curve through points |
| Sketch | ✗ |  |
| Helix | ✓ | `Helix <axis start> <axis end> <radius> <turns>` |
| Spiral | ✓ | `Spiral <center> <r0> <r1> <turns> [height]` |
| Conic | ✗ |  |
| TextObject | ✗ | text as curves / solids: needs font outlines |

## Drawing: shapes

| Rhino command | Forma | Notes |
|---|---|---|
| Rectangle (Corner) | ✓ | `Rectangle` (Rec) |
| Rectangle 3Point | ✓ | `Rectangle3Pt` |
| Rectangle Center | ✓ | `RectangleCenter` |
| Rectangle Rounded | ✓ | `RoundedRectangle <c1> <c2> <radius>` |
| Circle (Center, Radius) | ✓ | `Circle` (C) |
| Circle 2Point | ✓ | `Circle2Pt` (alias CircleD) |
| Circle 3Point | ✓ | `Circle3Pt` |
| Circle Tangent | ✗ |  |
| Arc (Center, Start, Angle) | ✓ | `Arc` |
| Arc Start End Point | ✓ | `Arc3Pt` |
| Arc Tangent / Direction | ✗ |  |
| Ellipse | ✓ | `Ellipse` (center, axis, axis) |
| Polygon | ✓ | `Polygon` with NumSides |
| Slot | ✓ | `Slot <c1> <c2> <width>` |

## Curve editing

| Rhino command | Forma | Notes |
|---|---|---|
| Offset | ✓ | `Offset` planar curves (arcs kept exact) |
| OffsetCrvOnSrf | needs kernel |  |
| Fillet | partial | `Fillet` between two lines / polyline segments; arcs and NURBS not yet |
| FilletCorners | ✓ | `FilletCorners` (FC) |
| Chamfer | partial | `Chamfer` between two lines |
| BlendCrv | ✗ |  |
| MatchCrv | ✗ |  |
| Extend | ✓ | `Extend` to boundary objects |
| Trim | ✓ | `Trim` curves with cutting objects |
| Split | ✓ | `Split` curves at cutting objects |
| Join | ✓ | `Join` curves → polycurves, meshes → one mesh |
| Explode | ✓ | `Explode` curves → segments, meshes → faces |
| Rebuild | ✓ | `Rebuild <points> [degree]` (curves; NURBS surfaces need the kernel) |
| Fair | ✗ |  |
| Smooth | ✗ |  |
| SimplifyCrv | ✗ |  |
| Convert | ✓ | `Convert [tolerance]` → polylines |
| CloseCrv | ✓ | `CloseCrv` |
| Dir / Flip | ✓ | `Flip` (alias Dir) |
| Seam / CrvSeam | ✗ |  |
| CurveBoolean | partial | `CurveBoolean union|intersection|difference [delete]`; no region picking, result as polylines |
| PointsOn / EditPtOn | ✓ | F10 / F11: click, window-select and drag control points and mesh vertices (snaps, grid, Ortho); `MoveGrips` |
| InsertKnot / RemoveKnot | ✗ |  |

## Curves from objects

| Rhino command | Forma | Notes |
|---|---|---|
| Divide | ✓ | `Divide <segments>` |
| Divide Length | ✓ | `DivideByLength <length>` |
| Contour | partial | `Contour <base> <dir> <spacing>` on meshes (and points on curves) |
| Section | partial | `Section <start> <end>` on meshes |
| Project | partial | `Project [direction]` onto meshes, first surface met; polyline result |
| Pull | partial | `Pull` onto meshes (closest point); polyline result |
| DupEdge | partial | `DupEdge` on meshes (visible edges) |
| DupBorder | partial | `DupBorder` on meshes |
| DupFaceBorder | partial | `DupFaceBorder #id <point>` flat faces of meshes |
| Intersect | partial | `Intersect` curve–curve points only |
| ExtractIsocurve | needs kernel |  |
| ExtractPt | ✓ | `ExtractPt` |
| ProjectToCPlane | ✓ | `ProjectToCPlane` |
| Silhouette | ✗ |  |
| Make2D | partial | `Make2D [top|front|right|back|left|bottom] [hidden]`: parallel views, meshes and curves; no perspective |

## Surfaces

| Rhino command | Forma | Notes |
|---|---|---|
| PlanarSrf | partial | `PlanarSrf` with holes — display mesh |
| Plane | partial | `Plane <c1> <c2>` — mesh |
| SrfPt | partial | `SrfPt` 3 or 4 corners — mesh |
| EdgeSrf | partial | `EdgeSrf` 2–4 curves (Coons patch) — mesh |
| Loft | partial | `Loft` — mesh, no style options |
| Revolve | partial | `Revolve` — mesh |
| RailRevolve | ✗ |  |
| Sweep1 | partial | `Sweep1` — mesh, rotation-minimising frames |
| Sweep2 | ✗ |  |
| NetworkSrf | needs kernel |  |
| Patch | needs kernel |  |
| ExtrudeCrv | partial | `Extrude` / `ExtrudeCrv` — closed curves give closed meshes |
| ExtrudeCrvAlongCrv | partial | `ExtrudeCrvAlongCrv #path` — mesh |
| ExtrudeCrvTapered | partial | `ExtrudeCrvTapered <distance> <draft°>` — mesh |
| ExtrudeCrvToPoint | partial | `ExtrudeCrvToPoint <apex>` — mesh, closed planar curves capped |
| ExtrudeSrf | partial | `ExtrudeSrf` — mesh |
| Pipe | partial | `Pipe <radius> [open]` — mesh |
| OffsetSrf | partial | `OffsetSrf <distance>` on closed meshes (OpenCascade), mesh result |
| FilletSrf | needs kernel |  |
| BlendSrf | needs kernel |  |
| MatchSrf | needs kernel |  |
| ExtendSrf | needs kernel |  |
| Rebuild (surfaces) | needs kernel |  |
| Split / Trim (surfaces) | needs kernel |  |
| ShrinkTrimmedSrf | needs kernel |  |
| Untrim | needs kernel |  |
| ExtractSrf | partial | mesh faces via `Explode`; face picking with Ctrl+Shift+click |

## Solids

| Rhino command | Forma | Notes |
|---|---|---|
| Box | ✓ | `Box` |
| Sphere | ✓ | `Sphere` |
| Cylinder | ✓ | `Cylinder` |
| Cone | ✓ | `Cone` |
| TCone | ✓ | `TCone` |
| Pyramid | ✓ | `Pyramid` |
| Ellipsoid | ✓ | `Ellipsoid` |
| Torus | ✓ | `Torus` |
| Tube | ✓ | `Tube` |
| Slab | ✓ | `Slab <thickness> <height> [center|left|right]` |
| Cap | ✓ | `Cap` planar holes |
| MoveFace | partial | `MoveFace` flat faces of mesh solids |
| PushPull | partial | `PushPull` flat faces of mesh solids |
| BooleanUnion | partial | `BooleanUnion` on closed meshes (OpenCascade), mesh result |
| BooleanDifference | partial | `BooleanDifference #a #b …` closed meshes, mesh result |
| BooleanIntersection | partial | closed meshes, mesh result |
| BooleanSplit | partial | closed meshes, mesh result |
| FilletEdge | partial | `FilletEdge #id <radius> <points near edges>` closed meshes |
| ChamferEdge | partial | `ChamferEdge #id <distance> <points near edges>` closed meshes |
| Shell | partial | `Shell #id <thickness> <points on removed faces>` closed meshes |
| MergeAllFaces | needs kernel |  |
| CreateSolid | needs kernel |  |
| Wirecut | needs kernel |  |

## Mesh

| Rhino command | Forma | Notes |
|---|---|---|
| Mesh (from NURBS) | partial | every surface / solid is a display mesh already |
| MeshToNURB | needs kernel |  |
| Weld | ✓ | `Weld [angle]` |
| Unweld | ✓ | `Unweld [angle]` |
| ExplodeMesh | ✓ | `Explode` |
| Join (meshes) | ✓ | `Join` |
| MeshBooleanUnion / Difference | ✓ | the Boolean commands work on closed meshes |
| ReduceMesh | ✗ |  |
| QuadRemesh | ✗ |  |
| FillMeshHoles | partial | `Cap` fills planar holes only |
| UnifyMeshNormals | ✓ | `UnifyMeshNormals` |
| MeshRepair | ✗ |  |

## Transform

| Rhino command | Forma | Notes |
|---|---|---|
| Move | ✓ | `Move` (M) + gumball |
| Copy | ✓ | `Copy` (Co) |
| Rotate | ✓ | `Rotate` (Ro) |
| Rotate3D | ✓ | `Rotate3D` |
| Scale | ✓ | `Scale` |
| Scale1D | ✓ | `Scale1D` |
| Scale2D | ✓ | `Scale2D` |
| ScaleNU | ✓ | `ScaleNU` |
| Mirror | ✓ | `Mirror` (line in the CPlane) |
| Mirror 3Point | ✓ | `Mirror3Pt` |
| Orient (2 points) | ✓ | `Orient` |
| Orient3Pt | ✓ | `Orient3Pt` (3 reference → 3 target points) |
| OrientOnSrf | ✗ |  |
| Array | ✓ | `Array nx ny nz dx,dy,dz` |
| ArrayLinear | ✓ | `ArrayLinear` |
| ArrayPolar | ✓ | `ArrayPolar` |
| ArrayCrv | ✓ | `ArrayCrv #path <count> [norotate]` |
| ArraySrf | needs kernel |  |
| Align | ✓ | `Align left|right|top|bottom|hcenter|vcenter|center` |
| Distribute | ✓ | `Distribute x|y|z [gap]` |
| Shear | ✓ | `Shear` |
| Bend | partial | `Bend <start> <end> <through>`: circular bend; meshes are subdivided |
| Twist | partial | `Twist`: meshes are subdivided |
| Taper | partial | `Taper`: meshes are subdivided |
| Flow | ✗ |  |
| Splop | ✗ |  |
| Stretch | ✓ | `Stretch <corner> <corner> <from> <to>` (window on the CPlane, through all heights) |
| Cage / CageEdit | ✗ |  |
| BoxEdit | partial | `BoxEdit x= y= z= [uniform] [center] [at=]` on the command line, no panel |
| SetPt | ✓ | `SetPt <point> [x] [y] [z]` |
| Gumball | ✓ | move / rotate / extrude handles; scale handles not yet |
| Flip | ✓ | `Flip` |

## Edit and objects

| Rhino command | Forma | Notes |
|---|---|---|
| Undo / Redo | ✓ |  |
| Delete | ✓ |  |
| CopyToClipboard / Cut / Paste | ✓ | internal clipboard |
| Properties | ✓ | Properties panel (F3) |
| MatchProperties | ✓ | `MatchProperties` |
| SetObjectName (object name) | ✓ | `SetObjectName`, saved to .3dm |
| Group / Ungroup | ✓ |  |
| SelGroup | ✓ |  |
| Block | partial | `Block <name>`: a named group; Rhino block instances open expanded, one group each (not written back as blocks) |
| Insert | partial | `Insert <name> <point> [scale] [angle]` copies a block group |
| BlockEdit | ✗ |  |
| ExplodeBlock | ✓ | `ExplodeBlock` (= `Ungroup`) |
| History (Record History) | ✗ |  |

## Selection

| Rhino command | Forma | Notes |
|---|---|---|
| SelAll | ✓ |  |
| SelNone | ✓ |  |
| Invert | ✓ |  |
| SelLast | ✓ |  |
| SelPrev | ✓ | `SelPrev` |
| SelCrv | ✓ |  |
| SelPt | ✓ |  |
| SelMesh | ✓ |  |
| SelSrf | partial | `SelOpenMesh` (surfaces are meshes) |
| SelPolysrf / SelClosedPolysrf | partial | `SelClosedMesh` (alias SelClosedPolysrf) |
| SelOpenCrv | ✓ |  |
| SelClosedCrv | ✓ |  |
| SelPolyline | ✓ |  |
| SelLine | ✓ |  |
| SelDup | ✓ |  |
| SelSmall | ✓ | `SelSmall <size>` |
| SelColor | ✓ | `SelColor <colour>` |
| SelLayer | ✓ | `SelLayer <layer>` |
| SelName | ✓ | `SelName <name>` (* wildcards) |
| SelText | ✓ |  |
| SelDim | ✓ |  |
| SelVisible | ✓ |  |
| SelBoundary | ✓ | `SelBoundary #curve` (objects inside a closed planar curve) |
| SelID | ✓ | `Select #id …` |
| Window / crossing selection | ✓ | mouse drag →/← |

## Visibility

| Rhino command | Forma | Notes |
|---|---|---|
| Hide | ✓ |  |
| Show | ✓ |  |
| ShowSelected | ✓ | `ShowSelected [#id …]` shows and selects hidden objects |
| HideSwap | ✓ |  |
| Isolate | ✓ |  |
| Unisolate | ✓ | alias of `Show` (shows every hidden object) |
| Lock | ✓ |  |
| Unlock | ✓ |  |
| UnlockSelected | ✓ | `UnlockSelected [#id …]` |
| LockSwap | ✓ |  |

## Layers

| Rhino command | Forma | Notes |
|---|---|---|
| Layer (panel, current layer) | ✓ | Layers panel, `Layer <name>` |
| ChangeLayer | ✓ | `ChangeLayer <name>` |
| ChangeToCurrentLayer | ✓ |  |
| CopyObjectsToLayer | ✓ |  |
| OneLayerOn | ✓ |  |
| OneLayerOff | ✓ |  |
| AllLayersOn | ✓ |  |
| Layer rename | ✓ | `RenameLayer <old> <new>` (sub-layers follow) |
| Layer delete | ✓ | `DeleteLayer <layer>` (with its objects) |
| Layer colour / lock / visibility | ✓ | panel, `LayerColor`, `LayerLock`, `LayerVisible` |
| Purge | partial | `Purge` removes empty layers (no blocks / materials yet) |
| LayerStateManager | ✗ |  |

## Analysis

| Rhino command | Forma | Notes |
|---|---|---|
| Distance | ✓ |  |
| Length | ✓ |  |
| Angle | ✓ | `Angle <vertex> <p1> <p2>` |
| Radius | ✓ |  |
| EvaluatePt | ✓ |  |
| Area | ✓ |  |
| AreaCentroid | ✓ |  |
| Volume | ✓ | closed meshes |
| VolumeCentroid | ✓ |  |
| What | ✓ |  |
| BoundingBox | ✓ |  |
| CurvatureGraph | ✗ |  |
| Curvature | ✗ |  |
| ShowEdges | ✗ |  |
| Zebra | needs kernel |  |
| DraftAngleAnalysis | ✗ |  |
| ClosestPt | ✓ | `ClosestPt <point>` adds points on the selected objects |
| CrvDeviation | ✗ |  |
| List / Check | partial | `List` (= `What`); no validity check |

## Dimensions and annotation

| Rhino command | Forma | Notes |
|---|---|---|
| Dim / DimLinear | ✓ | `Dim` (alias DimLinear): horizontal or vertical from the line point |
| DimAligned | ✓ |  |
| DimRotated | ✗ |  |
| DimRadius | ✓ |  |
| DimDiameter | ✓ |  |
| DimAngle | ✓ |  |
| DimOrdinate | ✗ |  |
| DimCurveLength | ✗ |  |
| Leader | ✓ | `Leader <p1> <p2> … <text>` |
| Text | partial | `Text <origin> <height|*> [normal] <text>`: one line, UI font; saved to .3dm as a text dot |
| TextDot | ✓ | `Dot` (alias TextDot) |
| Hatch | partial | `Hatch <spacing> [angle]`: grouped lines, no hatch object / patterns |
| AnnotationStyles / DimStyle | partial | `DimStyle [height] [decimals]` per document |
| EditText | ✓ | `EditText #id <text>` (texts, dots, dimension overrides) |
| Layout / Detail / Print | ✗ |  |

## View and display

| Rhino command | Forma | Notes |
|---|---|---|
| Zoom Extents (ZE / ZEA) | ✓ |  |
| Zoom Selected (ZS) | ✓ |  |
| Zoom Window | ✓ | `ZW`, then drag a rectangle |
| Pan / Rotate / Zoom (mouse) | ✓ |  |
| SetView Top / Front / Right / Perspective | ✓ |  |
| SetView Bottom / Left / Back | ✓ | `Bottom`, `Left`, `Back` |
| 4 viewports, MaxViewport | ✓ |  |
| NamedView | ✗ |  |
| CPlane (custom) | ✓ | `CPlane World / <origin> / 3 points / Face #id <point>` per view, with its grid |
| Grid | ✓ | F7 |
| Wireframe / Shaded / Ghosted / X-Ray | ✓ |  |
| Rendered / Arctic / Technical / Pen | ✗ |  |
| ClippingPlane | ✗ |  |
| Ortho / Snap / Planar | ✓ | F8 / F9 |
| Osnap | ✓ | End, Near, Point, Mid, Cen, Int, Perp, Tan, Quad, Knot, Vertex |
| SmartTrack | ✓ |  |
| ViewCaptureToFile | partial | `forma-cli render` (offscreen PNG) |

## File

| Rhino command | Forma | Notes |
|---|---|---|
| New | ✓ | `New [mm|cm|m]` |
| Open | ✓ | .3dm (display geometry) |
| Save / SaveAs | ✓ | .3dm: curves exact, surfaces / solids as meshes, annotations simplified |
| Import | ✓ | .3dm |
| Export (selected) | partial | .3dm, .obj, .stl; glTF to come |
| Recent files | ✓ |  |
| Units / DocumentProperties | partial | units from `New`; no dialog |
| IncrementalSave | ✓ | `IncrementalSave` → model_001.3dm, model_002.3dm… |
| Worksession | ✗ |  |
| Exit | ✓ |  |
