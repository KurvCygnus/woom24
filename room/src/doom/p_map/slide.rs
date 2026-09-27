//! Slide movement: the slide-state statics, the velocity projection onto
//! the hit wall (`hit_slide_line`), the wall-finding traversal callback,
//! and the three-trace `slide_move` resolver -- bit-exact with the slide
//! half of `vendor/doomgeneric/p_map.c`.

use std::ffi::{c_int, c_uint};
use std::ptr;

use crate::doom::c_ffi::{intercept_t, line_t, mobj_t, LinedefFlag};
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACUNIT};
use crate::doom::p_maputl::{
    openbottom, openrange, opentop, P_AproxDistance, P_LineOpening, P_PathTraverse,
    P_PointOnLineSide, PT_ADDLINES,
};
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::tables::{finecosine, finesine, ANG180, ANGLETOFINESHIFT};
use crate::i_error;

use super::consts::{ST_HORIZONTAL, ST_VERTICAL};
use super::move_::try_move;

/// Fractional distance along the current path to the first blocking wall.
///
/// Set by `ptr_slide_traverse` and consumed by `slide_move`.
/// Initialised to `FRACUNIT + 1` (beyond the end of the path) before each
/// path traversal so any real hit is closer.
#[no_mangle]
pub static mut bestslidefrac: fixed_t = 0;

/// Fractional distance to the second-best (backup) blocking wall.
///
/// Stored alongside `secondslideline` so that if the primary wall cannot
/// be slid along, `slide_move` can fall back to the next candidate.
#[no_mangle]
pub static mut secondslidefrac: fixed_t = 0;

/// The linedef closest to the sliding object along its current path.
///
/// Set by `ptr_slide_traverse`; used by `slide_move` to project the
/// remaining momentum along the wall via `hit_slide_line`.
#[no_mangle]
pub static mut bestslideline: *mut line_t = ptr::null_mut();

/// Backup linedef for the second-closest blocking wall found during sliding.
#[no_mangle]
pub static mut secondslideline: *mut line_t = ptr::null_mut();

/// The map object currently performing a slide move.
///
/// Set by `slide_move` and read by `ptr_slide_traverse` and
/// `hit_slide_line`.
#[no_mangle]
pub static mut slidemo: *mut mobj_t = ptr::null_mut();

/// Remaining x-component of momentum after clipping to a slide wall.
///
/// Written by `hit_slide_line` and applied by `slide_move`.
#[no_mangle]
pub static mut tmxmove: fixed_t = 0;

/// Remaining y-component of momentum after clipping to a slide wall.
///
/// Written by `hit_slide_line` and applied by `slide_move`.
#[no_mangle]
pub static mut tmymove: fixed_t = 0;

/// Project the current `(tmxmove, tmymove)` velocity onto linedef `ld`.
///
/// Computes the component of the velocity vector that runs parallel to the
/// wall so that the next `try_move` call will slide rather than stop.
/// Handles horizontal and vertical walls as fast special cases; uses
/// trigonometry for diagonal walls.
///
/// ## Technical Details
///
/// The finetable indexing is exact-trig load-bearing: the wall angle
/// (flipped by `ANG180` on the back side) and the `deltaangle` fixup
/// (`> ANG180 -> += ANG180`) feed `finecosine`/`finesine` lookups whose
/// exact index decides the projected momentum the next tic -- a one-ulp
/// difference in either index is a movement desync.
///
/// ## On Calling
///
/// `ld` must be a valid, non-null pointer to an initialised `line_t`;
/// `slidemo`, `tmxmove`, and `tmymove` must be set by the caller
/// (`slide_move`) before this function is called. Raw fixed-point
/// momentum in/out; single-threaded sim only.
///
/// # Safety
///
/// Raw `line_t` / `mobj_t` dereferences and finetable pointer walks; see
/// On Calling.
#[doc(alias = "P_HitSlideLine")]
#[export_name = "P_HitSlideLine"]
pub unsafe extern "C" fn hit_slide_line(ld: *mut line_t)
{
    let ld = &*ld;
    if ld.slopetype == ST_HORIZONTAL
    {
        tmymove = 0;
        return;
    }
    if ld.slopetype == ST_VERTICAL
    {
        tmxmove = 0;
        return;
    }
    let side = P_PointOnLineSide((*slidemo).x, (*slidemo).y, ld as *const _ as *mut _);
    let mut lineangle = R_PointToAngle2(0, 0, ld.dx, ld.dy);
    if side == 1
    {
        lineangle = lineangle.wrapping_add(ANG180);
    }
    let moveangle = R_PointToAngle2(0, 0, tmxmove, tmymove);
    let mut deltaangle = moveangle.wrapping_sub(lineangle);
    if deltaangle > ANG180
    {
        deltaangle = deltaangle.wrapping_add(ANG180);
    }
    let lineangle = (lineangle >> ANGLETOFINESHIFT) as usize;
    let deltaangle = (deltaangle >> ANGLETOFINESHIFT) as usize;
    let movelen = P_AproxDistance(tmxmove, tmymove);
    let newlen = FixedMul(movelen, *finecosine.0.add(deltaangle));
    tmxmove = FixedMul(newlen, *finecosine.0.add(lineangle));
    tmymove = FixedMul(newlen, finesine[lineangle]);
}

/// Path-traversal callback that finds blocking walls for slide movement.
///
/// Called by `P_PathTraverse` from `slide_move`. If the intercept is
/// not a line, the engine aborts with an error. For each blocking line the
/// fractional intercept is compared with `bestslidefrac`; closer hits
/// displace the current best and push the old best to the second slot.
///
/// Returns 1 to continue traversal (line is passable), 0 to stop.
///
/// ## Technical Details
///
/// The opening-accept predicate (`openrange >= height`, `opentop - z >=
/// height`, `openbottom - z <= 24*FRACUNIT`) and the best/second swap
/// order decide which wall the slide locks onto -- per-intercept
/// statement order is the demo-visible surface. The not-a-line case
/// reaches the `i_error!` abort, a panic-adjacent path kept at the moved
/// position unchanged.
///
/// ## On Calling
///
/// C ABI callback passed to `P_PathTraverse` by `slide_move` (retained
/// via the `PTR_SlideTraverse` export pin); never call it directly.
/// `in_` must be a valid, non-null pointer to an initialised
/// `intercept_t`; `slidemo` must point to the object being moved.
///
/// # Safety
///
/// Raw `intercept_t` / `line_t` dereferences and the `i_error!` path;
/// see On Calling.
#[doc(alias = "PTR_SlideTraverse")]
#[export_name = "PTR_SlideTraverse"]
pub unsafe extern "C" fn ptr_slide_traverse(in_: *mut intercept_t) -> c_uint
{
    let in_ = &*in_;
    if in_.isaline == 0
    {
        i_error!("PTR_SlideTraverse: not a line?");
    }
    let li = in_.d.line;
    if (*li).flags as c_int & LinedefFlag::TWOSIDED as c_int == 0
    {
        if P_PointOnLineSide((*slidemo).x, (*slidemo).y, li) != 0
        {
            return 1;
        }
    }
    else
    {
        P_LineOpening(li);
        if openrange >= (*slidemo).height
            && opentop - (*slidemo).z >= (*slidemo).height
            && openbottom - (*slidemo).z <= 24 * FRACUNIT
        {
            return 1;
        }
    }
    if in_.frac < bestslidefrac
    {
        secondslidefrac = bestslidefrac;
        secondslideline = bestslideline;
        bestslidefrac = in_.frac;
        bestslideline = li;
    }
    0
}

/// Move `mo` while sliding along the first wall it would otherwise hit.
///
/// Traces three leading-corner paths with `ptr_slide_traverse` to find
/// the nearest blocking wall, moves flush to it (with a small fudge
/// factor), then projects the remaining momentum along the wall via
/// `hit_slide_line` and calls `try_move` again. Falls back to a
/// "stairstep" (try pure-y then pure-x move) if no wall is found or after
/// three retries to prevent infinite loops.
///
/// ## Technical Details
///
/// The three-trace order (lead-x, trail-x/lead-y, lead-x/trail-y), the
/// `-= 0x800` wall-fudge, and the stairstep fallback ladder are all
/// statement-order demo surface: each retry consumes a trace whose
/// opening decisions can differ per tic, so any reorder changes the
/// position the next tic sees. The `hitcount == 3` cap is what keeps the
/// loop finite in vanilla; never "fix" it into a while-let.
///
/// ## On Calling
///
/// `mo` must be a valid, non-null pointer to a live `mobj_t`; map data
/// must be fully initialised. Mutates the slide statics above and drives
/// `try_move` -- single-threaded sim only.
///
/// # Safety
///
/// Raw `mobj_t` dereferences and global slide-state mutation; see On
/// Calling.
#[doc(alias = "P_SlideMove")]
#[export_name = "P_SlideMove"]
pub unsafe extern "C" fn slide_move(mo: *mut mobj_t)
{
    slidemo = mo;
    let mut hitcount = 0;

    'retry: loop
    {
        hitcount += 1;
        if hitcount == 3
        {
            // stairstep
            if try_move(mo, (*mo).x, (*mo).y + (*mo).momy) == 0
            {
                try_move(mo, (*mo).x + (*mo).momx, (*mo).y);
            }
            return;
        }

        let (leadx, trailx) = if (*mo).momx > 0
        {
            ((*mo).x + (*mo).radius, (*mo).x - (*mo).radius)
        }
        else
        {
            ((*mo).x - (*mo).radius, (*mo).x + (*mo).radius)
        };
        let (leady, traily) = if (*mo).momy > 0
        {
            ((*mo).y + (*mo).radius, (*mo).y - (*mo).radius)
        }
        else
        {
            ((*mo).y - (*mo).radius, (*mo).y + (*mo).radius)
        };

        bestslidefrac = FRACUNIT + 1;

        P_PathTraverse(
            leadx,
            leady,
            leadx + (*mo).momx,
            leady + (*mo).momy,
            PT_ADDLINES,
            Some(ptr_slide_traverse),
        );
        P_PathTraverse(
            trailx,
            leady,
            trailx + (*mo).momx,
            leady + (*mo).momy,
            PT_ADDLINES,
            Some(ptr_slide_traverse),
        );
        P_PathTraverse(
            leadx,
            traily,
            leadx + (*mo).momx,
            traily + (*mo).momy,
            PT_ADDLINES,
            Some(ptr_slide_traverse),
        );

        if bestslidefrac == FRACUNIT + 1
        {
            // stairstep
            if try_move(mo, (*mo).x, (*mo).y + (*mo).momy) == 0
            {
                try_move(mo, (*mo).x + (*mo).momx, (*mo).y);
            }
            return;
        }

        bestslidefrac -= 0x800;
        if bestslidefrac > 0
        {
            let newx = FixedMul((*mo).momx, bestslidefrac);
            let newy = FixedMul((*mo).momy, bestslidefrac);
            if try_move(mo, (*mo).x + newx, (*mo).y + newy) == 0
            {
                continue 'retry;
            }
        }

        bestslidefrac = FRACUNIT - (bestslidefrac + 0x800);
        if bestslidefrac > FRACUNIT
        {
            bestslidefrac = FRACUNIT;
        }
        if bestslidefrac <= 0
        {
            return;
        }
        tmxmove = FixedMul((*mo).momx, bestslidefrac);
        tmymove = FixedMul((*mo).momy, bestslidefrac);
        hit_slide_line(bestslideline);
        (*mo).momx = tmxmove;
        (*mo).momy = tmymove;
        if try_move(mo, (*mo).x + tmxmove, (*mo).y + tmymove) == 0
        {
            continue 'retry;
        }
        return;
    }
}
