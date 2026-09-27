//! Switch and button logic ported from `vendor/doomgeneric/p_switch.c`.
//!
//! Manages the switch texture table (`P_InitSwitchList`), timed button
//! reset (`button_t`, `P_StartButton`), switch texture toggling
//! (`P_ChangeSwitchTexture`), and linedef-use dispatch for all switch and
//! button specials (`P_UseSpecialLine`) -- bit-exact with the C original.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `repr(C)` `button_t` record with its compile-time
//!   layout checks and the layout-pinning test, the `SwitchDef` table
//!   mirroring `alphSwitchList[]`, the `switchlist` / `numswitches` /
//!   `buttonlist` statics, and the dispatch/type constants (`MAXSWITCHES`,
//!   `MAXBUTTONS`, `BUTTONTIME`, the `top`/`middle`/`bottom` slots and the
//!   `vld_*` / `floor_*` / `ceiling_*` / `plat_*` / `stair_*` enum values)
//! - `switches.rs` -- `P_InitSwitchList` (the setup-time table build),
//!   `P_StartButton`, and `P_ChangeSwitchTexture`
//! - `use_lines.rs` -- `P_UseSpecialLine`, the switch/button special
//!   dispatch
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]` symbol
//!   alive
//!
//! The module adjudicated with no `dtmc` surface: there is no `dtmc.rs`,
//! and the reasoning is stated under Deterministic Aspects.  The module
//! root is documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_switch::*` across the freeze zone -- the
//! `P_UseSpecialLine` consumers p_map and p_enemy, the
//! `P_InitSwitchList` consumer p_setup, the `buttonlist` / `MAXBUTTONS` /
//! `P_ChangeSwitchTexture` consumers in p_spec, the `doomgeneric_Create`
//! anchor call, and the wave's only c_tests consumer
//! `c_tests/p_switch_c.rs`, which reads the constants and statics by
//! module-root path); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_InitSwitchList` | `switches::P_InitSwitchList` | glue | setup-time build of a lookup table from the static `ALPH_SWITCH_LIST` (runs once at `P_SetupLevel`, `p_setup.rs:1335`); no per-tic observable of its own -- the table it fills is data; carries the in-code FIXME that C wraps each texture name in `DEH_String()` (`FEATURE_DEHACKED` disabled in this port); upstream `p_switch.c:101` |
//! | `P_StartButton` | `switches::P_StartButton` | dtmc (whole-body; nothing extracts) | `buttonlist` slot state feeds the per-tic `P_UpdateSpecials` countdown (`p_spec/ticker.rs:91`), and the already-pressed no-op gate changes the texture/sound sequence; the `MAXBUTTONS` overflow stays a fatal `I_Error` exactly as in C; upstream `p_switch.c:149` |
//! | `P_ChangeSwitchTexture` | `switches::P_ChangeSwitchTexture` | dtmc (whole-body; nothing extracts) | clears `line->special` when one-shot (sim state), flips textures via the `switchlist[i ^ 1]` partner rule -- a data index, not a pure computation to extract -- exit-11 sound branch, queues timed buttons; upstream `p_switch.c:195` |
//! | `P_UseSpecialLine` | `use_lines::P_UseSpecialLine` | dtmc (whole-body; nothing extracts) | the dispatch IS the surface: gates which `EV_*` fire mid-sim; the return value is consumed by `p_map.rs:1270` (`PTR_UseTraverse`) and the spechit use-emulation (`p_enemy/chase.rs:move_step`); monster gating on `SECRET` + specials 1/32/33/34; upstream `p_switch.c:270` |
//! | `P_Switch_Link_Anchor` | `anchor::P_Switch_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `button_t` / `switchlist` / `numswitches` / `buttonlist` | `state` | data | layout-pinned (32-byte `button_t`, `soundorg` at 24: compile-time `layout_checks` plus the layout test moved with them); the statics keep their `#[no_mangle]` C symbols; `buttonlist` is consumed field-wise by `p_spec/ticker.rs:91` through the module root |
//! | `SwitchDef` / `ALPH_SWITCH_LIST` | `state` | data | the built-in switch-texture table (`switchlist_t` / `alphSwitchList[]`, `p_switch.c:39-95`) |
//! | `MAXSWITCHES` / `MAXBUTTONS` / `BUTTONTIME`, `top`/`middle`/`bottom`, `vld_*`, `floor_*`, `ceiling_*`, `plat_*`, `stair_*` | `state` consts | data | dispatch/type constants (`doomdef.h`, `p_local.h`); the duplicated `floor_*`/`ceiling_*`/`plat_*`/`stair_*` sets in `p_spec/consts.rs:41` must NOT be unified during graduation -- value identity with the owning modules' constants is exactly what keeps the dispatch arms auditable against the C |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all four exported functions keep their C symbol and `#[no_mangle]`,
//! which keeps the wasm export surface byte-identical (the
//! `switchlist` / `numswitches` / `buttonlist` statics keep their
//! `#[no_mangle]` too; the c_tests reach them by path).  No compiled C
//! translation unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes p_switch.c), so the retention
//! is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! Three of the four dispatch functions adjudicated dtmc with no
//! extraction (`P_StartButton`, `P_ChangeSwitchTexture`,
//! `P_UseSpecialLine`): their bodies are interleaved static-mut and
//! linedef/sector state mutation whose order is itself demo-visible, so
//! each stays whole in its responsibility file (the `M_ClearRandom` /
//! whole-body precedents).  `P_InitSwitchList` is glue: a setup-time
//! table build with no per-tic observable of its own.  The module draws
//! zero `P_Random` bytes, and the one non-trivial rule -- the
//! `switchlist[i ^ 1]` texture partner -- is a data index, not a pure
//! computation, so there is nothing to extract into a `dtmc.rs`.  The
//! `buttonlist` table is consumed per-tic by the freeze-zone
//! `P_UpdateSpecials` button countdown (`p_spec/ticker.rs:91`), which
//! keeps reading the statics through the module root; the 32-byte
//! `button_t` layout (sound origin at offset 24) is load-bearing there
//! and is pinned by the tests that moved with the struct.  The golden
//! demo tests are the net under the dtmc bodies: the dispatch itself has
//! no unit fixtures without map data, and the c_tests checks pin the
//! constants/globals data surface (`MAXSWITCHES`, `MAXBUTTONS`,
//! `BUTTONTIME`, array sizes and zero-initialization).

pub mod anchor;
pub mod state;
pub mod switches;
pub mod use_lines;

//* path-stability re-export: the switch constants, the layout-pinned
//* button state, and the switch-table statics keep their module-root
//* paths (p_spec type/table consumers and c_tests/p_switch_c.rs).
pub use state::{button_t, buttonlist, numswitches, switchlist, BUTTONTIME, MAXBUTTONS, MAXSWITCHES};

//* path-stability re-export: the switch mechanics keep their module-root
//* paths (p_setup, p_spec).
pub use switches::{P_ChangeSwitchTexture, P_InitSwitchList, P_StartButton};

//* path-stability re-export: the use-dispatch keeps its module-root path
//* (p_map, p_enemy).
pub use use_lines::P_UseSpecialLine;

//* path-stability re-export: the link anchor keeps its module-root path
//* (doomgeneric_Create).
pub use anchor::P_Switch_Link_Anchor;
