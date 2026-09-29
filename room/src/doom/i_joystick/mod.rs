//! Rust port of vendor/doomgeneric/i_joystick.c.
//!
//! Stub for the SDL joystick backend. Chocolate-doom uses SDL_joystick to
//! open a device, read axes / buttons / hats and post `ev_joystick` events
//! into the input queue. Doomgeneric compiles all of that out behind
//! `#ifdef ORIGCODE`; only the [`I_BindJoystickVariables`] entry point
//! survives so the configuration file format remains compatible. The Rust
//! port mirrors that: the lifecycle functions are no-ops, and the
//! configuration variables are still registered with `m_config::bind_variable` (C `M_BindVariable`) so
//! reads/writes of `default.cfg` round-trip the joystick settings even
//! though they have no runtime effect.
//!
//! ## Submodule Responsibility
//!
//! - `config.rs` -- the `NUM_VIRTUAL_BUTTONS` constant, the nine
//!   config-bound statics, the `bind_variable` path call, and
//!   `bind_joystick_variables`
//! - `lifecycle.rs` -- the three no-op lifecycle stubs
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the upstream-name shims below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `I_InitJoystick` | `lifecycle::init_joystick` | glue | no-op stub (the original opened the SDL joystick and registered an at-exit hook); C symbol pinned via `#[export_name]` (dead-but-exported: zero in-tree callers, kept for wasm symbol-set parity); upstream `vendor/doomgeneric/i_joystick.c:115` |
//! | `I_ShutdownJoystick` | `lifecycle::shutdown_joystick` | glue | no-op stub; C symbol pinned via `#[export_name]` (wasm symbol-set parity); upstream `i_joystick.c:77` |
//! | `I_UpdateJoystick` | `lifecycle::update_joystick` | glue | no-op stub (the original posted `ev_joystick` events); C symbol pinned via `#[export_name]` (wasm symbol-set parity); upstream `i_joystick.c:321` |
//! | `I_BindJoystickVariables` | `config::bind_joystick_variables` | glue | registers the nine config variables; C symbol pinned via `#[export_name]` (caller `d_main/bind.rs` imports the upstream name through the root shim); upstream `i_joystick.c:339` |
//! | 9 config statics | `config::` | data | `usejoystick`, `joystick_index`, `joystick_x_axis`, `joystick_x_invert`, `joystick_y_axis`, `joystick_y_invert`, `joystick_strafe_axis`, `joystick_strafe_invert`, `joystick_physical_buttons` -- statics keep names (config-bound) |
//! | `NUM_VIRTUAL_BUTTONS` (const) | `config::NUM_VIRTUAL_BUTTONS` | data | constant keeps its name |
//!
//! The C-only helpers (`IsValidAxis`, `IsAxisButton`, `ReadButtonState`,
//! `GetButtonsState`, `GetAxisState`) were compiled out by `#ifdef
//! ORIGCODE` upstream and are not ported -- nothing to map.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the module is platform plumbing with no joystick
//! backend -- everything is no-op lifecycle glue plus config-schema
//! preservation. The `bind_variable` call order and name strings are
//! `.cfg` round-trip compatibility surface (`default.cfg` byte-stability),
//! which is boot-time configuration, not the per-tic demo synchronization
//! surface.

pub mod config;
pub mod lifecycle;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use config::bind_joystick_variables as I_BindJoystickVariables;
pub use lifecycle::{init_joystick as I_InitJoystick, shutdown_joystick as I_ShutdownJoystick, update_joystick as I_UpdateJoystick};
