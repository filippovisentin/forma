# ADR 0009 — Application icon in forma.exe (`winresource`)

- Status: accepted
- Date: 2026-10-11

## Context
Filippo wants Forma to have its own icon: in the title bar and taskbar, and for
`forma.exe` itself in Explorer, on the Desktop shortcut and in the Start menu. The
window icon can be set at run time through eframe; the icon of the executable file has
to be a Windows resource linked into it.

## Options
1. **Ship `forma.ico` next to the exe** and point the shortcut at it: no dependency, but
   the exe keeps the generic icon in Explorer, the taskbar pin and Alt+Tab before the
   window has opened.
2. **Our own build script calling `rc.exe`**: no crate, but finding the Windows SDK's
   resource compiler (versioned folders, MSVC vs GNU) is exactly what existing crates
   already handle.
3. **`winresource`** (maintained fork of `winres`, MIT): a build-dependency that writes
   the `.rc` file, finds the resource compiler and links the result. Windows only.

## Decision
Option 3, as a `[target.'cfg(windows)'.build-dependencies]` of `apps/forma` only: Linux
and macOS builds do not fetch or compile it. When the resource compiler is missing, the
build prints a warning and goes on without the icon rather than failing.

The artwork is original (a golden free-form NURBS surface with its control polygon on a
graphite tile), drawn by `packaging/icon/make_icon.py` (Pillow, run by hand) into
`packaging/icon/forma.ico` (16–256 px) and `forma-256.png`; the window icon decodes the
PNG with the `png` crate (ADR 0005).

## Consequences
- `forma.exe` shows the Forma icon everywhere in Windows.
- Changing the icon: edit and run `make_icon.py`, commit both outputs.
