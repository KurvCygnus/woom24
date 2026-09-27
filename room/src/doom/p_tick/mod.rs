//! Thinker management and the per-tic game heartbeat: the circular
//! thinker list every per-tic game object links into, the tic
//! dispatcher that advances players, thinkers, and specials in the
//! demo-pinned order, and the `leveltime` clock the cadence consumers
//! read -- bit-exact with `vendor/doomgeneric/p_tick.c`.
//!
//! ## Submodule Responsibility
//!
//! - `thinkers.rs` -- the thinker list: the `repr(C)` vocabulary every
//!   thinker-based game object embeds (`actionf_t`, `thinker_t`), the
//!   `thinkercap` list head, and the five list operations
//!   (`P_InitThinkers`, `P_AddThinker`, `P_RemoveThinker`, the
//!   upstream no-op `P_AllocateThinker` stub, `P_RunThinkers`) with
//!   their test module
//! - `ticker.rs` -- the tic: the `leveltime` static and its only
//!   writer `P_Ticker` (per-player `P_PlayerThink` dispatch, then
//!   `P_RunThinkers` -> `P_UpdateSpecials` -> `P_RespawnSpecials` ->
//!   `leveltime += 1`, in exactly that order) with its test module
//!
//! `dtmc` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_tick::*` across the freeze zone, the
//! `room::doom::p_tick::leveltime` test-crate reads, and the web
//! shell's debug overlay); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_Ticker` | `ticker::P_Ticker` | dtmc (whole-body; nothing extracts) | the tic skeleton itself -- pause-guard reads, per-player dispatch, `P_RunThinkers` -> `P_UpdateSpecials` -> `P_RespawnSpecials` -> `leveltime += 1` in exactly that order -- is the per-tic sequence demos pin; pure dispatch over freeze-zone statics with no separable computation (same shape as `M_ClearRandom` in the m_random precedent); `#[no_mangle]` retained; upstream `p_tick.c:123-151` |
//! | `P_RunThinkers` | `thinkers::P_RunThinkers` | dtmc (whole-body; nothing extracts) | iteration order over the circular list + lazy sentinel removal + `acp1` dispatch produce the exact thinker execution sequence; the raw-pointer walk IS the load-bearing integer sequence, so there is no pure part to lift out; carries one documented deviation: C advances the cursor after `Z_Free` (UB, `p_tick.c:113`), the port saves `next` first -- observably identical, must not be re-C-ified; upstream `p_tick.c:94-115` |
//! | `P_AddThinker` | `thinkers::P_AddThinker` | dtmc (whole-body) | tail-append fixes each thinker's position in the demo-visible execution order (savegame rebuild replays it in archive order); pointer surgery inseparable from the list state it mutates; upstream `p_tick.c:58-64` |
//! | `P_RemoveThinker` | `thinkers::P_RemoveThinker` | dtmc (whole-body) | the sentinel store defers unlink+free to the thinker's own next turn; that timing is demo-observable (a removed thinker does not act on its removal tic); upstream `p_tick.c:73-77` |
//! | `P_InitThinkers` | `thinkers::P_InitThinkers` | dtmc (whole-body) | the self-pointing cap is the initial empty order every level's sequence starts from (`P_SetupLevel`, `P_UnArchiveThinkers`); upstream `p_tick.c:46-49` |
//! | `P_AllocateThinker` | `thinkers::P_AllocateThinker` | glue | empty body upstream (`p_tick.c:85-87`) and zero consumers in-tree; kept so the wasm export surface stays byte-identical |
//! | `leveltime` | `ticker::leveltime` (static) | data | demo-visible tic counter: savegame bytes (3, big-endian), specials cadence masks, weapon bob, the frame_split golden pin (`EXPECTED.leveltime = 1029`), the web-shell overlay; `#[no_mangle]` retained; upstream `p_tick.c:27` |
//! | `thinkercap` | `thinkers::thinkercap` (static) | data | the demo-visible thinker chain head; walked by AI/boss/teleport/precache/saveg/interp; `#[no_mangle]` retained; upstream `p_tick.c:40` |
//! | `actionf_t` | `thinkers::actionf_t` (union) | data | header type (upstream `d_think.h:39-45`, not in p_tick.c); layout pinned within the 24-byte `thinker_t` prefix |
//! | `thinker_t` | `thinkers::thinker_t` (struct) | data | header type (upstream `d_think.h:56-64`); embedded at offset 0 of every thinker object, size pinned to 24 bytes (LP64) |
//! | `sentinel_ac` | `dtmc::sentinel_ac` | dtmc (extracted) | the pure removal-sentinel computation; upstream counterpart is the `(actionf_v)(-1)` literal (`p_tick.c:76`, `p_tick.c:101`) -- no named C function, so no `#[doc(alias)]` |
//! | `is_sentinel` | `dtmc::is_sentinel` | dtmc (extracted) | the compare half of the same lazy-removal contract; upstream `p_tick.c:101` (inline compare against the literal) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all eight exported items (`leveltime`, `thinkercap`,
//! `P_InitThinkers`, `P_AddThinker`, `P_RemoveThinker`,
//! `P_AllocateThinker`, `P_RunThinkers`, `P_Ticker`) keep their C
//! symbol and `#[no_mangle]`, which keeps the wasm export surface
//! byte-identical (zero in-tree `extern "C"` re-declarers exist; the
//! only extern-declaring crate, `c2rust-intermediate`, does not link
//! room). The root re-exports below carry the unchanged names for
//! path stability only; `c_ffi` re-exports `leveltime` onward for the
//! C-test bridge.
//!
//! ## Deterministic Aspects
//!
//! The module owns the per-tic synchronization skeleton: the
//! thinker-list lifecycle and ordering (`P_InitThinkers`' self-pointing
//! cap, `P_AddThinker`'s tail-append, `P_RemoveThinker`'s lazy
//! sentinel, `P_RunThinkers`' walk order), the tic dispatch order in
//! `P_Ticker`, and the `leveltime` clock the cadence consumers read
//! (`& 7` / `& 3` / `& 0x1f` / `& 31` masks, weapon bob, texture
//! animation). The `leveltime` increment is the LAST statement of
//! `P_Ticker` (upstream `p_tick.c:149-150`) -- reordering it changes
//! r_interp's latches and moves the frame_split goldens. The extracted
//! `dtmc::{sentinel_ac, is_sentinel}` must keep the exact all-ones
//! pattern (`usize::MAX` transmuted into the function pointer): it is
//! the single sentinel source for both `P_RunThinkers` and (since the
//! F10 wave B3a p_mobj graduation retired the freeze-zone duplicate)
//! p_mobj's `mobj_thinker`; a different discriminant (e.g. an
//! `Option::None`-based redesign) breaks mid-tic removal detection. `P_RunThinkers`'
//! documented deviation stands: `next` is saved before `Z_Free` (the C
//! reads freed memory there) -- observably identical, never
//! re-C-ified. The baseline vectors in `dtmc`'s test module were
//! written and run against the original in-file sentinel bodies
//! before extraction (F10 graduate #4).

pub mod dtmc;
pub mod thinkers;
pub mod ticker;

//* path-stability re-export: thinker-list vocabulary and operations
//* keep their module-root paths for the freeze-zone callers
//* (p_ceilng, p_doors, p_enemy, p_floor, p_lights, p_map, p_mobj,
//* p_plats, p_saveg, p_spec, p_telept, r_data, r_interp, c_ffi).
pub use thinkers::{
    actionf_t, thinker_t, thinkercap, P_AddThinker, P_AllocateThinker, P_InitThinkers,
    P_RemoveThinker, P_RunThinkers,
};

//* path-stability re-export: the tic clock and ticker keep their
//* module-root paths (g_game.rs, the p_*.rs imports, c_ffi.rs, the F9
//* harnesses, and the web shell's debug overlay).
pub use ticker::{leveltime, P_Ticker};
