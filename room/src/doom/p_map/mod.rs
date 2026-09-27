//! Movement, collision handling, shooting, and aiming for the Doom
//! engine: position checking, teleport stomping, slide movement,
//! hitscan attacks and auto-aim, the Use action, radius splash damage,
//! sector-height change propagation, and the spechit overrun emulation
//! -- bit-exact with `vendor/doomgeneric/p_map.c`.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the spechit bounds (`MAXSPECIALCROSS` /
//!   `MAXSPECIALCROSS_ORIGINAL`), blockmap query expansion
//!   (`MAXRADIUS`), Use reach (`USERANGE`), the `MF_*` / `MT_*` flag and
//!   type values, `S_GIBS`, the `ST_*` slope classes, and
//!   `DEH_DEFAULT_SPECIES_INFIGHTING`
//! - `state.rs` -- the movement scratchpad statics (`tmbbox` ...
//!   `numspechit`) plus the `TeleptMobj` alias for the cross-module
//!   `p_inter` / `p_mobj` calls
//! - `move.rs` (module `move_` -- `move` is a Rust keyword) -- the
//!   movement/collision core: `pit_stomp_thing`, `teleport_move`,
//!   `pit_check_line` (the catalog entry-1 guarded store + emulation
//!   trigger), `pit_check_thing`, `check_position`, `try_move` (the
//!   bounded spechit drain), `thing_height_clip`
//! - `slide.rs` -- the slide statics plus `hit_slide_line`,
//!   `ptr_slide_traverse`, `slide_move`
//! - `attack.rs` -- the attack statics plus `ptr_aim_traverse`,
//!   `ptr_shoot_traverse`, `spawn_wall_puff` (the C `goto hitline`
//!   decode), `aim_line_attack`, `line_attack`
//! - `use_lines.rs` -- `usething` plus `ptr_use_traverse`, `use_lines`
//! - `radius.rs` -- the explosion statics plus `pit_radius_attack`,
//!   `radius_attack`
//! - `sector.rs` -- the crush statics plus `pit_change_sector`,
//!   `change_sector`
//! - `spechit.rs` -- the stateful entry-1 emulation
//!   (`spechit_overrun`: `-spechit` parse, census record, trample
//!   writes) and the whole-body regression pins
//! - `dtmc.rs` -- the extracted demo-synchronization surface (below)
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below. Two invisible consumer paths
//! drove the wiring: `p_enemy.rs` reads
//! `crate::doom::p_map::MAXSPECIALCROSS` by FULL PATH (the drain guard),
//! and every freeze-zone importer (`p_enemy`, `p_mobj`, `g_game`,
//! `p_floor/mover`, `p_pspr/weapons`, `p_telept/teleport`,
//! `p_user/think`, `c_tests/p_map_c`) still names the upstream
//! identifiers through the root.
//!
//! Cross-module contracts documented once here:
//! - **validcount stamp protocol** (shared with p_sight / p_maputl):
//!   `teleport_move` / `check_position` bump
//!   `crate::doom::r_main::validcount` on entry; `P_BlockLinesIterator`
//!   (p_maputl) and p_sight's seg walk consume the same stamp to dedupe
//!   lines/segs spanning multiple cells.
//! - **topslope/bottomslope dual-writer protocol** (p_sight): this
//!   module's `ptr_aim_traverse` narrows and `aim_line_attack`
//!   initializes the same two p_sight statics `P_CheckSight` uses for
//!   LOS; the F9 goldens pin the interaction.
//! - **Dependency directions**: `attack.rs` reads p_maputl's `trace` ray
//!   by FULLY QUALIFIED path (provenance kept obvious -- do not rewrite
//!   into `use` imports) and `use_lines.rs` casts into the not-yet-
//!   graduated `p_lights::line_t` mirror (freeze-zone quirk, carried
//!   verbatim). The mutual module cycles with p_sight and p_maputl
//!   resolve through module-root re-exports on both sides.
//! - p_map never had a link anchor (`doomgeneric.rs` anchor list has
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
//! comes with freeze-zone retirement). No C translation unit references
//! any of these symbols (`doomgeneric-sys/build.rs` excludes `p_map.c`).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `PIT_StompThing` | `move_::pit_stomp_thing` | dtmc (whole-body) | 10000-damage stomp decision (`gamemap != 30` gate) is demo-visible; shim `PIT_StompThing` + `#[export_name]` pin; upstream `p_map.c:97` |
//! | `P_TeleportMove` | `move_::teleport_move` | dtmc (whole-body) | stomp + re-link order; `numspechit = 0` reset interacts with the spechit protocol; shim + pin; upstream `p_map.c:130` |
//! | `PIT_CheckLine` | `move_::pit_check_line` | dtmc (whole-body) | catalog entry-1 / G1 site: guarded spechit push + unbounded counter + emulation trigger; bbox reject runs BEFORE the push; shim + pin; upstream `p_map.c:206` |
//! | `PIT_CheckThing` | `move_::pit_check_thing` | dtmc (whole-body; RNG draws stay at call site) | skull slam + missile damage rolls at two arms: draw COUNT and ORDER are the demo surface; shim + pin; upstream `p_map.c:275` |
//! | `P_CheckPosition` | `move_::check_position` | dtmc (whole-body) | scratchpad init + two-pass blockmap sweep order; `tmbbox` re-initialization closes the G1 trample window; shim + pin; upstream `p_map.c:401` |
//! | `P_TryMove` | `move_::try_move` | dtmc (whole-body) | height/dropoff gate ladder, re-link order, then the bounded spechit drain -> `P_CrossSpecialLine` ordering; shim + pin; upstream `p_map.c:477` |
//! | `P_ThingHeightClip` | `move_::thing_height_clip` | dtmc (whole-body) | crush-support predicate (`ceilingz - floorz < height`); shim + pin; upstream `p_map.c:557` |
//! | `P_HitSlideLine` | `slide::hit_slide_line` | dtmc (whole-body) | velocity projection via finetables -- exact trig indexing; shim + pin; upstream `p_map.c:611` |
//! | `PTR_SlideTraverse` | `slide::ptr_slide_traverse` | dtmc (whole-body) | opening accept predicate + best/second swap order; the not-a-line `i_error!` stays unchanged; shim + pin; upstream `p_map.c:663` |
//! | `P_SlideMove` | `slide::slide_move` | dtmc (whole-body) | three-trace order, `-= 0x800` fudge, stairstep fallback; statement order is the surface; shim + pin; upstream `p_map.c:722` |
//! | `PTR_AimTraverse` | `attack::ptr_aim_traverse` | dtmc (whole-body) | writes p_sight's `topslope`/`bottomslope`; `aimslope = (top+bottom)/2`; shim + pin; upstream `p_map.c:842` |
//! | `PTR_ShootTraverse` | `attack::ptr_shoot_traverse` | dtmc (whole-body) | special activation + sky/puff/blood side effects in order; reads p_maputl's `trace` by fully qualified path; shim + pin; upstream `p_map.c:928` |
//! | (`goto hitline` decode; Rust helper `goto_hitline`) | `attack::spawn_wall_puff` | dtmc (whole-body) | private, never a C symbol: sky-hack suppression chain; `#[doc(alias = "goto_hitline")]` only |
//! | `P_AimLineAttack` | `attack::aim_line_attack` | dtmc (whole-body) | ray endpoint math (finecosine/finesine index), scratchpad setup, LOS-window init; shim + pin; upstream `p_map.c:1067` |
//! | `P_LineAttack` | `attack::line_attack` | dtmc (whole-body) | same scratchpad setup; `damage == 0` is the trace-only variant; shim + pin; upstream `p_map.c:1109` |
//! | `PTR_UseTraverse` | `use_lines::ptr_use_traverse` | dtmc (whole-body) | Noway sound vs `P_UseSpecialLine` side computation; shim + pin; upstream `p_map.c:1142` |
//! | `P_UseLines` | `use_lines::use_lines` | dtmc (whole-body) | USERANGE ray; the `p_lights::line_t` cast (freeze-zone quirk) carried verbatim; shim + pin; upstream `p_map.c:1177` |
//! | `PIT_RadiusAttack` | `radius::pit_radius_attack` | dtmc (whole-body) | Chebyshev distance + boss immunity list + `P_CheckSight` LOS gate; shim + pin; upstream `p_map.c:1211` |
//! | `P_RadiusAttack` | `radius::radius_attack` | dtmc (whole-body) | block-range expansion + y-outer/x-inner iteration order; shim + pin; upstream `p_map.c:1252` |
//! | `PIT_ChangeSector` | `sector::pit_change_sector` | dtmc (whole-body; RNG draws stay at call site) | gib/drop/crush ladder; `leveltime & 3` gate; the blood-momentum `(P_Random() - P_Random()) << 12` pair; shim + pin; upstream `p_map.c:1304` |
//! | `P_ChangeSector` | `sector::change_sector` | dtmc (whole-body) | `blockbox` sweep order; return feeds the movers' revert logic; shim + pin; upstream `p_map.c:1367` |
//! | `SpechitOverrun` | `spechit::spechit_overrun` | dtmc (whole-body; pure cores extracted to `dtmc`) | catalog entry-1 emulation core: `-spechit` parse + lazy `baseaddr` + census record + writes stay at the call site; private before and after -- no C symbol, doc alias only; upstream `p_map.c:1391` |
//! | -- (new extraction; no named C counterpart) | `dtmc::spechit_trample_addr` | dtmc (extracted) | pure addr formula `baseaddr + (ld - lines) * 0x3e`, 32-bit wrap; no alias required; baseline vectors written/run against the pre-split body |
//! | -- (new extraction; no named C counterpart) | `dtmc::trample_target` / `dtmc::TrampleTarget` | dtmc (extracted) | the chocolate/woof case table: 9..=12 -> `tmbbox[0..=3]`, 13 -> `crushchange`, 14 -> `nofit` (NOT dsda's 13/14 swap); baseline vectors as above |
//! | 31 `#[no_mangle]` statics (`tmbbox` ... `usething`, `crushchange`, `nofit`) | `state` / `slide` / `attack` / `use_lines` / `radius` / `sector` | data | upstream names + `#[no_mangle]` retained; every freeze-zone/c_tests root path held by the `path-stability re-export` blocks below |
//! | `MAXSPECIALCROSS` / `MAXSPECIALCROSS_ORIGINAL` / `MAXRADIUS` / `USERANGE` / `MF_*` / `MT_*` / `S_GIBS` / `ST_*` / `DEH_DEFAULT_SPECIES_INFIGHTING` | `consts` | data | `MAXSPECIALCROSS` stays `pub(crate)` via the root re-export (`p_enemy.rs` reads it by full path); the rest are `pub(super)` |
//! | `type TeleptMobj` | `state` | data | alias for `p_telept::mobj_t`; load-bearing for the `p_inter` / `p_mobj` calls -- carried verbatim, do not "simplify" the casts |
//!
//! ## Deterministic Aspects
//!
//! `p_map` is sim-side wholesale: every function here writes
//! demo-observable state or feeds a demo-observable decision, so the
//! F10 §2 verdicts are "dtmc (whole-body)" with nothing pure left to
//! lift except the spechit trample cores. Load-bearing pieces:
//!
//! - **RNG ledger**: this module draws from `P_Random` at exactly four
//!   sites -- `pit_check_thing` (skull slam + missile damage, two
//!   `(P_Random() % 8) + 1` draws) and `pit_change_sector` (the blood
//!   momentum `(P_Random() - P_Random()) << 12` pair). Draw count and
//!   order are pinned by the F9 goldens; no other module may absorb or
//!   add draws here.
//! - **spechit emulation** (`spechit.rs` + `dtmc.rs`, catalog entry 1 /
//!   G1): the store in `pit_check_line` is BOUNDS-GUARDED while the
//!   counter advances unbounded (killough-style limit removal, woof/dsda
//!   parity), and counts past `MAXSPECIALCROSS_ORIGINAL` replay the
//!   chocolate/woof `.bss` trample through `spechit_overrun`. The
//!   guarded store + the bounded drains (`try_move` here, `P_Move` in
//!   p_enemy) are KEEP dispositions (catalog G2) -- never "fix" them
//!   into plain vanilla unguarded writes: this port has no DOS `.bss`
//!   layout to trample, and an unguarded write would clobber arbitrary
//!   wasm `.bss` neighbors (observed as the browser ticdup=0 freeze).
//!   The `dtmc` cores' baseline vectors were written and run GREEN
//!   against the pre-split in-file body BEFORE the move (F10 §2.3) --
//!   same vectors, same results; the whole-body drives live in
//!   `spechit::tests`.
//! - **`validcount` stamp protocol**: see the cross-module contract note
//!   above; `teleport_move` / `check_position` bump the shared stamp on
//!   entry.
//! - **`topslope`/`bottomslope` dual-writer protocol** (p_sight): see
//!   the cross-module contract note above; narrowing order in
//!   `ptr_aim_traverse` is the demo-visible window.

#[path = "move.rs"]
pub mod move_;

pub mod attack;
pub mod consts;
pub mod dtmc;
pub mod radius;
pub mod sector;
pub mod slide;
pub mod spechit;
pub mod state;
pub mod use_lines;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::p_map::P_*` / `PIT_*` / `PTR_*`). The C
//* symbol each shim forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name set
//* stays byte-identical to the pre-split module. Shims die with the
//* freeze zone.
pub use attack::{aim_line_attack as P_AimLineAttack, line_attack as P_LineAttack};
pub use move_::{
    check_position as P_CheckPosition, pit_check_line as PIT_CheckLine,
    pit_check_thing as PIT_CheckThing, pit_stomp_thing as PIT_StompThing,
    teleport_move as P_TeleportMove, thing_height_clip as P_ThingHeightClip,
    try_move as P_TryMove,
};
pub use radius::{pit_radius_attack as PIT_RadiusAttack, radius_attack as P_RadiusAttack};
pub use sector::{change_sector as P_ChangeSector, pit_change_sector as PIT_ChangeSector};
pub use slide::{
    hit_slide_line as P_HitSlideLine, ptr_slide_traverse as PTR_SlideTraverse,
    slide_move as P_SlideMove,
};
pub use use_lines::{ptr_use_traverse as PTR_UseTraverse, use_lines as P_UseLines};
pub use attack::{ptr_aim_traverse as PTR_AimTraverse, ptr_shoot_traverse as PTR_ShootTraverse};

//* path-stability re-export: the spechit bound keeps its FULL-PATH
//* consumer (`p_enemy.rs` reads `crate::doom::p_map::MAXSPECIALCROSS`
//* inside the P_Move drain guard).
pub(crate) use consts::MAXSPECIALCROSS;

//* path-stability re-export: the movement scratchpad keeps its module-
//* root paths (p_enemy, p_mobj, c_tests/p_map_c zero-default pins).
pub use state::{
    ceilingline, floatok, numspechit, spechit, tmbbox, tmceilingz, tmdropoffz, tmflags, tmfloorz,
    tmthing, tmx, tmy,
};

//* path-stability re-export: the slide state keeps its module-root paths
//* (c_tests/p_map_c).
pub use slide::{
    bestslideline, bestslidefrac, secondslideline, secondslidefrac, slidemo, tmxmove, tmymove,
};

//* path-stability re-export: the attack state keeps its module-root paths
//* (p_mobj sky-hack read, p_pspr/weapons, c_tests/p_map_c).
pub use attack::{aimslope, attackrange, la_damage, linetarget, shootthing, shootz};

//* path-stability re-export: the use/radius/sector state keeps its
//* module-root paths (p_user, c_tests/p_map_c, catalog entry-1 target
//* visibility).
pub use radius::{bombdamage, bombspot, bombsource};
pub use sector::{crushchange, nofit};
pub use use_lines::usething;
