# Spike S2 — results (2026-10-09)

Code: `spikes/kernel/` (standalone workspace, not part of the product or CI).
Run: `cd spikes/kernel && cargo run --release -p kernel-truck` / `cargo run -p kernel-occt`.
Each case writes an STL to `spikes/kernel/out/` for visual inspection.

Measurements: the result is tessellated (tolerance 0.5 mm), welded, then checked for
watertightness (every edge shared by exactly two oppositely oriented triangles) and its
volume compared with the exact analytic value. Small volume errors (< 0.01 %) come from
tessellating curved faces, not from the kernel.

## truck 0.6 + curvo 0.3 (pure Rust)

| case | result | watertight | volume vs exact | time | notes |
|---|---|---|---|---|---|
| 1 niche in wall (coplanar faces) | **FAILED** | – | – | 1 ms | boolean returns `None` |
| 1b same, cutter overlapping (no coplanar faces) | ok | **no** (4 open edges) | +0.0000 % | 1 ms | diagnostic |
| 2 cable hole Ø80 in table top | ok | yes | +0.0042 % | 11 s | slow |
| 3 five cabinet modules, union | **FAILED** | – | – | 1 ms | fails at module 2 (shared faces) |
| 3b two modules overlapping 1 mm | **FAILED** | – | – | 1 ms | still coplanar top/bottom/back |
| 4 fillet r3 on panel edges | not available | – | – | – | no solid fillet in truck |
| 5 square→round loft, then cut | **FAILED** | – | – | 2.7 s | ruled loft ok, boolean fails |
| 6 offset of rounded profile (curvo) | ok | – | area −0.0000 % | 21 ms | exact |

## OpenCascade 7.8 via `opencascade` 0.3

| case | result | watertight | volume vs exact | time | notes |
|---|---|---|---|---|---|
| 1 niche in wall (coplanar faces) | ok | yes | +0.0000 % | 4 ms | 11 faces |
| 2 cable hole Ø80 in table top | ok | yes | +0.0031 % | 5 ms | 7 faces |
| 3 five cabinet modules, union | ok | yes | +0.0000 % | 14 ms | 22 faces → **6 after `clean()`** (unify coplanar faces) |
| 4 fillet r3 on panel edges | ok | yes | −0.0056 % | 13 ms | 26 faces, matches the analytic rounded box |
| 5 square→round loft, then cut | ok | yes | n/a | 37 ms | smooth loft (ThruSections) |
| 6 offset of rounded profile | ok | – | area −0.0000 % | 2 ms | exact |

## Costs

| | truck + curvo | OpenCascade |
|---|---|---|
| Language | Rust | C++ (via `cxx` bindings) |
| Licence | Apache-2.0 / MIT | LGPL-2.1 (with OCCT exception) |
| First build | ~2 min | **~55 min on 2 cores** (cached afterwards) |
| Binary size of the spike | 2.6 MB (release) | 36 MB (debug) |
| Rust API coverage | complete for what exists | partial wrapper; missing pieces need our own C++ shim, like openNURBS |

## Verdict

truck fails exactly where interior design lives: solids that touch or share faces (walls,
niches, cabinets, shelves), and has no fillets. OpenCascade passes every case, fast and
watertight. Decision recorded in ADR 0001.
