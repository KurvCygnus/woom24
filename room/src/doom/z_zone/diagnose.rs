//! Zone heap diagnostics: the stdout dump, the invariant checkers,
//! the free-byte counter, and the stubbed file dumper.
//!
//! Two of these have no C counterpart (`check_heap_quiet`,
//! `check_heap_after`) and two carry Rust-only additions over
//! upstream: the 2000-block walk cap in `check_heap_quiet` and the
//! stderr traces in `check_heap`.

use std::ffi::{c_char, c_int};

use super::tags::{PU_FREE, PU_PURGELEVEL};
use super::zone::{mainzone, memblock_t};
use crate::doom::crt::{c_printf, c_printf2, c_printf4};
use crate::i_error;

/// `void Z_DumpHeap(int lowtag, int hightag)` — diagnostic dump of the
/// Zone to stdout via `libc::printf`. Reports zone size/location, the
/// tag range, and every block whose tag is in `[lowtag, hightag]`, plus
/// inline `ERROR:` lines for invariant violations (size/next mismatch,
/// broken back link, consecutive free blocks).
///
/// # Safety
/// - `mainzone` must be initialised.
/// - Calls into `libc::printf` with hard-coded format strings; the
///   varargs must match (`%i`, `%p`, etc.).
#[doc(alias = "Z_DumpHeap")]
pub unsafe extern "C" fn dump_heap(lowtag: c_int, hightag: c_int)
{
    let zone = mainzone;
    let sentinel = std::ptr::addr_of_mut!((*zone).blocklist);

    let msg = b"zone size: %i  location: %p\n\0";
    c_printf2(msg.as_ptr() as *const c_char, (*zone).size, zone);

    let msg2 = b"tag range: %i to %i\n\0";
    c_printf2(msg2.as_ptr() as *const c_char, lowtag, hightag);

    let mut block = (*zone).blocklist.next;
    loop
    {
        if (*block).tag >= lowtag && (*block).tag <= hightag
        {
            let msg3 = b"block:%p    size:%7i    user:%p    tag:%3i\n\0";
            c_printf4(
                msg3.as_ptr() as *const c_char,
                block,
                (*block).size,
                (*block).user,
                (*block).tag,
            );
        }

        if (*block).next == sentinel
        {
            break;
        }

        if (block as *mut u8).add((*block).size as usize) != (*block).next as *mut u8
        {
            let msg4 = b"ERROR: block size does not touch the next block\n\0";
            c_printf(msg4.as_ptr() as *const c_char);
        }

        if (*(*block).next).prev != block
        {
            let msg5 = b"ERROR: next block doesn't have proper back link\n\0";
            c_printf(msg5.as_ptr() as *const c_char);
        }

        if (*block).tag == PU_FREE && (*(*block).next).tag == PU_FREE
        {
            let msg6 = b"ERROR: two consecutive free blocks\n\0";
            c_printf(msg6.as_ptr() as *const c_char);
        }

        block = (*block).next;
    }
}

/// `void Z_CheckHeap(void)` — walk the Zone block list and abort via
/// `I_Error` if any invariant is violated (size/next mismatch, broken
/// back link, consecutive free blocks).
///
/// First defers to [`check_heap_quiet`] for a non-fatal pre-pass so
/// that stderr diagnostics show up before the abort.
///
/// # Safety
/// - `mainzone` must be initialised. May call `I_Error` (does not
///   return) on corruption.
#[doc(alias = "Z_CheckHeap")]
pub unsafe extern "C" fn check_heap()
{
    if !check_heap_quiet()
    {
        // Already printed diagnostic info.
    }
    let zone = mainzone;
    let sentinel = std::ptr::addr_of_mut!((*zone).blocklist);
    let mut block = (*zone).blocklist.next;

    loop
    {
        if (*block).next == sentinel
        {
            break;
        }

        if (block as *mut u8).add((*block).size as usize) != (*block).next as *mut u8
        {
            eprintln!(
                "Z_CheckHeap FAIL: block {:?} size={} next={:?} expected_next={:?}",
                block,
                (*block).size,
                (*block).next,
                (block as *mut u8).add((*block).size as usize)
            );
            i_error!("Z_CheckHeap: block size does not touch the next block\n");
        }

        if (*(*block).next).prev != block
        {
            eprintln!(
                "Z_CheckHeap FAIL: block {:?} next={:?} next.prev={:?}",
                block,
                (*block).next,
                (*(*block).next).prev
            );
            i_error!("Z_CheckHeap: next block doesn't have proper back link\n");
        }

        if (*block).tag == PU_FREE && (*(*block).next).tag == PU_FREE
        {
            i_error!("Z_CheckHeap: two consecutive free blocks\n");
        }

        block = (*block).next;
    }
}

/// Check heap integrity without aborting. Returns true if valid.
///
/// Rust-only addition (no C counterpart): used by [`check_heap_after`]
/// and [`check_heap`] for non-fatal diagnostics. Caps the walk at 2000
/// blocks to avoid hanging on a cyclic free-list bug. The 10 MB size
/// heuristic also flags legitimately merged free blocks on large
/// zones -- pinned by the baseline tests to stay unusable there
/// rather than silently "fixed".
///
/// # Safety
/// - Reads `mainzone` and its linked block list; the caller must ensure
///   no other thread mutates the Zone concurrently.
/// - Returns `true` if `mainzone` is null (nothing to check yet).
#[doc(alias = "Z_CheckHeapQuiet")]
pub unsafe extern "C" fn check_heap_quiet() -> bool
{
    let zone = mainzone;
    if zone.is_null()
    {
        return true;
    }
    let sentinel = std::ptr::addr_of_mut!((*zone).blocklist);
    let mut block = (*zone).blocklist.next;
    let mut valid = true;
    let mut count = 0;

    while block != sentinel && count < 2000
    {
        let size = (*block).size;
        if size <= 0 || size > 10_000_000
        {
            eprintln!(
                "Z_CheckHeapQuiet: block {:?} has invalid size {}",
                block, size
            );
            valid = false;
            break;
        }

        // The last block's next points to the sentinel (at the start of the zone),
        // not to block+size (at the end of the zone). This is by design.
        if (*block).next != sentinel
        {
            let expected_next = (block as *mut u8).add(size as usize) as *mut memblock_t;
            if (*block).next != expected_next
            {
                eprintln!(
                    "Z_CheckHeapQuiet: block {:?} size={} next={:?} expected={:?}",
                    block,
                    size,
                    (*block).next,
                    expected_next
                );
                valid = false;
            }
        }

        // Check prev link (works for all blocks including the last one whose next is sentinel)
        if !(*block).next.is_null() && (*(*block).next).prev != block
        {
            eprintln!(
                "Z_CheckHeapQuiet: block {:?} next={:?} next.prev={:?}",
                block,
                (*block).next,
                (*(*block).next).prev
            );
            valid = false;
        }

        // Check for consecutive free blocks (sentinel has tag=PU_STATIC, so skip it)
        if (*block).tag == PU_FREE && (*block).next != sentinel && (*(*block).next).tag == PU_FREE
        {
            eprintln!(
                "Z_CheckHeapQuiet: two consecutive free blocks at {:?} and {:?}",
                block,
                (*block).next
            );
            valid = false;
        }

        block = (*block).next;
        count += 1;
    }
    if count >= 2000
    {
        eprintln!("Z_CheckHeapQuiet: too many blocks, possible loop!");
        valid = false;
    }
    valid
}

/// Rust-only debug helper: run [`check_heap_quiet`] after a logical
/// allocator-touching step and log a labelled error if it fails. No C
/// counterpart; used to localise corruption in tricky porting changes.
///
/// # Safety
/// - Same preconditions as [`check_heap_quiet`]: `mainzone` may be null
///   (nothing to check) but otherwise the list must be quiescent.
#[doc(alias = "Z_CheckHeapAfter")]
pub unsafe fn check_heap_after(name: &str)
{
    if !check_heap_quiet()
    {
        eprintln!("*** Heap corruption detected after {} ***", name);
    }
}

/// `int Z_FreeMemory(void)` — return the number of bytes that could be
/// allocated immediately: the sum of all `PU_FREE` block sizes plus the
/// sizes of purgeable blocks (`tag >= PU_PURGELEVEL`) which can be
/// reclaimed.
///
/// # Safety
/// - `mainzone` must be initialised.
#[doc(alias = "Z_FreeMemory")]
pub unsafe extern "C" fn free_memory() -> c_int
{
    let zone = mainzone;
    let sentinel = std::ptr::addr_of_mut!((*zone).blocklist);
    let mut free: c_int = 0;
    let mut block = (*zone).blocklist.next;

    while block != sentinel
    {
        if (*block).tag == PU_FREE || (*block).tag >= PU_PURGELEVEL
        {
            free += (*block).size;
        }
        block = (*block).next;
    }

    free
}

/// `void Z_FileDumpHeap(FILE *f)` — stubbed out: the C original wrote a
/// per-block dump to a `FILE *` for the diehard debug build, but no
/// Rust call site exists. Kept as a no-op to satisfy the C linker.
///
/// # Safety
/// - `_f` is ignored; any value (including null) is accepted.
#[doc(alias = "Z_FileDumpHeap")]
pub unsafe extern "C" fn file_dump_heap(_f: *mut libc::FILE)
{
    // No Rust caller found; stubbed per plan.
}
