# woom24 — Architecture & Roadmap (living document)

**Status:** Active. This is the umbrella every feature hangs from; the design decisions of all
landed units are distilled here (working specs are local-only, git-ignored under `docs/specs/`).
Update this file whenever a layer's contract changes.
**Decided:** 2026-09-18 (roadmap approved by the user; F8 visual design deferred to a Visual
Companion session). 2026-09-26: working specs/plans retired from the repository - they are
local-only under `docs/specs/` (git-ignored); this document carries the distilled decisions.

## Mission recap

woom24 = a WASM Doom (1993) port in Rust. Hard rules that shape every layer below: deterministic
integer simulation pinned at 35 tics/s; rendering is presentation and never feeds back into
simulation; compatibility is selected by an explicit complevel; the shipped artifact is
self-contained; upstream (`sunsided/room`) behavior fixes arrive as cherry-picks ported through
graduated modules' name-mapping tables. Full contract: `AGENTS.md`.

## Layer map

```
L0  Foundation (landed)         room ported engine, crt.rs seam, DG_* boundary,
                                AudioBackend control plane, SynthEngine, Presenter trait,
                                vanilla demo playback, c_tests differential culture
L1  Complevel behavior layer    F2 (extended by F3/F4)   doom/compat.rs query tables
L2  r_interp snapshot board     F1                       prev/curr tic snapshots, fraction
L3  video_cfg resolution/fov    F1                       W×H raster, aspect/fov modes
L4  deh mutation pipeline       F3 (reserved for F4)     typed DehMutation IR
L5  demo format layer           F2 (extended by F3/F4)   per-complevel DemoFormat tables
L6  audio fallback chain        F5                       SF2 → OPL2 → silence selector
L7  StorageHost persistence     F5                       native=fs, web=IndexedDB writeback
L8  NetTransport boundary       F6                       declaration only, no implementation
L9  settings registry           F7                       typed Setting descriptors
L10 ui toolkit (backend)        F8                       screen stack, widgets, primitives
```

Rule: engines layers live in `room/src/` (new modules, English comments, `//`-only markers).
Shell layers never own logic. Anything that would put a `match complevel` inside a ported `p_*` /
`g_*` file is a design violation — route it through the owning layer's query surface.

Freeze zone (F10): ported `room/src/doom/` files stay frozen (no renames, splits, reorders) until they
graduate — graduation is the only exit and adopts the module-directory anatomy with an extracted `dtmc`
demo-synchronization surface and boundary shims re-exporting upstream names. Landed pilots: `m_fixed`
(dtmc extraction) and `w_wad` (mechanics split); rolling graduation per the F10 process (local spec
`docs/specs/F10-foundation-reform.md`).

## Roadmap

| Unit | Spec | Delivers | Depends on | Status |
|---|---|---|---|---|
| F1 modern rendering | local spec | uncapped FPS (full visible-motion interpolation), arbitrary internal raster resolution, 4:3 correction baseline, extended-frustum widescreen (configurable) | — | M1+M2 landed (r_interp + frame/pump split; `video_cfg` + arbitrary raster); M3 (widescreen) next |
| F2 complevel backbone | local spec | `Complevel` enum + detection + behavior/limit tables, demo format layer (vanilla/longtics/Boom/MBF), limit-removing | F1 (demo golden infrastructure) | spec ready |
| F3 MBF/DeHacked | local spec | MBF behaviors, `deh` typed mutation pipeline (DSDHacked-ready), MBF21 | F2 | spec ready |
| F4 ID24 | local spec | ID24 additive layer (the name's promise) | F3 | spec ready |
| F5 experience completion | local spec | OPL2 music fallback, launcher options, web IndexedDB persistence (L7 writeback), console-logger polish | none (interleave anywhere) | spec ready |
| F6 multiplayer (optional) | local spec | `NetTransport` boundary declaration + lockstep contract; no implementation | all of the above settled | boundary-only spec |
| F7 in-game settings | local spec | `settings` typed registry (L9), persistence via L7, bindings for vanilla menu + F8 UI | F1 (video settings exist), F2 (compat override), F5 (L7 storage, settings blob) | spec ready |
| F8 fullscreen modern UI | local spec | `ui` toolkit backend (L10): screen stack, widget model, input routing, framebuffer primitives — Eternity-Engine-inspired; **visual design deferred** to a Visual Companion session | F7, F1 (presenters) | backend-only spec |
| F9 e2e infra | local spec | engine-framebuffer pixel goldens (wasm `harness` exports + Node scenario runner), host/wasm cross-target hash gate, I_Error overlay + audio-health counters, CI (GitHub Actions), CI-only Freedoom live-corpus soak (non-blocking) | F1 (frame/pump driver), spec ② (shell exports) | M1-M3 landed (goldens, twin, CI); M4 (browser smoke, playwright approved) / M5 (soak, Freedoom vendored) next |
| F10 foundation reform | local spec | freeze-zone + graduation process; per-module `dtmc` extraction with pinned doc templates; module-directory anatomy (`mod.rs` = docs + wiring only); m_fixed-pattern naming (plain-English internal names, upstream-name root shims, `#[export_name]` C-symbol pins) | F9 (golden gates as the graduation safety net) | pilots landed (m_fixed, w_wad); rolling graduation: 28/76 modules graduated (through p_saveg; sim core p_*/d_items fully done), all reviews gated, goldens unmoved; continues per the F10 process (local spec §6) |

Iron rules (every spec inherits): (1) landing any unit must leave the full host suite AND the
vanilla demo bit-exact baseline green; (2) interpolation/resolution/widescreen/net never mutate
simulation state; (3) prefer central-table queries > boundary parameterization > in-place hooks
when touching ported code; (4) every unit ships a coverage-list test (snapshot census / behavior
table conformance / demo golden).

## Sequencing

F1 → F2 → F3 → F4 is the long march (F5 interleaves anywhere; F6 last, optional). Rationale:
F1 is self-contained and user-visible, and its demo-golden hardening doubles as the safety net
that certifies "interpolation did not touch the simulation" before compat work begins.

## Vanilla violations

Every shipped emulation of a vanilla DOOM bug or memory violation is cataloged in
`docs/vanilla-workarounds.md` -- binding per `AGENTS.md` (Documentation Standards): adding or
changing a workaround requires updating that catalog in the same change. Each cataloged hook
records its trigger into the diagnostics census `room/src/doom/violations.rs`
(`VanillaViolation` counters -- pure diagnostics, never read by simulation); tests and audits
read `violations::hits(...)` to prove which workarounds a given input exercises. Status: ten
completed entries; the tmbbox trample family needs no emulation beyond the spechit model (G1,
pinned), and the browser ticdup=0 freeze is fixed by the spechit store bound, with the root-cause
audit resolved pending a human browser re-test (G2).

## Open follow-ups (from completed specs)

`woom24-libc` relocation out of `shells/web/` (layering); upstream PRs (doom lint fixes, per-arity CRT
wrappers, SynthEngine extraction); Chinese runtime string sweep before any public release. Closed by F9
(2026-09-26): headless boot smoke (covered by the Node scenario runner + host twin; real-browser smoke
is F9 M4) and the in-framebuffer error screen hook (landed as the shell DOM I_Error overlay, spec-4 E).

`woom24-diag` in-browser forensics (spec-4 defect C): strip `auto_diag_tick`/`woom24_diag_frame` from
the production wasm when defect C closes (browser forensics conclude) — owner: defect-C closure,
tracker: this line.

## Pending audit (2026-09-19, user directive)

**Multi-platform abstraction audit** — after the spec refresh completes: sweep the core for
WASM-specific leakage (cfg gates, wasm-shape assumptions in shared code) and platform couplings that
need abstraction, per the expanded platform roadmap (WASM first; desktop 3 as shipping shells;
Android = control wrapper + WebView over the WASM build). Deliverable: an audit doc + required
boundary changes. Do not start before the spec batch is closed.
