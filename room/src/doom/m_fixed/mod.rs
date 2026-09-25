//! 16.16 fixed-point arithmetic for the DOOM engine: the shared
//! vocabulary (`fixed_t`, `FRACBITS`, `FRACUNIT`) plus the graduated
//! demo-synchronization surface in `dtmc`, bit-exact with
//! `vendor/doomgeneric/m_fixed.c`.
//!
//! ## Submodule Responsibility
//!
//! None yet -- `dtmc` only: the F10 pilot extracted the module's whole
//! qualifying surface into `dtmc.rs`, and no non-dtmc responsibility
//! split exists. The type aliases and constants below are shared
//! vocabulary, deliberately not part of the dtmc surface.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location        | Surface | Notes                                                                                                                  |
//! |--------------|---------------------|---------|------------------------------------------------------------------------------------------------------------------------|
//! | `FixedMul`   | `dtmc::fixed_mul`   | dtmc    | pure 16.16 multiply feeding movement/angle/damage every tic; upstream `vendor/doomgeneric/m_fixed.c:33-39`              |
//! | `FixedDiv`   | `dtmc::fixed_div`   | dtmc    | `>> 14` saturation guard + 64-bit division, demo-observable; upstream `vendor/doomgeneric/m_fixed.c:47-61`               |
//! | `FixedDiv2`  | -- (never ported)    | --       | absent from this port: the vendored C is the Chocolate-style version with only `FixedMul`/`FixedDiv`, so vanilla's asm `FixedDiv2` (the `I_Error` abort path) has no counterpart and no Rust callers -- nothing to extract |
//!
//! ## Deterministic Aspects
//!
//! `dtmc::fixed_mul` and `dtmc::fixed_div` are pure integer functions on
//! the demo synchronization surface: movement (`p_map`, `p_user`,
//! `p_mobj`), aiming and attacks (`p_enemy`, `p_pspr`), damage and
//! pickup scaling (`p_inter`), and sight/render math (`p_sight`, `r_*`)
//! all consume their results inside the 35 tics/s simulation, so every
//! wrap, shift, and saturation must stay bit-exact across hosts and
//! targets. LSB exactness is load-bearing: `fixed_mul` computes the
//! product in a full 64-bit intermediate before the arithmetic (floor)
//! shift, and `fixed_div` routes overflow through the `>> 14` guard --
//! pinned by the baseline vectors in `dtmc`'s test module, written and
//! run against the original bodies before extraction (F10 pilot).

#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::c_int;

/// `fixed_t` -- matches `typedef int fixed_t;` in m_fixed.h.
pub type fixed_t = c_int;

/// `angle_t` -- BAM angle, matches `typedef unsigned int angle_t;` in tables.h.
pub type angle_t = u32;

/// Number of fractional bits in `fixed_t`. Matches `FRACBITS` in
/// `m_fixed.h` and is the right-shift used by `dtmc::fixed_mul`.
pub const FRACBITS: u32 = 16;

/// `1.0` expressed in 16.16 fixed-point (`1 << FRACBITS`). Matches
/// `FRACUNIT` in `m_fixed.h`.
pub const FRACUNIT: fixed_t = 1 << FRACBITS;

pub mod dtmc;

//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use dtmc::fixed_div as FixedDiv;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use dtmc::fixed_mul as FixedMul;
