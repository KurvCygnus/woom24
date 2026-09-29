//! Rust port of vendor/doomgeneric/i_input.c.
//!
//! Keyboard input handling: reads keys from the doomgeneric platform layer
//! (`DG_GetKey`) and posts them as Doom `event_t` events through `D_PostEvent`.
//! The platform layer already returns Doom key codes, so the AT-scancode
//! translation table from the original C source is omitted; `TranslateKey`
//! is effectively an identity function, matching the active behaviour in
//! `i_input.c` (the lookup table there is dead code behind a comment block).
//!
//! ## Submodule Responsibility
//!
//! - `keymap.rs` -- the shift transform: `SHIFTXFORM`, `translate_key`,
//!   `typed_char`, and `update_shift_status`
//! - `pump.rs` -- the event pump: the config statics
//!   (`vanilla_keyboard_mapping`, `shiftdown`), the `DG_GetKey` extern
//!   (reverse contract), `init_input`, `pump_key_events`, and the
//!   `I_Input_Link_Anchor` link anchor
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `TranslateKey` | `keymap::translate_key` | glue | identity (the platform layer already returns Doom key codes, matching the active C code); upstream `vendor/doomgeneric/i_input.c:225` |
//! | `GetTypedChar` | `keymap::typed_char` | glue | applies `SHIFTXFORM` when `shiftdown > 0`; upstream `i_input.c:242` |
//! | `UpdateShiftStatus` | `keymap::update_shift_status` | glue | tracks `KEY_RSHIFT` only; upstream `i_input.c:263` |
//! | `I_InitInput` | `pump::init_input` | glue | no-op in the doomgeneric platform layer; C symbol pinned via `#[export_name]` (caller `i_video.rs` imports the upstream name through the root shim); upstream `i_input.c:338` |
//! | `I_GetEvent` | `pump::pump_key_events` | glue | keydown loop with the first-keyup-ends-the-call quirk preserved verbatim (see below); C symbol pinned via `#[export_name]` (caller `i_video.rs` imports the upstream name through the root shim); upstream `i_input.c:279` |
//! | `vanilla_keyboard_mapping` (static) | `pump::vanilla_keyboard_mapping` | data | static keeps its upstream name and `#[no_mangle]` export (config-bound; `m_config` entry `vanilla_keyboard_mapping`) |
//! | `shiftdown` / `SHIFTXFORM` (statics) | `pump::shiftdown` / `keymap::SHIFTXFORM` | data | statics keep names; `SHIFTXFORM` mirrors `shiftxform[]` verbatim including the Watcom shift-backslash-to-`'!'` quirk |
//! | `I_Input_Link_Anchor` | `pump::I_Input_Link_Anchor` | glue | Rust-only retention anchor kept verbatim; retires with the freeze zone |
//!
//! `D_PostEvent` was extern-declared pre-graduation; `d_event` (graduated)
//! exports the same-ABI `D_PostEvent`, so the pump now calls it by Rust
//! path and the extern declaration is gone. `DG_GetKey` stays extern: it is
//! implemented by the shells, never by the core.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the module is platform event marshalling. Events DO feed
//! the simulation (through `D_PostEvent`), but this module only forwards
//! what the host hands it; the demo-relevant responder/build-ticcmd
//! conversion lives in `g_game` (graduated C3), and the queue timing is
//! async host input, not a tic-observable computation. The
//! first-keyup-ends-the-call behaviour of `pump_key_events` is upstream
//! behaviour (`i_input.c`'s loop `break`s on the first keyup), not a bug to
//! fix -- it stays pinned by the carried doc comment on the body.

pub mod keymap;
pub mod pump;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use pump::{init_input as I_InitInput, pump_key_events as I_GetEvent};

//* path-stability re-export: the config static and the link anchor keep
//* their module-root paths (`m_menu` and `i_video` respectively).
pub use pump::{vanilla_keyboard_mapping, I_Input_Link_Anchor};
