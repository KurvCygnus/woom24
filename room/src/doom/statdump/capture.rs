//! The capture path: the module-private snapshot buffer, the libc
//! `memcpy` extern, `StatCopy`, and the `StatDump` no-op stub, moved
//! wholesale from the pre-graduation `statdump.rs` (F10 wave A3).

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_void};

use crate::doom::m_argv::M_ParmExists;
use crate::doom::statdump::types::{wbplayerstruct_t, wbstartstruct_t};

/// Maximum number of intermission snapshots that can be captured during
/// a session. Mirrors the C `MAX_CAPTURES` define.
const MAX_CAPTURES: usize = 32;

/// Ring of captured end-of-level snapshots. The first
/// [`num_captured_stats`] entries are valid; the rest are zero-initialised
/// via the `DEFAULT_WB` constant below. Mirrors the C `captured_stats`
/// array.
static mut captured_stats: [wbstartstruct_t; MAX_CAPTURES] = {
    const DEFAULT: wbplayerstruct_t = wbplayerstruct_t {
        in_: 0,
        skills: 0,
        sitems: 0,
        ssecret: 0,
        stime: 0,
        frags: [0; 4],
    };
    const DEFAULT_WB: wbstartstruct_t = wbstartstruct_t {
        epsd: 0,
        last: 0,
        partime: 0,
        plyr: [DEFAULT; 4],
    };
    [DEFAULT_WB; MAX_CAPTURES]
};

/// Number of snapshots currently stored in [`captured_stats`]. Increments
/// on every [`StatCopy`] call until [`MAX_CAPTURES`] is reached. Mirrors
/// the C `num_captured_stats` static.
static mut num_captured_stats: c_int = 0;

extern "C" {
    /// C `memcpy` from libc. Used for the bulk struct copy below; an
    /// `std::ptr::copy_nonoverlapping` would do equally well but the
    /// direct FFI call keeps the code byte-identical to the C source.
    fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
}

/// Capture a snapshot of `stats` into `captured_stats` if statistics
/// dumping was requested on the command line.
///
/// Behaviour matches the C original: when `-statdump` is present in
/// `argv` and the buffer is not yet full, the entire `wbstartstruct_t`
/// is `memcpy`'d into the next free slot and `num_captured_stats`
/// is bumped. When the buffer is full or the flag is absent, the call
/// is silently dropped.
///
/// Called from `WI_Drawer` / `WI_Ticker` (via the intermission code)
/// at the end of every level.
#[no_mangle]
pub extern "C" fn StatCopy(stats: *mut wbstartstruct_t)
{
    unsafe
    {
        if M_ParmExists(c"-statdump".as_ptr()) != 0 && num_captured_stats < MAX_CAPTURES as c_int
        {
            //? Latent offset-incompatibility (A3 investigation finding,
            //? recorded here, NEVER fixed in this wave): `stats` points at
            //? wi_stuff's FULL `wbstartstruct_t`, and this module's trimmed
            //? mirror is not a prefix-compatible view of it -- `last`
            //? (offset 4) reads wminfo.didsecret, `partime` (offset 8)
            //? reads wminfo.last, `plyr` (offset 12) starts at wminfo.next;
            //? `wbplayerstruct_t` additionally omits the C `score` field
            //? (36 B vs 40 B). Latent only: `captured_stats` is write-only
            //? (`StatDump` below is a stub, no reader exists), so nothing
            //? can observe the mismatch. Post-graduation fix: alias
            //? wi_stuff's real struct, or reorder + correct the size test
            //? in `types.rs`.
            memcpy(
                std::ptr::addr_of_mut!(captured_stats[0]).offset(num_captured_stats as isize)
                    as *mut c_void,
                stats as *const c_void,
                std::mem::size_of::<wbstartstruct_t>(),
            );
            num_captured_stats += 1;
        }
    }
}

/// Write captured statistics to the file named by `-statdump` (or
/// stdout when the path is `-`).
///
/// No-op stub: the entire dump implementation - banner printing, par
/// time interpretation, the `PrintFragsTable` grid, the gamemode
/// discovery heuristic - sits inside `#if ORIGCODE` in the C source
/// and is not ported. [`StatCopy`] still fills the buffer, so the
/// data is there for any future implementation that wants it.
#[no_mangle]
pub extern "C" fn StatDump()
{
    // All implementation is wrapped in #if ORIGCODE which is not defined
}

#[cfg(test)]
mod tests
{
    use super::*;

    /// StatDump must not panic (it is currently a no-op stub).
    #[test]
    fn stat_dump_does_not_panic()
    {
        StatDump();
    }
}
