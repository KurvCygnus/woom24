//! Vertical door animation: opening, closing, and key-locked doors,
//! driven by the `T_VerticalDoor` thinker over the `vldoor_t` state
//! and the sector ceiling plane, with the linedef-triggered
//! activation/lock events, the setup-time timed-door spawners, and
//! the locked-door DeHacked message helpers -- bit-exact with
//! `vendor/doomgeneric/p_doors.c`.

//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `repr(C)` `vldoor_t` thinker state with its
//!   compile-time layout checks, the `VDOORSPEED` / `VDOORWAIT`
//!   movement constants, the `vld_*` door-type / `result_*`
//!   return-value / keycard-index constants, the `PD_*` DeHacked
//!   message pointers, the `DEH_String` identity shim, and the two
//!   `locked_*_message` helpers, plus the layout-pinning and
//!   message-map tests
//! - `thinker.rs` -- `T_VerticalDoor`, the per-tic door thinker
//! - `events.rs` -- the linedef-triggered events (`EV_DoDoor`,
//!   `EV_DoLockedDoor`, `EV_VerticalDoor`)
//! - `spawners.rs` -- the setup-time spawners
//!   (`P_SpawnDoorCloseIn30`, `P_SpawnDoorRaiseIn5Mins`)
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic Aspects.
//! The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing consumer
//! path valid (`crate::doom::p_doors::*` across the freeze zone --
//! the `EV_DoDoor` consumers p_enemy, p_spec and p_switch, the
//! `EV_DoLockedDoor` / `EV_VerticalDoor` consumer p_switch, the
//! `vldoor_t` / `T_VerticalDoor` consumer p_saveg, and the
//! `doomgeneric_Create` anchor call); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `T_VerticalDoor` | `thinker::T_VerticalDoor` | dtmc (whole-body; nothing extracts) | per-tic ceiling movement via `T_MovePlane`, direction/countdown machine, `S_StartSound` on the sim cadence; body is pointer marshalling -- nothing extracts; pointer-taken AND address-compared (`p_saveg.rs:341` make_actionf_p1, `:1879` reload compare, `:2003` relink, plus the self-compare in `EV_VerticalDoor` `events.rs` against `T_PlatRaise`) -- no rename, signature pinned; upstream `p_doors.c:57` |
//! | `EV_DoLockedDoor` | `events::EV_DoLockedDoor` | dtmc (whole-body; nothing extracts) | key/message/sound side effects ordered in front of the `EV_DoDoor` delegation -- demo-visible; upstream `p_doors.c:255` |
//! | `EV_DoDoor` | `events::EV_DoDoor` | dtmc (whole-body; nothing extracts) | sector-tag loop allocates thinkers and writes `specialdata`/heights/speeds in statement order; zero draws; pure marshalling; upstream `p_doors.c:199` |
//! | `EV_VerticalDoor` | `events::EV_VerticalDoor` | dtmc (whole-body; nothing extracts) | use-line event including the vanilla plat/door `specialdata` aliasing address-compare quirk (cataloged in `docs/vanilla-workarounds.md`) -- the quirk IS the observable behavior and stays whole; `eprintln!` vs C `fprintf(stderr)` is a pre-existing divergence carried verbatim; upstream `p_doors.c:340` |
//! | `P_SpawnDoorCloseIn30` | `spawners::P_SpawnDoorCloseIn30` | dtmc (whole-body; nothing extracts) | setup-time thinker spawn writing sim state (`topcountdown = 30*TICRATE`); marshalling only; upstream `p_doors.c:519` |
//! | `P_SpawnDoorRaiseIn5Mins` | `spawners::P_SpawnDoorRaiseIn5Mins` | dtmc (whole-body; nothing extracts) | same, plus the `topheight` computation from surrounding ceilings; upstream `p_doors.c:545` |
//! | `P_Doors_Link_Anchor` | `anchor::P_Doors_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `vldoor_t` | `state` | data | layout-pinned (64 bytes: compile-time `layout_checks` plus the layout test moved with them); read field-wise by `p_saveg` through the module root |
//! | `VDOORSPEED` / `VDOORWAIT`, `vld_*` (8), `result_*` (3), card indices (6), `PD_*` messages, `DEH_String` + `locked_*_message` | `state` | data / private helpers | door-type, movement, and message vocabulary (`p_local.h`, `p_doors.c:20-27`, `doomdef.h` `card_t`); the upstream sliding-door family (`p_doors.c:568-778`) is `#if 0` upstream ("abandoned to the mists of time") and correctly absent |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all seven exported functions keep their C symbol and
//! `#[no_mangle]`, which keeps the wasm export surface byte-identical.
//! `T_VerticalDoor` is transmuted into `thinker_t.function.acp1` AND
//! compared by address twice: in `p_saveg` (loaded-save relink) and in
//! `EV_VerticalDoor` against `T_PlatRaise` (imported through the
//! p_plats graduate's root re-export -- the same single function item
//! `EV_DoPlat` stores, per p_plats' one-item-per-name ruling), so its
//! name and exact signature are pinned. No compiled C translation unit
//! references any of these symbols (`doomgeneric-sys/build.rs`
//! excludes p_doors.c), so the retention is pure conservatism (zero
//! wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! Every function in the module adjudicated dtmc with no extraction
//! (zero `P_Random` draws anywhere, verified against `p_doors.c`
//! too): the bodies are interleaved sector/thinker/static-mut state
//! mutation whose statement order is itself demo-visible -- sector
//! ceiling writes via `T_MovePlane`, `specialdata` slot writes,
//! direction/countdown machines, and door sounds on the sim cadence --
//! so each stays whole in its responsibility file (the
//! `M_ClearRandom` / w_wad whole-body precedent). The only arithmetic
//! (`VDOORSPEED * 4`, `topheight -= 4 * FRACUNIT`, `TICRATE * 30`,
//! `5 * 60 * TICRATE`) is inline-constant; nothing qualifies as a
//! genuinely pure computation worth extracting, hence no `dtmc.rs`
//! (mirroring the p_plats ruling). The F9 state hash does not cover
//! sector heights, so the golden demo tests are the net under these
//! writes; no new baseline vectors were required (nothing extracted).

pub mod anchor;
pub mod events;
pub mod spawners;
pub mod state;
pub mod thinker;

//* path-stability re-export: the door thinker keeps its module-root
//* path for the freeze-zone consumers (p_saveg pointer takes and
//* address compares, plus the in-module address compare in events).
pub use thinker::T_VerticalDoor;

//* path-stability re-export: the linedef-triggered events keep their
//* module-root paths (p_enemy, p_spec, p_switch).
pub use events::{EV_DoDoor, EV_DoLockedDoor, EV_VerticalDoor};

//* path-stability re-export: the setup-time spawners keep their
//* module-root paths (p_spec).
pub use spawners::{P_SpawnDoorCloseIn30, P_SpawnDoorRaiseIn5Mins};

//* path-stability re-export: the layout-pinned door state keeps its
//* module-root path (p_saveg type consumer).
pub use state::vldoor_t;

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_Doors_Link_Anchor;
