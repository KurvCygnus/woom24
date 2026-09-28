//! The SHA-1 streaming state: context struct and digest alias.

#![allow(non_camel_case_types)] // ! `sha1_digest_t` keeps its upstream C name (types ruling).

use std::ffi::c_int;

/// 160-bit SHA-1 digest as 20 raw bytes, port of C `sha1_digest_t`.
pub type sha1_digest_t = [u8; 20];

/// SHA-1 streaming context, port of C `sha1_context_t`.
///
/// `h0..h4` are the five 32-bit chaining values; `nblocks` counts complete
/// 64-byte blocks already absorbed; `buf` is the partial-block staging area
/// and `count` (`0..=64`) is how many bytes of `buf` are currently valid.
/// `#[repr(C)]` because the still-C call sites in `d_loop.c` and the save
/// system stack-allocate this struct.
#[repr(C)]
pub struct SHA1Context
{
    /// Chaining variable A.
    pub h0: u32,
    /// Chaining variable B.
    pub h1: u32,
    /// Chaining variable C.
    pub h2: u32,
    /// Chaining variable D.
    pub h3: u32,
    /// Chaining variable E.
    pub h4: u32,
    /// Number of full 64-byte blocks already compressed.
    pub nblocks: u32,
    /// Partial-block staging area for the streaming API.
    pub buf: [u8; 64],
    /// Number of bytes currently held in `buf` (0..=64). Stored as `c_int`
    /// for ABI parity with the C `int count` field.
    pub count: c_int,
}
