//! Teleportation action special.
//!
//! Rust port of `vendor/doomgeneric/p_telept.c`.  Handles the `EV_Teleport`
//! linedef special: locating the destination `MT_TELEPORTMAN` marker, moving
//! the triggering thing to that position, and spawning teleport fog at both
//! the source and destination -- bit-exact with the C original.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- THE canonical `mobj_t` mirror of the freeze zone plus
//!   the `line_t` / `sector_t` / `subsector_t` / `mapthing_t` records and
//!   the opaque `state_t` / `mobjinfo_t` / `player_s` aliases, the
//!   `EXE_FINAL` version constant, and the four layout-pinning tests
//! - `teleport.rs` -- `EV_Teleport`, the teleporter event
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]` symbol
//!   alive
//!
//! The module adjudicated with no `dtmc` surface: there is no `dtmc.rs`,
//! and the reasoning is stated under Deterministic Aspects.  The module
//! root is documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_telept::*` across the freeze zone -- the type
//! consumers g_game, harness_hash (the F9 state hash), p_enemy, p_inter,
//! p_map, p_mobj, p_pspr, p_setup, p_sight (whose `pub use` re-export
//! chains through this root), p_spec, p_user, r_interp, r_main, and
//! st_stuff, the `EV_Teleport` consumer p_spec, and the
//! `doomgeneric_Create` anchor call); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `EV_Teleport` | `teleport::EV_Teleport` | dtmc (whole-body; nothing extracts) | pure sim: thinker-list walk in list order against the `P_MobjThinker` pointer, sector tag scan, `P_TeleportMove`, the `EXE_FINAL` `thing->z` quirk (upstream `p_telept.c:99-103`), fog mobj spawns + momentum/angle/reactiontime writes; wholly qualifying, wholly marshalling (the `M_ClearRandom` whole-body precedent); upstream `p_telept.c:42` |
//! | `P_Telept_Link_Anchor` | `anchor::P_Telept_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `mobj_t` | `types` | data | THE canonical freeze-zone mobj mirror (224 bytes, pinned by the moved layout tests); consumed by twelve-plus files including the F9 state hash (`harness_hash.rs:14`, fields hashed at `:50-54`) -- layout drift here would silently corrupt that hash; keeps its module-root path (`r_interp.rs:472/:963` cast `P_MobjThinker` through it; `p_sight.rs:264` re-exports it) |
//! | `line_t` / `sector_t` / `subsector_t` / `mapthing_t` | `types` | data | shared C-mirror vocabulary (`p_local.h` / `doomdata.h`), layout-pinned (sector 128 bytes, subsector 16) by the moved tests |
//! | `state_t` / `mobjinfo_t` / `player_s` | `types` | data | opaque pointer-only aliases (p_mobj casts through them) |
//! | `EXE_FINAL` | `types` const | data | `doomstat.h` `exe_final` (7); gates the Final Doom teleport z quirk |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! both exported functions keep their C symbol and `#[no_mangle]`,
//! which keeps the wasm export surface byte-identical.  `EV_Teleport`'s
//! `P_MobjThinker` address compare (the `acp1` transmute) reaches into
//! p_mobj (not graduating this wave): the import and comparison shape
//! are untouched, and the in-tree `pub use` re-exports keep one function
//! item per name.  No compiled C translation unit references any of
//! these symbols (`doomgeneric-sys/build.rs` excludes p_telept.c), so
//! the retention is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! `EV_Teleport` adjudicated dtmc as a whole body with no extraction:
//! its observable behavior IS the demo synchronization surface for
//! teleporters -- the sector-tag scan order, the thinker-list walk in
//! list order gated by the `P_MobjThinker` pointer compare, the
//! `P_TeleportMove` telefrag, the `EXE_FINAL` (Final Doom, version 7)
//! quirk that skips `thing->z = floorz`, the two fog mobj spawns and
//! their `S_StartSound` calls in tic order, and the
//! momentum/angle/reactiontime writes are interleaved state mutation
//! whose order is itself demo-visible, so nothing separates into a pure
//! `dtmc.rs` computation (the `M_ClearRandom` whole-body precedent).
//! The module draws zero `P_Random` bytes.  No unit fixtures are
//! feasible without map data, so the golden demo tests are the only net
//! under this body (the c2rust-intermediate `p_telept` crate remains
//! the differential reference for future audits).

pub mod anchor;
pub mod teleport;
pub mod types;

//* path-stability re-export: the canonical C-mirror family keeps its
//* module-root paths for the freeze-zone type consumers (g_game,
//* harness_hash, p_enemy, p_inter, p_map, p_mobj, p_pspr, p_setup,
//* p_sight, p_spec, p_user, r_interp, r_main, st_stuff).
pub use types::{line_t, mapthing_t, mobj_t, mobjinfo_t, player_s, sector_t, state_t, subsector_t};

//* path-stability re-export: the teleporter event keeps its module-root
//* path (p_spec).
pub use teleport::EV_Teleport;

//* path-stability re-export: the link anchor keeps its module-root path
//* (doomgeneric_Create).
pub use anchor::P_Telept_Link_Anchor;
