| kernel | case | result | watertight | volume | vs expected | mesh | time | notes |
|---|---|---|---|---|---|---|---|---|
| occt | 1-niche | ok | yes | 3060000000 | +0.0000% | 28 tris | 4 ms | 11 face(s) |
| occt | 2-cable-hole | ok | yes | 38250381 | +0.0031% | 132 tris | 5 ms | 7 face(s) |
| occt | 3-cabinet-union | ok | yes | 1252800000 | +0.0000% | 12 tris | 14 ms | 6 face(s) after clean, 22 before (ideal: 6) |
| occt | 4-fillet-panel | ok | yes | 4311931 | -0.0056% | 628 tris | 13 ms | 26 face(s) |
| occt | 5-loft-cut | ok | yes | 45295537 | n/a | 1538 tris | 37 ms | 6 face(s) (smooth loft) |
| occt | 6-offset-profile | ok | – | area 277393.8 | -0.0000% | – | 2 ms | profile area 237854.0 (exact 237854.0) |
