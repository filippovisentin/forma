# Forma

A personal, Rhino-style NURBS modeller written in Rust — built for my own interior-design
workflow and as an experiment in building a real CAD tool with AI agents.

> Status: **v0.4.0** — a Rhino 8-like interface (light theme, command line on top, toolbar
> tabs, viewport menus and display modes, 11 object snaps, gumball) with about 90 commands:
> curves (incl. NURBS Curve/InterpCrv/Ellipse), curve tools (Offset, Trim, Split, Extend, Fillet,
> Chamfer, Join, Explode), mesh surfaces (PlanarSrf, Loft, Revolve, Sweep1, ExtrudeSrf, Cap),
> solids (Box, Cylinder, Sphere, ExtrudeCrv), transforms (incl. Scale1D/2D, Orient, Align,
> arrays), visibility, groups, layers and colours, analysis, open/save/import/export `.3dm`.
> Download it from [Releases](https://github.com/filippovisentin/forma/releases).
> See [ROADMAP.md](ROADMAP.md).

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
