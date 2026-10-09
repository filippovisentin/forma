# Forma

A personal, Rhino-style NURBS modeller written in Rust — built for my own interior-design
workflow and as an experiment in building a real CAD tool with AI agents.

> Status: **v0.3.0** — Rhino-style modelling: four viewports with CPlanes, snaps, Ortho,
> curves (Line, Polyline, Rectangle, Circle, Arc), curve tools (Offset, Trim, Extend, Fillet,
> FilletCorners, Join, Explode), solids (Box, Cylinder, Sphere, ExtrudeCrv), transforms
> (Move, Copy, Rotate, Scale, Mirror, arrays) with a gumball, layers and object colours,
> Rhino-like menus and toolbar tabs, open/save `.3dm`. Download it from
> [Releases](https://github.com/filippovisentin/forma/releases). See [ROADMAP.md](ROADMAP.md).

## Try it

```sh
git submodule update --init --depth 1   # openNURBS, needed for .3dm I/O
cargo run -p forma-cli -- info model.3dm
cargo run -p forma-cli -- run --script "Polyline 0,0 600,0 600,400 0,400 c; Line 0,0 @0,0,720" --dump
cargo run -p forma -- model.3dm                       # desktop app
cargo run -p forma-cli -- render model.3dm view.png --view top   # headless screenshot
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
