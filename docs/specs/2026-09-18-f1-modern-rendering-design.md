# Design: F1 — Modern Rendering (uncapped FPS, arbitrary resolution, widescreen)

**Date:** 2026-09-18
**Status:** Approved (decisions locked with the user 2026-09-18; implementation plan deferred)
**Series:** F1 of F1-F8 — see `docs/DESIGN.md` (umbrella). Owns layers **L2 `r_interp`** and **L3 `video_cfg`**.
**Inherits:** specs ①②③ (platform shell, wasm shell, c_tests/LP64); AGENTS.md constraints 3+4.

## Problem

The engine renders exactly one frame per 35 Hz simulation tic: motion visibly steps at tick rate,
the framebuffer is a fixed 640×400 doomgeneric buffer, and widescreen displays show a stretched
4:3 picture with no aspect correction. AGENTS.md constraints 3+4 demand: uncapped FPS with
interpolation, resolution-independent rendering, vanilla 4:3 correction as the correctness
baseline, widescreen/custom aspect as render-side-only extension.

## Goals

1. **Uncapped FPS, full visible-motion interpolation (prboom model):** rendering runs at display
   rate; every visible per-tic motion (mobs, player camera, weapon sprite, sector movers) is
   interpolated between the last two tics. Simulation stays pinned at 35 tics/s and bit-identical.
2. **Arbitrary internal raster resolution:** the software raster renders at a configurable
   W×H (not only 320×200/640×400 doublings), reconfigurable without restarting the engine.
3. **Aspect semantics:** vanilla 4:3 correction (320×200 on non-square pixels) is the default
   correctness baseline; widescreen = horizontally extended view frustum rendering more world
   columns, user-configurable; no letterbox in v1 (config toggles come with F7's settings UI).
4. **Shell-agnostic:** both shells drive the same frame/pump split; the Presenter trait is
   unchanged (still consumes one W×H BGRA frame).

## Non-goals

- Any change to simulation state, RNG, or demo playback bytes (the red line — enforced by tests).
- Wide status-bar/HUD assets (v1 keeps the vanilla centered status bar; wide HUD art is follow-up).
- Texture filtering/HD assets (nearest-neighbor aesthetic stays).
- Interpolated scrolling floor/ceiling textures and thing-frame interpolation (v1 covers positions
  and heights; scroll offsets are a documented follow-up — prboom parity item).
- UI/launcher work (F7/F8 consume this layer via `VideoConfig`).

## Locked decisions (user, 2026-09-18)

| # | Decision |
|---|---|
| D1 | Interpolation covers **all visible per-tic motion** — prboom model; implemented as a centralized post-tic snapshot walker, not per-mover hooks. |
| D2 | Widescreen = **extended view frustum** (render more world columns), configurable; 4:3 correction is the default correctness baseline. |
| D3 | Custom resolution = **arbitrary internal raster W×H** (parameterized raster), not fixed-buffer upscaling. |

## Architecture

### L2 — `r_interp` snapshot board (new module `room/src/doom/r_interp.rs`)

**Contract:** the simulation stays untouched; a render-side board captures *where everything was*
at the last two tic boundaries, and rendering interpolates between them.

- **Capture:** at the end of each tic (single call site in the tic loop after player/thinker
  movement, before `DG_DrawFrame`-equivalent presentation), a centralized walker traverses the
  thinker list and sector mover state and records a snapshot: for every mobj → `(x, y, z, angle)`
  and its pointer identity; for every moving sector → `(floorheight, ceilingheight)` keyed by
  sector index; player camera → `(viewx, viewy, viewz, angle, pitch)`.
- **Storage:** two generations (prev/curr). On each capture, curr←prev reuse: the walker copies
  into a double-buffered arena (fixed-capacity, no per-tic heap traffic). Renderers address
  snapshots by the same pointer/index keys.
- **Read:** `sample(mobj_ptr) -> (x, y, z, angle, fraction_applied)`. If a pointer exists only in
  `curr` (newly spawned), return curr (no interpolation — prboom behavior). If only in `prev`
  (removed this tic), return prev at its last known state (so a dying mob does not teleport back).
- **Fraction:** derived from the engine's own clock — `fraction = (now_ms − last_tic_ms) /
  (1000/35)`, clamped to `[0, 1)`. No new time input crosses the shell boundary; the shell merely
  calls the present entry more often (see frame split).
- **Coverage census test:** a fixed fixture map with one of every mover kind (mob, door, platform,
  lift, floor) runs two tics; the test asserts every census entry has a snapshot pair and that
  interpolated positions equal the expected midpoints. The census list is the module's contract —
  a new mover type that forgets to register fails the test.

### Frame/pump split at the doomgeneric boundary

Today shells call one tic per loop iteration and the engine presents at tic rate. F1 splits
"advance simulation" from "present a frame":

- **Engine entry (new, in `room/src/doom/doomgeneric.rs`):** `doomgeneric_frame(now_ms: u32)` —
  pumps 0..N tics using the existing internal ticker/accumulator semantics (N capped at 4 to
  survive tab-suspend catch-up without a spiral; the cap is a rendering policy, never a simulation
  policy), then presents one interpolated frame via the existing `I_FinishUpdate` path.
- **Shell changes:** native `about_to_wait` calls `doomgeneric_frame(elapsed_ms)`; the web export
  `woom24_tick()` is replaced by `woom24_frame(now_ms: f64)` (loader JS passes
  `performance.now()`). The old `doomgeneric_Tick` stays for tests/compat. `DG_SleepMs` becomes
  vestigial on both shells (the accumulator owns pacing; the doc comment says so).
- **Determinism proof obligation:** a golden test runs a recorded demo twice — once driven at
  exact 35 Hz, once driven with jittered `now_ms` — and asserts identical final state hashes. The
  fraction and the accumulator are render-loop state only; this test is the red line for the whole
  feature.

### L3 — `video_cfg` (new module `room/src/doom/video_cfg.rs`)

```rust
pub struct VideoConfig {
    pub width: u32,          // internal raster width  (multiple of 4 not required)
    pub height: u32,         // internal raster height
    pub aspect: AspectMode,  // VanillaStretch (4:3 baseline) | Wide
}
pub enum AspectMode { VanillaStretch, Wide }
```

- **Raster parameterization:** `i_video`/`r_draw`/`r_segs`/`r_plane` lose their hardcoded
  320/640/200/400 assumptions where they gate the framebuffer: `SCREENWIDTH`/`SCREENHEIGHT`
  become runtime values owned by `video_cfg` (the existing `static mut` pattern is kept to honor
  the port), the zone-allocated frame buffer is re-created on config change via the existing
  `I_InitScale`/`i_scale` stretch-table infrastructure, and column/span render loops take the
  live width. The audit list of hardcoded uses is a plan-time deliverable (grep census).
- **Widescreen (Wide mode):** `width` extends beyond the 4:3 height ratio; the view renders
  additional world columns horizontally (the horizontal FOV extension is a pure render-side
  column-count effect — the auto map and menus keep their own layouts). Status bar stays vanilla
  sized and centered in v1.
- **4:3 baseline (VanillaStretch):** internal raster stays 320×200-semantics (rendered at
  `width`×`height` with the 1.2 pixel-aspect correction applied by the presenter's CSS/window
  sizing, exactly as today's presenters already do). F1 makes the *correction* explicit and
  configurable rather than implicit.
- **FOV:** `Wide` uses the prboom-style horizontal extension derived from the aspect ratio (no
  configurable FOV slider in v1 — YAGNI; the config field exists so F7 can add one later).
- **Reconfiguration:** changing `VideoConfig` re-runs the scale/allocator init (zone realloc of
  the frame buffer + view arrays). Failure to allocate → `I_Error` (host) / banner (web), matching
  the established degradation contract.

### Presenters (unchanged contract)

`Presenter` keeps consuming one W×H BGRA frame; Canvas2D/WebGL2 backends already scale to the
canvas. The canvas element's CSS size becomes decoupled from `VideoConfig` (the window is the
window; the raster resolution is the engine's choice). No presenter edits expected beyond
accepting dynamic sizes — verified during planning.

## Components

| Component | Location | Kind |
|---|---|---|
| `r_interp` snapshot board | `room/src/doom/r_interp.rs` | new (L2) |
| `video_cfg` config + reconfig | `room/src/doom/video_cfg.rs` | new (L3) |
| frame/pump split | `room/src/doom/doomgeneric.rs` (+ `d_main` wiring) | modify |
| raster parameterization | `i_video`, `r_draw`, `r_segs`, `r_plane`, `i_scale` | modify (audit-driven) |
| shell loops | `shells/native/src/main.rs`, `shells/web/src/{lib.rs,www/woom24.js}` | modify |
| README smoke additions | `shells/web/README.md` | modify |

## Error handling

- Frame-buffer realloc failure → `I_Error` (native) / launcher banner (web) — established contract.
- Snapshot arena capacity exceeded (absurd mobj counts) → capture stops at capacity, renderer
  falls back to curr-only for the overflow (log once); never grows the arena at tic time.
- `now_ms` going backwards (host clock adjust) → clamp elapsed to 0 (no tics, fraction 0) — never
  negative, never sim-visible.

## Testing

1. **Determinism red line:** jittered-clock demo replay (above) — identical state hashes.
2. **Snapshot census:** fixture-map mover census (above).
3. **Raster invariants (host):** for a set of W×H including 320×200, 640×400, 1366×768: column
   functions write only within bounds; 4:3 VanillaStretch reproduces today's 640×400 output
   pixel-for-pixel at that size (regression anchor for the parameterization refactor).
4. **Aspect math (host):** `Wide` width selection and FOV column extension unit tests.
5. **Shell parity:** native and web drive `doomgeneric_frame` with identical synthetic time
   sequences → identical frame buffers (deterministic renderer check).
6. **Browser smoke additions** (README checklist, human pass): motion smoothness at 144 Hz,
   resolution switch mid-game, widescreen on/off, tab-suspend catch-up cap.

## Milestones (independent, each lands green)

- **M1 — interpolation:** `r_interp` + frame/pump split at 640×400 (D1 complete; uncapped visible).
- **M2 — arbitrary raster:** `video_cfg` + raster parameterization (D3 complete).
- **M3 — widescreen/aspect:** Wide mode + column extension + config (D2 complete).
- **M4 — config surface:** `VideoConfig` exposed to F7 settings registry (thin, lands with F7).

## Risks

- **Raster parameterization depth:** doomgeneric's 640×400 buffer is baked into memory ownership
  and several column loops; the audit census (plan-time) must enumerate every hardcoded use before
  M2 starts. Mitigation: M1 deliberately stays at 640×400 so M2's diff is purely mechanical.
- **Automap/finale at widescreen:** these draw full-screen layouts; v1 lets them render at the
  wide raster with their vanilla coordinates centered — visual check deferred to the human pass.
- **Catch-up cap feel:** cap of 4 tics (~114 ms) chosen from prboom practice; the human pass may
  tune it (config constant, not a setting).
