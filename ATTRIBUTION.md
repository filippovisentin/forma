# Third-party code and assets

Every vendored library, font, icon or asset is listed here with its source and licence.

| Item | Source | Licence | Used for |
|---|---|---|---|
| openNURBS (incl. bundled zlib, freetype 2.6.3, android_uuid) | git submodule `third_party/opennurbs`, https://github.com/mcneel/opennurbs | openNURBS licence (permissive, see `third_party/opennurbs/LICENSE`); zlib licence; FreeType licence | Reading and writing `.3dm` files (`forma-io-3dm`) |
| OpenCascade 7.8.1 (built from the sources shipped in the `occt-sys` crate, linked statically) | https://dev.opencascade.org, https://crates.io/crates/occt-sys | LGPL-2.1 with the Open CASCADE exception 1.0 (`packaging/licenses/OpenCascade-*.txt`) | Solid kernel of `forma-geom` (feature `occt`): booleans, fillets, chamfers, shells, offsets (ADR 0001, 0004); also the kernel spike (`spikes/kernel`) |

## OpenCascade notice

Forma's solid modelling (feature `occt`, on in release builds) makes use of and is based on
facilities provided by the **Open CASCADE Technology** software (https://dev.opencascade.org),
version 7.8.1, © OPEN CASCADE SAS, distributed under the GNU Lesser General Public License
version 2.1 with the Open CASCADE exception version 1.0. The full texts are in
`packaging/licenses/OpenCascade-LGPL-2.1.txt` and `packaging/licenses/OpenCascade-LGPL-exception.txt`
and ship in the Windows zip (`licenses/`).

OCCT is used unmodified and linked statically. To relink Forma against your own build of
OCCT 7.8, point `FORMA_OCCT_DIR` at its install directory (with `include/` and `lib/`) and
rebuild with `cargo build --release -p forma --features occt` (see docs/adr/0004-occt-crates.md).
