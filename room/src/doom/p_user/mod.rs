//! Player movement, view, and weapon logic: the per-tic player
//! thinkers -- `P_PlayerThink` orchestrator, `P_MovePlayer`, the
//! `P_CalcHeight` view-bob path, `P_DeathThink` death camera, and the
//! `P_Thrust` impulse -- over the shared `onground` global and the
//! button/weapon/power/state constants -- bit-exact with
//! `vendor/doomgeneric/p_user.c`.

//! ## Submodule Responsibility
//!
//! - `state.rs` -- the shared `onground` global and the `BT_*` /
//!   `wp_*` / `pw_*` / `PST_*` / `MAXBOB` / `VIEWHEIGHT` / `ANG5` /
//!   `INVERSECOLORMAP` constants, plus the constant-value, `onground`
//!   default, and `PspdefT` layout tests
//! - `movement.rs` -- `P_Thrust`, `P_MovePlayer`, `P_DeathThink`
//! - `view.rs` -- `P_CalcHeight`
//! - `think.rs` -- `P_PlayerThink`
//! - `anchor.rs` -- the link anchor keeping every `#[no_mangle]`
//!   symbol alive
//!
//! `dtmc.rs` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_user::*` across the freeze zone -- the
//! `P_PlayerThink` consumer p_tick/ticker.rs and the
//! `doomgeneric_Create` anchor call); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_Thrust` | `movement::P_Thrust` | dtmc (whole-body; nothing extracts) | momentum writes via the fine-angle tables, `FixedMul` + `>>ANGLETOFINESHIFT` -- exactness sensitive, nothing extractable beyond the two additions; upstream `p_user.c:52` |
//! | `P_CalcHeight` | `view::P_CalcHeight` | dtmc (two helpers extracted) | bob amplitude/phase feed `viewz` (a render-interp surface, `r_interp.rs:786` context); the `viewheight` step machine and the viewz writes stay at the call site -- including the vanilla double write in the `CF_NOMOMENTUM`/airborne branch (`p_user.c:92-97`), carried VERBATIM and pinned by a baseline vector; upstream `p_user.c:70` |
//! | `P_MovePlayer` | `movement::P_MovePlayer` | dtmc (whole-body; nothing extracts) | `cmd` consumption, the `onground` global write, run-state transition; order inside `P_Ticker`'s player loop is the demo surface; upstream `p_user.c:141` |
//! | `P_DeathThink` | `movement::P_DeathThink` | dtmc (whole-body; nothing extracts) | attacker-turn arithmetic uses `wrapping_sub` + the `(!ANG5).wrapping_add(1)` unsigned-negation trick (C `(unsigned)-ANG5`, `p_user.c:157` region) -- the edgy behavior is the body; upstream `p_user.c:175` |
//! | `P_PlayerThink` | `think::P_PlayerThink` | dtmc (whole-body; nothing extracts) | the player-side sim orchestrator (chainsaw cmd override, teleport reactiontime gate, weapon-switch selection, power timers, colormap); in-place `cmd` field writes are consumed ticcmd state -- keep field-write order; upstream `p_user.c:229` |
//! | `bob_amplitude` | `dtmc::bob_amplitude` | dtmc (extracted) | the `FixedMul`-square, `>> 2`, `MAXBOB`-clamp of `P_CalcHeight` (`p_user.rs:166-172`, upstream `p_user.c:81-88`) -- no named C function, so no `#[doc(alias)]` |
//! | `bob_phase` | `dtmc::bob_phase` | dtmc (extracted) | the `(FINEANGLES/20 * leveltime) & (FINEANGLES - 1)` finetable index (`p_user.rs:185-186`, upstream `p_user.c:101`) -- C writes `&FINEMASK`, same value; no named C function, so no `#[doc(alias)]` |
//! | `P_User_Link_Anchor` | `anchor::P_User_Link_Anchor` | glue | link scaffolding, never runs in sim; keeps the `#[no_mangle]` set alive |
//! | `onground` | `state` | data | shared global (vanilla-faithful multiplayer-overwrite quirk, see its doc); keeps its `#[no_mangle]` C symbol; its pre-graduation doc falsely claimed ABI compatibility with a not-yet-ported `p_pspr.c` -- fixed in this graduation |
//! | `MAXBOB` / `VIEWHEIGHT` / `ANG5`, `BT_*` (6), `wp_*` (9), `pw_*` (6), `PST_*` (3), `INVERSECOLORMAP` | `state` consts | data | button/weapon/power/state/view vocabulary (`d_event.h`, `doomdef.h`, `d_player.h`, `p_local.h`); values pinned by the moved `constants_match_c_header_values` test |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! the five exported functions, the `onground` static, and the anchor
//! keep their C symbols and `#[no_mangle]`, which keeps the wasm
//! export surface byte-identical. No function-pointer takers or
//! address compares reach into p_user (the sole external caller is the
//! graduated p_tick/ticker.rs, resolved through the root re-export;
//! the anchor is called from `doomgeneric_Create`). No compiled C
//! translation unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes p_user.c), so the retention is
//! pure conservatism (zero wasm export churn).
//!
//! ## Deterministic Aspects
//!
//! The module draws ZERO `P_Random` bytes (none in `p_user.c` either);
//! its demo surface is the `leveltime`-phase bob math, `angle_t`
//! wrapping arithmetic, and the per-tic write order into player/mobj
//! state. The two genuinely pure computations -- `bob_amplitude` and
//! `bob_phase` -- are extracted into `dtmc` and pinned by baseline
//! vectors that were written and run GREEN against the original
//! in-file `P_CalcHeight` body BEFORE the extraction and re-run green
//! after (F10 §2.3), including a vector through the body for the
//! vanilla double `viewz` write (first `z + VIEWHEIGHT` with ceiling
//! clamp, then overwritten by `z + viewheight` -- never "fix" it) and
//! the mask-existence vector at `leveltime = 26` (unmasked, the
//! `finesine` index would overrun the table). `onground` is a shared
//! global, not per-player: in a multiplayer tic each player's
//! `P_MovePlayer`/`P_DeathThink` overwrites it and the next player's
//! `P_CalcHeight` guard reads the leftover -- vanilla-faithful; keep
//! the global, never thread-localize or fold into `PlayerT`.
//! `P_PlayerThink`'s in-place `cmd` writes are consumed-ticcmd state
//! whose order with g_game's demo read side is settled upstream of
//! this module. The F9 state hash covers mobj position, so the golden
//! demo tests net these writes directly.

pub mod anchor;
pub mod dtmc;
pub mod movement;
pub mod state;
pub mod think;
pub mod view;

//* path-stability re-export: the per-tic player thinkers keep their
//* module-root paths (p_tick/ticker.rs, doomgeneric).
pub use movement::{P_DeathThink, P_MovePlayer, P_Thrust};
pub use think::P_PlayerThink;
pub use view::P_CalcHeight;

//* path-stability re-export: the shared onground global keeps its
//* module-root path.
pub use state::onground;

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric_Create).
pub use anchor::P_User_Link_Anchor;
