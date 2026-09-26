//! Link stubs for the not-yet-ported or feature-disabled engine
//! pieces: the two constant-zero networking booleans and the empty
//! Timidity-config initializer -- bit-exact with
//! `vendor/doomgeneric/dummy.c` (`FEATURE_SOUND` undefined).
//!
//! ## Submodule Responsibility
//!
//! - `stubs.rs` -- the two `#[no_mangle]` `static mut` boolean stubs
//!   (`net_client_connected`, `drone`), the empty
//!   `I_InitTimidityConfig` stub, and their tests
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic
//! Aspects. The module qualifies for the zero-logic allowance
//! (`mod.rs` + one data file). The module root is documentation +
//! wiring only: the `mod` declaration and the defensive re-exports
//! below; no content lives here. Nothing in the tree path-imports
//! these symbols today -- the two statics are consumed through raw
//! `extern` blocks, not module paths -- so the re-exports exist only
//! to keep the module-root paths stable should a consumer appear.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `net_client_connected` | `stubs::net_client_connected` (static) | data | constant-zero stub read by the old-sync lag gate (`d_loop.rs:261`, `MAKETIC - gameticdiv > 2`); `#[no_mangle]` MANDATORY -- extern-declared at `d_loop.rs:187`; the declarer says `c_int` while this definition is `c_uint` (`doomtype.h` `boolean` is `unsigned int`; size-identical, resolves today) -- the asymmetry is load-bearing history, never "fix" it silently; upstream `dummy.c:27` |
//! | `drone` | `stubs::drone` (static) | data | constant-zero stub read by the net-wait gate (`d_loop.rs:256`) and the drone mouse-release path (`d_main.rs:891`); extern-declared twice (`d_loop.rs:185`, `d_main.rs:344`), same `c_int`-vs-`c_uint` declarer asymmetry as above; `#[no_mangle]` MANDATORY; upstream `dummy.c:29` |
//! | `I_InitTimidityConfig` | `stubs::I_InitTimidityConfig` | glue | empty no-op stub with zero in-tree references (the C-side referencer `i_sound.c` is not compiled: `FEATURE_SOUND` undefined, `doomgeneric-sys/build.rs` does not compile i_sound.c); `#[no_mangle]` retained so the wasm export surface stays byte-identical; upstream `dummy.c:43-49` (`#ifndef FEATURE_SOUND`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin;
//! no link anchor is added either -- the two statics are kept alive
//! by the extern-block references themselves, and adding an anchor
//! could change linker decisions for the unreferenced
//! `I_InitTimidityConfig` (its current dead-strip-or-not state is
//! pre-existing and must survive graduation unchanged).
//!
//! ## Deterministic Aspects
//!
//! Module adjudicated with no `dtmc` surface: both statics are
//! constant-zero stubs gating network-play paths (`d_loop.rs:256,261`)
//! and are never set anywhere in the tree -- net play is out of scope
//! by constraint (AGENTS.md constraint 6), so there is no computation
//! to extract and no demo-visible state to pin beyond the zero
//! initialization itself. `I_InitTimidityConfig` is an empty stub;
//! its body has no behavior to adjudicate.

pub mod stubs;

//* path-stability re-export (defensive): nothing path-imports these
//* today -- the statics are consumed through raw `extern` blocks
//* (d_loop.rs, d_main.rs) -- but the module-root paths stay stable
//* should a consumer appear.
pub use stubs::{I_InitTimidityConfig, drone, net_client_connected};
