# Forma

A personal, Rhino-style NURBS modeller written in Rust — built for my own interior-design
workflow and as an experiment in building a real CAD tool with AI agents.

> Status: **M0 — skeleton**. Headless command engine, document with undo/redo, CLI.
> No window yet. See [ROADMAP.md](ROADMAP.md).

## Try it

```sh
cargo run -p forma-cli -- run --script "Polyline 0,0 600,0 600,400 0,400 c; Line 0,0 @0,0,720" --dump
cargo run -p forma            # interactive command line (Help, List, Exit)
cargo xtask ci                # the gate every change must pass
```

Coordinates follow Rhino conventions: `x,y`, `x,y,z`, relative `@dx,dy[,dz]`, polar `@d<angle`.

## Design

- **Everything is a command** — UI, CLI, scripts and agents (MCP) all go through the same engine.
- **Layered crates** — geometry → document → I/O & render → engine → UI/MCP → apps, enforced by `cargo xtask layering`.
- **Clean-room** — Rhino is a behavioural reference only. Not affiliated with Robert McNeel & Associates;
  Rhino and Rhinoceros are their trademarks.

Read [ARCHITECTURE.md](ARCHITECTURE.md) and, if you are an agent, [CLAUDE.md](CLAUDE.md).

## License

MIT OR Apache-2.0.
