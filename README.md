# woom24

**woom24 = W(ASM)oom(ID)24** — a WebAssembly port of DOOM (1993) written in Rust, with the
ID24 specification as its long-range compatibility target.

| Fragment | Meaning |
|---|---|
| **W** | **WASM** — the primary shipping target is a browser, via `wasm32-unknown-unknown`. |
| **oom** | **(D)oom** — engine work: demo-exact simulation, Boom/MBF-family compatibility. |
| **24** | **ID24** — the compatibility ceiling the project is named after; the roadmap ends there. |

woom24 is a fork of [sunsided/room](https://github.com/sunsided/room) — a complete, bit-exact
Rust port of [doomgeneric](https://github.com/ozkl/doomgeneric) — restructured for multi-platform
deployment: the engine is a pure library crate and every platform is a thin shell over it.

## Status

- **Engine**: full Vanilla DOOM simulation in Rust (demo playback included), with the original
  C-vs-Rust differential-test culture intact.
- **Shells**: native desktop (winit + wgpu + rodio) and web (`wasm32-unknown-unknown` +
  wasm-bindgen, WebGL2/Canvas2D presenters, Web Audio, self-contained static deployment).
- **Modern rendering** (in progress): uncapped FPS with full visible-motion interpolation,
  arbitrary internal raster resolution, widescreen frustum.
- **Compatibility roadmap**: Vanilla → Limit-Removing → Boom → MBF → MBF21 → ID24
  (spec-first; see `docs/DESIGN.md`).

The deterministic simulation is load-bearing: demo playback must be bit-exact, and render-side
features (interpolation, resolution, widescreen) are structurally forbidden from touching
simulation state.

## Repository layout

```
woom24/
├── room/               Engine library: the ported DOOM modules (game, render, audio core),
│                       the CRT compatibility seam, and the differential-test suites.
├── shells/
│   ├── native/         Native platform shell: winit window, wgpu presenter,
│   │                   rodio audio backend, DG_* callbacks.
│   └── web/            WASM shell: wasm-bindgen exports, CRT/VFS shim, WebGL2/Canvas2D
│                       presenters, Web Audio backend, static deployment in www/.
├── doomgeneric-sys/    Build glue for the vendored C reference (test oracle only).
├── c2rust-intermediate/ c2rust transpilation reference (nightly-only, not built by default).
├── docs/               Architecture (DESIGN.md), design specs, audit artifacts, plans.
├── reference/          Local read-only clones of reference source ports (never committed).
└── AGENTS.md           The binding contract for humans and AI agents working on this repo.
```

## Getting started

Prerequisites: a recent stable Rust toolchain.

### Native (development shell)

```sh
cargo run -p room-shell-native --bin room -- -iwad doom1.wad
```

A `doom1.wad` (shareware) or commercial IWAD must be supplied by you — WAD files are
copyrighted and never distributed with this repository.

### Web (WASM)

```sh
# Build the self-contained static artifact (PowerShell; a bash twin exists).
powershell -NoProfile -ExecutionPolicy Bypass -File shells/web/scripts/build-www.ps1

# Serve locally (any static file server works).
python -m http.server 8000 --directory shells/web/www
```

Then open <http://localhost:8000/>, pick your IWAD (and optionally PWADs / a SoundFont) in the
launcher, and start. The browser smoke checklist lives in `shells/web/README.md`.

Toolchain notes: the wasm target (`rustup target add wasm32-unknown-unknown`) and the
wasm-bindgen CLI (version-locked to the `wasm-bindgen` crate; the build script verifies and
prints the exact install command on mismatch).

## Testing

```sh
cargo test                                                        # host suites
cargo test -p room --test demo_playthrough                        # golden demo determinism
cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown
```

- **Golden demo tests** are the project's core regression mechanism: recorded inputs must
  produce bit-identical simulation state, and render-side features must never perturb them.
- A jittered-clock determinism harness drives the frame/pump split at deliberately irregular
  cadences and compares full simulation state against the exact-cadence run.
- `c_tests/` contains the differential C-vs-Rust suites (LP64 platforms; see
  `docs/audit-c-long.md`).

## Documentation

- `AGENTS.md` — the binding engineering contract (constraints, style, workflows).
- `docs/DESIGN.md` — the architecture umbrella: layer map, roadmap, and design decisions.
- `docs/vanilla-workarounds.md` — the binding catalog of emulated vanilla DOOM bugs and
  memory violations (root cause, emulation site, semantics, reference derivation).
- `docs/e2e/` — the end-to-end verification story: golden-flow fixtures provenance and the
  mutation-demo gate evidence.
- Working design specs and implementation plans are **local-only** (git-ignored under
  `docs/specs/`); the repository carries only the distilled, reader-facing documents above.

## License & provenance

The Doom engine sources are GPL-2.0; woom24 inherits **GPL-2.0** (see `LICENSE-GPL-2.0`).
Credits and gratitude to:

- [id Software](https://github.com/id-Software/DOOM) for DOOM and the original sources;
- [sunsided/room](https://github.com/sunsided/room) — the Rust port this project forks;
- [doomgeneric](https://github.com/ozkl/doomgeneric) — the portability seam woom24's
  platform boundary is modeled on;
- the Chocolate Doom, Crispy Doom, PrBoom+/dsda-doom, MBF and Woof! communities —
  the compatibility references documented in `docs/`.

WAD files (IWADs and PWADs) are copyrighted works: never commit, bundle, or redistribute them.
