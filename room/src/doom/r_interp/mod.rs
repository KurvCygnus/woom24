//! L2 `r_interp` — the render-side interpolation snapshot board (F1 M1).
//!
//! The simulation is never touched. After each simulated tic completes, a
//! centralized capture walker (single call site in `d_loop.rs`) records where
//! every visible mover *was* into a fixed-capacity, double-buffered arena:
//! mobj `(x, y, z, angle)` pairs keyed by mobj pointer identity, sector
//! `(floorheight, ceilingheight)` pairs keyed by sector index, the per-player
//! camera `(x, y, viewz, angle)` and the weapon-sprite `(sx, sy)` positions.
//! The renderer samples interpolated values between the last two tic
//! boundaries; with the board disabled (or before any capture) every sampler
//! returns the live simulation value, reproducing today's output bit-for-bit.
//!
//! Reference practice (all verified in the local clones):
//! - Centralized once-per-tic registry + order-independent capture after tic
//!   movement: dsda-doom `r_fps.c:331-340` (`R_UpdateInterpolations`) and
//!   `prboom2/src/p_mobj.c:1267-1280`. We do not hook individual movement
//!   sites.
//! - Guard set adopted from Woof!: no interpolation on spawn
//!   (`p_mobj.c:917-926`), player-missile first-tic suppression
//!   (`p_mobj.c:752-756`, sentinel set at spawn), paused guard
//!   `leveltime > oldleveltime` (`r_main.c:722`, latch in `g_game.c:3519`),
//!   first-level-tic camera guard `leveltime > 1` (`r_main.c:709-713`), and
//!   the psprite state-change snap (`p_pspr.c:1221`).
//! - The teleport snap is RENDER-side here: a prev/curr pair whose XY
//!   displacement exceeds `2 x MAXMOVE` is treated as two different objects
//!   (snap to curr, never lerp). This is the replacement for Woof!'s
//!   sim-side `interp = false` write in `p_map.c:319-320` — a simulation
//!   write our determinism red line forbids.
//! - Fixed-point lerp math from `woof/src/r_main.h:133-157`: `LerpFixed` is
//!   integer-exact; angles interpolate along the SHORT arc (Woof!'s
//!   `LerpAngle` uses an ANG270 wrap threshold, which takes the long way for
//!   90..180 degree turns; the F1 contract text says short arc, so the
//!   threshold here is ANG180).
//! - Fraction from the engine's own clock: `(rel_ms * TICRATE % 1000) *
//!   FRACUNIT / 1000`, the exact quantity Woof! computes in
//!   `i_timer.c:113-116` (`I_GetFracTime_Scaled`), refreshed once per present
//!   (Woof! refreshes per `D_Display`, `d_main.c:255-261`). u64 intermediates
//!   avoid the 32-bit overflow Woof!'s C int math hits after ~17 hours.
//!
//! Struct-layout note: Woof! embeds prev state inside `mobj_t`/`sector_t` —
//! impossible here because our `#[repr(C)]` mirrors must stay byte-identical
//! to the C engine's structs. Hence an external keyed board.
//!
//! ## Submodule Responsibility
//!
//! - `board.rs` -- the gate (`r_interp_enabled`, `enabled`, `set_enabled`),
//!   `PosSample` and the slot/static storage, `board_active`, `begin_frame`,
//!   the tic-loop pair (`begin_tic` / `end_tic_and_capture`),
//!   `reset_board`, the capture walkers + mobj hash, the teleport
//!   threshold, and the census/guard test suite (the GlobalsGuard +
//!   fabricated-world tests live beside the board state they drive)
//! - `math.rs` -- `lerp_fixed`, `lerp_angle`, the fraction trio
//!   (`fraction`/`set_fraction`/`refresh_fraction`),
//!   `fraction_from_rel_ms`, plus the pure-math tests
//! - `sampling.rs` -- `sample_camera`, `sample_mobj`, `sector_floor`,
//!   `sector_ceiling`, `sample_psp`, `sector_index`, and the
//!   `struct_layout_parity_for_mirrors` test (it guards
//!   `sector_index`'s pointer math)
//!
//! The module root is documentation + wiring only: no renames happened
//! here (the names were already house-style), so the re-exports below are
//! path-stability re-exports -- NOT upstream-name shims -- and there are no
//! C symbols to pin (zero `#[no_mangle]`, zero pins, zero anchors).
//! Consumers keep their exact paths: `d_loop/tic_pump.rs:347/:353` calls
//! the capture pair inside the ticdup loop, `r_main`/`r_things` sample the
//! camera/mobj/psprite (F1 M1 call points), `d_main/display.rs` and
//! `d_main/entries.rs` drive the frame/wipe gates, the four test binaries
//! and `scenario_harness` arm/inspect the board, and the web shell
//! saves/restores the fraction around surgical re-presents
//! (`shells/web/src/lib.rs`, `harness.rs`).
//!
//! ## Identity Name Mapping
//!
//! Already house-idiom at graduation time (Allman, snake_case English
//! names, no `#[no_mangle]`, provenance in doc comments): the mapping is
//! an identity mapping -- every name keeps its spelling, only its
//! location moves. `r_interp_enabled` stays addressable as
//! `crate::doom::r_interp::r_interp_enabled` through the root re-export.
//!
//! | Current name | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `BOARD_MOBJ_SLOTS` / `BOARD_SECTOR_SLOTS` | `board` | data | fixed capacities; overflow stops the capture, never grows at tic time |
//! | `r_interp_enabled` | `board` | data | master gate; tests write it directly (root re-export holds the path) |
//! | `enabled` / `set_enabled` | `board` | glue | gate read/flip |
//! | `fraction` / `set_fraction` / `refresh_fraction` | `math` | glue | fraction accessors + wipe re-sample; root re-export |
//! | `board_active` | `board` | glue | legacy Tick path stays bit-exact when false |
//! | (private) `fraction_from_rel_ms` | `math` | glue-adjacent | pure reference math (Woof! `i_timer.c:113-116`); consumer is the renderer only |
//! | `begin_frame` | `board` | glue | once per presented frame |
//! | `lerp_fixed` / `lerp_angle` | `math` | glue | integer-exact; short-arc threshold ANG180 (documented deviation from Woof!) |
//! | (private) `teleported` | `board` | glue | the 2 x MAXMOVE render-side teleport snap |
//! | `begin_tic` / `end_tic_and_capture` | `board` | glue | THE capture pair: single call sites in `d_loop/tic_pump.rs`; write-isolated to board statics |
//! | `reset_board` | `board` | glue | level-change reset |
//! | (private) `mobj_thinker_addr`, `capture_mobjs`, `mobj_hash_insert`, `mobj_find_or_insert`, `mobj_lookup`, `mobj_hash`, `capture_sectors`, `capture_cameras`, `capture_psprites`, `sector_index` | `board` / `sampling` | glue | the centralized walker + index machinery |
//! | `sample_camera` / `sample_mobj` / `sector_floor` / `sector_ceiling` / `sample_psp` | `sampling` | glue | read-only samplers; every fallback arm returns the live value |
//! | statics `MOBJ_SLOTS`/`MOBJ_HASH`/`MOBJ_LEN`/`SECTOR_*`/`CAM_SLOTS`/`PSPR_SLOTS`/`OLD_LEVELTIME`/`LAST_CAPTURE_LEVELTIME`/`BOARD_ACTIVE`/`FRACTION`/`MOBJ_OVERFLOW_LOGGED` | `board` | data | private beside their writers (`FRACTION`/`CAM_SLOTS`/`PSPR_SLOTS` `pub(super)` beside `math`/`sampling`) |
//! | `PosSample`, `MobjSlot`, `CamSlot`, `PsprSlot` | `board` | data | `PosSample` pub + root re-export; slot types private |
//!
//! ## Deterministic Aspects
//!
//! **Render-only by design -- no dtmc surface.** The F1 contract is that
//! the simulation is never touched: the board captures AFTER a tic
//! completes, samples BEFORE the next, and every sampler returns the live
//! simulation value bit-for-bit when the gate is off, the board is
//! inactive, a guard trips, or the pair is missing (each sampler's
//! fallback arm). No function here belongs to the demo synchronization
//! surface -- by design, the opposite invariant holds: sim observables
//! must be a function of tics alone, never of render timing. The board is
//! the mechanism that let uncapped rendering exist without violating
//! AGENTS constraint 3. `begin_tic`/`end_tic_and_capture` sit inside the
//! tic loop but are write-isolated to board statics (hard firewall: the
//! `d_loop/tic_pump.rs` call sites stay the ONLY capture entries). The
//! frame goldens DO see the samplers' output when enabled
//! (`video_anchor`'s golden history documents the sampling-fraction
//! sensitivity); with sampling disabled the pre/post trees are
//! byte-identical (`0xf3f8bc0c69cf6ca5`). `fraction_from_rel_ms` is the
//! closest thing to a pure core, but its consumer is the renderer only --
//! stating "render-only by design" here is the honest record; no `dtmc`
//! file exists for this module on purpose.

#![allow(non_upper_case_globals, non_snake_case)]

pub mod board;
pub mod math;
pub mod sampling;

//* path-stability re-export (identity mapping -- NOT upstream-name shims;
//* nothing was renamed): the public surface keeps its exact module-root
//* paths. Load-bearing: `d_loop/tic_pump.rs` (capture pair), `r_main`/
//* `r_things` (camera/mobj/psprite samplers), `d_main` display/entries
//* (frame/wipe gates), `tests/sprite_interp_probe.rs`,
//* `tests/sprite_apply_regression.rs`, `tests/scenario_harness`,
//* `tests/video_raster.rs`'s boot path, and the web shell
//* (fraction save/restore + launcher preview sampling).
pub use board::{
    board_active, begin_frame, begin_tic, enabled, end_tic_and_capture, reset_board,
    r_interp_enabled, set_enabled, PosSample, BOARD_MOBJ_SLOTS, BOARD_SECTOR_SLOTS,
};
pub use math::{fraction, lerp_angle, lerp_fixed, refresh_fraction, set_fraction};
pub use sampling::{sample_camera, sample_mobj, sample_psp, sector_ceiling, sector_floor};
