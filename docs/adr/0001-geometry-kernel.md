# ADR 0001 — Geometry kernel: OpenCascade for solids, our own types at the boundary

- Status: accepted
- Date: 2026-10-09
- Evidence: `docs/spikes/S2-results.md`

## Context
Forma needs booleans, edge fillets, offsets and lofts that work on everyday interior
design geometry: walls with niches, table tops with holes, cabinets made of modules that
share faces. Those cases are full of coplanar and touching faces, the classic weak spot of
young kernels.

## Options
1. **truck + curvo** (pure Rust): no C++ toolchain, wasm-friendly, small binaries.
2. **OpenCascade (OCCT)**: industrial kernel, C++, LGPL-2.1.
3. Hybrid: Rust for curves/display, OCCT only for solids.

## Decision
**Option 3, with OCCT doing all solid modelling.**

- Solids (booleans, fillets, chamfers, shells, lofts, sweeps, offsets of faces) go through
  OCCT. In S2 truck failed 4 of 6 cases (all coplanar ones, no fillet), OCCT passed 6 of 6.
- Plain curve work (polylines, arcs, 2D offsets, snapping math) stays in Rust in
  `forma-geom`; curvo is good enough there and is a candidate for 2D profile operations.
- **OCCT never leaks out of `forma-geom`.** The rest of Forma sees only our own types
  (`Brep`, `Curve`, `Mesh` handles). This keeps the option to swap kernels later.
- Start with the `opencascade` crate (`opencascade-sys` + `occt-sys`, static OCCT build).
  Where it lacks an API (mass properties, selective fillets, healing, conversions), add
  functions to a thin C++ shim in `forma-geom`, as already done for openNURBS (ADR 0002).

## Consequences
- **Build cost:** the first OCCT build takes ~55 minutes on 2 cores. Therefore OCCT sits
  behind a cargo feature `occt` on `forma-geom` (on for the desktop app). Default CI runs
  without it; a separate CI job builds with `occt` and caches the OCCT build.
- **Licence:** OCCT is LGPL-2.1 with an additional exception. Forma is open source
  (MIT/Apache), so users can always rebuild against a modified OCCT; we must keep OCCT's
  licence text in `ATTRIBUTION.md` and not modify OCCT without publishing the changes.
- **Rhino interoperability:** `.3dm` breps (openNURBS `ON_Brep`) and OCCT `TopoDS_Shape`
  are both NURBS-based but differ in topology conventions. A converter ON_Brep ↔ TopoDS is
  needed before booleans can run on imported Rhino geometry; this is new work for M4/M5
  and gets its own spike (S3) before M4.
- **Wasm/web build:** OCCT makes a browser build of the solid modeller hard. Accepted:
  Forma is a desktop tool for one user.
