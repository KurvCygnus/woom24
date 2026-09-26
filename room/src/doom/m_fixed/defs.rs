//! Shared fixed-point vocabulary for the DOOM engine: the `fixed_t`
//! and `angle_t` type aliases and the `FRACBITS` / `FRACUNIT`
//! constants, matching the upstream headers exactly.

#![allow(non_camel_case_types)]

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
