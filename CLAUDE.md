# CLAUDE.md — rules for agents working on Forma

Forma is a personal, Rhino-style NURBS modeller written in Rust. It is built mostly by AI
agents, with Filippo as architect, reviewer and only user. Read `ARCHITECTURE.md` and
`ROADMAP.md` before starting any task.

## Ground rules

1. **Everything is a command.** Every user-facing action (menu item, toolbar button,
   command-line word, MCP call, CLI script line) goes through `forma-engine`'s command
   registry. Never put modelling logic in `forma-ui`, `forma-mcp` or the apps.
2. **Respect the layering.** A crate may only depend on crates in a *lower* layer
   (see `ARCHITECTURE.md`). `cargo xtask layering` enforces this. `forma-geom` knows
   nothing about documents; nothing below `forma-ui` knows about egui.
3. **CI must be green before you finish.** Run `cargo xtask ci` (fmt check, clippy with
   `-D warnings`, tests, layering). A task is not done while it is red.
4. **Every command ships with tests.** At minimum: one test that runs it through the
   command line string (`engine.run_line("...")`) and asserts on the resulting document.
   Geometry code gets unit tests with explicit tolerances.
5. **No new dependencies without an ADR.** Adding a crate to any `Cargo.toml` requires a
   short decision record in `docs/adr/NNNN-title.md` (context, options, decision).
   Small, well-known crates (serde, glam, thiserror) still need a one-paragraph ADR.
6. **Clean-room.** Rhino is the behavioural reference only: observe how it behaves
   (commands, prompts, results on test models). Never copy McNeel code, docs text, icons
   or UI assets. openNURBS is used as a library under its own licence, never copied into
   our sources.
7. **Tolerances are explicit.** Use `forma_geom::Tolerance` (absolute, default 0.001 model
   units; angle 1°). Never compare floats with `==`.
8. **Small, reviewable changes.** One milestone item per branch/PR. Update `ROADMAP.md`
   status when you finish an item.

## Conventions

- Rust edition 2021, `rustfmt.toml` at the root, `clippy::pedantic` is *not* required.
- Errors: `thiserror` in libraries (once its ADR lands), `anyhow` only in apps/xtask.
- Command names follow Rhino's spelling where one exists (`Line`, `Polyline`, `ExtrudeCrv`,
  `BooleanUnion`), case-insensitive, with short aliases (`L`, `PL`).
- Units: centimetres by default in the app and for `New` (Filippo works in cm); `Document::default()` stays mm for tests.
- Code, comments and docs in English. Conversations with Filippo may be in Italian.

## Setup

- After cloning: `git submodule update --init --depth 1` (openNURBS, see ADR 0002).
- A C++ toolchain and CMake are required; the first build of `forma-io-3dm` takes ~2 min.
- The solid kernel (OpenCascade) is behind feature `occt` (`cargo xtask ci --occt`,
  `cargo run -p forma --features occt`). Its first build takes ~1 h; set `FORMA_OCCT_DIR` to
  an existing OCCT build (`include/` + `lib/`) to reuse it (ADR 0004).
- Filippo's real models live in `tests/data/private/` (git-ignored). Tests that need them
  skip themselves when the files are missing; never commit them.
- `cargo run -p forma-cli -- info model.3dm` prints a summary of any `.3dm` file.

## How to verify your own work

- `cargo run -p forma-cli -- run --script "Line 0,0,0 100,0,0" --dump` prints the
  document so you can check results headlessly.
- Once `forma-render` has an offscreen path, add golden PNG tests under
  `tests/golden/` and compare against reference images.
