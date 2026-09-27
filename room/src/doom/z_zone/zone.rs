//! Zone allocator core: the block-list structures, the global zone
//! instance, and the alloc/free/retag entry points, bit-faithful to
//! `vendor/doomgeneric/z_zone.c`.
//!
//! Allocations are tagged with a `PU_*` purge level (see
//! [`super::tags`]): blocks at or above `PU_PURGELEVEL` can be
//! reclaimed automatically by future allocations when space is tight.
//! Tags below `PU_PURGELEVEL` are pinned. Callers never inspect the
//! header, so only the user-visible pointer and behaviour matter.

#![allow(non_camel_case_types, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use super::tags::{PU_FREE, PU_PURGELEVEL};
use crate::doom::i_system::I_ZoneBase;
use crate::i_error;

/// Magic number stamped into `memblock_t::id` so frees/retags can
/// detect double-free and pointer arithmetic mistakes. Mirrors the
/// `ZONEID` macro in `z_zone.c`.
///
//* Byte-stable: the DOS zone-header words modeled in
//* `p_setup::reject` quote `0x1d4a11` literally (vanilla-workarounds
//* entry 4) -- never change this value.
const ZONEID: u32 = 0x1d4a11;
/// Minimum free fragment that justifies splitting a block during
/// allocation. Smaller leftovers stay attached to the allocation as
/// internal padding. Matches `MINFRAGMENT` in `z_zone.c`.
const MINFRAGMENT: c_int = 64;
/// Allocation alignment (pointer size, matching `MEM_ALIGN` in
/// `z_zone.c`). Sizes passed to the allocator are rounded up to this.
const MEM_ALIGN: usize = std::mem::size_of::<*mut ()>();

/// Header that precedes every Zone allocation in memory. Matches
/// `memblock_t` in `z_zone.c`; the `id` field carries `ZONEID` so
/// stray frees can be detected.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct memblock_t {
    /// Total bytes including header and any trailing fragment.
    pub(super) size: c_int,
    /// Optional back-pointer the user gave at allocation; nulled when
    /// the block is freed or purged.
    pub(super) user: *mut *mut c_void,
    /// `PU_*` tag controlling lifetime / purgeability.
    pub(super) tag: c_int,
    /// `ZONEID` for live blocks, 0 once freed.
    pub(super) id: c_int,
    /// Next block in the doubly-linked free/used list.
    pub(super) next: *mut memblock_t,
    /// Previous block in the doubly-linked free/used list.
    pub(super) prev: *mut memblock_t,
}

/// Zone-level header sitting at the start of `mainzone`. Matches the
/// anonymous `memzone_t` struct in `z_zone.c`.
#[repr(C)]
pub(super) struct memzone_t {
    /// Total bytes of the backing buffer (including this header).
    pub(super) size: c_int,
    /// Sentinel block used as the doubly-linked-list anchor.
    pub(super) blocklist: memblock_t,
    /// "Next-fit" cursor consulted by the allocator.
    pub(super) rover: *mut memblock_t,
}

/// `memzone_t *mainzone` — the single global Zone instance. Initialised
/// by [`zone_init`]; null beforehand. Most allocator entry points dereference
/// this unconditionally, so calls before `zone_init` are programmer errors.
pub static mut mainzone: *mut memzone_t = ptr::null_mut();

/// `void Z_ClearZone(memzone_t *zone)` — reset `zone` to a single free
/// block spanning the entire backing buffer.
///
/// Upstream inlines this into `Z_Init` (`z_zone.c:105-119`); the port
/// extracts it as a private helper. No C counterpart to pin or shim.
///
/// # Safety
/// - `zone` must point to a writable `memzone_t` whose backing buffer is
///   at least `zone.size` bytes long.
/// - No outstanding pointers into `zone` may exist; this overwrites the
///   entire allocator state.
unsafe fn zone_clear(zone: *mut memzone_t)
{
    let block = (zone as *mut u8).add(std::mem::size_of::<memzone_t>()) as *mut memblock_t;

    (*zone).blocklist.next = block;
    (*zone).blocklist.prev = block;
    (*zone).blocklist.user = zone as *mut *mut c_void;
    (*zone).blocklist.tag = 1; // PU_STATIC
    (*zone).rover = block;

    (*block).prev = &mut (*zone).blocklist;
    (*block).next = &mut (*zone).blocklist;
    (*block).tag = PU_FREE;
    (*block).size = (*zone).size - std::mem::size_of::<memzone_t>() as c_int;
}

/// `void Z_Init(void)` — request the platform Zone buffer from
/// `I_ZoneBase`, install it as `mainzone`, and clear it to one free
/// block. Called once early in `D_DoomMain`.
///
/// # Safety
/// - Must be called exactly once at program startup, before any other
///   Zone entry point.
/// - Reads/writes the global `mainzone`; not thread-safe.
#[doc(alias = "Z_Init")]
pub unsafe extern "C" fn zone_init()
{
    let mut size: c_int = 0;
    mainzone = I_ZoneBase(&mut size) as *mut memzone_t;
    (*mainzone).size = size;

    zone_clear(mainzone);
}

/// `void Z_Free(void *ptr)` — release a block previously returned by
/// [`zone_alloc`]. Clears the user back-pointer, marks the block as
/// free, and coalesces with adjacent free neighbours so two consecutive
/// free blocks never coexist.
///
//* Freeze-zone legacy `extern "C"` blocks link this function by its
//* upstream C symbol (`f_wipe.rs`, `r_draw.rs`), so the symbol is
//* pinned with `#[export_name]` instead of being dropped with the
//* rename.
///
/// # Safety
/// - `ptr` must be a value returned by [`zone_alloc`] that has not yet
///   been freed; its header (immediately before `ptr`) must still carry
///   `ZONEID`. Mismatches call `I_Error`.
/// - `mainzone` must be initialised (`zone_init` ran).
#[doc(alias = "Z_Free")]
#[export_name = "Z_Free"]
pub unsafe extern "C" fn zone_free(ptr: *mut c_void)
{
    let block = (ptr as *mut u8).sub(std::mem::size_of::<memblock_t>()) as *mut memblock_t;

    if (*block).id as u32 != ZONEID
    {
        i_error!("Z_Free: freed a pointer without ZONEID");
    }

    if (*block).tag != PU_FREE && !(*block).user.is_null()
    {
        *(*block).user = ptr::null_mut();
    }

    (*block).tag = PU_FREE;
    (*block).user = ptr::null_mut();
    (*block).id = 0;

    let mut block = block;
    let other = (*block).prev;
    if (*other).tag == PU_FREE
    {
        (*other).size += (*block).size;
        (*other).next = (*block).next;
        (*(*block).next).prev = other;

        if block == (*mainzone).rover
        {
            (*mainzone).rover = other;
        }
        block = other;
    }

    let other = (*block).next;
    if (*other).tag == PU_FREE
    {
        (*block).size += (*other).size;
        (*block).next = (*other).next;
        (*(*block).next).prev = block;

        if other == (*mainzone).rover
        {
            (*mainzone).rover = block;
        }
    }
}

/// `void *Z_Malloc(int size, int tag, void *user)` — allocate `size`
/// bytes (rounded up to `MEM_ALIGN`) tagged with `tag`. If `user` is
/// non-null it is treated as a `void **` and stored in the block header
/// so the allocator can clear it on purge / free.
///
/// Walks the next-fit `rover`, evicting purgeable blocks
/// (`tag >= PU_PURGELEVEL`) and merging adjacent free space until a big
/// enough free block is found. Splits oversized blocks only when the
/// leftover would exceed `MINFRAGMENT`.
///
/// The returned buffer is zeroed before being handed back. A non-purgeable
/// allocation without a user back-pointer is permitted; a purgeable one
/// without a user back-pointer is fatal (`I_Error`).
///
//* Port addition pinned in the baseline tests: the returned buffer is
//* zero-filled byte-by-byte while upstream C does NOT zero
//* (`z_zone.c:274-289`). Callers may (unverifiably) rely on the
//* zeroing -- never "fix" this into a memset or remove it.
//* Freeze-zone legacy `extern "C"` blocks link this function by its
//* upstream C symbol (`f_wipe.rs`, `r_draw.rs`), so the symbol is
//* pinned with `#[export_name]` instead of being dropped with the
//* rename.
///
/// # Safety
/// - `mainzone` must be initialised (`zone_init` ran). The function panics
///   if it is null.
/// - If `user` is non-null it must be a valid `*mut *mut c_void` that
///   the caller is willing to have written by both this call and any
///   future purge/free.
/// - `tag` must be a valid `PU_*` value (other values are silently
///   accepted but break the purge logic, matching C).
#[doc(alias = "Z_Malloc")]
#[export_name = "Z_Malloc"]
pub unsafe extern "C" fn zone_alloc(size: c_int, tag: c_int, user: *mut c_void) -> *mut c_void
{
    if mainzone.is_null()
    {
        panic!("Z_Malloc: mainzone is null!");
    }
    let size = (size + MEM_ALIGN as c_int - 1) & !(MEM_ALIGN as c_int - 1);
    let size = size + std::mem::size_of::<memblock_t>() as c_int;

    let mut base = (*mainzone).rover;
    if base.is_null()
    {
        panic!("Z_Malloc: rover is null!");
    }
    if (*base).prev.is_null()
    {
        panic!(
            "Z_Malloc: rover->prev is null! base={:?} base.tag={} base.size={}",
            base,
            (*base).tag,
            (*base).size
        );
    }
    if (*(*base).prev).tag == PU_FREE
    {
        base = (*base).prev;
    }

    let mut rover = base;
    let start = (*base).prev;

    loop
    {
        if rover == start
        {
            i_error!("Z_Malloc: failed on allocation");
        }

        if (*rover).tag != PU_FREE
        {
            if (*rover).tag < PU_PURGELEVEL
            {
                base = rover;
                rover = (*rover).next;
            }
            else
            {
                base = (*base).prev;
                let rover_ptr = rover as *mut u8;
                zone_free(rover_ptr.add(std::mem::size_of::<memblock_t>()) as *mut c_void);
                base = (*base).next;
                rover = (*base).next;
            }
        }
        else
        {
            rover = (*rover).next;
        }

        if (*base).tag == PU_FREE && (*base).size >= size
        {
            break;
        }
    }

    let extra = (*base).size - size;

    if extra > MINFRAGMENT
    {
        let newblock = (base as *mut u8).add(size as usize) as *mut memblock_t;
        (*newblock).size = extra;
        (*newblock).tag = PU_FREE;
        (*newblock).user = ptr::null_mut();
        (*newblock).prev = base;
        (*newblock).next = (*base).next;
        (*(*base).next).prev = newblock;
        (*base).next = newblock;
        (*base).size = size;
    }

    if user.is_null() && tag >= PU_PURGELEVEL
    {
        i_error!("Z_Malloc: an owner is required for purgable blocks");
    }

    (*base).user = user as *mut *mut c_void;
    (*base).tag = tag;

    let result = (base as *mut u8).add(std::mem::size_of::<memblock_t>()) as *mut c_void;

    // Zero the user data area using a byte-by-byte loop to avoid
    // memset writing past the end into the next block header.
    // (ptr::write_bytes / memset may use SIMD that overshoots on
    //  non-aligned sizes under ASan instrumentation.)
    let user_size = size - std::mem::size_of::<memblock_t>() as c_int;
    let result_bytes = result as *mut u8;
    for i in 0..user_size as usize
    {
        *result_bytes.add(i) = 0;
    }

    if !(*base).user.is_null()
    {
        *(*base).user = result;
    }

    (*mainzone).rover = (*base).next;
    (*base).id = ZONEID as c_int;

    result
}

/// `void Z_FreeTags(int lowtag, int hightag)` — free every live block
/// whose tag falls in `[lowtag, hightag]`. Called during level shutdown
/// to mass-release `PU_LEVEL` allocations.
///
/// The Rust port adds a 2000-block walk cap and stderr trace
/// (`[Z_FreeTags] ...`) not present in the C original; useful for
/// catching list corruption when porting bugs creep in.
///
/// # Safety
/// - `mainzone` must be initialised. All blocks in the active list must
///   be live allocations (the sentinel's tag is PU_STATIC, so it is
///   skipped by the tag-range check).
#[doc(alias = "Z_FreeTags")]
pub unsafe extern "C" fn zone_free_tags(lowtag: c_int, hightag: c_int)
{
    let zone = mainzone;
    let sentinel = std::ptr::addr_of_mut!((*zone).blocklist);
    let mut block = (*zone).blocklist.next;
    let mut freed = 0;
    let mut walked = 0;

    while block != sentinel && walked < 2000
    {
        walked += 1;
        let next = (*block).next;

        if (*block).tag != PU_FREE && (*block).tag >= lowtag && (*block).tag <= hightag
        {
            let block_ptr = block as *mut u8;
            zone_free(block_ptr.add(std::mem::size_of::<memblock_t>()) as *mut c_void);
            freed += 1;
        }

        block = next;
    }
    eprintln!(
        "[Z_FreeTags] lowtag={} hightag={} walked={} freed={}",
        lowtag, hightag, walked, freed
    );
}

/// `void Z_ChangeTag2(void *ptr, int tag, char *file, int line)` —
/// change the tag of an existing block. The `file`/`line` arguments come
/// from the `Z_ChangeTag` macro in `z_zone.h`; the Rust port ignores
/// them (the macro callsite is rewritten on the C side).
///
/// Refuses to mark a block purgeable (`tag >= PU_PURGELEVEL`) without a
/// user back-pointer, since the purge would otherwise leave a dangling
/// reference.
///
/// # Safety
/// - `ptr` must be a live allocation return value (header must carry
///   `ZONEID`); otherwise `I_Error` is called.
/// - `_file` / `_line` are unused but must still be valid pointers per
///   the C calling convention.
#[doc(alias = "Z_ChangeTag2")]
pub unsafe extern "C" fn change_tag(ptr: *mut c_void, tag: c_int, _file: *const c_char, _line: c_int)
{
    let block = (ptr as *mut u8).sub(std::mem::size_of::<memblock_t>()) as *mut memblock_t;

    if (*block).id as u32 != ZONEID
    {
        i_error!("Z_ChangeTag: block without a ZONEID!");
    }

    if tag >= PU_PURGELEVEL && (*block).user.is_null()
    {
        i_error!("Z_ChangeTag: an owner is required for purgable blocks");
    }

    (*block).tag = tag;
}

/// `void Z_ChangeUser(void *ptr, void **user)` — repoint a block's user
/// back-pointer and store `ptr` into `*user`. Used when the owning
/// structure moves but the data stays put.
///
/// # Safety
/// - `ptr` must be a live allocation return value (header must carry
///   `ZONEID`); otherwise `I_Error` is called.
/// - `user` must be a writable `*mut *mut c_void` whose storage will
///   outlive the block.
#[doc(alias = "Z_ChangeUser")]
pub unsafe extern "C" fn change_user(ptr: *mut c_void, user: *mut *mut c_void)
{
    let block = (ptr as *mut u8).sub(std::mem::size_of::<memblock_t>()) as *mut memblock_t;

    if (*block).id as u32 != ZONEID
    {
        i_error!("Z_ChangeUser: Tried to change user for invalid block!");
    }

    (*block).user = user;
    *user = ptr;
}

/// `unsigned int Z_ZoneSize(void)` — return the total backing-buffer
/// size in bytes as reported to [`zone_init`] by `I_ZoneBase`. Zone
/// sizing is ruled not demo-observable
/// (`docs/vanilla-workarounds.md`); this reports our size
/// consistently with `i_system`'s `DEFAULT_RAM`.
///
/// # Safety
/// - `mainzone` must be initialised; otherwise this dereferences a null
///   pointer.
#[doc(alias = "Z_ZoneSize")]
pub unsafe extern "C" fn zone_size() -> u32
{
    (*mainzone).size as u32
}

/// Baseline vectors for the Zone allocator, written against the
/// pre-move `Z_*` bodies and re-pointed to the graduated names after
/// the split -- same vectors, same results (F10 wave B5). They pin
/// the free-list behaviours the graduation must preserve: the
/// alloc/free round-trip, the coalescing invariant, tag-range
/// release, purge eviction, and the zone-size report.
#[cfg(test)]
mod tests
{
    use std::ffi::c_void;
    use std::sync::Mutex;

    use super::super::tags::{PU_CACHE, PU_LEVSPEC, PU_LEVEL, PU_STATIC};
    use super::*;

    //* Serialises every test that boots the Zone: `mainzone` is a
    //* process-global and `zone_init` swaps it, so allocator tests
    //* must not interleave with each other (per-module lock, r_draw
    //* precedent).
    static LOCK: Mutex<()> = Mutex::new(());

    //* Coalescing is asserted structurally (free-block count), not via
    //* `check_heap_quiet`: its 10 MB size heuristic flags the
    //* legitimately merged ~32 MiB tail free block as invalid on a
    //* fresh zone, so it can never return true here.
    unsafe fn count_free_blocks() -> usize
    {
        let zone = mainzone;
        let sentinel = std::ptr::addr_of_mut!((*zone).blocklist);
        let mut block = (*zone).blocklist.next;
        let mut count = 0;
        while block != sentinel
        {
            if (*block).tag == PU_FREE
            {
                count += 1;
            }
            block = (*block).next;
        }
        count
    }

    /// Alloc/free round-trip: a fresh allocation arrives zeroed (the
    /// documented Rust-port addition), the owner back-pointer is
    /// filled and cleared on free, and a both-sides-merge free order
    /// keeps the coalescing invariant (everything after the sentinel
    /// coalesces into ONE free block).
    #[test]
    fn alloc_free_roundtrip_and_coalesce()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            zone_init();
            assert!(!mainzone.is_null());

            let mut u1: *mut c_void = std::ptr::null_mut();
            let mut u2: *mut c_void = std::ptr::null_mut();
            let mut u3: *mut c_void = std::ptr::null_mut();

            let p1 = zone_alloc(128, PU_STATIC, &mut u1 as *mut _ as *mut c_void);
            let p2 = zone_alloc(128, PU_STATIC, &mut u2 as *mut _ as *mut c_void);
            let p3 = zone_alloc(128, PU_STATIC, &mut u3 as *mut _ as *mut c_void);
            assert!(!p1.is_null() && !p2.is_null() && !p3.is_null());
            assert!(p1 != p2 && p2 != p3 && p1 != p3);

            //* Port addition pinned here on purpose: upstream C does
            //* NOT zero; consumers may (unverifiably) rely on it.
            assert_eq!(*p1.cast::<u8>().add(127), 0);

            assert_eq!(u1, p1);

            // Free in an order that forces left+right merging around
            // the middle block's neighbours; everything after the
            // sentinel coalesces into ONE free block.
            zone_free(p2);
            zone_free(p1);
            zone_free(p3);

            assert_eq!(u1, std::ptr::null_mut());
            assert_eq!(u2, std::ptr::null_mut());
            assert_eq!(u3, std::ptr::null_mut());

            assert_eq!(count_free_blocks(), 1);
        }
    }

    /// Tag-range release: `zone_free_tags(PU_LEVEL, PU_PURGELEVEL - 1)`
    /// (the level-shutdown call shape in `p_setup`) frees
    /// `PU_LEVEL` and `PU_LEVSPEC` blocks and leaves `PU_CACHE`
    /// blocks live.
    #[test]
    fn free_tags_releases_level_range()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            zone_init();

            let mut ul: *mut c_void = std::ptr::null_mut();
            let mut us: *mut c_void = std::ptr::null_mut();
            let mut uc: *mut c_void = std::ptr::null_mut();

            let pl = zone_alloc(64, PU_LEVEL, &mut ul as *mut _ as *mut c_void);
            let ps = zone_alloc(64, PU_LEVSPEC, &mut us as *mut _ as *mut c_void);
            let pc = zone_alloc(64, PU_CACHE, &mut uc as *mut _ as *mut c_void);
            assert!(!pl.is_null() && !ps.is_null() && !pc.is_null());

            zone_free_tags(PU_LEVEL, PU_PURGELEVEL - 1);

            assert_eq!(ul, std::ptr::null_mut());
            assert_eq!(us, std::ptr::null_mut());
            assert_eq!(uc, pc);
            // The two freed blocks coalesce with each other and with
            // the zone tail; the live `PU_CACHE` block keeps its own
            // entry.
            assert_eq!(count_free_blocks(), 2);
        }
    }

    /// Purge eviction: when free space cannot satisfy a pinned
    /// request, `zone_alloc` reclaims purgeable blocks
    /// (`tag >= PU_PURGELEVEL`) and clears their user back-pointer.
    /// The entry rover is placed ON the purgeable victim -- the
    /// canonical entry state a fragmented cache-filled zone produces
    /// (the walk only adopts the freed block when `base` starts on
    /// it; every other layout errors exactly as upstream does).
    #[test]
    fn malloc_evicts_purgeable_blocks_when_full()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            zone_init();

            let avail = crate::doom::z_zone::diagnose::free_memory();
            assert!(avail > 1 << 20);

            let mut uv: *mut c_void = std::ptr::null_mut();
            // Purgeable victim taking nearly the whole Zone, then a
            // small pinned block after it.
            let victim = zone_alloc(avail - 16384, PU_CACHE, &mut uv as *mut _ as *mut c_void);
            let pin = zone_alloc(64, PU_STATIC, std::ptr::null_mut());
            assert!(!victim.is_null());
            assert_eq!(uv, victim);
            assert!(!pin.is_null());

            //* White-box entry state: point the rover at the victim's
            //* block header, the position a wandering rover holds in
            //* a real cache-filled zone when the walk must evict.
            (*mainzone).rover =
                (victim as *mut u8).sub(std::mem::size_of::<memblock_t>()) as *mut memblock_t;

            // Fits in the victim but not without evicting it (it is
            // still tagged PU_CACHE here).
            let probe = zone_alloc(avail - 32768, PU_STATIC, std::ptr::null_mut());
            assert!(!probe.is_null());
            assert_eq!(uv, std::ptr::null_mut(), "evicted block's owner must be cleared");
        }
    }

    /// Retag via `change_tag`: a `PU_STATIC` block retagged to
    /// `PU_CACHE` becomes releasable by the `PU_CACHE` tag range
    /// (the `w_wad` cache call shape). The
    /// purgable-without-owner `I_Error` corner is a process-exit
    /// surface (`std::process::exit`, not a panic) and stays
    /// untested by design.
    #[test]
    fn change_tag2_retag_makes_block_releasable()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            zone_init();

            let mut u: *mut c_void = std::ptr::null_mut();
            let p = zone_alloc(64, PU_STATIC, &mut u as *mut _ as *mut c_void);
            assert!(!p.is_null());

            change_tag(p, PU_CACHE, std::ptr::null(), 0);
            zone_free_tags(PU_CACHE, PU_CACHE);

            assert_eq!(u, std::ptr::null_mut());
            // The freed block coalesced into the single tail block.
            assert_eq!(count_free_blocks(), 1);
        }
    }

    /// `zone_size` reports exactly the buffer size `I_ZoneBase`
    /// handed to `zone_init` (zone sizing is ruled not
    /// demo-observable in `docs/vanilla-workarounds.md`; this pins
    /// consistency, not the size itself).
    #[test]
    fn zone_size_matches_i_zone_base()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            zone_init();

            let mut size: c_int = 0;
            let base = I_ZoneBase(&mut size);
            assert!(!base.is_null());
            assert_eq!(zone_size(), size as u32);
        }
    }
}
