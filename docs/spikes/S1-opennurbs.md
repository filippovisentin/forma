# Spike S1 — openNURBS for `.3dm` I/O

**Goal:** prove we can read and write `.3dm` files from Rust through openNURBS, on
Windows (MSVC) and Linux.

## Tasks

1. Vendor openNURBS as a git submodule under `third_party/opennurbs` (do not copy
   sources into our crates). Record the licence in `ATTRIBUTION.md`.
2. Build it from `forma-io-3dm/build.rs` with the `cc` crate (or `cmake` crate if simpler).
3. Expose a minimal C++ shim (`shim.cpp`) with: open file, count objects, iterate objects
   returning type + layer name + bounding box, write a file containing one line curve.
   Bind it with `cxx`.
4. Rust API: `forma_io_3dm::read_summary(path) -> Summary`, `write_line(path, a, b)`.
5. Test on Filippo's sample files in `tests/data/` (not committed if private; use a
   `.gitignore`d folder and a synthetic file for CI).

## Success criteria

- `cargo test -p forma-io-3dm` passes on Windows and Linux.
- A file written by Forma opens in Rhino with the line on the correct layer.
- Build time impact documented.

## Output

ADR `docs/adr/0002-3dm-io.md` with the decision and any gotchas.
