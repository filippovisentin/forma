# ADR 0007: Migrate from ttf-parser to skrifa for font parsing

## Context

The `ttf-parser` crate (v0.25.1) is used transitively via `ab_glyph` → `owned_ttf_parser` → `ttf-parser` for font rendering in the UI (via egui/winit).

**Security finding**: `cargo audit` reports RUSTSEC-2026-0192 — `ttf-parser` is unmaintained. The author has stated it will not receive further fixes.

**Current dependency chain**:
```
forma-ui
  └─ eframe
       └─ egui-winit
            └─ winit
                 └─ sctk-adwaita (optional, for Wayland)
                      └─ ab_glyph
                           └─ owned_ttf_parser
                                └─ ttf-parser (unmaintained)
```

## Alternatives Considered

1. **Stay on ttf-parser** — Accept the risk; no known exploitable vulnerabilities, just unmaintained.
2. **Fork ttf-parser** — Maintain a private fork with security patches.
3. **Migrate to skrifa** — Google Fonts' actively maintained font parsing library (part of the "oxidize"/fontations project).
4. **Wait for upstream migration** — egui/winit/ab_glyph ecosystem to migrate first.

## Decision

**Track upstream migration to skrifa** (Option 4) with periodic re-evaluation.

Rationale:
- The vulnerability is "unmaintained" not "exploitable" — no CVEs reported.
- Migration requires ecosystem changes (ab_glyph, egui, winit) beyond our control.
- `skrifa` is the designated successor but adoption in the Rust GUI ecosystem is still in progress.
- Forking adds maintenance burden for a transitive dependency.

## Action Plan

- [ ] Monitor `ab_glyph` for skrifa support (issue: https://github.com/linebender/ab_glyph/issues)
- [ ] Monitor `egui` font system updates
- [ ] Re-evaluate at each major egui upgrade
- [ ] If no progress in 12 months, consider Option 2 (fork) or direct skrifa integration

## Status

**Deferred** — Waiting for ecosystem migration. Tracked in cargo-deny with `ignore = ["RUSTSEC-2026-0192"]`.

## References

- RUSTSEC-2026-0192: https://rustsec.org/advisories/RUSTSEC-2026-0192
- ttf-parser unmaintained announcement: https://github.com/harfbuzz/ttf-parser/issues/217
- skrifa crate: https://crates.io/crates/skrifa
- fontations project: https://github.com/googlefonts/fontations