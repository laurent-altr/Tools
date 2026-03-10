# inp2rad — Rust edition

A compact Abaqus `.inp` → OpenRadioss `.rad` converter written in Rust.

## Motivation

The Python converter (`../inp2rad/`) is feature-complete and battle-tested,
but spans ~7 500 lines in a single file.  This Rust port covers the most
commonly used keywords in under **1 000 lines** across four focused source
files, with zero external dependencies.

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | ~60 | CLI entry-point |
| `src/model.rs` | ~120 | Data structures |
| `src/parse.rs` | ~400 | `.inp` parser |
| `src/write.rs` | ~380 | Radioss writer |

## Building

```bash
cargo build --release
# binary at: target/release/inp2rad
```

## Usage

```bash
inp2rad model.inp
```

Produces:
- `model_0000.rad` — Radioss starter deck
- `model_0001.rad` — Radioss engine deck (template; review before running)

## Supported keywords

### Geometry
| `.inp` keyword | Radioss output |
|---|---|
| `*NODE` | `/NODE` |
| `*NSET` (+ `GENERATE`) | `/GRNOD/NODE` |
| `*ELSET` (+ `GENERATE`) | stored for property lookup |
| `*INCLUDE` | inlined (recursive) |

### Elements
| `.inp` type(s) | Radioss block |
|---|---|
| S3, S3R, M3D3, R3D3 | `/SH3N` |
| S4, S4R, R3D4, M3D4R | `/SHELL` |
| C3D6, SC6R | `/PENTA6` |
| COH3D6, COH3D8 | `/BRICK` + `/PROP/TYPE43` |
| C3D4 | `/TETRA` |
| C3D8, C3D8R, C3D8I, SC8R | `/BRICK` |
| C3D10, C3D10M | `/TETRA10` |
| SPRINGA, CONN3D2 | `/SPRING` |
| MASS | `/SPRING` |

### Sections → Properties + Parts
| `.inp` keyword | Radioss output |
|---|---|
| `*SHELL SECTION` | `/PROP/SHELL` + `/PART` |
| `*MEMBRANE SECTION` | `/PROP/SHELL` (Iplas=0) + `/PART` |
| `*SOLID SECTION` | `/PROP/SOLID` + `/PART` |
| `*COHESIVE SECTION` | `/PROP/TYPE43` + `/PART` |

### Materials
| `.inp` definition | Radioss law |
|---|---|
| `*ELASTIC` only | `/MAT/ELAST` |
| `*ELASTIC` + `*PLASTIC` | `/MAT/PLAS_TAB` + inline `/FUNCT` |
| `*HYPERELASTIC, NEO HOOKE` | `/MAT/LAW42` |

### Loads and BCs
| `.inp` keyword | Radioss output |
|---|---|
| `*BOUNDARY` (zero value) | `/BCS` |
| `*BOUNDARY` (non-zero / with amplitude) | `/IMPDISP` |
| Named BCs: ENCASTRE, PINNED, XSYMM, YSYMM, ZSYMM | translated to DOF masks |
| `*AMPLITUDE` | `/FUNCT` |

### Contacts and Constraints
| `.inp` keyword | Radioss output |
|---|---|
| `*SURFACE` | `/SURF/PART` or `/GRNOD/NODE` |
| `*CONTACT PAIR` | `/INTER/TYPE7` |
| `*FRICTION` | friction coefficient on interface |
| `*TIE` | `/INTER/TYPE2` |

### Engine control
`*DYNAMIC, EXPLICIT` time-step and run-time are read to populate the
engine deck (`/DT`, `/RUN`, `/ANIM/DT`, `/TH/DT`).

## Relationship to the Python converter

The Python `inp2rad.py` remains the **reference implementation** and supports
many additional features (hyperfoam, viscoelastic Prony series, rigid bodies,
distributing couplings, MPC ties, …).  This Rust port is intended as a
faster, dependency-free alternative for models that use the supported subset
above.

## License

MIT — Copyright 1986-2026 Altair Engineering Inc.
