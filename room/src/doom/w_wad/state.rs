//! The in-memory lump directory state: the `lumpinfo_t` entry layout
//! shared across the FFI boundary and the three directory statics --
//! the `lumpinfo` array, its `numlumps` count, and the lazily built
//! `lumphash` table.

#![allow(non_upper_case_globals, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::ptr;

use crate::doom::w_file::wad_file_t;

/// One entry in the global lump directory. Mirrors `lumpinfo_t` in
/// `w_wad.h`. The size (40 bytes on x86_64) is asserted by a
/// `cfg(test)` test below since other modules read this layout
/// across the FFI boundary.
#[repr(C)]
pub struct lumpinfo_t
{
    /// 8-char ASCII lump name, **not** NUL-terminated when full.
    pub name: [c_char; 8],
    /// File the lump lives in.
    pub wad_file: *mut wad_file_t,
    /// Offset of the lump payload inside the file, in bytes.
    pub position: c_int,
    /// Payload size in bytes.
    pub size: c_int,
    /// Zone-allocated cache pointer, or null if not yet loaded.
    /// Memory-mapped files leave this null; `W_CacheLumpNum`
    /// returns a pointer into the mapping directly.
    pub cache: *mut c_void,
    /// Next entry in the per-hash-bucket chain when `lumphash` is
    /// populated; null otherwise.
    pub next: *mut lumpinfo_t,
}

/// Pointer to the global lump directory. Mirrors the `lumpinfo` C
/// global; sized by `numlumps`. Reallocated by `ExtendLumpInfo`
/// every time a new file is added. C linkage so other translation
/// units (and tests) can reach it.
#[no_mangle]
pub static mut lumpinfo: *mut lumpinfo_t = ptr::null_mut();

/// Number of entries in `lumpinfo`. Mirrors the C `numlumps` global.
#[no_mangle]
pub static mut numlumps: c_uint = 0;

/// Hash table: `numlumps` buckets, each a singly-linked list through
/// `lumpinfo_t::next`. Built lazily by `W_GenerateHashTable` and
/// dropped whenever a new file is added.
pub(super) static mut lumphash: *mut *mut lumpinfo_t = ptr::null_mut();

#[cfg(test)]
mod tests
{
    use super::lumpinfo_t;

    /// `lumpinfo_t` is shared with C code: its 40-byte size on
    /// x86_64 must not silently change.
    #[test]
    fn lumpinfo_size_matches_c()
    {
        // C lumpinfo_t = name[8] + wad_file* + position + size + cache + next
        // On x86_64: 8 + 8 + 4 + 4 + 8 + 8 = 40 bytes
        assert_eq!(std::mem::size_of::<lumpinfo_t>(), 40);
    }
}
