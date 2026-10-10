# Contributing to Forma

Thanks for looking. Forma is a personal project: it is shaped around one person's
interior-design work, and most of the code is written by AI coding agents with
Filippo Antonio Visentin as architect and reviewer. That said, outside help is welcome.

## Issues

Issues are the most useful contribution.

- **Bugs**: use the [bug report](https://github.com/filippovisentin/forma/issues/new?template=bug_report.md)
  template. A small `.3dm` file and the exact command you typed make a bug easy to fix.
  Please do not attach files you are not allowed to share.
- **Features**: use the [feature request](https://github.com/filippovisentin/forma/issues/new?template=feature_request.md)
  template. Describe what you want to model and, if it exists, which Rhino command you would use.
  Scope is "the commands an interior designer actually uses", not full Rhino parity.
- Italian or English, both fine.

## Pull requests

Small, focused pull requests are welcome; for anything larger, open an issue first so we can
agree on the approach. The rules are the same for humans and agents and are written in
[CLAUDE.md](CLAUDE.md). In short:

1. **Everything is a command.** Modelling logic lives in `forma-engine`'s command registry,
   never in the UI, the MCP server or the apps.
2. **Respect the layering** described in [ARCHITECTURE.md](ARCHITECTURE.md).
3. **Every command ships with tests**, at least one that runs it through `engine.run_line("...")`
   and checks the resulting document. Geometry gets unit tests with explicit tolerances
   (`forma_geom::Tolerance`; never compare floats with `==`).
4. **No new dependency without an ADR** in [docs/adr](docs/adr) (context, options, decision).
5. **One roadmap item per pull request**, and update [ROADMAP.md](ROADMAP.md) when it is done.

### The CI gate

```sh
git submodule update --init --depth 1   # once, for openNURBS
cargo xtask ci                          # rustfmt check, clippy -D warnings, tests, layering
```

A change is not done while `cargo xtask ci` is red. GitHub Actions runs the same gate on Linux
and Windows.

### Clean-room rule

Rhino is a **behavioural reference only**: you may observe how Rhino behaves (command names,
prompts, results on test models). Do **not** copy McNeel code, documentation text, icons,
images or UI assets into Forma, and do not use Rhino's trademarks in a way that suggests
affiliation. openNURBS is used as a library under its own licence and is never copied into
Forma's sources. Forma is not affiliated with Robert McNeel & Associates.

## Licence of contributions

Unless you say otherwise, any contribution you submit is dual-licensed under MIT or
Apache-2.0, like the rest of Forma, without additional terms.
