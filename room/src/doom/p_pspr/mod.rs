//! Weapon sprite animation: the two-slot psprite state machine per
//! player (weapon overlay + muzzle flash), the `P_SetPsprite` runner
//! that invokes the `A_*` action functions stored in the `states`
//! table, the raise/lower/fire orchestration, the hitscan and
//! projectile attacks, and the BFG tracer spray -- bit-exact with
//! `vendor/doomgeneric/p_pspr.c`.

//! ## Submodule Responsibility
//!
//! - `state.rs` -- the `swingx` / `swingy` / `bulletslope` statics
//!   (intercepts-overrun trample targets, `#[no_mangle]` kept) and the
//!   weapon / ammo / button / angle / range constants, plus the
//!   constant-value and static-default tests
//! - `engine.rs` -- `P_SetPsprite` (the table-invoking state runner),
//!   `P_CalcSwing` (dead-but-kept), `P_BringUpWeapon`, `P_CheckAmmo`,
//!   `P_FireWeapon`, `P_DropWeapon`, `P_SetupPsprites`,
//!   `P_MovePsprites`
//! - `actions.rs` -- the draw-free weapon actions (`A_WeaponReady`,
//!   `A_ReFire`, `A_CheckReload`, `A_Lower`, `A_Raise`, `A_GunFlash`,
//!   `A_Light0/1/2`, `A_BFGsound`) and the `DecreaseAmmo` vanilla
//!   ammo-array overflow emulation
//! - `weapons.rs` -- the roll-bearing attacks (`A_Punch`, `A_Saw`,
//!   `A_FirePistol`, `A_FireShotgun`, `A_FireShotgun2`, `A_FireCGun`,
//!   `A_FireMissile`, `A_FirePlasma`, `A_FireBFG`, `A_BFGSpray`) and
//!   the shared hitscan helpers (`P_BulletSlope`, `P_GunShot`), plus
//!   the projectile mobjinfo regression tests
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! `dtmc.rs` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_pspr::*` across the freeze zone -- the info.rs
//! `states` table resolves the 20 `A_*` extern declarations BY SYMBOL,
//! the `A_ReFire` consumer p_enemy, the `P_DropWeapon` consumer
//! p_inter, the `P_MovePsprites` consumer p_user, the
//! `P_SetupPsprites` consumer p_mobj, the `swingx` / `swingy`
//! re-export c_ffi, the `bulletslope` trample writer p_maputl, and the
//! `doomgeneric_Create` anchor call); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_SetPsprite` | `engine::P_SetPsprite` | dtmc (whole-body; nothing extracts) | the psprite state-machine runner; action-chain loop until nonzero tics; transitively runs every `A_*` (the draws) -- marshalling; upstream `p_pspr.c:50` |
//! | `P_CalcSwing` | `engine::P_CalcSwing` | dtmc (whole-body, DEAD) | writes `swingx`/`swingy` from the `leveltime` phase; ZERO callers in both trees and upstream (rg: definition + anchor + c_tests doc comments only; C: only `p_pspr.c:103`) -- vanilla-dead, anchor-kept for C parity (deleting it would shrink the wasm export surface and fail the export-list diff gate); its body is pinned by `baseline_zero_draw_and_swing_pins` |
//! | `P_BringUpWeapon` | `engine::P_BringUpWeapon` | dtmc (whole-body; nothing extracts) | pendingweapon resolution, chainsaw sound, `WEAPONBOTTOM` set; order with `P_SetPsprite`; upstream `p_pspr.c:129` |
//! | `P_CheckAmmo` | `engine::P_CheckAmmo` | dtmc (whole-body; nothing extracts) | weapon-preference chain writes pendingweapon; BFG/SSG count constants; fires `P_SetPsprite`; zero draws; upstream `p_pspr.c:152` |
//! | `P_FireWeapon` | `engine::P_FireWeapon` | dtmc (whole-body; nothing extracts) | attack-state + `P_NoiseAlert` ordering; upstream `p_pspr.c:237` |
//! | `P_DropWeapon` | `engine::P_DropWeapon` | dtmc (whole-body; nothing extracts) | downstate transition (death path; consumer p_inter `P_KillMobj`); upstream `p_pspr.c:256` |
//! | `A_WeaponReady` | `actions::A_WeaponReady` | dtmc (whole-body; nothing extracts) | bob math from `leveltime` (`p_pspr.rs:356-359`), attack-state exits, fire gating; called per-tic via the table; zero draws; upstream `p_pspr.c:273` |
//! | `A_ReFire` | `actions::A_ReFire` | dtmc (whole-body; nothing extracts) | refire counter (feeds `P_GunShot` accuracy later -- cross-tic RNG coupling); CALLED CROSS-MODULE from p_enemy `A_CloseShotgun2` through the root re-export; upstream `p_pspr.c:334` |
//! | `A_CheckReload` | `actions::A_CheckReload` | dtmc (whole-body; nothing extracts) | `P_CheckAmmo` wrapper (C `#if 0` remnant carried); upstream `p_pspr.c:357` |
//! | `A_Lower` / `A_Raise` | `actions` | dtmc (whole-body; nothing extracts) | `LOWERSPEED`/`RAISESPEED` stepping, weapon commit at bottom; upstream `p_pspr.c:376/414` |
//! | `A_GunFlash` | `actions::A_GunFlash` | dtmc (whole-body; nothing extracts) | flash overlay transition; upstream `p_pspr.c:440` |
//! | `A_Punch` | `weapons::A_Punch` | dtmc (whole-body; `melee_roll` + `spread_angle` extracted) | 3 draws (damage, spread pair); berserk ×10 at the site; linetarget turn; upstream `p_pspr.c:459` |
//! | `A_Saw` | `weapons::A_Saw` | dtmc (whole-body; `melee_roll` + `spread_angle` extracted) | 3 draws; the guided-turn angle math (`ANG90/20|21`, edgy wrapping) stays at the site; upstream `p_pspr.c:493` |
//! | `DecreaseAmmo` | `actions::DecreaseAmmo` | dtmc (whole-body) | vanilla ammo-array overflow emulation (`ammonum >= NUMAMMO` indexes `maxammo`, `p_pspr.c:542-552`) -- DeHacked-compat behavior, kept exactly; private then, `pub(super)` now for the `weapons.rs` callers |
//! | `A_FireMissile` / `A_FireBFG` | `weapons` | dtmc (whole-body; nothing extracts) | ammo + projectile spawn; upstream `p_pspr.c:559/572` |
//! | `A_FirePlasma` | `weapons::A_FirePlasma` | dtmc (whole-body; nothing extracts) | 1 draw rides the flashstate argument (`+ (P_Random() & 1)`) -- inline at the site, too small to lift; upstream `p_pspr.c:588` |
//! | `P_BulletSlope` | `weapons::P_BulletSlope` | dtmc (whole-body; nothing extracts) | three `P_AimLineAttack` probes writing the module global; no draws; upstream `p_pspr.c:610` |
//! | `P_GunShot` | `weapons::P_GunShot` | dtmc (whole-body; `gunshot_roll` + `spread_angle` extracted) | 1-3 draws conditional on `accurate`; upstream `p_pspr.c:635` |
//! | `A_FirePistol` / `A_FireShotgun` | `weapons` | dtmc (whole-body; nothing extracts) | ordering of sound/state/ammo/flash/slope/shot; upstream `p_pspr.c:656/679` |
//! | `A_FireShotgun2` | `weapons::A_FireShotgun2` | dtmc (whole-body; `gunshot_roll` + `spread_angle` extracted) | 80 draws across 20 pellets -- the densest RNG consumer in the engine per fire; the SSG slope variant keeps its signed `c_int` shape inline (`(P_Random() - P_Random()) << 5`), NOT routed through the u32 helper; upstream `p_pspr.c:705` |
//! | `A_FireCGun` | `weapons::A_FireCGun` | dtmc (whole-body; nothing extracts) | flash-state pointer arithmetic (`psp->state - &states[S_CHAIN1]`, `p_pspr.c:754-758`) depends on the states table layout -- carried verbatim; zero own draws; upstream `p_pspr.c:742` |
//! | `A_Light0/1/2` | `actions` | dtmc (whole-body; nothing extracts) | extralight writes (render-visible); upstream `p_pspr.c:770/775/780` |
//! | `A_BFGSpray` | `weapons::A_BFGSpray` | dtmc (whole-body; `spray_step` extracted) | up to 600 draws (40 targets x 15); the 15-draw loop stays at the site; upstream `p_pspr.c:790` |
//! | `A_BFGsound` | `actions::A_BFGsound` | dtmc (whole-body; nothing extracts) | sound on sim cadence; upstream `p_pspr.c:827` |
//! | `P_SetupPsprites` / `P_MovePsprites` | `engine` | dtmc (whole-body; nothing extracts) | per-tic psprite clock + slot-1 position copy (`p_pspr.c:860+`); upstream `p_pspr.c:840/860` |
//! | `melee_roll` | `dtmc::melee_roll` | dtmc (extracted) | the `((rand % 10) + 1) << 1` damage roll of `A_Punch`/`A_Saw` (`p_pspr.rs:486/:522`) -- no named C function, so no `#[doc(alias)]` |
//! | `gunshot_roll` | `dtmc::gunshot_roll` | dtmc (extracted) | the `5 * ((rand % 3) + 1)` damage roll of `P_GunShot`/`A_FireShotgun2` (`p_pspr.rs:662/:753`) -- no named C function, so no `#[doc(alias)]` |
//! | `spread_angle` | `dtmc::spread_angle` | dtmc (extracted) | the `((r1 - r2) as u32) << shift` spread pair (`p_pspr.rs:492/:524/:666` shift 18, `:755` shift 19) -- draws stay at the call sites in first/second order; the SSG slope keeps its signed shape inline; no named C function, so no `#[doc(alias)]` |
//! | `spray_step` | `dtmc::spray_step` | dtmc (extracted) | the `acc + (rand & 7) + 1` BFG spray ladder step (`p_pspr.rs:862-865`) -- the 15-draw loop stays at the site; no named C function, so no `#[doc(alias)]` |
//! | `P_Pspr_Link_Anchor` | `anchor::P_Pspr_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive (including dead `P_CalcSwing`) |
//! | `swingx` / `swingy` / `bulletslope` | `state` | data | intercepts-overrun TRAMPLE TARGETS (`p_maputl/intercepts.rs:148-152`: bulletslope is write slot 10, swingx/swingy the skipped slots 11/12 of the emulated BSS order); `pub`, module-root reachable, `c_int`-sized, order pinned -- a reorder would be a p_maputl edit; `#[no_mangle]` kept |
//! | `wp_*` (10), `am_*` (5), `pw_strength`, `BT_ATTACK`, `PST_DEAD`, `ANG90/180`, `MELEERANGE`, `MISSILERANGE`, `DEH_DEFAULT_BFG_CELLS_PER_SHOT` | `state` consts | data | weapon/ammo/angle vocabulary (`doomdef.h`, `p_local.h`, `p_pspr.c:38-42`); values pinned by the moved tests |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! all 30 exported functions, the 3 statics, and the anchor keep
//! their C symbols and `#[no_mangle]`, which keeps the wasm export
//! surface byte-identical. The 20 `A_*` names are function-pointer
//! VALUES inside info.rs's `states` table (extern decls `info.rs:935-984`,
//! `Some(A_*)` across the table) -- name = symbol = pin, the
//! p_plats one-item-per-name rule applied to the biggest table in the
//! engine; touching info.rs to chase a rename is forbidden (freeze
//! zone), and the root re-exports make the question moot. The
//! `A_OpenShotgun2` / `A_LoadShotgun2` / `A_CloseShotgun2` trio lives
//! in p_enemy (same extern block, same table) and is NOT this
//! module's to touch. No compiled C translation unit references any
//! of these symbols (`doomgeneric-sys/build.rs` excludes p_pspr.c),
//! so the retention is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! This module is the engine's densest `P_Random` consumer, and the
//! DRAW ORDER is the demo surface: a fist/saw swing draws exactly 3
//! bytes (damage, then the `(P_Random() - P_Random())` spread pair);
//! a gunshot draws 1 (accurate) or 3 (refire); each SSG pellet draws
//! exactly 5 (damage, spread pair, slope pair) for 20 pellets; each
//! BFG spray hit draws exactly 15 (`(rand & 7) + 1` ladder) and a
//! miss draws 0; the plasma flash draws 1. The swing/gunshot/SSG/BFG
//! counts are pinned against the real bodies by the
//! `baseline_*_draw_pins` vectors in `dtmc.rs`, which were written and
//! run GREEN against the pre-extraction in-file bodies BEFORE the
//! move, then retargeted (same vectors, same results, F10 §2.3). The
//! four extracted helpers take the drawn bytes as arguments and never
//! draw; the plasma flash draw and the SSG slope pair stay inline at
//! their sites (too
//! shape-specific to lift). `A_ReFire`'s refire counter couples the
//! RNG across tics (it gates `P_GunShot` accuracy), so its whole-body
//! adjudication guards the counter's write order. The F9 state hash
//! covers mobj position and the RNG cursors, so the golden demo tests
//! net these draws directly.

pub mod actions;
pub mod anchor;
pub mod dtmc;
pub mod engine;
pub mod state;
pub mod weapons;

//* path-stability re-export: the psprite engine keeps its module-root
//* paths (p_inter, p_user, p_mobj, and the in-module actions).
pub use engine::{
    P_BringUpWeapon, P_CalcSwing, P_CheckAmmo, P_DropWeapon, P_FireWeapon, P_MovePsprites,
    P_SetPsprite, P_SetupPsprites,
};

//* path-stability re-export: the weapon actions keep their
//* module-root paths -- every one of these names is a fn-pointer
//* VALUE in info.rs's states table (resolved by symbol, but the
//* Rust-side paths stay stable too).
pub use actions::{
    A_BFGsound, A_CheckReload, A_GunFlash, A_Light0, A_Light1, A_Light2, A_Lower, A_Raise,
    A_ReFire, A_WeaponReady,
};

//* path-stability re-export: the roll-bearing attacks and the shared
//* hitscan helpers keep their module-root paths (p_enemy A_ReFire
//* caller, p_map, c_tests).
pub use weapons::{
    A_BFGSpray, A_FireBFG, A_FireCGun, A_FireMissile, A_FirePistol, A_FirePlasma, A_FireShotgun,
    A_FireShotgun2, A_Punch, A_Saw, P_BulletSlope, P_GunShot,
};

//* path-stability re-export: the trample-target statics keep their
//* module-root paths (c_ffi re-export, p_maputl trample writer).
pub use state::{bulletslope, swingx, swingy};

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_Pspr_Link_Anchor;
