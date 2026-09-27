//! The cheat-matcher vocabulary: the `cheatseq_t` state struct and its
//! length constants. The struct is consumer-owned state -- instances
//! live in `st_stuff` / `am_map` statics, never here.

// Belt-and-suspenders (see p_spec/anims.rs): `cheatseq_t` and the C-suffix
// type names carry the allow regardless of toolchain lint probing.
#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int};

/// Maximum length of a cheat sequence including the NUL terminator,
/// matching `MAX_CHEAT_LEN` in `m_cheat.h`.
pub const MAX_CHEAT_LEN: usize = 25;

/// Maximum number of trailing parameter characters captured per cheat,
/// matching `MAX_CHEAT_PARAMS` in `m_cheat.h`.
pub const MAX_CHEAT_PARAMS: usize = 5;

/// Matches `cheatseq_t` from `m_cheat.h`.
///
/// Layout on 64-bit (`usize` = 8):
///   sequence[25]      -> offset 0, size 25
///   (padding 7)       -> offset 25-31
///   sequence_len      -> offset 32, size 8
///   parameter_chars   -> offset 40, size 4
///   (padding 4)       -> offset 44-47
///   chars_read        -> offset 48, size 8
///   param_chars_read  -> offset 56, size 4
///   parameter_buf[5]  -> offset 60, size 5
///   (padding 7)       -> offset 65-71
/// Total: 72 bytes
///
/// Layout on 32-bit (`usize` = 4):
///   sequence[25]      -> offset 0, size 25
///   (padding 3)       -> offset 25-27
///   sequence_len      -> offset 28, size 4
///   parameter_chars   -> offset 32, size 4
///   chars_read        -> offset 36, size 4
///   param_chars_read  -> offset 40, size 4
///   parameter_buf[5]  -> offset 44, size 5
///   (padding 3)       -> offset 49-51
/// Total: 52 bytes
///
//* The layout comments and the size asserts below are LP64-policy
//* model-aware guards (spec 3); the c_tests LP64 gate reads these
//* conventions -- keep them intact.
#[repr(C)]
pub struct cheatseq_t {
    /// NUL-terminated cheat sequence bytes (e.g. `"IDKFA"`).
    pub sequence: [c_char; MAX_CHEAT_LEN],
    /// Declared sequence length (vanilla uses this to suppress short
    /// cheats on parameter-bearing entries; see `check_cheat`).
    pub sequence_len: usize,
    /// Number of trailing parameter characters expected after the
    /// sequence matches (e.g. 2 for `IDCLEVxx`).
    pub parameter_chars: c_int,
    /// Cursor into `sequence`: number of correct keystrokes consumed.
    pub chars_read: usize,
    /// Cursor into `parameter_buf`: number of parameter chars consumed.
    pub param_chars_read: c_int,
    /// Captured parameter characters; readable via `get_param`.
    pub parameter_buf: [c_char; MAX_CHEAT_PARAMS],
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(
    std::mem::size_of::<cheatseq_t>() == 72,
    "cheatseq_t size mismatch on 64-bit platform"
);

#[cfg(target_pointer_width = "32")]
const _: () = assert!(
    std::mem::size_of::<cheatseq_t>() == 52,
    "cheatseq_t size mismatch on 32-bit platform"
);
