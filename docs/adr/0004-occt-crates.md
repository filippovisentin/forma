# ADR 0004 — How OpenCascade is built and called: `occt-sys` + our own C ABI shim

- Status: accepted
- Date: 2026-10-10
- Follows: ADR 0001 (OpenCascade for solids, behind `forma-geom` feature `occt`)

## Context
ADR 0001 chose OpenCascade (OCCT) for solid modelling and suggested starting with the
`opencascade` crate (`opencascade` → `opencascade-sys` (cxx bindings) → `occt-sys`).
The first kernel milestone needs: sewing a triangle mesh into a solid, merging coplanar
faces (`ShapeUpgrade_UnifySameDomain`), booleans with fuzzy tolerance, fillets and
chamfers on *chosen* edges, shelling with removed faces, offsets, triangulation with
normals, volume and validity checks. The spike (S2) showed the `opencascade` crate covers
only part of this and wraps every call in its own types.

## Options
1. **`opencascade` crate** + C++ shim for what is missing: two binding layers (cxx and
   our C ABI), `cxx` and `glam 0.24` as extra dependencies, OCCT types reachable from Rust.
2. **`occt-sys` only + our own C ABI shim** (same pattern as openNURBS, ADR 0002): `occt-sys`
   just builds OCCT 7.8.1 as static libraries with CMake; `crates/geom/occt/shim.cpp`
   (compiled with `cc`) exposes ~20 `extern "C"` functions on opaque shape handles.
3. Link a system OCCT (apt / vcpkg): fast builds, but no static Windows binary without
   vcpkg gymnastics, and versions drift between machines.

## Decision
Option 2.

- New build dependencies of `forma-geom`, both optional and enabled only by feature `occt`:
  - `occt-sys = "=7.8.1"` (LGPL-2.1; ships OCCT 7.8.1 sources and a CMake recipe for a
    static build without visualisation, Tcl, FreeType…). Pinned exactly: the cached build
    key in CI names the version.
  - `cc = "1"` (already used by `forma-io-3dm`), compiles the shim.
- No `opencascade`, `opencascade-sys` or `cxx`. OCCT types never cross the C boundary;
  `forma-geom` exposes only functions on `Mesh`/`Point3`/`f64` (`solid_boolean`,
  `fillet_edges`, `shell_solid`, `offset_solid`, `solid_info`, `rebuild_solid`).
- `build.rs` honours `FORMA_OCCT_DIR`: if it contains `include/` and `lib/` of an OCCT
  build it is used as is; if it is set but empty, OCCT is built once and its `include/` and
  `lib/` are copied there (CI caches that directory); if unset, OCCT is built into
  `target/OCCT` by `occt-sys`.
- Without the feature every kernel function returns `KernelError::Unavailable` and the
  engine commands say "needs the solid kernel (build with --features occt)". The default
  `cargo xtask ci` therefore stays fast; `cargo xtask ci --occt` runs the same gate with the
  kernel. Release builds (`release.yml`) enable `forma/occt` and `forma-cli/occt`.
- OCCT is linked **statically** on every platform, so `forma.exe` needs no OCCT DLLs.

## Consequences
- First OCCT build: ~55 min on 2 cores (Linux), similar or longer on the Windows runner;
  cached afterwards (`.occt/`, ~300 MB, key `occt-7.8.1-static-<os>-v1`). Bumping OCCT means
  bumping the pin and the cache key together.
- Every missing OCCT call is a few lines of C++ in `shim.cpp` instead of a binding
  generator; exceptions are caught in the shim and returned as error strings.
- Binary size grows by the OCCT toolkits we use (TKernel … TKOffset); measured in CI.
- Licence: OCCT is LGPL-2.1 with the Open CASCADE exception; notice in `ATTRIBUTION.md`.
  We do not modify OCCT; anyone can rebuild Forma against another OCCT through
  `FORMA_OCCT_DIR`, which satisfies the relinking requirement for static linking.
