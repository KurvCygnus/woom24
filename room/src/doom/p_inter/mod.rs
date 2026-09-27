//! Player/item interactions: the pickup helpers (`P_GiveAmmo`,
//! `P_GiveWeapon`, ...), the item-pickup dispatcher
//! (`P_TouchSpecialThing`), kill accounting (`P_KillMobj`), and damage
//! application with armor absorption and knockback (`P_DamageMobj`),
//! over the ammo tables and the runtime DEHacked-tunable globals --
//! bit-exact with `vendor/doomgeneric/p_inter.c`.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the numeric constants (`BONUSADD`, skill/power/
//!   weapon/ammo/card indices, power durations), the `GOT*` pickup
//!   message pointers, the `DEH_DEFAULT_*` fallbacks, and the
//!   `DEH_String` identity shim
//! - `state.rs` -- the `maxammo` / `clipammo` ammo tables and the
//!   seven runtime `deh_*` statics (all `#[no_mangle]`), plus their
//!   default-value tests
//! - `give.rs` -- `P_GiveAmmo` / `P_GiveWeapon` / `P_GiveBody` /
//!   `P_GiveArmor` / `P_GiveCard` / `P_GivePower` with their
//!   `p_give_*` `&mut PlayerT` bodies
//! - `touch.rs` -- `P_TouchSpecialThing`, the pickup dispatcher
//! - `damage.rs` -- `P_KillMobj` and `P_DamageMobj`
//! - `anchor.rs` -- the link anchor keeping the `#[no_mangle]` set
//!   alive
//!
//! `dtmc.rs` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_inter::*` across the freeze zone -- the
//! `P_DamageMobj` consumers p_map, p_enemy, p_spec and p_pspr, the
//! `P_TouchSpecialThing` consumer p_map, the `P_GivePower` consumer
//! st_stuff, the `maxammo` consumer g_game, the c_tests root paths,
//! and the never-wired-but-kept anchor); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_GiveAmmo` / `p_give_ammo` | `give::P_GiveAmmo` / `give::p_give_ammo` | dtmc (whole-body; nothing extracts) | mutates player ammo/maxammo/pendingweapon during the pickup tick; zero draws; the `p_give_*` bodies are the pre-existing Rust-side `&mut PlayerT` split (kept, not renamed); upstream `p_inter.c:66` |
//! | `P_GiveWeapon` / `p_give_weapon` | `give::P_GiveWeapon` / `give::p_give_weapon` | dtmc (whole-body; nothing extracts) | netgame/deathmatch weapon ownership + console-player sound branch; sim state writes; upstream `p_inter.c:160` |
//! | `P_GiveBody` / `p_give_body` | `give::P_GiveBody` / `give::p_give_body` | dtmc (whole-body; nothing extracts) | health + mirrored mobj health; trivial cap; nothing pure enough to lift; upstream `p_inter.c:223` |
//! | `P_GiveArmor` / `p_give_armor` | `give::P_GiveArmor` / `give::p_give_armor` | dtmc (whole-body; nothing extracts) | armorpoints/armortype writes; the `armortype * 100` cap is inline trivial arithmetic; upstream `p_inter.c:246` |
//! | `P_GiveCard` / `p_give_card` | `give::P_GiveCard` / `give::p_give_card` | dtmc (whole-body; nothing extracts) | cards + bonuscount writes; upstream `p_inter.c:268` |
//! | `P_GivePower` | `give::P_GivePower` | dtmc (whole-body; nothing extracts) | power timers, `MF_SHADOW`, the berserk -> `P_GiveBody` chain; ordering with the pickup flow is demo-visible; upstream `p_inter.c:284` |
//! | `P_TouchSpecialThing` | `touch::P_TouchSpecialThing` | dtmc (whole-body; nothing extracts) | the pickup dispatcher: sprite match mutates all player surfaces, removes the mobj, plays the sound; body is marshalling by design (raw-pointer reads up front, and the deliberate `deh_*` statics snapshot carried verbatim -- do not re-read the statics inside arms); upstream `p_inter.c:333` |
//! | `P_KillMobj` | `damage::P_KillMobj` | dtmc (whole-body; `death_tic_roll` extracted) | kill/frag accounting (the `offset_from` frags idiom carried verbatim), corpse flags, death state, drop-spawn; the single draw rides `dtmc::death_tic_roll` at the original statement position; upstream `p_inter.c:666` |
//! | `P_DamageMobj` | `damage::P_DamageMobj` | dtmc (whole-body; `thrust_for` / `fall_forward_flip` / `armor_absorption` extracted) | the widest sim entry in the wave: thrust math, hell-hack cap, god/invul gate, armor absorption, pain/target acquisition -- statement order is the demo surface; the painchance draw (`p_inter.c:898-899`) stays at the call site, evaluated BEFORE the `MF_SKULLFLY` compare; upstream `p_inter.c:779` |
//! | `armor_absorption` | `dtmc::armor_absorption` | dtmc (extracted) | the `damage/3 \| damage/2` split + exhaustion clamp of `P_DamageMobj` (`p_inter.rs:1229-1242`, upstream `p_inter.c:858-873`) -- no named C function, so no `#[doc(alias)]` |
//! | `fall_forward_flip` | `dtmc::fall_forward_flip` | dtmc (extracted) | the four-arm predicate at `p_inter.rs:1203-1207` EXCLUDING the draw (rand passed in; upstream `p_inter.c:824-831`) -- the caller keeps the three cheap arms ahead of the call in its `&&` chain so the draw only fires when the guard would draw; no named C function, so no `#[doc(alias)]` |
//! | `thrust_for` | `dtmc::thrust_for` | dtmc (extracted) | the `damage * (FRACUNIT >> 3) * 100 / mass` knockback magnitude (`p_inter.rs:1201`, upstream `p_inter.c:821`) -- truncating division, no mass-0 guard to add; no named C function, so no `#[doc(alias)]` |
//! | `death_tic_roll` | `dtmc::death_tic_roll` | dtmc (extracted) | the `(rand & 3)` subtract + `< 1` clamp of `P_KillMobj` (`p_inter.rs:1116-1119`, upstream `p_inter.c:724-727`); no named C function, so no `#[doc(alias)]` |
//! | `P_Inter_Link_Anchor` | `anchor::P_Inter_Link_Anchor` | glue | link scaffolding, never runs in sim; carried verbatim including its NEVER-CALLED status: `doomgeneric_Create`'s anchor list does not call it (pre-existing gap, kept for anchor parity -- every symbol is Rust-reachable through p_map/p_enemy/p_spec/st_stuff/g_game, so retention is not at risk) |
//! | `maxammo` / `clipammo` | `state` | data | ammo tables mutated in place by the pickup flow (a backpack doubles `maxammo` in place); `#[no_mangle]` C symbols kept; the c_tests (`p_inter_c.rs`, `lookup_tables.rs`) read them through the module-root re-export |
//! | `deh_max_health` ... `deh_megasphere_health` (7) | `state` | data | runtime DEHacked-tunable globals; no in-tree consumers (DEH loader not ported) -- C-parity retention only, `#[no_mangle]` kept (wasm export diff gate) |
//! | consts, `GOT*` messages, `DEH_DEFAULT_*`, `DEH_String` | `consts` | data / private helpers | pickup/damage vocabulary (`p_inter.c:43-51`, headers); values pinned by the moved `constants_match` / `deh_defaults_match` tests |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! the nine exported functions, the nine `#[no_mangle]` statics, and
//! the anchor keep their C symbols, which keeps the wasm export
//! surface byte-identical. `P_DamageMobj` has 18 call sites across
//! p_map / p_enemy / p_spec / p_pspr -- all resolved through the root
//! re-export, untouched. No function-pointer takers or address
//! compares reach into p_inter. The mutual import with p_pspr
//! (`P_DropWeapon` <-> `P_DamageMobj`) resolves through both modules'
//! root re-exports. No compiled C translation unit references any of
//! these symbols (`doomgeneric-sys/build.rs` excludes p_inter.c), so
//! the retention is pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! The module draws exactly three `P_Random` bytes across `P_DamageMobj`
//! and `P_KillMobj` territory, and their SHORT-CIRCUIT ORDER is the
//! demo surface: (1) the kill roll `P_KillMobj` consumes exactly one
//! byte per kill; (2) the fall-forward guard draws ONLY when
//! `damage < 40 && damage > target.health && z_delta > 64*FRACUNIT`
//! all pass -- reordering the predicate arms changes how many RNG
//! bytes a tic consumes; (3) the painchance probe `P_Random() <
//! painchance` evaluates the draw BEFORE the `MF_SKULLFLY` compare,
//! so a surviving skull-fly target still consumes its byte. All three
//! properties are pinned against the real body by the
//! `baseline_*_draw_pins` vectors in `dtmc.rs` (written and run GREEN
//! against the pre-extraction in-file bodies BEFORE the move, then
//! retargeted -- same vectors, same results, F10 §2.3). The four
//! extracted helpers take the drawn byte (or no byte) as an argument;
//! none of them draws. The `deh_*` statics snapshot at the top of
//! `P_TouchSpecialThing` is deliberate statics-aliasing hygiene and
//! is carried verbatim. The F9 state hash covers mobj position and
//! the RNG cursors, so the golden demo tests net these draws
//! directly.

pub mod anchor;
pub mod consts;
pub mod damage;
pub mod dtmc;
pub mod give;
pub mod state;
pub mod touch;

//* path-stability re-export: the six pickup grants keep their
//* module-root paths (st_stuff, touch).
pub use give::{
    P_GiveAmmo, P_GiveArmor, P_GiveBody, P_GiveCard, P_GivePower, P_GiveWeapon,
};

//* path-stability re-export: the pickup dispatcher keeps its
//* module-root path (p_map).
pub use touch::P_TouchSpecialThing;

//* path-stability re-export: kill accounting and damage application
//* keep their module-root paths (p_map, p_enemy, p_spec, p_pspr --
//* the widest sim entry of this wave).
pub use damage::{P_DamageMobj, P_KillMobj};

//* path-stability re-export: the ammo tables and the runtime deh_*
//* globals keep their module-root paths (g_game, touch, c_tests).
pub use state::{
    clipammo, deh_blue_armor_class, deh_green_armor_class, deh_max_armor, deh_max_health,
    deh_max_soulsphere, deh_megasphere_health, deh_soulsphere_health, maxammo,
};

//* path-stability re-export: the link anchor keeps its module-root
//* path (never called from doomgeneric_Create; kept for anchor
//* parity -- see the mapping table).
pub use anchor::P_Inter_Link_Anchor;
