# woom24 — Architecture & Roadmap (living document)

**Status:** Active. This is the umbrella every feature spec hangs from; per-unit detail lives in
`docs/specs/2026-09-18-fN-*-design.md`. Update this file whenever a layer's contract changes.
**Decided:** 2026-09-18 (roadmap F1-F8 + layer map approved by the user; F8 visual design deferred
to a Visual Companion session).

## Mission recap

woom24 = a WASM Doom (1993) port in Rust. Hard rules that shape every layer below: deterministic
integer simulation pinned at 35 tics/s; rendering is presentation and never feeds back into
simulation; compatibility is selected by an explicit complevel; the shipped artifact is
self-contained; upstream (`sunsided/room`) merge friction is a cost. Full contract: `AGENTS.md`.

## Layer map

```
L0  Foundation (landed)         room ported engine, crt.rs seam, DG_* boundary,
                                AudioBackend control plane, SynthEngine, Presenter trait,
                                vanilla demo playback, c_tests differential culture
L1  Complevel behavior layer    F2 (extended by F3/F4)   doom/compat.rs query tables
L2  r_interp snapshot board     F1                       prev/curr tic snapshots, fraction
L3  video_cfg resolution/fov    F1                       W×H raster, aspect/fov modes
L4  deh mutation pipeline       F3 (reserved for F4)     typed DehMutation IR
L5  demo format layer           F2                       per-complevel DemoFormat tables
L6  audio fallback chain        F5                       SF2 → OPL2 → silence selector
L7  StorageHost persistence     F5                       native=fs, web=IndexedDB writeback
L8  NetTransport boundary       F6                       declaration only, no implementation
L9  settings registry           F7                       typed Setting descriptors
L10 ui toolkit (backend)        F8                       screen stack, widgets, primitives
```

Rule: engines layers live in `room/src/` (new modules, English comments, `//`-only markers).
Shell layers never own logic. Anything that would put a `match complevel` inside a ported `p_*` /
`g_*` file is a design violation — route it through the owning layer's query surface.

## Roadmap

| Unit | Spec | Delivers | Depends on | Status |
|---|---|---|---|---|
| F1 modern rendering | `2026-09-18-f1-modern-rendering-design.md` | uncapped FPS (full visible-motion interpolation), arbitrary internal raster resolution, 4:3 correction baseline, extended-frustum widescreen (configurable) | — | spec ready, next to implement |
| F2 complevel backbone | `2026-09-18-f2-complevel-backbone-design.md` | `Complevel` enum + detection + behavior/limit tables, demo format layer (vanilla/longtics/Boom/MBF), limit-removing | F1 (demo golden infrastructure) | spec ready |
| F3 MBF/DeHacked | `2026-09-18-f3-mbf-dehacked-design.md` | MBF behaviors, `deh` typed mutation pipeline (DSDHacked-ready), MBF21 | F2 | spec ready |
| F4 ID24 | `2026-09-18-f4-id24-design.md` | ID24 additive layer (the name's promise) | F3 | spec ready |
| F5 experience completion | `2026-09-18-f5-experience-design.md` | OPL2 music fallback, launcher options, web IndexedDB persistence (L7 writeback), console-logger polish | none (interleave anywhere) | spec ready |
| F6 multiplayer (optional) | `2026-09-18-f6-multiplayer-design.md` | `NetTransport` boundary declaration + lockstep contract; no implementation | all of the above settled | boundary-only spec |
| F7 in-game settings | `2026-09-18-f7-settings-design.md` | `settings` typed registry (L9), persistence via L7, bindings for vanilla menu + F8 UI | F1 (video settings exist), F2 (compat override) | spec ready |
| F8 fullscreen modern UI | `2026-09-18-f8-modern-ui-design.md` | `ui` toolkit backend (L10): screen stack, widget model, input routing, framebuffer primitives — Eternity-Engine-inspired; **visual design deferred** to a Visual Companion session | F7, F1 (presenters) | backend-only spec |

Iron rules (every spec inherits): (1) landing any unit must leave the full host suite AND the
vanilla demo bit-exact baseline green; (2) interpolation/resolution/widescreen/net never mutate
simulation state; (3) prefer central-table queries > boundary parameterization > in-place hooks
when touching ported code; (4) every unit ships a coverage-list test (snapshot census / behavior
table conformance / demo golden).

## Sequencing

F1 → F2 → F3 → F4 is the long march (F5 interleaves anywhere; F6 last, optional). Rationale:
F1 is self-contained and user-visible, and its demo-golden hardening doubles as the safety net
that certifies "interpolation did not touch the simulation" before compat work begins.

## Open follow-ups (from completed specs)

`woom24-libc` relocation out of `shells/web/` (layering); `wasm-bindgen-test` headless boot smoke;
DOM-surfacing `I_Error` hook (fold into F8); upstream PRs (doom lint fixes, per-arity CRT
wrappers, SynthEngine extraction); Chinese runtime string sweep before any public release.
