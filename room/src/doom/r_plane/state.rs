//! The visplane renderer's data vocabulary and mutable frame state: the
//! `visplane_t` pool type (pad-byte sentinel layout pinned by compile-time
//! asserts), the nine private constants, the `finecosine` accessor, and
//! the 22 `#[no_mangle]` statics of `r_plane.c` -- one data home, upstream
//! names + C linkage retained. `r_main` writes `yslope`/`distscale` at
//! view-size init; `r_bsp` writes `floorplane`/`ceilingplane` and calls
//! `R_FindPlane`; `r_segs` writes the clip arrays and allocates from
//! `openings` via `lastopening` -- every root path is held by the
//! re-export block in `mod.rs`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_short, c_uchar};
use std::ptr;

use crate::doom::m_fixed::fixed_t;
use crate::doom::tables::{self, FINEMASK};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Compile-time array cap for per-column tables (F1 M2, boom
/// `MAX_SCREENWIDTH` shape): every per-column array in this module is sized
/// to the cap, and `video_cfg` validation rejects rasters past it.
pub(super) const MAXW: usize = crate::doom::video_cfg::MAX_SCREENWIDTH as usize;

/// Compile-time array cap for per-row tables; see [`MAXW`].
pub(super) const MAXH: usize = crate::doom::video_cfg::MAX_SCREENHEIGHT as usize;

/// Maximum number of simultaneous visplanes per frame.
///
/// Doom aborts with `I_Error` when this limit is exceeded.  The Rust port
/// currently returns a null pointer instead (see `find_plane`).
pub(super) const MAXVISPLANES: usize = 128;

/// Maximum number of `c_short` slots in the [`openings`] array.
///
/// Used as scratch space for sprite clipping arrays stored by
/// `R_StoreWallRange` (in `r_segs`).  The C source comments this as `"?"`.
pub(super) const MAXOPENINGS: usize = MAXW * 64;

/// Number of distinct light levels used by the colormap tables.
pub(super) const LIGHTLEVELS: usize = 16;

/// Shift applied to a sector's `lightlevel` to derive a colormap-table index.
pub(super) const LIGHTSEGSHIFT: u32 = 4;

/// Number of distance-based light entries per light level in `zlight`.
pub(super) const MAXLIGHTZ: usize = 128;

/// Shift applied to a world distance to derive an index into `zlight[n]`.
pub(super) const LIGHTZSHIFT: u32 = 20;

/// Shift applied to a view angle to derive a sky-texture column index.
///
/// Produces a coarser (less precise) mapping than `ANGLETOFINESHIFT` so that
/// the sky texture wraps once around the full horizontal field of view.
///
/// Upstream this constant lives in `r_sky.h`; the port defines it here
/// (moving it would churn `r_sky` for no gain).
pub(super) const ANGLETOSKYSHIFT: u32 = 22;

/// Safe accessor for finecosine table (it's a pointer into finesine).
///
/// # Safety
///
/// The index is masked to `FINEMASK` before dereferencing, so the access is
/// always within the bounds of the static sine table exported by
/// [`tables`].
#[inline]
pub(super) fn finecosine(idx: usize) -> c_int {
    unsafe { *tables::finecosine.0.add(idx & FINEMASK as usize) }
}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Colormap entry type: a single palette index byte.
pub(super) type lighttable_t = c_uchar;

/// Function pointer type for horizontal-span and sky-column draw callbacks.
///
/// The two parameters are the left (`x1`) and right (`x2`) screen columns of
/// the span, matching the C `planefunction_t` signature
/// `void (*)(int top, int bottom)`.
pub type planefunction_t = unsafe extern "C" fn(c_int, c_int);

// ---------------------------------------------------------------------------
// visplane_t — mirrors the C struct including pad bytes
// ---------------------------------------------------------------------------

/// One floor or ceiling plane accumulator.
///
/// A visplane records which columns (screen x) are covered by a particular
/// flat/height/lightlevel combination.  For each covered column, `top` and
/// `bottom` store the inclusive screen-y range that the span rasterizer must
/// fill.  A value of `0xFF` in `top[x]` means column `x` is not yet used.
///
/// The `pad1`/`pad2`/`pad3`/`pad4` bytes are intentional: the C renderer uses
/// `pl->top[pl->minx-1]` and `pl->top[pl->maxx+1]` as sentinel writes, which
/// land in the padding when the plane spans the full screen width.  The layout
/// is verified at compile time by the `assert!` blocks below.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct visplane_t {
    /// World height of the flat, in fixed-point units.
    pub height: fixed_t,
    /// Flat texture number (lump index relative to `firstflat`).
    pub picnum: c_int,
    /// Sector light level (0-255) for this plane.
    pub lightlevel: c_int,
    /// Leftmost screen column used by this plane (inclusive).
    pub minx: c_int,
    /// Rightmost screen column used by this plane (inclusive).
    pub maxx: c_int,
    /// Padding byte before `top[]`; acts as a sentinel slot for
    /// `top[minx-1]` when `minx == 0`.
    pub pad1: c_uchar,
    /// Per-column top clip (inclusive screen-y).  `0xFF` means unused.
    pub top: [c_uchar; MAXW],
    /// Padding byte after `top[]`; sentinel for `top[maxx+1]` when
    /// `maxx == SCREENWIDTH - 1`.
    pub pad2: c_uchar,
    /// Padding byte before `bottom[]`; mirrors the pad3 slot in C.
    pub pad3: c_uchar,
    /// Per-column bottom clip (inclusive screen-y).
    pub bottom: [c_uchar; MAXW],
    /// Padding byte after `bottom[]`.
    pub pad4: c_uchar,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(
    std::mem::size_of::<visplane_t>() == 2 * MAXW + 24,
    "visplane_t size mismatch"
);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, height) == 0);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, picnum) == 4);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, lightlevel) == 8);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, minx) == 12);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, maxx) == 16);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, pad1) == 20);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, top) == 21);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, pad2) == 21 + MAXW);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, pad3) == 22 + MAXW);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, bottom) == 23 + MAXW);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::offset_of!(visplane_t, pad4) == 23 + 2 * MAXW);

/// Default initialization for visplane pool entries.
impl Default for visplane_t {
    /// Returns a zeroed `visplane_t`, used to initialize the visplane pool.
    fn default() -> Self {
        Self {
            height: 0,
            picnum: 0,
            lightlevel: 0,
            minx: 0,
            maxx: 0,
            pad1: 0,
            top: [0xFF; MAXW],
            pad2: 0,
            pad3: 0,
            bottom: [0; MAXW],
            pad4: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Global mutable state (exported for C consumers)
// ---------------------------------------------------------------------------

/// Legacy floor-span callback pointer, carried over from the C source.
///
/// Not used in this port; `map_plane` does not invoke it.  Retained and
/// exported as `#[no_mangle]` for ABI compatibility with C consumers
/// (dead-but-exported; retires with the freeze zone).
#[no_mangle]
pub static mut floorfunc: Option<planefunction_t> = None;

/// Legacy ceiling-span callback pointer, carried over from the C source.
///
/// Not used in this port; `map_plane` does not invoke it.  Retained and
/// exported as `#[no_mangle]` for ABI compatibility with C consumers.
/// Parallels [`floorfunc`] (dead-but-exported; retires with the freeze
/// zone).
#[no_mangle]
pub static mut ceilingfunc: Option<planefunction_t> = None;

/// Fixed-size pool of visplane accumulators for one frame.
///
/// Exported as `#[no_mangle]` for C callers.  [`lastvisplane`] tracks how
/// many are in use.  Initialized to all-zero at program start; each slot is
/// reset via `find_plane` before use.
///
/// # Note
///
/// The compile-time initializer leaves `top[]` as `0x00` instead of `0xFF`.
/// `clear_planes` does not reset individual slots; only `find_plane`
/// writes the sentinel `0xFF` pattern when it claims a new slot.  This matches
/// the C behavior where `lastvisplane` is rewound to `visplanes[0]` each frame
/// and slots are re-initialized on demand.
#[no_mangle]
pub static mut visplanes: [visplane_t; MAXVISPLANES] = {
    // Can't use Default::default() in const context, so we transmute
    // from zeroed bytes. Each visplane_t is initialized at runtime.
    const ZERO: visplane_t = visplane_t {
        height: 0,
        picnum: 0,
        lightlevel: 0,
        minx: 0,
        maxx: 0,
        pad1: 0,
        top: [0; MAXW],
        pad2: 0,
        pad3: 0,
        bottom: [0; MAXW],
        pad4: 0,
    };
    let arr: [visplane_t; MAXVISPLANES] = [ZERO; MAXVISPLANES];
    // Initialize top to 0xFF for each visplane at runtime via memset-like
    // approach. For now leave as 0; R_ClearPlanes will reset them.
    arr
};

/// Pointer to the next free slot in [`visplanes`].
///
/// Exported as `#[no_mangle]` for C callers.  Reset to `&visplanes[0]` by
/// `clear_planes` at the start of each frame.
#[no_mangle]
pub static mut lastvisplane: *mut visplane_t = ptr::null_mut();

/// The current floor visplane being built for this BSP subtree.
///
/// Exported as `#[no_mangle]` for C callers.  Updated by
/// `R_StoreWallRange` (in `r_segs`) via `check_plane`.
#[no_mangle]
pub static mut floorplane: *mut visplane_t = ptr::null_mut();

/// The current ceiling visplane being built for this BSP subtree.
///
/// Exported as `#[no_mangle]` for C callers.  Updated by
/// `R_StoreWallRange` (in `r_segs`) via `check_plane`.
#[no_mangle]
pub static mut ceilingplane: *mut visplane_t = ptr::null_mut();

/// Scratch buffer for sprite clipping arrays.
///
/// Exported as `#[no_mangle]` for C callers.  Slices within this buffer are
/// handed out by `R_StoreWallRange` (in `r_segs`) and later read by the
/// sprite renderer.  [`lastopening`] tracks the allocation watermark.
#[no_mangle]
pub static mut openings: [c_short; MAXOPENINGS] = [0; MAXOPENINGS];

/// Allocation watermark within [`openings`].
///
/// Exported as `#[no_mangle]` for C callers.  Advanced by
/// `R_StoreWallRange` each time it allocates a clipping sub-array.
/// Reset to `&openings[0]` by `clear_planes`.
#[no_mangle]
pub static mut lastopening: *mut c_short = ptr::null_mut();

/// Per-column floor clip (highest opaque pixel so far, inclusive).
///
/// Exported as `#[no_mangle]` for C callers.  `floorclip[x]` is the
/// screen-y of the lowest pixel that is still open for floor rendering in
/// column `x`.  Initialized to `viewheight` (fully open) by
/// `clear_planes`.
#[no_mangle]
pub static mut floorclip: [c_short; MAXW] = [0; MAXW];

/// Per-column ceiling clip (lowest opaque pixel so far, inclusive).
///
/// Exported as `#[no_mangle]` for C callers.  `ceilingclip[x]` is the
/// screen-y of the highest pixel that is still open for ceiling rendering in
/// column `x`.  Initialized to `-1` (fully open) by `clear_planes`.
#[no_mangle]
pub static mut ceilingclip: [c_short; MAXW] = [0; MAXW];

/// Per-row span start columns, indexed by screen-y.
///
/// Exported as `#[no_mangle]` for C callers.  `make_spans` writes the
/// left edge of a span in progress here; `map_plane` reads it to obtain
/// `x1` when the span ends.
#[no_mangle]
pub static mut spanstart: [c_int; MAXH] = [0; MAXH];

/// Per-row span stop columns (unused in the current implementation).
///
/// Exported as `#[no_mangle]` for C callers.  Present in the C source but
/// never written; kept for ABI compatibility (dead-but-exported; retires
/// with the freeze zone).
#[no_mangle]
pub static mut spanstop: [c_int; MAXH] = [0; MAXH];

/// Pointer into the distance-based light table for the current plane.
///
/// Exported as `#[no_mangle]` for C callers.  Set by `draw_planes` from
/// `zlight[light]` before iterating a visplane's columns.
#[no_mangle]
pub static mut planezlight: *const *const lighttable_t = ptr::null();

/// Absolute height difference between the current flat and the viewpoint.
///
/// Exported as `#[no_mangle]` for C callers.  Written by `draw_planes`
/// before calling `make_spans`; read by `map_plane` to derive the
/// per-row texture step.
#[no_mangle]
pub static mut planeheight: fixed_t = 0;

/// Per-row slope factor used to convert plane distance to a screen-y fraction.
///
/// Exported as `#[no_mangle]` for C callers.  Precomputed by `R_ExecuteSetViewSize`
/// in `r_main` for each possible screen row.
#[no_mangle]
pub static mut yslope: [fixed_t; MAXH] = [0; MAXH];

/// Per-column angular distance scale from the screen center.
///
/// Exported as `#[no_mangle]` for C callers.  Precomputed by `R_ExecuteSetViewSize`
/// in `r_main`; used by [`map_plane`] to project a flat texel onto a column.
#[no_mangle]
pub static mut distscale: [fixed_t; MAXW] = [0; MAXW];

/// Base x-axis texture step per unit of distance, computed from `viewangle`.
///
/// Exported as `#[no_mangle]` for C callers.  Recomputed each frame by
/// `clear_planes`.
#[no_mangle]
pub static mut basexscale: fixed_t = 0;

/// Base y-axis texture step per unit of distance, computed from `viewangle`.
///
/// Exported as `#[no_mangle]` for C callers.  Recomputed each frame by
/// `clear_planes`.
#[no_mangle]
pub static mut baseyscale: fixed_t = 0;

/// Cache of the last `planeheight` value computed for each screen row.
///
/// Exported as `#[no_mangle]` for C callers.  [`map_plane`] avoids
/// recomputing `distance`, `xstep`, and `ystep` when the plane height has not
/// changed since the previous span on the same row.
#[no_mangle]
pub static mut cachedheight: [fixed_t; MAXH] = [0; MAXH];

/// Cache of the last computed world distance for each screen row.
///
/// Exported as `#[no_mangle]` for C callers.  Paired with [`cachedheight`].
#[no_mangle]
pub static mut cacheddistance: [fixed_t; MAXH] = [0; MAXH];

/// Cache of the last computed flat x-step for each screen row.
///
/// Exported as `#[no_mangle]` for C callers.  Paired with [`cachedheight`].
#[no_mangle]
pub static mut cachedxstep: [fixed_t; MAXH] = [0; MAXH];

/// Cache of the last computed flat y-step for each screen row.
///
/// Exported as `#[no_mangle]` for C callers.  Paired with [`cachedheight`].
#[no_mangle]
pub static mut cachedystep: [fixed_t; MAXH] = [0; MAXH];
