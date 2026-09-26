//! Plane movement, floors, and staircases: the shared `T_MovePlane`
//! height primitive every mover subsystem rides, the `T_MoveFloor`
//! thinker, the `EV_DoFloor` / `EV_BuildStairs` events, and the
//! `repr(C)` `floormove_t` / `side_t` state -- bit-exact with
//! `vendor/doomgeneric/p_floor.c`.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `repr(C)` `floormove_t` thinker state and the
//!   `side_t` partial mirror (read through the module root by
//!   `p_saveg`), the movement/type/return-value constants, the
//!   compile-time layout checks, and the two layout-pinning tests
//! - `mover.rs` -- `T_MovePlane`, the shared plane primitive (its
//!   overshoot/landing decision routes through `dtmc::step_toward`)
//! - `thinker.rs` -- `T_MoveFloor`, the per-tic floor thinker
//! - `events.rs` -- the linedef-triggered events (`EV_DoFloor`,
//!   `EV_BuildStairs`)
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! `dtmc` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_floor::*` across the freeze zone -- the
//! `T_MovePlane` consumers p_ceilng, p_doors, and p_plats; the
//! `EV_DoFloor` consumers p_enemy and p_spec; the p_saveg and p_spec
//! type consumers; and the `doomgeneric_Create` anchor call); no
//! content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `T_MovePlane` | `mover::T_MovePlane` | dtmc (qualifying part extracted) | THE shared plane primitive (floors, plats, ceilings, doors): height math + the paired `P_ChangeSector` call/call-restore pattern, incl. the preserved `#if 0` ceiling-up quirk (`p_floor.rs:273-277` vs `p_floor.c:186-193`) -- crush-restore order is state-visible; the overshoot/landing step decision extracted to `dtmc::step_toward`, every `P_ChangeSector` pair stays at the call site in order; consumers p_ceilng:161, p_plats:176/:222, p_doors:236/:281 keep the module-root path; upstream `p_floor.c:42` |
//! | `T_MoveFloor` | `thinker::T_MoveFloor` | dtmc (whole-body; nothing extracts) | per-tic movement, `leveltime&7` sound cadence (`:310`), texture/special application on arrival (`:320-328`); marshalling; pointer-taken AND address-compared (`p_saveg.rs:1887`) -- no rename, signature pinned; upstream `p_floor.c:202` |
//! | `EV_DoFloor` | `events::EV_DoFloor` | dtmc (whole-body; nothing extracts) | per-type parameter selection is demo-visible (heights, speeds, textures); the raiseToTexture/lowerAndChange scans are order-dependent marshalling; carries two PRE-EXISTING divergences, intentionally NOT fixed and FIXME-carried verbatim: `floor_raiseFloorCrush` sets only `crush = 1` so specials 55/65 never move floors (C falls through into `raiseFloor`, `p_floor.c:317-332`), and the `floor_raiseFloor24` dead assignment; upstream `p_floor.c:251` |
//! | `EV_BuildStairs` | `events::EV_BuildStairs` | dtmc (whole-body; nothing extracts) | sector-walk order and the vanilla quirk "`height += stairsize` even when the next sector is occupied" (`:608-612`) are demo-visible; marshalling; upstream `p_floor.c:444` |
//! | `P_Floor_Link_Anchor` | `anchor::P_Floor_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `step_toward` | `dtmc::step_toward` | dtmc (extracted) | the four overshoot/landing branch decisions of `T_MovePlane` (`p_floor.rs:185`, `:207`, `:238`, `:263`) -- no named C function, so no `#[doc(alias)]` |
//! | `floormove_t` / `side_t` | `state` | data | layout-pinned (64/24 bytes: compile-time `layout_checks` plus the two layout tests moved with them); `side_t` consumed through the module root by `p_saveg.rs:41` only |
//! | `FLOORSPEED`, `result_ok` / `result_crushed` / `result_pastdest`, `floor_e` (13 values), `stair_e` (2 values) | `state` consts | data | movement/type/return-value constants (`p_floor.c:27-40`, `p_local.h`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all four exported functions keep their C symbol and
//! `#[no_mangle]`, which keeps the wasm export surface
//! byte-identical. `T_MoveFloor` is transmuted into
//! `thinker_t.function.acp1` AND compared by address
//! (`p_saveg.rs:1887`), so its name and exact signature are pinned;
//! the in-tree `pub use` re-exports keep one function item per name,
//! which is what keeps the pointer compares valid. No compiled C
//! translation unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes p_floor.c), so the retention
//! is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! The module owns plane movement for the whole specials family:
//! every floor, plat, ceiling, and door motion is a `T_MovePlane`
//! step, so the exact arithmetic and the `P_ChangeSector`
//! call/call-restore order are demo-visible state. The extracted
//! `dtmc::step_toward` is pure integer decision-making; its strict
//! comparisons mean an exact landing is a normal step (the plane
//! reports `result_pastdest` only on the following call) -- pinned by
//! the `==` baseline vectors -- and no `P_ChangeSector` call moved:
//! each branch keeps its call/restore pair at the call site in the
//! original order, including the ceiling-up step branch that performs
//! no crush handling at all (the upstream `#if 0`). `T_MoveFloor`
//! stays whole: its `leveltime & 7` sound cadence and its
//! texture/special writes on arrival ride the sim's own cadence.
//! `EV_DoFloor` / `EV_BuildStairs` stay whole as order-dependent
//! marshalling (tag-scan, texture-height scan, stair sector-walk --
//! including the vanilla quirk that still bumps `height` past an
//! occupied sector). The two PRE-EXISTING `EV_DoFloor` divergences
//! (`floor_raiseFloorCrush` no-op floors for specials 55/65;
//! `floor_raiseFloor24` dead assignment) are held bit-exact, never
//! "fixed" during graduation; the F9 state hash does not cover sector
//! heights (harness_hash pins gametic/RNG cursors/mobj position only),
//! so the golden demo tests are the only net under those arms. The
//! baseline vectors in `dtmc`'s test module were written and run
//! against the original in-file branches BEFORE the extraction and
//! re-run green after (F10 wave A2).

pub mod anchor;
pub mod dtmc;
pub mod events;
pub mod mover;
pub mod state;
pub mod thinker;

//* path-stability re-export: the shared plane primitive keeps its
//* module-root path for the freeze-zone consumers (p_ceilng, p_doors,
//* p_plats) and the floor thinker for p_saveg/p_spec.
pub use mover::T_MovePlane;
pub use thinker::T_MoveFloor;

//* path-stability re-export: the linedef-triggered events keep their
//* module-root paths (p_enemy, p_spec, p_switch).
pub use events::{EV_BuildStairs, EV_DoFloor};

//* path-stability re-export: the layout-pinned mover/side state keeps
//* its module-root paths (p_saveg, p_spec type consumers).
pub use state::{floormove_t, side_t};

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_Floor_Link_Anchor;
