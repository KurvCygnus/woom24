//! Map object (mobj) lifecycle: creation, movement, state-machine updates,
//! spawning from map data, and item-respawn in deathmatch mode --
//! bit-exact with `vendor/doomgeneric/p_mobj.c`.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the numeric vocabulary: friction/stopping
//!   constants (`STOPSPEED`/`FRICTION`), the movement budget
//!   (`MAXMOVE`/`GRAVITY`/`FLOATSPEED`), position sentinels
//!   (`ONFLOORZ`/`ONCEILINGZ`), spawn defaults
//!   (`VIEWHEIGHT`/`MELEERANGE`), `MTF_AMBUSH`, the player states,
//!   and the `exe_ultimate` version gate
//! - `state.rs` -- the state machine: `set_mobj_state` (zero-tic
//!   chain + action dispatch) and `explode_missile`
//! - `movement.rs` -- the momentum integrators `xy_movement`
//!   (sub-steps, sliding, sky hack, friction) and `z_movement`
//!   (gravity, floating, the version-gated Lost Soul bounce)
//! - `spawn.rs` -- creation: `spawn_mobj` plus the effect/projectile
//!   spawners (`spawn_puff`, `spawn_blood`, `spawn_missile`,
//!   `spawn_player_missile`), `check_missile_spawn`, and the vanilla
//!   null-deref dummy `subst_null_mobj` (catalog entry 12)
//! - `mapthings.rs` -- the THINGS-lump entry: `spawn_map_thing`
//!   (map-load dispatcher) and `spawn_player` (with the entry-10
//!   type-0 guard)
//! - `lifecycle.rs` -- the per-tic `mobj_thinker`,
//!   `nightmare_respawn`, `remove_mobj`, `respawn_specials`, and the
//!   four item-respawn queue statics
//!
//! `dtmc` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below. Every freeze-zone importer (`p_enemy`,
//! `p_inter`, `p_map`, `p_pspr`, `p_setup`, `p_telept`, `p_tick`,
//! `p_user`, `r_data`, `r_interp`, `g_game`) still names the upstream
//! identifiers through the root.
//!
//! Cross-module contracts documented once here:
//! - **mobj-thinker identity** (p_saveg / r_data / r_interp /
//!   p_telept): `spawn_mobj` stores exactly one
//!   `lifecycle::mobj_thinker` item into each mobj's `acp1`, and every
//!   consumer identifies mobj thinkers by comparing that address --
//!   the extern declarer `p_saveg.rs:91` (compare at `:1673`), the
//!   direct call `p_saveg.rs:96` (at `:1726`), `r_data/precache.rs`'s
//!   sprite-precache scan, `r_interp.rs:471`/`:962`'s walker filter,
//!   and `p_telept/teleport.rs:77`'s transmute compare all resolve
//!   through the root shims, which are plain `pub use` re-exports of
//!   the SAME function item -- any wrapper would silently break every
//!   compare. The `#[export_name]` pins are load-bearing for the two
//!   `p_saveg.rs` extern declarations (`P_MobjThinker`,
//!   `P_RemoveMobj`); the other fifteen ride the name = symbol = pin
//!   conservatism rule (p_pspr precedent).
//! - **sentinel single source** (shared with p_tick): the removal
//!   sentinel pair lives only in `crate::doom::p_tick::dtmc`
//!   (`sentinel_ac`/`is_sentinel`); the file-local duplicate that used
//!   to sit beside the thinker retired with this graduation
//!   (p_tick/dtmc.rs anticipated exactly that). `mobj_thinker`
//!   consumes `is_sentinel`; the p_tick dtmc vectors keep guarding the
//!   all-ones pattern.
//! - **Dependency directions**: movement reads p_map's scratch state
//!   (`ceilingline`) and slide/tryspace entry points; the module never
//!   touches renderer state. No C translation unit references any of
//!   these symbols (`doomgeneric-sys/build.rs` comments out
//!   `p_mobj.c`); `p_saveg.rs` is the only extern-by-symbol consumer.
//!   p_mobj has no link anchor (`doomgeneric.rs` anchor list has
//!   none) -- the absence is preserved.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the `upstream-name shim` re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module. Functions only:
//! statics/consts/tables keep their upstream names (data-tier renaming
//! comes with freeze-zone retirement).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_SetMobjState` | `state::set_mobj_state` | dtmc (whole-body) | zero-tic chain + action dispatch order; the `S_NULL` arm removes via `remove_mobj`; shim + pin; upstream `p_mobj.c:48-50` |
//! | `P_ExplodeMissile` | `state::explode_missile` | dtmc (whole-body; RNG draw stays at call site) | death-state transition BEFORE the tic-jitter draw (one `P_Random` draw via `dtmc::tics_jitter_clamp`), then flag clear + deathsound; shim + pin; upstream `p_mobj.c:84` |
//! | `P_XYMovement` | `movement::xy_movement` | dtmc (whole-body) | `MAXMOVE/2` sub-step ladder, missile/sky-hack arm order, stop-snap vs friction; shim + pin; upstream `p_mobj.c:108` |
//! | `P_ZMovement` | `movement::z_movement` | dtmc (whole-body) | view smoothing, float tracking, gravity init `-2*GRAVITY`, and the Lost Soul bounce emulated twice (pre/post `exe_ultimate` order); shim + pin; upstream `p_mobj.c:240` |
//! | `P_NightmareRespawn` | `lifecycle::nightmare_respawn` | dtmc (whole-body) | obstructed probe, fog-fog-monster spawn order, angle quantize, `reactiontime = 18`, corpse removal last; shim + pin; upstream `p_mobj.c:383` |
//! | `P_MobjThinker` | `lifecycle::mobj_thinker` | dtmc (whole-body) | per-tic gate ladder + sentinel re-checks + Nightmare `leveltime & 31` / `P_Random() > 4` gates; pin LOAD-BEARING (`p_saveg.rs:91` extern + `:1673` compare); shim; upstream `p_mobj.c:441/473` |
//! | `P_SpawnMobj` | `spawn::spawn_mobj` | dtmc (whole-body) | init order + `lastlook` draw + spawnstate copy; stores the single `mobj_thinker` item into `acp1` (identity contract above); shim + pin; upstream `p_mobj.c:503-506` |
//! | `P_RemoveMobj` | `lifecycle::remove_mobj` | dtmc (whole-body) | respawn-queue push + head/tail ring order before unlink/stop/lazy-remove; pin LOAD-BEARING (`p_saveg.rs:96` extern call); shim; upstream `p_mobj.c:572` |
//! | `P_RespawnSpecials` | `lifecycle::respawn_specials` | dtmc (whole-body) | `deathmatch == 2` + `30 * TICRATE` gates, fog+item spawn order, tail step; shim + pin; upstream `p_mobj.c:604/651` |
//! | `P_SpawnPlayer` | `mapthings::spawn_player` | dtmc (whole-body; entry-10 workaround guard inside) | type-0 guard records `PlayeringameOverrun` and spawns nothing; init order pinned; shim + pin; upstream `p_mobj.c:668/696` |
//! | `P_SpawnMapThing` | `mapthings::spawn_map_thing` | dtmc (whole-body) | type 11/<=0/1-4 dispatch, skill filter (`dtmc::spawn_skill_bit`), doomednum walk, initial-tic draw `1 + P_Random() % tics`, kill/item counters; shim + pin; upstream `p_mobj.c:739/823` |
//! | `P_SpawnPuff` | `spawn::spawn_puff` | dtmc (whole-body; RNG draws stay) | `(P_Random() - P_Random()) << 10` pair + tic jitter + melee-range `S_PUFF3` retarget; shim + pin; upstream `p_mobj.c:846/851` |
//! | `P_SpawnBlood` | `spawn::spawn_blood` | dtmc (whole-body; RNG draws stay) | same jitter pair + damage ladder (`9..=12` -> `S_BLOOD2`, `< 9` -> `S_BLOOD3`); shim + pin; upstream `p_mobj.c:875/878` |
//! | `P_CheckMissileSpawn` | `spawn::check_missile_spawn` | dtmc (whole-body; RNG draw stays) | jitter draw FIRST, then half-step probe + explode-on-block; shim + pin; upstream `p_mobj.c:907` |
//! | `P_SubstNullMobj` | `spawn::subst_null_mobj` | glue | vanilla null-deref dummy (catalog entry 12); pure pointer swap, no sim sequence; shim + pin; upstream `p_mobj.c:929` |
//! | `P_SpawnMissile` | `spawn::spawn_missile` | dtmc (whole-body; shadow-jitter RNG stays) | `MF_SHADOW` `(P_Random() - P_Random()) << 20` aim pair + fine-table velocity + `momz` from travel time; shim + pin; upstream `p_mobj.c:947-950` |
//! | `P_SpawnPlayerMissile` | `spawn::spawn_player_missile` | dtmc (whole-body) | three-cone auto-aim ladder (`+ 1 << 26`, `- 2 << 26`, fall back); no RNG; shim + pin; upstream `p_mobj.c:992/996` |
//! | 4 `#[no_mangle]` statics (`itemrespawnque`, `itemrespawntime`, `iquehead`, `iquetail`) | `lifecycle` | data | upstream names + `#[no_mangle]` retained; every freeze-zone/c_tests root path held by the `path-stability re-export` block below (`p_setup.rs:395`, `c_tests/p_mobj_c.rs`) |
//! | `sentinel_ac` / `is_sentinel` (file-local helpers) | retired -> `crate::doom::p_tick::dtmc` | dtmc (shared) | the freeze-zone duplicate retired in favor of p_tick's single source; `lifecycle` consumes `is_sentinel`; no C symbol existed |
//! | -- (new extraction; no named C counterpart) | `dtmc::mapthing_angle_quantize` | dtmc (extracted) | the `ANG45 * (angle / 45)` BAM quantization shared by 4 spawn sites (nightmare/respawn-specials/spawn-player/spawn-mapthing); baseline vectors landed pre-move (commit `377ca48`) |
//! | -- (new extraction; no named C counterpart) | `dtmc::respawn_queue_step` | dtmc (extracted) | the `(i + 1) & (ITEMQUESIZE - 1)` ring step at 3 sites; the wrap IS the deathmatch respawn surface; same pre-move baseline |
//! | -- (new extraction; no named C counterpart) | `dtmc::spawn_skill_bit` | dtmc (extracted) | the skill 0..=4 -> bit 1/1/2/4/4 THINGS-lump filter ladder; same pre-move baseline |
//! | -- (new extraction; no named C counterpart) | `dtmc::tics_jitter_clamp` | dtmc (extracted) | the `t -= draw & 3` floor-at-1 pair at 4 sites (explode/puff/blood/check-missile); RNG draws stay at call sites; same pre-move baseline |
//! | `type CffiMobj` / `type SetupMapThing` (file-local aliases) | `movement` / `lifecycle` / `spawn` / `mapthings` | data | carried verbatim next to their users; do not "simplify" the `#[repr(C)]`-identical casts |
//!
//! ## Deterministic Aspects
//!
//! `p_mobj` is sim-side wholesale: every exported function writes
//! demo-observable state or gates an RNG/statement-order sequence, so
//! the F10 §2 verdicts are "dtmc (whole-body)" except the
//! `subst_null_mobj` glue swap; the four pure cores lifted into
//! `dtmc` are the only separable computations. Load-bearing pieces:
//!
//! - **RNG ledger**: this module draws from `P_Random` at exactly the
//!   following sites -- `spawn_mobj`'s `lastlook` (`% MAXPLAYERS`),
//!   `spawn_map_thing`'s initial-tic draw (`1 + P_Random() % tics`),
//!   `spawn_puff`/`spawn_blood`'s `(P_Random() - P_Random()) << 10`
//!   pairs plus their tic-jitter draws, `explode_missile`'s tic
//!   jitter, `check_missile_spawn`'s tic jitter, `spawn_missile`'s
//!   `MF_SHADOW` `<< 20` pair, and `mobj_thinker`'s Nightmare gate
//!   (`P_Random() > 4`, drawn only after every earlier gate passes).
//!   Draw count and order are pinned by the F9 goldens; the
//!   `dtmc::tics_jitter_clamp` extraction moved the mask+floor, never
//!   the draws.
//! - **respawn queue**: `remove_mobj`'s push and `respawn_specials`'
//!   tail step share `dtmc::respawn_queue_step`; the head-catches-tail
//!   overwrite order decides which slot a deathmatch demo respawns.
//!   The queue statics are process globals re-zeroed by
//!   `P_SetupLevel` (p_setup.rs:1306) -- no per-level state leaks.
//! - **thinker identity**: `spawn_mobj` stores the one
//!   `lifecycle::mobj_thinker` item; the savegame/precache/interp
//!   compares (contract note above) and the p_saveg extern pins all
//!   ride the root re-exports. The sentinel pair is p_tick::dtmc's
//!   single source; the two-sources failure mode from the freeze zone
//!   is gone, and p_tick's dtmc vectors keep guarding the pattern.

pub mod consts;
pub mod dtmc;
pub mod lifecycle;
pub mod mapthings;
pub mod movement;
pub mod spawn;
pub mod state;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::p_mobj::P_*`). The C symbol each shim
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. Shims die with
//* the freeze zone.
pub use lifecycle::{
    mobj_thinker as P_MobjThinker, nightmare_respawn as P_NightmareRespawn,
    remove_mobj as P_RemoveMobj, respawn_specials as P_RespawnSpecials,
};
pub use mapthings::{spawn_map_thing as P_SpawnMapThing, spawn_player as P_SpawnPlayer};
pub use movement::{xy_movement as P_XYMovement, z_movement as P_ZMovement};
pub use spawn::{
    check_missile_spawn as P_CheckMissileSpawn, spawn_blood as P_SpawnBlood,
    spawn_mobj as P_SpawnMobj, spawn_missile as P_SpawnMissile,
    spawn_player_missile as P_SpawnPlayerMissile, spawn_puff as P_SpawnPuff,
    subst_null_mobj as P_SubstNullMobj,
};
pub use state::{set_mobj_state as P_SetMobjState, explode_missile as P_ExplodeMissile};

//* path-stability re-export: the item-respawn queue statics keep their
//* module-root paths (p_setup.rs, c_tests/p_mobj_c.rs -- data tier,
//* upstream names + #[no_mangle] retained).
pub use lifecycle::{itemrespawnque, itemrespawntime, iquehead, iquetail};
