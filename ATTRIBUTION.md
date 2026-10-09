# Third-party code and assets

Every vendored library, font, icon or asset is listed here with its source and licence.

| Item | Source | Licence | Used for |
|---|---|---|---|
| openNURBS (incl. bundled zlib, freetype 2.6.3, android_uuid) | git submodule `third_party/opennurbs`, https://github.com/mcneel/opennurbs | openNURBS licence (permissive, see `third_party/opennurbs/LICENSE`); zlib licence; FreeType licence | Reading and writing `.3dm` files (`forma-io-3dm`) |
| OpenCascade 7.8 (via `occt-sys` / `opencascade` crates) | https://dev.opencascade.org, crates.io | LGPL-2.1 with Open CASCADE exception | Kernel spike only (`spikes/kernel`); planned for `forma-geom` feature `occt` (ADR 0001) |
