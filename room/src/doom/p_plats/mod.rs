//! Elevator platforms: raising and lowering floor sectors, driven by
//! the `T_PlatRaise` thinker over the `plat_t` state and the
//! fixed-size `activeplats` table, with the linedef-triggered
//! activation/suspension events -- bit-exact with
//! `vendor/doomgeneric/p_plats.c`.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `repr(C)` `plat_t` thinker state, the
//!   `activeplats` table, the movement/type/return-value constants,
//!   the compile-time layout checks, and the layout-pinning test
//! - `thinker.rs` -- `T_PlatRaise`, the per-tic plat thinker
//! - `events.rs` -- the linedef-triggered events (`EV_DoPlat`,
//!   `EV_StopPlat`) and the active-table maintenance
//!   (`P_ActivateInStasis`, `P_AddActivePlat`, `P_RemoveActivePlat`)
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic Aspects.
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing consumer
//! path valid (`crate::doom::p_plats::*` across the freeze zone --
//! the `T_PlatRaise`/`plat_t` consumers p_doors and p_saveg, the
//! `EV_DoPlat` consumers p_spec and p_switch, and the
//! `doomgeneric_Create` anchor call); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `T_PlatRaise` | `thinker::T_PlatRaise` | dtmc (whole-body; nothing extracts) | per-tic floor movement via `T_MovePlane`, wait counters, direction flips, `P_RemoveActivePlat` on one-shot arrival; sound rides the sim cadence (`leveltime&7`); pointer-taken AND address-compared (`p_saveg.rs:1895` AND the vanilla `EV_VerticalDoor` "wasn't a door" quirk branch `p_doors.rs:522-533`) -- no rename, signature pinned; upstream `p_plats.c:45` |
//! | `EV_DoPlat` | `events::EV_DoPlat` | dtmc (whole-body; nothing extracts) | spawner in the demo surface: per-type parameter selection, sector writes, and ONE `P_Random()&1` draw (`p_plats.rs:380`) fixing the perpetualRaise initial direction -- statement order preserved verbatim, and the draw itself belongs to m_random's already-graduated surface; upstream `p_plats.c:129` |
//! | `P_ActivateInStasis` | `events::P_ActivateInStasis` | dtmc (whole-body; nothing extracts) | resumes in-stasis plats (restores `acp1` + `status`) -- tick-order observable; upstream `p_plats.c:248` |
//! | `EV_StopPlat` | `events::EV_StopPlat` | dtmc (whole-body; nothing extracts) | sets `acv = None` stasis -- suppresses a thinker from the tick order; upstream `p_plats.c:263` |
//! | `P_AddActivePlat` | `events::P_AddActivePlat` | dtmc (whole-body; nothing extracts) | writes the `activeplats` slot (sim state) including the fatal `I_Error` overflow path -- never softened; upstream `p_plats.c:278` |
//! | `P_RemoveActivePlat` | `events::P_RemoveActivePlat` | dtmc (whole-body; nothing extracts) | clears `specialdata` + removes the thinker -- direct sim state; upstream `p_plats.c:291` |
//! | `P_Plats_Link_Anchor` | `anchor::P_Plats_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `plat_t` / `activeplats` | `state` | data | layout-pinned (72 bytes: compile-time `layout_checks` plus the layout test moved with them); `plat_t` read field-wise by `p_saveg`/`p_doors` through the module root; `activeplats` keeps its `#[no_mangle]` C symbol |
//! | `PLATSPEED` / `PLATWAIT` / `MAXPLATS`, `result_*`, `up` / `down` / `waiting` / `in_stasis`, `plattype_e` values | `state` consts | data | movement/type/return-value constants (`p_plats.c:27-40`, `p_local.h`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all seven exported functions keep their C symbol and
//! `#[no_mangle]`, which keeps the wasm export surface byte-identical
//! (the `activeplats` static keeps its `#[no_mangle]` too).
//! `T_PlatRaise` is transmuted into `thinker_t.function.acp1` AND
//! compared by address in `p_saveg.rs:1895` and in the
//! `p_doors.rs:522-533` vanilla `EV_VerticalDoor` quirk branch (a
//! `sector->specialdata` slot can hold a plat), so its name and exact
//! signature are pinned; the in-tree `pub use` re-exports keep one
//! function item per name, which is what keeps the pointer compares
//! valid. No compiled C translation unit references any of these
//! symbols (`doomgeneric-sys/build.rs` excludes p_plats.c), so the
//! retention is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! Every function in the module adjudicated dtmc with no extraction:
//! the bodies are interleaved sector/thinker/static-mut state
//! mutation whose order is itself demo-visible, so each stays whole
//! in its responsibility file (the `M_ClearRandom` / w_wad
//! whole-body precedents). The one qualifying computation is
//! `EV_DoPlat`'s single `P_Random() & 1` draw selecting the
//! `perpetualRaise` initial direction (`p_plats.rs:380`); the draw
//! sequence is demo-visible, its position inside the spawn loop's
//! statement order is preserved exactly, and the RNG cursor it feeds
//! belongs to m_random's already-graduated surface -- hence no
//! `dtmc.rs` for this module. `T_PlatRaise`'s `leveltime & 7` sound
//! cadence and its `activeplats` bookkeeping ride the sim's own
//! cadence; `MAXPLATS` overflow stays a fatal `I_Error` exactly as in
//! C. The F9 state hash does not cover sector heights (harness_hash
//! pins gametic/RNG cursors/mobj position only), so the golden demo
//! tests are the net under these writes.

pub mod anchor;
pub mod events;
pub mod state;
pub mod thinker;

//* path-stability re-export: the plat thinker keeps its module-root
//* path for the freeze-zone consumers (p_doors, p_saveg).
pub use thinker::T_PlatRaise;

//* path-stability re-export: the linedef-triggered events and the
//* active-table maintenance keep their module-root paths (p_spec,
//* p_switch, p_saveg).
pub use events::{
    EV_DoPlat, EV_StopPlat, P_ActivateInStasis, P_AddActivePlat, P_RemoveActivePlat,
};

//* path-stability re-export: the layout-pinned plat state keeps its
//* module-root paths (p_saveg, p_doors type consumers).
pub use state::{activeplats, plat_t};

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_Plats_Link_Anchor;
