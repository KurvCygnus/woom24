//! The automap scale/window family: the `FTOM`/`MTOF`/`CXMTOF`/`CYMTOF`
//! coordinate conversions, the viewport save/restore/activate trio, the
//! level bounding-box scan, and the zoom/pan adjusters.

use std::ffi::c_int;

use super::consts::{islope_t, mline_t, PLAYERRADIUS};
use super::state::{
    f_h, f_oldloc, f_w, f_x, f_y, finit_height, finit_width, followplayer, max_h, max_scale_mtof,
    max_w, max_x, max_y, min_h, min_scale_mtof, min_w, min_x, min_y, m_h, m_paninc, m_w, m_x,
    m_x2, m_y, m_y2, old_m_h, old_m_w, old_m_x, old_m_y, plr, scale_ftom, scale_mtof,
};
use crate::doom::c_ffi::mobj_t;
use crate::doom::m_fixed::{fixed_t, FixedDiv, FixedMul};
use crate::doom::m_fixed::{FRACBITS, FRACUNIT};
use crate::doom::p_setup::{numvertexes, vertexes};

/// Convert a frame (screen-pixel) distance to a map-coordinate distance.
///
/// Equivalent to the C macro `FTOM(x)` which expands to
/// `FixedMul((x) << FRACBITS, scale_ftom)`.
///
/// # Safety
///
/// Reads `scale_ftom` which is a mutable static; must only be called while
/// the automap invariants hold.
#[inline(always)]
pub(super) unsafe fn ftom(x: c_int) -> fixed_t {
    FixedMul((x as fixed_t) << FRACBITS, scale_ftom)
}

/// Convert a map-coordinate distance to a frame (screen-pixel) distance.
///
/// Equivalent to the C macro `MTOF(x)` which expands to
/// `FixedMul((x), scale_mtof) >> FRACBITS`.
///
/// # Safety
///
/// Reads `scale_mtof` which is a mutable static; must only be called while
/// the automap invariants hold.
#[inline(always)]
pub(super) unsafe fn mtof(x: fixed_t) -> c_int {
    (FixedMul(x, scale_mtof) >> FRACBITS) as c_int
}

/// Convert a map x-coordinate to a frame x-coordinate (absolute screen column).
///
/// Accounts for the current viewport origin `m_x` and the frame offset `f_x`.
/// Equivalent to the C macro `CXMTOF(x)`.
///
/// # Safety
///
/// Reads multiple mutable statics; must only be called while the automap
/// invariants hold.
#[inline(always)]
pub(super) unsafe fn cxmtof(x: fixed_t) -> c_int {
    f_x + mtof(x - m_x)
}

/// Convert a map y-coordinate to a frame y-coordinate (absolute screen row).
///
/// Y is flipped: larger map y values correspond to smaller screen y values
/// (map north is screen up).  Equivalent to the C macro `CYMTOF(y)`.
///
/// # Safety
///
/// Reads multiple mutable statics; must only be called while the automap
/// invariants hold.
#[inline(always)]
pub(super) unsafe fn cymtof(y: fixed_t) -> c_int {
    f_y + (f_h - mtof(y - m_y))
}

/// Compute the forward and inverse slopes of the map line `ml`, storing results
/// in `*is`.
///
/// If `dy == 0` (horizontal line) the inverse slope is set to `±INT_MAX` to
/// avoid division by zero; likewise for `dx == 0`.  Used by the (currently
/// unused) slope-clipping code.
///
/// # Safety
///
/// `ml` and `is` must be valid non-null pointers.
#[doc(alias = "AM_getIslope")]
pub(super) unsafe fn get_islope(ml: *mut mline_t, is: *mut islope_t) {
    let dy = (*ml).a.y - (*ml).b.y;
    let dx = (*ml).b.x - (*ml).a.x;
    if dy == 0 {
        (*is).islp = if dx < 0 { -c_int::MAX } else { c_int::MAX };
    } else {
        (*is).islp = FixedDiv(dx, dy);
    }
    if dx == 0 {
        (*is).slp = if dy < 0 { -c_int::MAX } else { c_int::MAX };
    } else {
        (*is).slp = FixedDiv(dy, dx);
    }
}

/// Recompute the map viewport dimensions after a zoom change.
///
/// Keeps the viewport centred on its previous midpoint and updates `m_x2` /
/// `m_y2`.  C origin: `AM_activateNewScale` in am_map.c.
///
/// # Safety
///
/// Reads and writes multiple mutable statics; must only be called with the
/// automap active.
#[doc(alias = "AM_activateNewScale")]
pub(super) unsafe fn activate_new_scale() {
    m_x += m_w / 2;
    m_y += m_h / 2;
    m_w = ftom(f_w);
    m_h = ftom(f_h);
    m_x -= m_w / 2;
    m_y -= m_h / 2;
    m_x2 = m_x + m_w;
    m_y2 = m_y + m_h;
}

/// Save the current viewport position and zoom level into the `old_m_*` statics.
///
/// Called before switching to min-zoom so that the previous view can be
/// restored.  C origin: `AM_saveScaleAndLoc` in am_map.c.
///
/// # Safety
///
/// Reads mutable statics; must only be called with the automap active.
#[doc(alias = "AM_saveScaleAndLoc")]
pub(super) unsafe fn save_scale_and_loc() {
    old_m_x = m_x;
    old_m_y = m_y;
    old_m_w = m_w;
    old_m_h = m_h;
}

/// Restore the viewport position and zoom level from the `old_m_*` statics.
///
/// If follow mode is active, re-centres the viewport on the player rather than
/// restoring the saved origin.  Recalculates both scale factors.
/// C origin: `AM_restoreScaleAndLoc` in am_map.c.
///
/// # Safety
///
/// Reads and writes multiple mutable statics; `plr` must be a valid pointer.
#[doc(alias = "AM_restoreScaleAndLoc")]
pub(super) unsafe fn restore_scale_and_loc() {
    m_w = old_m_w;
    m_h = old_m_h;
    if followplayer == 0 {
        m_x = old_m_x;
        m_y = old_m_y;
    } else {
        m_x = (*((*plr).mo as *mut mobj_t)).x - m_w / 2;
        m_y = (*((*plr).mo as *mut mobj_t)).y - m_h / 2;
    }
    m_x2 = m_x + m_w;
    m_y2 = m_y + m_h;
    scale_mtof = FixedDiv((f_w as fixed_t) << FRACBITS, m_w);
    scale_ftom = FixedDiv(FRACUNIT, scale_mtof);
}

/// Compute the axis-aligned bounding box of all level vertices and derive the
/// min/max scale factors.
///
/// Sets `min_x`, `min_y`, `max_x`, `max_y`, `max_w`, `max_h`, `min_w`,
/// `min_h`, `min_scale_mtof`, and `max_scale_mtof`.
/// `min_scale_mtof` is the smaller of the x- and y-axis "fit whole level"
/// scales.  `max_scale_mtof` fits `2 * PLAYERRADIUS` within the frame height.
///
/// C origin: `AM_findMinMaxBoundaries` in am_map.c.
///
/// # Safety
///
/// `vertexes` and `numvertexes` must be valid (populated by `P_LoadVertexes`).
#[doc(alias = "AM_findMinMaxBoundaries")]
pub(super) unsafe fn find_min_max_boundaries() {
    min_x = c_int::MAX;
    min_y = c_int::MAX;
    max_x = -c_int::MAX;
    max_y = -c_int::MAX;

    for i in 0..numvertexes {
        let v = &*vertexes.add(i as usize);
        if v.x < min_x {
            min_x = v.x;
        } else if v.x > max_x {
            max_x = v.x;
        }
        if v.y < min_y {
            min_y = v.y;
        } else if v.y > max_y {
            max_y = v.y;
        }
    }

    max_w = max_x - min_x;
    max_h = max_y - min_y;

    min_w = 2 * PLAYERRADIUS;
    min_h = 2 * PLAYERRADIUS;

    let a = FixedDiv((f_w as fixed_t) << FRACBITS, max_w);
    let b = FixedDiv((f_h as fixed_t) << FRACBITS, max_h);
    min_scale_mtof = if a < b { a } else { b };
    max_scale_mtof = FixedDiv((f_h as fixed_t) << FRACBITS, 2 * PLAYERRADIUS);
}

/// Apply the pending pan increment and clamp the viewport to the level bounds.
///
/// If panning is active, disables follow mode (sets `followplayer = 0` and
/// invalidates `f_oldloc`).  The viewport centre is clamped so it cannot move
/// beyond the level bounding box.
///
/// C origin: `AM_changeWindowLoc` in am_map.c.
///
/// # Safety
///
/// Reads and writes multiple mutable statics; must only be called with the
/// automap active.
#[doc(alias = "AM_changeWindowLoc")]
pub(super) unsafe fn change_window_loc() {
    if m_paninc.x != 0 || m_paninc.y != 0 {
        followplayer = 0;
        f_oldloc.x = c_int::MAX;
    }

    m_x += m_paninc.x;
    m_y += m_paninc.y;

    if m_x + m_w / 2 > max_x {
        m_x = max_x - m_w / 2;
    } else if m_x + m_w / 2 < min_x {
        m_x = min_x - m_w / 2;
    }

    if m_y + m_h / 2 > max_y {
        m_y = max_y - m_h / 2;
    } else if m_y + m_h / 2 < min_y {
        m_y = min_y - m_h / 2;
    }

    m_x2 = m_x + m_w;
    m_y2 = m_y + m_h;
}

/// Perform per-level automap initialisation.
///
/// Clears marks, finds the level bounding box, and sets the initial scale
/// factor to show approximately 70 % of the minimum fit (clamped to the
/// maximum fit if that produces a smaller value).
///
/// C origin: `AM_LevelInit` in am_map.c.
///
/// # Safety
///
/// `vertexes` / `numvertexes` must be valid (level must be loaded).
#[doc(alias = "AM_LevelInit")]
pub(super) unsafe fn level_init() {
    super::state::leveljuststarted = 0;

    f_x = 0;
    f_y = 0;
    f_w = finit_width;
    f_h = finit_height;

    super::lifecycle::clear_marks();
    find_min_max_boundaries();

    scale_mtof = FixedDiv(min_scale_mtof, (0.7 * FRACUNIT as f64) as fixed_t);
    if scale_mtof > max_scale_mtof {
        scale_mtof = min_scale_mtof;
    }
    scale_ftom = FixedDiv(FRACUNIT, scale_mtof);
}

/// Set the zoom to the minimum scale (most zoomed out; fits the whole level).
///
/// C origin: `AM_minOutWindowScale` in am_map.c.
///
/// # Safety
///
/// Must be called with the automap active and map boundaries already computed.
#[doc(alias = "AM_minOutWindowScale")]
pub(super) unsafe fn min_out_window_scale() {
    scale_mtof = min_scale_mtof;
    scale_ftom = FixedDiv(FRACUNIT, scale_mtof);
    activate_new_scale();
}

/// Set the zoom to the maximum scale (most zoomed in; `2 * PLAYERRADIUS` fills
/// the frame height).
///
/// C origin: `AM_maxOutWindowScale` in am_map.c.
///
/// # Safety
///
/// Must be called with the automap active and map boundaries already computed.
#[doc(alias = "AM_maxOutWindowScale")]
pub(super) unsafe fn max_out_window_scale() {
    scale_mtof = max_scale_mtof;
    scale_ftom = FixedDiv(FRACUNIT, scale_mtof);
    activate_new_scale();
}

/// Apply the current zoom multipliers and clamp to the allowed scale range.
///
/// If the new scale would fall below `min_scale_mtof`, snaps to minimum zoom.
/// If it exceeds `max_scale_mtof`, snaps to maximum zoom.  Otherwise calls
/// `activate_new_scale` to recompute the viewport dimensions.
///
/// C origin: `AM_changeWindowScale` in am_map.c.
///
/// # Safety
///
/// Reads and writes mutable statics; must only be called with the automap active.
#[doc(alias = "AM_changeWindowScale")]
pub(super) unsafe fn change_window_scale() {
    use super::state::mtof_zoommul;
    scale_mtof = FixedMul(scale_mtof, mtof_zoommul);
    scale_ftom = FixedDiv(FRACUNIT, scale_mtof);

    if scale_mtof < min_scale_mtof {
        min_out_window_scale();
    } else if scale_mtof > max_scale_mtof {
        max_out_window_scale();
    } else {
        activate_new_scale();
    }
}

/// Centre the automap viewport on the player if the player has moved.
///
/// Compares the player's current position against `f_oldloc`; if different,
/// recentres the viewport and updates `f_oldloc`.  The centre is snapped to
/// the nearest map unit that corresponds to an integer screen pixel in order
/// to reduce jitter (`FTOM(MTOF(pos))`).
///
/// C origin: `AM_doFollowPlayer` in am_map.c.
///
/// # Safety
///
/// `plr` must be a valid pointer; reads mutable statics.
#[doc(alias = "AM_doFollowPlayer")]
pub(super) unsafe fn do_follow_player() {
    if f_oldloc.x != (*((*plr).mo as *mut mobj_t)).x
        || f_oldloc.y != (*((*plr).mo as *mut mobj_t)).y
    {
        m_x = ftom(mtof((*((*plr).mo as *mut mobj_t)).x)) - m_w / 2;
        m_y = ftom(mtof((*((*plr).mo as *mut mobj_t)).y)) - m_h / 2;
        m_x2 = m_x + m_w;
        m_y2 = m_y + m_h;
        f_oldloc.x = (*((*plr).mo as *mut mobj_t)).x;
        f_oldloc.y = (*((*plr).mo as *mut mobj_t)).y;
    }
}
