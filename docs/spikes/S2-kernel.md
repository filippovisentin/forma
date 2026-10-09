# Spike S2 — geometry kernel choice

**Goal:** choose how Forma does the hard operations (booleans, edge fillets, offsets)
before M2 builds on top of a kernel.

## Candidates

| Option | Pros | Cons |
|---|---|---|
| `curvo` + `truck` (pure Rust) | No C++ toolchain, wasm-friendly, great for learning | Booleans/fillets less robust, smaller community |
| OpenCascade via Rust bindings | Industrial-strength booleans, fillets, offsets, STEP | Heavy C++ build, LGPL, harder on Windows |
| Hybrid | Rust for curves/surfaces/display, OCCT only for booleans/fillets | Two representations to keep in sync |

## Test cases (interior design)

1. Box minus box (niche in a wall), with coplanar faces.
2. Cylinder minus box (cable hole in a table top).
3. Union of 5 boxes sharing faces (modular cabinet).
4. Fillet r=3 mm on all edges of a 600×400×18 panel.
5. Loft between a rectangle and a rounded rectangle, then boolean with a box.
6. Offset of a closed planar curve with arcs (skirting / profile).

## Tasks

1. Implement each case behind a small trait in a throwaway crate `spikes/kernel`.
2. Measure: success/failure, watertightness of the result, time, build complexity on Windows.
3. Export results as STL and inspect them.

## Output

ADR `docs/adr/0001-geometry-kernel.md` with the table filled in and the decision.
