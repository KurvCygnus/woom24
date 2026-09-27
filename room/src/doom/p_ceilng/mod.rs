//! Ceiling animation: lowering, crushing, and raising, driven by the
//! `T_MoveCeiling` thinker over the `ceiling_t` state and the
//! fixed-size `activeceilings` table, with the linedef-triggered
//! activation/stop events -- bit-exact with
//! `vendor/doomgeneric/p_ceilng.c`, including the PRE-EXISTING port
//! divergences recorded in the mapping table (never normalize them
//! here).
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `repr(C)` `ceiling_t` thinker state, the
//!   `activeceilings` table, the speed/type/return-value constants,
//!   the compile-time layout checks, and the layout-pinning test
//! - `thinker.rs` -- `T_MoveCeiling`, the per-tic ceiling thinker
//! - `events.rs` -- the linedef-triggered events (`EV_DoCeiling`,
//!   `EV_CeilingCrushStop`) and the active-table maintenance
//!   (`P_AddActiveCeiling`, `P_RemoveActiveCeiling`,
//!   `P_ActivateInStasisCeiling`)
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic Aspects.
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing consumer
//! path valid (`crate::doom::p_ceilng::*` across the freeze zone --
//! the `T_MoveCeiling`/`ceiling_t`/`MAXCEILINGS`/`activeceilings`
//! consumers in p_saveg, the `EV_DoCeiling` consumers p_spec and
//! p_switch, and the `doomgeneric_Create` anchor call); no content
//! lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `T_MoveCeiling` | `thinker::T_MoveCeiling` | dtmc (whole-body; nothing extracts) | per-tic ceiling heights/speed/direction via `T_MovePlane`; pointer-taken AND address-compared (`p_saveg.rs:1871`) -- no rename, signature pinned; carries two PRE-EXISTING divergences, intentionally NOT fixed and FIXME-carried verbatim: (1) the `pastdest`/`crushed` handling is not gated on `direction` (FIXME, `p_ceilng.rs:137-151`; C gates via `switch(ceiling->direction)` with a `case 0` stasis no-op, `p_ceilng.c:49-145`), and (2) the single crusher `pastdest` arm sets `direction = -1` where C's down-phase sets `direction = 1` (`p_ceilng.rs:197-199` vs `p_ceilng.c:113-127`); upstream `p_ceilng.c:45` |
//! | `EV_DoCeiling` | `events::EV_DoCeiling` | dtmc (whole-body; nothing extracts) | spawner in the demo surface: tag-scan order + initial state; carries a PRE-EXISTING divergence, intentionally NOT fixed: the `silentCrushAndRaise`/`crushAndRaise` arm sets only `crush` + `topheight` (`p_ceilng.rs:280-283`) where C falls through into the `lowerAndCrush`/`lowerToFloor` config (`bottomheight` = floor(+8), `direction = -1`, `speed = CEILSPEED`, `p_ceilng.c:199-214`) -- deterministic here only because the port's `Z_Malloc` zeroes (`z_zone.rs:189`); upstream `p_ceilng.c:161` |
//! | `P_AddActiveCeiling` | `events::P_AddActiveCeiling` | dtmc (whole-body; nothing extracts) | `activeceilings` slot write; the silent overflow discard is sim-observable and C-faithful (`p_ceilng.rs:309-311`); upstream `p_ceilng.c:240` |
//! | `P_RemoveActiveCeiling` | `events::P_RemoveActiveCeiling` | dtmc (whole-body; nothing extracts) | clears `specialdata` + removes the thinker; upstream `p_ceilng.c:259` |
//! | `P_ActivateInStasisCeiling` | `events::P_ActivateInStasisCeiling` | dtmc (whole-body; nothing extracts) | restores `direction` + reinstates `acp1` -- tick-order observable; upstream `p_ceilng.c:280` |
//! | `EV_CeilingCrushStop` | `events::EV_CeilingCrushStop` | dtmc (whole-body; nothing extracts) | stasis transitions; the return flag is consumed by `P_CrossSpecialLine` (`p_spec/crossline.rs:69`); upstream `p_ceilng.c:303` |
//! | `P_Ceilng_Link_Anchor` | `anchor::P_Ceilng_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `ceiling_t` / `activeceilings` / `MAXCEILINGS` | `state` | data | layout-pinned (72 bytes: compile-time `layout_checks` plus the layout test moved with them); `ceiling_t` read field-wise by `p_saveg` through the module root; `activeceilings` keeps its `#[no_mangle]` C symbol and its module-root path (`p_saveg.rs:1854` scans it via `addr_of!(crate::doom::p_ceilng::activeceilings[0])`) |
//! | `CEILSPEED`, `result_*`, `ceiling_e` values | `state` consts | data | speed/type/return-value constants (`p_ceilng.c:27-35`, `p_local.h`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all seven exported functions keep their C symbol and
//! `#[no_mangle]`, which keeps the wasm export surface byte-identical
//! (the `activeceilings` static keeps its `#[no_mangle]` too).
//! `T_MoveCeiling` is transmuted into `thinker_t.function.acp1` AND
//! compared by address (`p_saveg.rs:1871`), so its name and exact
//! signature are pinned; the in-tree `pub use` re-exports keep one
//! function item per name, which is what keeps the pointer compares
//! and the `activeceilings` array scan valid. No compiled C
//! translation unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes p_ceilng.c), so the retention
//! is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! Every function in the module adjudicated dtmc with no extraction:
//! the module draws no `P_Random` bytes at all, and every qualifying
//! body is sector/thinker/static-mut state marshalling whose order is
//! itself demo-visible, so each stays whole in its responsibility
//! file and the module carries no `dtmc.rs`. `T_MoveCeiling`'s
//! `leveltime & 7` movement-sound cadence (suppressed for
//! `silentCrushAndRaise`) rides the sim's own cadence. The three
//! PRE-EXISTING divergences recorded in the mapping table
//! (direction-gating, the crusher down-phase `direction = -1`, and
//! the `silentCrushAndRaise`/`crushAndRaise` missing fall-through
//! configuration) are held bit-exact, never "fixed" during
//! graduation; the F9 state hash does not cover sector heights
//! (harness_hash pins gametic/RNG cursors/mobj position only), so the
//! golden demo tests are the only net under those arms. Adjacent but
//! out-of-module: `p_saveg.rs:1970` allocates the restored
//! `ceiling_t` with `PU_LEVSPEC` where C uses `PU_LEVEL` -- a
//! documented zone-flag divergence observed, NOT normalized here.

pub mod anchor;
pub mod events;
pub mod state;
pub mod thinker;

//* path-stability re-export: the ceiling thinker keeps its module-root
//* path for the freeze-zone consumer (p_saveg).
pub use thinker::T_MoveCeiling;

//* path-stability re-export: the linedef-triggered events and the
//* active-table maintenance keep their module-root paths (p_spec,
//* p_switch, p_saveg).
pub use events::{
    EV_CeilingCrushStop, EV_DoCeiling, P_ActivateInStasisCeiling, P_AddActiveCeiling,
    P_RemoveActiveCeiling,
};

//* path-stability re-export: the layout-pinned ceiling state keeps its
//* module-root paths (p_saveg type/table consumers).
pub use state::{activeceilings, ceiling_t, MAXCEILINGS};

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_Ceilng_Link_Anchor;
