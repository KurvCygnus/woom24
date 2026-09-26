//! The WAD lump directory: parsing WAD headers and directories into
//! the global `lumpinfo` table, resolving lump names to indices, and
//! serving the engine's cached lump bytes.
//!
//! ## Submodule Responsibility
//!
//! - `file.rs` -- file ingestion: `add_file` opens a WAD or
//!   single-lump file, validates the header magic, and appends its
//!   lumps to the global directory (with the `ExtendLumpInfo` growth
//!   helper)
//! - `lookup.rs` -- name resolution: the case-insensitive 8-char name
//!   hash, the hash-table probe with backwards-linear fallback, and
//!   hash-table construction
//! - `cache.rs` -- lump access: the count/size accessors, disk reads
//!   bracketed by `I_BeginRead`/`I_EndRead`, and the zone-managed
//!   cache acquire/release surface, by lump number or by name
//! - `iwad.rs` -- launch-time validation: refuse an IWAD whose unique
//!   marker lump belongs to a different game
//!
//! The global `lumpinfo` array and its companion `numlumps` /
//! `lumphash` form the in-memory directory of every lump loaded from
//! all WADs; they live here as shared state, and the submodules reach
//! them through `super::`.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `W_AddFile` | `file::add_file` | glue | opens the file, validates the `IWAD`/`PWAD` magic, appends the directory; drops any stale hash table; upstream `vendor/doomgeneric/w_wad.c:139` |
//! | `ExtendLumpInfo` (file-static) | `file::ExtendLumpInfo` | glue | private directory-growth helper with zone-user fixup and chain re-link; upstream file-static name kept (private, no shim needed); upstream `vendor/doomgeneric/w_wad.c:87` |
//! | `W_LumpNameHash` | `lookup::lump_name_hash` | data | case-insensitive djb2 over the 8-char name; upstream `vendor/doomgeneric/w_wad.c:70` |
//! | `W_CheckNumForName` | `lookup::check_num_for_name` | data | hash-table probe, else backwards linear scan so later WADs override earlier ones; C symbol pinned via `#[export_name]` (legacy `extern "C"` consumer in `d_net.rs`); upstream `vendor/doomgeneric/w_wad.c:256` |
//! | `W_GetNumForName` | `lookup::get_num_for_name` | data | strict variant; `I_Error` on miss; upstream `vendor/doomgeneric/w_wad.c:306` |
//! | `W_GenerateHashTable` | `lookup::generate_hash_table` | data | builds the per-bucket chains through `lumpinfo_t::next`; upstream `vendor/doomgeneric/w_wad.c:539` |
//! | `W_NumLumps` | `cache::num_lumps` | data | directory count accessor; upstream `vendor/doomgeneric/w_wad.c:244` |
//! | `W_LumpLength` | `cache::lump_length` | data | directory size accessor, range-checked via `I_Error`; upstream `vendor/doomgeneric/w_wad.c:325` |
//! | `W_ReadLump` | `cache::read_lump` | glue | disk read bracketed by `I_BeginRead`/`I_EndRead`; upstream `vendor/doomgeneric/w_wad.c:342` |
//! | `W_CacheLumpNum` | `cache::cache_lump_num` | glue | mmap shortcut / zone retag / fill-on-miss; upstream `vendor/doomgeneric/w_wad.c:382` |
//! | `W_CacheLumpName` | `cache::cache_lump_name` | glue | name-resolving wrapper; C symbol pinned via `#[export_name]` (legacy `extern "C"` consumers in `hu_stuff.rs`, `r_draw.rs`); upstream `vendor/doomgeneric/w_wad.c:429` |
//! | `W_ReleaseLumpNum` | `cache::release_lump_num` | glue | demotes the cached block to `PU_CACHE`; upstream `vendor/doomgeneric/w_wad.c:444` |
//! | `W_ReleaseLumpName` | `cache::release_lump_name` | glue | name-resolving wrapper; upstream `vendor/doomgeneric/w_wad.c:465` |
//! | `W_CheckCorrectIWAD` | `iwad::check_correct_iwad` | glue | launch-time mission/IWAD mismatch guard; upstream `vendor/doomgeneric/w_wad.c:586` |
//! | `W_Profile` | -- (never ported) | -- | upstream's own debug lump-cache profiler, called `UNUSED` in upstream `p_setup.c:771` and absent from this port; no Rust body and no Rust callers -- nothing to split |
//! | `W_Wad_Link_Anchor` | stays in this file | glue | link-only anchor, not part of the upstream API; kept at module root beside the shims it pins |
//!
//! All renamed functions keep their `pub extern "C"` ABI kind but drop
//! `#[no_mangle]` with the rename: `#[no_mangle]` binds the exported C
//! symbol to the fn name, so keeping it would export spurious
//! snake_case symbols, and the `pub use` shims above already restore
//! the upstream Rust paths. Two exceptions pin their upstream C
//! symbols with `#[export_name]` because freeze-zone legacy
//! `extern "C"` blocks link against them and must compile unchanged:
//! `check_num_for_name` (declared in `d_net.rs`) and
//! `cache_lump_name` (declared in `hu_stuff.rs`, `r_draw.rs`). The
//! `lumpinfo` / `numlumps` statics and the `W_Wad_Link_Anchor` anchor
//! keep their `#[no_mangle]` C symbols unchanged (they are not
//! renamed).
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 pilot): WAD I/O shapes game
//! *content* -- which lumps exist and which bytes they hold -- at load
//! time, not the per-tic simulation observables the demo
//! synchronization surface pins. Given identical WAD inputs, the
//! lookups and cached reads here return identical bytes on every host
//! and target, and the 35 tics/s simulation consumes that content
//! through integer-exact code elsewhere; nothing in this module
//! participates in a tic's arithmetic. Accordingly `w_wad` has no
//! `dtmc` submodule: every function adjudicates to `glue` (file I/O,
//! zone-allocator marshalling, engine-lifecycle guards) or `data`
//! (pure directory and name computation) in the mapping table above.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::ptr;

use crate::doom::w_file::wad_file_t;

pub mod cache;
pub mod file;
pub mod iwad;
pub mod lookup;

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

/// On-disk WAD header. Read from offset 0 of every `.wad` file
/// loaded by `W_AddFile`. Layout matches `wadinfo_t` in `w_wad.c`,
/// 12 bytes on x86_64.
#[repr(C)]
struct wadinfo_t
{
    /// Magic identifier: `"IWAD"` for the main IWAD, `"PWAD"` for
    /// a patch WAD. Anything else triggers `I_Error`.
    identification: [c_char; 4],
    /// Little-endian number of lumps in the directory.
    numlumps: c_int,
    /// Little-endian byte offset to the lump-directory table.
    infotableofs: c_int,
}

/// On-disk directory entry as it appears at `infotableofs`. Mirrors
/// `filelump_t` in `w_wad.c`, 16 bytes on x86_64.
#[repr(C)]
struct filelump_t
{
    /// Little-endian byte offset of the lump payload inside the WAD.
    filepos: c_int,
    /// Little-endian payload length in bytes.
    size: c_int,
    /// 8-char ASCII lump name, NUL-padded.
    name: [c_char; 8],
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
static mut lumphash: *mut *mut lumpinfo_t = ptr::null_mut();

extern "C"
{
    /// libc: byte-equal compare of first `n` bytes.
    fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int;
    /// libc: copy up to `n` bytes, NUL-padding the destination.
    fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char;
    /// libc: NUL-terminated string length.
    fn strlen(s: *const c_char) -> usize;
    /// libc: ASCII-upper-case.
    fn toupper(c: c_int) -> c_int;

    /// libc: zero-initialised allocation.
    fn calloc(nmemb: usize, size: usize) -> *mut c_void;
    /// libc: free a `malloc`/`calloc` block.
    fn free(ptr: *mut c_void);
}

//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::cache_lump_name as W_CacheLumpName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::cache_lump_num as W_CacheLumpNum;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::lump_length as W_LumpLength;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::num_lumps as W_NumLumps;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::read_lump as W_ReadLump;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::release_lump_name as W_ReleaseLumpName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::release_lump_num as W_ReleaseLumpNum;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use file::add_file as W_AddFile;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use iwad::check_correct_iwad as W_CheckCorrectIWAD;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::check_num_for_name as W_CheckNumForName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::generate_hash_table as W_GenerateHashTable;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::get_num_for_name as W_GetNumForName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::lump_name_hash as W_LumpNameHash;

/// Link anchor referencing every public C symbol in this module so
/// the linker keeps them all. Not part of the original Doom API.
///
/// # Safety
///
/// Passes null pointers everywhere and would crash if called.
/// Treat as link-only.
#[no_mangle]
pub unsafe extern "C" fn W_Wad_Link_Anchor()
{
    W_LumpNameHash(ptr::null());
    W_AddFile(ptr::null_mut());
    W_NumLumps();
    W_CheckNumForName(ptr::null());
    W_GetNumForName(ptr::null());
    W_LumpLength(0);
    W_ReadLump(0, ptr::null_mut());
    W_CacheLumpNum(0, 0);
    W_CacheLumpName(ptr::null(), 0);
    W_ReleaseLumpNum(0);
    W_ReleaseLumpName(ptr::null());
    W_GenerateHashTable();
    W_CheckCorrectIWAD(0);
}

#[cfg(test)]
mod tests
{
    use super::filelump_t;
    use super::lumpinfo_t;
    use super::wadinfo_t;

    /// `lumpinfo_t` is shared with C code: its 40-byte size on
    /// x86_64 must not silently change.
    #[test]
    fn lumpinfo_size_matches_c()
    {
        // C lumpinfo_t = name[8] + wad_file* + position + size + cache + next
        // On x86_64: 8 + 8 + 4 + 4 + 8 + 8 = 40 bytes
        assert_eq!(std::mem::size_of::<lumpinfo_t>(), 40);
    }

    /// `wadinfo_t` must match the on-disk WAD header layout: 12
    /// packed bytes.
    #[test]
    fn wadinfo_size_matches_c()
    {
        // C wadinfo_t = ident[4] + numlumps + infotableofs
        // On x86_64: 4 + 4 + 4 = 12 bytes
        assert_eq!(std::mem::size_of::<wadinfo_t>(), 12);
    }

    /// `filelump_t` must match the on-disk directory-entry layout:
    /// 16 packed bytes.
    #[test]
    fn filelump_size_matches_c()
    {
        // C filelump_t = filepos + size + name[8]
        // On x86_64: 4 + 4 + 8 = 16 bytes
        assert_eq!(std::mem::size_of::<filelump_t>(), 16);
    }
}
