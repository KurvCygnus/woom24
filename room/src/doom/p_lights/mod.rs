//! Sector lighting effects: the four per-tic light animations (fire
//! flicker, random light flash, strobe, smooth glow) driven by
//! thinkers over the sector `lightlevel`, the linedef-triggered light
//! events, and the `repr(C)` `sector_t` / `line_t` partial mirrors
//! shared across the specials family -- bit-exact with
//! `vendor/doomgeneric/p_lights.c`.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the shared vocabulary: the `repr(C)` partial
//!   mirrors of `sector_t` and `line_t`, read by nine other
//!   freeze-zone files through the module root, with the five
//!   layout-pinning tests
//! - `effects.rs` -- the four thinker states, the per-tic `T_*`
//!   updaters, the `P_Spawn*` spawners, and the effect timing
//!   constants
//! - `events.rs` -- the linedef-triggered events (`EV_StartLightStrobing`,
//!   `EV_TurnTagLightsOff`, `EV_LightTurnOn`)
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! `dtmc` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_lights::*` across the freeze zone -- the `sector_t`
//! / `line_t` type consumers p_ceilng, p_doors, p_enemy, p_floor,
//! p_map, p_plats, p_saveg, p_spec, p_switch, and the
//! `doomgeneric_Create` anchor call); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `T_FireFlicker` | `effects::T_FireFlicker` | dtmc (qualifying part extracted) | per-tic `lightlevel` writes in fixed thinker order + one `P_Random` draw (`p_lights.rs:219`); the draw sequence is demo-visible -- amount + clamp extracted to `dtmc::fire_flicker_level`, the draw stays at the call site; pointer-taken AND address-compared (`p_saveg.rs:1903`) -- no rename; upstream `p_lights.c:39` |
//! | `P_SpawnFireFlicker` | `effects::P_SpawnFireFlicker` | dtmc (whole-body; nothing extracts) | joins a thinker into the tick order with the `P_FindMinSurroundingLight` seed; body is marshalling -- nothing pure to separate (w_wad precedent); upstream `p_lights.c:61` |
//! | `T_LightFlash` | `effects::T_LightFlash` | dtmc (qualifying part extracted) | two masked duration draws `(P_Random()&mintime\|maxtime)+1` (`p_lights.rs:282`, `:285`) -- exactly the "sequence of draws is demo-visible" case; mask formula extracted to `dtmc::flash_duration`; address-compared (`p_saveg.rs:1911`); upstream `p_lights.c:91` |
//! | `P_SpawnLightFlash` | `effects::P_SpawnLightFlash` | dtmc (qualifying part extracted) | initial count is a `P_Random` draw (`p_lights.rs:316`, routed through `dtmc::flash_duration`); rest is marshalling; upstream `p_lights.c:117` |
//! | `T_StrobeFlash` | `effects::T_StrobeFlash` | dtmc (whole-body; nothing extracts) | deterministic two-phase `lightlevel` state machine, no RNG -- the whole function is the surface, no pure part (`M_ClearRandom` precedent); address-compared (`p_saveg.rs:1919`); upstream `p_lights.c:148` |
//! | `P_SpawnStrobeFlash` | `effects::P_SpawnStrobeFlash` | dtmc (qualifying part extracted) | `(P_Random()&7)+1` sync draw (`p_lights.rs:383`, routed through `dtmc::flash_duration`) + spawn-state marshalling; upstream `p_lights.c:174` |
//! | `EV_StartLightStrobing` | `events::EV_StartLightStrobing` | dtmc (whole-body; nothing extracts) | tag-scan order fixes spawn order -> tick order; marshalling only; upstream `p_lights.c:208` |
//! | `EV_TurnTagLightsOff` | `events::EV_TurnTagLightsOff` | dtmc (whole-body; nothing extracts) | bulk `lightlevel` writes; neighbor-scan order observable in state; upstream `p_lights.c:229` |
//! | `EV_LightTurnOn` | `events::EV_LightTurnOn` | dtmc (whole-body; nothing extracts) | bulk `lightlevel` writes; `bright == 0` neighbor-max search is order-dependent; upstream `p_lights.c:264` |
//! | `T_Glow` | `effects::T_Glow` | dtmc (whole-body; nothing extracts) | deterministic oscillation of `lightlevel`; no extraction; upstream `p_lights.c:307` |
//! | `P_SpawnGlowingLight` | `effects::P_SpawnGlowingLight` | dtmc (whole-body; nothing extracts) | spawn-state marshalling; no RNG; upstream `p_lights.c:334` |
//! | `P_Lights_Link_Anchor` | `anchor::P_Lights_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `flash_duration` | `dtmc::flash_duration` | dtmc (extracted) | the `(rand & mask) + 1` masked-duration expression behind the `:282` / `:285` / `:316` / `:383` draws -- no named C function, so no `#[doc(alias)]` |
//! | `fire_flicker_level` | `dtmc::fire_flicker_level` | dtmc (extracted) | the `(P_Random()&3)*16` amount + minlight clamp of `T_FireFlicker` (`:219`, `:222-226`) -- no named C function, so no `#[doc(alias)]` |
//! | `sector_t` / `line_t` (partial mirrors) | `types::sector_t` / `types::line_t` | data | layout-pinned shared vocabulary: nine freeze-zone files read them through the module root; the five layout tests moved with them |
//! | `fireflicker_t` / `lightflash_t` / `strobe_t` / `glow_t` | `effects` | data | layout-pinned thinker states (48/56/56/48 bytes; size tests live in `types.rs`) |
//! | `GLOWSPEED` / `STROBEBRIGHT` / `FASTDARK` / `SLOWDARK` | `effects` consts | data | effect timing constants (`p_lights.c:25-27` region); `FASTDARK` has no in-tree Rust reader (`p_spec` reads the `c_ffi` twin at `p_spec.rs:1424-1436`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all twelve exported functions keep their C symbol and
//! `#[no_mangle]`, which keeps the wasm export surface
//! byte-identical. The four `T_*` thinkers are transmuted into
//! `thinker_t.function.acp1` AND compared by address in
//! `p_saveg.rs:1903` / `:1911` / `:1919`, so their names and exact
//! signatures are pinned; the in-tree `pub use` re-exports keep one
//! function item per name, which is what keeps the pointer compares
//! valid. No compiled C translation unit references any of these
//! symbols (`doomgeneric-sys/build.rs` excludes p_lights.c), so the
//! retention is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! The module owns the sector-`lightlevel` half of the demo
//! surface: every effect writes `lightlevel` in the fixed thinker
//! order of `P_RunThinkers`, and four functions consume `P_Random`
//! bytes mid-tic. The extracted `dtmc::{fire_flicker_level,
//! flash_duration}` are pure integer computations; the draws
//! themselves stay at their original call sites in the original
//! order -- exactly one byte per fired tic in `T_FireFlicker` /
//! `T_LightFlash`, the spawn draws of `P_SpawnLightFlash` /
//! `P_SpawnStrobeFlash` interleaved with thinker registration in the
//! original statement order -- so the demo-visible draw sequence is
//! unchanged. `T_StrobeFlash` and `T_Glow` draw nothing (pinned by a
//! zero-draw baseline vector); the `EV_*` events and remaining
//! spawners draw nothing and stay whole in their responsibility
//! files as marshalling whose scan order is itself demo-visible.
//! Two load-bearing details the extractions pinned: the
//! `fire_flicker_level` clamp predicate reads the CURRENT sector
//! lightlevel (not `maxlight` -- vector-discriminated), and the
//! `lightflash` bright/dark edges mask with different thinkers
//! fields (`maxtime` vs `mintime` -- vector-discriminated). The
//! baseline vectors in `dtmc`'s test module were written and run
//! against the original in-file bodies BEFORE the extraction and
//! re-run green after (F10 wave A2). Note the F9 state hash does not
//! cover `lightlevel` (harness_hash pins gametic/RNG
//! cursors/mobj position only) -- the golden demo tests are the only
//! net under these writes.

pub mod anchor;
pub mod dtmc;
pub mod effects;
pub mod events;
pub mod types;

//* path-stability re-export: the shared sector/line mirrors keep
//* their module-root paths for the nine freeze-zone type consumers
//* (p_ceilng, p_doors, p_enemy, p_floor, p_map, p_plats, p_saveg,
//* p_spec, p_switch).
pub use types::{line_t, sector_t};

//* path-stability re-export: the effect thinkers, spawners, and
//* thinker states keep their module-root paths (p_spec, p_saveg,
//* doomgeneric).
pub use effects::{
    fireflicker_t, glow_t, lightflash_t, strobe_t, P_SpawnFireFlicker, P_SpawnGlowingLight,
    P_SpawnLightFlash, P_SpawnStrobeFlash, T_FireFlicker, T_Glow, T_LightFlash, T_StrobeFlash,
};

//* path-stability re-export: the linedef-triggered events keep their
//* module-root paths (p_spec, p_switch).
pub use events::{EV_LightTurnOn, EV_StartLightStrobing, EV_TurnTagLightsOff};

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_Lights_Link_Anchor;
