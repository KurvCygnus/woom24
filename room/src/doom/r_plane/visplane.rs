//! Visplane pool management: the per-frame reset, the startup no-op, the
//! pool search/claim, and the split-or-extend decision. The pool-limit
//! divergence (C `I_Error`s at 128 visplanes, Rust returns null) and the
//! unchecked `lastvisplane` growth in `check_plane` are vanilla-faithful
//! carries -- limit-removal intake, never normalized here.

use std::ffi::{c_int, c_short};
use std::ptr;

use crate::doom::i_video::SCREENWIDTH;
use crate::doom::m_fixed::{fixed_t, FixedDiv};
use crate::doom::r_draw::{viewheight, viewwidth};
use crate::doom::r_main::centerxfrac;
use crate::doom::r_sky;
use crate::doom::tables::{self, ANG90, ANGLETOFINESHIFT, FINEMASK};

use super::state::{
    basexscale, baseyscale, ceilingclip, cachedheight, finecosine, floorclip, lastopening,
    lastvisplane, openings, visplane_t, visplanes, MAXVISPLANES,
};

/// Initialises the plane renderer at game startup.
///
/// The C source contains only a comment `"Doh!"`.  There is nothing to
/// initialise; the function exists so that `R_Init` can call it unconditionally
/// alongside the other renderer subsystems.
///
/// Exported as `#[no_mangle]` for C callers.
#[doc(alias = "R_InitPlanes")]
#[export_name = "R_InitPlanes"]
pub extern "C" fn init_planes() {
    // Doh! — nothing to do here, matching original C
}

/// Resets all plane state at the start of each frame.
///
/// Initialises clip arrays, rewinds the visplane and opening allocation
/// pointers, clears the per-row height cache, and recomputes the base
/// texture-step scales from the current `viewangle`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Writes to several `static mut` renderer globals and reads `viewwidth`,
/// `viewheight`, and `viewangle`.  Must be called exactly once per frame
/// before any BSP traversal begins.
#[doc(alias = "R_ClearPlanes")]
#[export_name = "R_ClearPlanes"]
pub extern "C" fn clear_planes() {
    unsafe {
        let vw = viewwidth as usize;
        let vh = viewheight as c_short;

        for i in 0..vw {
            floorclip[i] = vh;
            ceilingclip[i] = -1;
        }

        lastvisplane = std::ptr::addr_of_mut!(visplanes[0]);
        lastopening = std::ptr::addr_of_mut!(openings[0]);

        // Reset cache heights
        for h in std::slice::from_raw_parts_mut(std::ptr::addr_of_mut!(cachedheight[0]), 200) {
            *h = 0;
        }

        // Left to right mapping
        let angle = (crate::doom::r_main::viewangle.wrapping_sub(ANG90)) >> ANGLETOFINESHIFT;
        basexscale = FixedDiv(finecosine(angle as usize), centerxfrac);
        baseyscale = 0i32.wrapping_sub(FixedDiv(
            tables::finesine[angle as usize & FINEMASK as usize],
            centerxfrac,
        ));
    }
}

/// Returns a visplane for the given `height`, `picnum`, and `lightlevel`.
///
/// Searches the already-allocated visplanes for an exact match.  If none is
/// found, claims the next free slot from [`visplanes`] and initialises it.
/// All sky flats (`picnum == skyflatnum`) share a single visplane at height 0
/// and light level 0.
///
/// Returns a null pointer if the visplane pool (128 entries) is exhausted
/// (the C source would call `I_Error` instead).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads and writes the `static mut` globals [`visplanes`], [`lastvisplane`],
/// and `r_sky::skyflatnum`.  The returned pointer is valid for the lifetime
/// of the current frame (until the next `clear_planes` call).
// FIXME: C aborts with I_Error on MAXVISPLANES overflow; Rust returns null.
//        Callers in r_segs dereference the result unconditionally, which will
//        cause undefined behavior if the limit is hit.
#[doc(alias = "R_FindPlane")]
#[export_name = "R_FindPlane"]
pub extern "C" fn find_plane(
    height: fixed_t,
    picnum: c_int,
    lightlevel: c_int,
) -> *mut visplane_t {
    unsafe {
        let mut h = height;
        let mut ll = lightlevel;

        // All sky planes map together
        if picnum == r_sky::skyflatnum {
            h = 0;
            ll = 0;
        }

        // Search existing visplanes
        let mut check = std::ptr::addr_of_mut!(visplanes[0]);
        let end = lastvisplane;

        while check < end {
            if h == (*check).height && picnum == (*check).picnum && ll == (*check).lightlevel {
                return check;
            }
            check = check.add(1);
        }

        // Need a new visplane
        if (lastvisplane as usize - std::ptr::addr_of!(visplanes[0]) as usize)
            / std::mem::size_of::<visplane_t>()
            >= MAXVISPLANES
        {
            // Would call I_Error — for now just return null
            return ptr::null_mut();
        }

        let new_vp = lastvisplane;
        lastvisplane = lastvisplane.add(1);

        (*new_vp).height = h;
        (*new_vp).picnum = picnum;
        (*new_vp).lightlevel = ll;
        (*new_vp).minx = SCREENWIDTH as c_int;
        (*new_vp).maxx = -1;

        // memset top to 0xFF
        for t in (*new_vp).top.iter_mut() {
            *t = 0xFF;
        }

        new_vp
    }
}

/// Ensures the visplane `pl` can accommodate the column range `[start, stop]`.
///
/// If the range does not overlap any already-filled column in `pl`, the
/// plane's `minx`/`maxx` bounds are extended to cover the union and `pl` is
/// returned unchanged.  Otherwise a new visplane with the same flat/height/
/// lightlevel is allocated for `[start, stop]` and returned.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads and writes the `static mut` globals [`lastvisplane`] and the
/// [`visplane_t`] pointed to by `pl`.  `pl` must be a non-null pointer to a
/// valid, frame-lived visplane obtained from `find_plane`.
#[doc(alias = "R_CheckPlane")]
#[export_name = "R_CheckPlane"]
pub extern "C" fn check_plane(pl: *mut visplane_t, start: c_int, stop: c_int) -> *mut visplane_t {
    unsafe {
        let intrl = if start < (*pl).minx {
            (*pl).minx
        } else {
            start
        };
        let unionl = if start < (*pl).minx {
            start
        } else {
            (*pl).minx
        };

        let intrh = if stop > (*pl).maxx { (*pl).maxx } else { stop };
        let unionh = if stop > (*pl).maxx { stop } else { (*pl).maxx };

        // Check for overlap in the intersection range
        let mut x = intrl;
        while x <= intrh {
            if (*pl).top[x as usize] != 0xFF {
                break;
            }
            x += 1;
        }

        if x > intrh {
            // No overlap — extend the visplane
            (*pl).minx = unionl;
            (*pl).maxx = unionh;
            return pl;
        }

        // Make a new visplane
        let new_vp = lastvisplane;
        lastvisplane = lastvisplane.add(1);

        (*new_vp).height = (*pl).height;
        (*new_vp).picnum = (*pl).picnum;
        (*new_vp).lightlevel = (*pl).lightlevel;
        (*new_vp).minx = start;
        (*new_vp).maxx = stop;

        for t in (*new_vp).top.iter_mut() {
            *t = 0xFF;
        }

        new_vp
    }
}
