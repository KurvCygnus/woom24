//! Lump-name resolution for the WAD directory: the case-insensitive
//! 8-char name hash, the hash-table probe with backwards-linear
//! fallback, and construction of the hash table itself.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::ptr;

use super::{lumpinfo, lumpinfo_t, lumphash, numlumps, toupper};
use crate::doom::crt::strncasecmp;
use crate::doom::z_zone::{Z_Free, Z_Malloc, PU_STATIC};
use crate::i_error;

/// djb2-style hash of an 8-char lump name (NUL-terminated or padded).
///
/// The hash uses `((h << 5) ^ h) ^ toupper(ch)` per character, so
/// the result is case-insensitive and matches the C reference in
/// `w_wad.c` exactly. Caller must ensure `s` points to at least 8
/// bytes (or a shorter NUL-terminated string).
///
/// # Safety
///
/// `s` must point to at least 8 readable bytes (or a shorter
/// NUL-terminated string); the scan stops at the first NUL within
/// the 8-char window.
#[doc(alias = "W_LumpNameHash")]
pub extern "C" fn lump_name_hash(s: *const c_char) -> c_uint
{
    unsafe
    {
        let mut result: c_uint = 5381;
        for i in 0..8
        {
            let ch = *s.add(i);
            if ch == 0
            {
                break;
            }
            result = ((result << 5) ^ result) ^ (toupper(ch as c_int) as c_uint);
        }
        result
    }
}

/// Look up a lump by name and return its index, or `-1` if not
/// found.
///
/// Uses the hash table when `W_GenerateHashTable` has been called;
/// otherwise scans `lumpinfo` backwards so that later-loaded WADs
/// override earlier ones (matching the C implementation).
/// Comparison is via `strncasecmp` over 8 bytes, so trailing bytes
/// past a NUL must match too (they're zeroed in `lumpinfo_t::name`
/// after `strncpy`).
///
/// # Safety
///
/// `name` must point to a readable NUL-terminated string (only the
/// first 8 bytes are compared).
//* Freeze-zone legacy `extern "C"` blocks link this function by its
//* upstream C symbol (`d_net.rs`), so the symbol is pinned with
//* `#[export_name]` instead of being dropped with the rename.
#[doc(alias = "W_CheckNumForName")]
#[export_name = "W_CheckNumForName"]
pub extern "C" fn check_num_for_name(name: *const c_char) -> c_int
{
    unsafe
    {
        if !lumphash.is_null()
        {
            let hash = (lump_name_hash(name) % numlumps) as usize;
            let mut lump_p = *lumphash.add(hash);
            while !lump_p.is_null()
            {
                if strncasecmp((*lump_p).name.as_ptr(), name, 8) == 0
                {
                    return (lump_p as usize - lumpinfo as usize) as c_int
                        / std::mem::size_of::<lumpinfo_t>() as c_int;
                }
                lump_p = (*lump_p).next;
            }
        }
        else
        {
            // Linear search, backwards so patch lumps take precedence
            let mut i = numlumps as i32 - 1;
            while i >= 0
            {
                if strncasecmp((*lumpinfo.add(i as usize)).name.as_ptr(), name, 8) == 0
                {
                    return i;
                }
                i -= 1;
            }
        }
        -1
    }
}

/// Look up a lump by name and return its index. Calls `I_Error`
/// (and never returns) if the lump is missing. The C source uses
/// this as the strict variant; callers that tolerate misses use
/// `W_CheckNumForName` directly.
///
/// # Safety
///
/// `name` must point to a readable NUL-terminated string.
#[doc(alias = "W_GetNumForName")]
pub extern "C" fn get_num_for_name(name: *const c_char) -> c_int
{
    unsafe
    {
        let i = check_num_for_name(name);
        if i < 0
        {
            i_error!(
                "W_GetNumForName: {} not found!",
                CStr::from_ptr(name).to_string_lossy()
            );
        }
        i
    }
}

/// Build the lump-name hash table. Allocates `numlumps` buckets in
/// the zone heap, then inserts every lump into the bucket given by
/// `W_LumpNameHash(name) % numlumps`, chaining through
/// `lumpinfo_t::next`. Frees any pre-existing table first.
///
/// Called once after the last `W_AddFile`; subsequent additions
/// invalidate (free) the table so a regeneration must be requested
/// explicitly.
#[doc(alias = "W_GenerateHashTable")]
pub extern "C" fn generate_hash_table()
{
    unsafe
    {
        if !lumphash.is_null()
        {
            Z_Free(lumphash as *mut c_void);
        }

        if numlumps > 0
        {
            lumphash = Z_Malloc(
                (std::mem::size_of::<*mut lumpinfo_t>() * numlumps as usize) as c_int,
                PU_STATIC,
                ptr::null_mut(),
            ) as *mut *mut lumpinfo_t;
            std::ptr::write_bytes(lumphash, 0, numlumps as usize);

            for i in 0..numlumps
            {
                let hash =
                    (lump_name_hash((*lumpinfo.add(i as usize)).name.as_ptr()) % numlumps) as usize;
                (*lumpinfo.add(i as usize)).next = *lumphash.add(hash);
                *lumphash.add(hash) = lumpinfo.add(i as usize);
            }
        }
    }
}

#[cfg(test)]
mod tests
{
    use super::lump_name_hash;
    use std::ffi::c_char;

    /// Build a `[c_char; 8]` lump name from a byte slice, NUL-padding
    /// or truncating to 8 bytes -- the same shape `lumpinfo_t::name`
    /// holds after `strncpy`.
    fn name8(s: &[u8]) -> [c_char; 8]
    {
        let mut name = [0 as c_char; 8];
        for (i, &b) in s.iter().take(8).enumerate()
        {
            name[i] = b as c_char;
        }
        name
    }

    /// Sanity contract (F10 pilot): `lump_name_hash` is a pure
    /// function of its 8 input bytes -- no engine state, no WAD file,
    /// no globals -- so its baseline pins are self-contained. The
    /// constants below were computed by hand from the transcribed
    /// formula `h = 5381; h = ((h << 5) ^ h) ^ toupper(ch)` (u32
    /// wrap-around per character, scan stops at NUL) and pinned
    /// BEFORE the body moved here; they are the drift alarm for the
    /// shift width, the xor chain, and the case folding.
    #[test]
    fn hash_pins()
    {
        // Deterministic across calls: same bytes, same hash.
        let a = name8(b"TESTLUMP");
        assert_eq!(lump_name_hash(a.as_ptr()), lump_name_hash(a.as_ptr()));

        // Pinned value (u32 arithmetic): 0x4f6c_35d7.
        assert_eq!(lump_name_hash(a.as_ptr()), 0x4f6c_35d7);

        // Case-insensitive through `toupper`: identical bytes after
        // folding hash identically.
        assert_eq!(
            lump_name_hash(a.as_ptr()),
            lump_name_hash(name8(b"testlump").as_ptr())
        );

        // A different name lands elsewhere in the u32 space
        // (pinned: 0xcf6e_8210).
        assert_ne!(
            lump_name_hash(a.as_ptr()),
            lump_name_hash(name8(b"OTHERLMP").as_ptr())
        );

        // NUL-terminated short names stop the scan at the NUL
        // (pinned: 0x5072_e085).
        assert_eq!(lump_name_hash(name8(b"EXIT").as_ptr()), 0x5072_e085);
    }
}
