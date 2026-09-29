//! The BSP traversal's mutable frame state: the three constants, the
//! diagnostic probe counter and seg-index helper, the `cliprange_t`
//! occlusion-entry type, the 7 `#[no_mangle]` statics of `r_bsp.c`, and
//! the module-private `solidsegs` / `newend` / `CHECKCOORD`. One data
//! home; upstream names + C linkage retained. `r_segs` consumes
//! `curline`/`frontsector`/`backsector`/`drawsegs`/`ds_p` (the
//! module-pair contract), `r_things` scans `drawsegs`/`ds_p` back-to-front
//! during sprite clipping -- every root path is held by the re-export
//! block in `mod.rs`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;
use std::ptr;

use crate::doom::p_setup::segs;

use super::types::{drawseg_t, line_t, seg_t, side_t, sector_t, ZERO_DRAWSEG};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of draw-segs that can be prepared in a single frame.
pub(super) const MAXDRAWSEGS: usize = 256;

/// Maximum number of solid clip-ranges tracked by the occlusion list.
pub(super) const MAXSEGS: usize = 32;

/// Flag bit in a BSP child index indicating the child is a subsector leaf,
/// not an interior node. Matches `NF_SUBSECTOR` in `doomdef.h`.
pub(super) const NF_SUBSECTOR: u32 = 0x8000;

// ---------------------------------------------------------------------------
// Diagnostic probes for the "walls disappear / different room appears" bug
// ---------------------------------------------------------------------------
//
// Enable with `RUST_LOG=room::doom::r_bsp=trace`. These are gated at TRACE so
// the existing debug log stays usable; terminate the app the moment the
// glitch appears and we inspect the tail of the trace.

/// Monotonically increasing frame counter used by diagnostic trace logging.
/// Incremented once per frame in `clear_clip_segs`.
pub(super) static mut PROBE_FRAME: u64 = 0;

/// Returns the index of `line` within the global `segs` array, or -1 if
/// either pointer is null. Used only for diagnostic trace logging.
#[inline]
pub(super) unsafe fn seg_index(line: *const seg_t) -> isize {
    if segs.is_null() || line.is_null() {
        -1
    } else {
        (line as isize - segs as isize) / std::mem::size_of::<seg_t>() as isize
    }
}

// ---------------------------------------------------------------------------
// cliprange_t (internal)
// ---------------------------------------------------------------------------

/// A solid horizontal screen-column range `[first, last]` that has been fully
/// covered by a previously drawn wall. The `solidsegs` array is a sorted,
/// non-overlapping list of these ranges, bounded by two sentinel entries.
/// Mirrors the `cliprange_t` struct defined locally in `r_bsp.c`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct cliprange_t {
    /// First (leftmost) solid column in this range, inclusive.
    pub first: c_int,
    /// Last (rightmost) solid column in this range, inclusive.
    pub last: c_int,
}

// ---------------------------------------------------------------------------
// Globals exported with #[no_mangle]
// ---------------------------------------------------------------------------

/// The seg currently being processed by `R_AddLine` and passed down to
/// `R_StoreWallRange`. Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut curline: *mut seg_t = ptr::null_mut();

/// The sidedef of the current seg (`curline->sidedef`). Set by `R_StoreWallRange`
/// in `r_segs.c`. Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut sidedef: *mut side_t = ptr::null_mut();

/// The linedef of the current seg (`curline->linedef`). Set by `R_StoreWallRange`
/// in `r_segs.c`. Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut linedef: *mut line_t = ptr::null_mut();

/// The sector on the front side of the current seg. Set by `subsector`
/// and read by `r_segs.c` and `r_plane`. Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut frontsector: *mut sector_t = ptr::null_mut();

/// The sector on the back side of the current seg, or null for single-sided
/// lines. Set by `add_line` and read by `r_segs.c`.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut backsector: *mut sector_t = ptr::null_mut();

/// Fixed-size array of prepared draw-segs for the current frame. Filled
/// sequentially via [`ds_p`]. Exported as `#[no_mangle]` for C callers
/// (read during sprite clipping in `r_things.c`).
#[no_mangle]
pub static mut drawsegs: [drawseg_t; MAXDRAWSEGS] = [ZERO_DRAWSEG; MAXDRAWSEGS];

/// Pointer to the next free slot in [`drawsegs`]. Advanced by
/// `R_StoreWallRange` each time a new draw-seg is committed.
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut ds_p: *mut drawseg_t = ptr::null_mut();

// ---------------------------------------------------------------------------
// Module-local state
// ---------------------------------------------------------------------------

/// Sorted array of solid screen-column ranges (clip list). Entries
/// `[0..newend)` are valid; the first and last entries are permanent
/// sentinels initialized by `clear_clip_segs`.
pub(super) static mut solidsegs: [cliprange_t; MAXSEGS] =
    [cliprange_t { first: 0, last: 0 }; MAXSEGS];

/// One-past-the-end pointer into [`solidsegs`], tracking how many ranges are
/// currently active. Maintained by `clip_solid_wall_segment` and reset by
/// `clear_clip_segs`.
pub(super) static mut newend: *mut cliprange_t = ptr::null_mut();

// checkcoord — 12 rows, only 11 initialised in C (rows 3 and 7 are {0})
/// Lookup table that selects the two diagonal corners of a bounding box to
/// use for angle computation in `check_bbox`, indexed by the 4-bit
/// combined viewer-position code `(boxy << 2) | boxx`. Four rows are
/// zero-filled and never meaningfully indexed: row 5 is the `boxpos == 5`
/// case (viewer inside the box) where `check_bbox` returns early before
/// reaching the table; rows 3 and 7 correspond to `boxx == 3`, which is
/// impossible because `boxx` can only be 0, 1, or 2; row 11 is padding
/// added by Rust to complete the array (the C original has only 11 entries,
/// indices 0-10). Each entry is four `BBox::{TOP,BOTTOM,LEFT,RIGHT}` indices.
/// Mirrors the `checkcoord` table from `r_bsp.c`.
pub(super) static CHECKCOORD: [[c_int; 4]; 12] = [
    [3, 0, 2, 1],
    [3, 0, 2, 0],
    [3, 1, 2, 0],
    [0, 0, 0, 0],
    [2, 0, 2, 1],
    [0, 0, 0, 0],
    [3, 1, 3, 0],
    [0, 0, 0, 0],
    [2, 0, 3, 1],
    [2, 1, 3, 1],
    [2, 1, 3, 0],
    [0, 0, 0, 0], // C has no 12th row; zero-padded
];
