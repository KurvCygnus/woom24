//! The house-native video re-seat trio (fix round 1, Critical 2;
//! maintainer hand-pass era -- names kept, no C origin).

use std::ffi::c_int;

use super::state::{
    automapactive, f_h, f_w, f_x, f_y, fb, finit_height, finit_width, max_scale_mtof,
    min_scale_mtof, scale_ftom, scale_mtof,
};
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH, I_VideoBuffer};
use crate::doom::m_fixed::{fixed_t, FixedDiv, FRACUNIT};

/// Re-seat the automap's latched video state after the raster moved under
/// it (fix round 1, Critical 2).
///
/// A `video_cfg` reconfiguration swaps `I_VideoBuffer`; the automap's
/// `fb` copy then points into freed zone memory (UAF) and stale
/// `finit_width`/`finit_height` defeat the `AM_drawFline` clipping bounds.
/// While the map is open the view window and scale are also re-fit over
/// the new extent — `AM_LevelInit`'s re-seat minus the per-level mark
/// clear, because a resolution switch must not erase player marks.
///
/// Chosen over a per-frame re-seat in `AM_Drawer` (the other option the
/// review offered): re-reading `I_VideoBuffer` every frame keeps `fb`
/// fresh but leaves `finit_*`/`f_w`/`f_h` stale — still an out-of-bounds
/// hazard on a down-switch — and adds static reads to the draw hot path.
/// The change-detecting re-seat (see `AM_Drawer`) runs once per
/// reconfiguration and re-fits everything together.
pub unsafe fn AM_reseatVideoState()
{
    fb = I_VideoBuffer;
    finit_width = SCREENWIDTH;
    finit_height = SCREENHEIGHT - 32;

    if automapactive != 0
    {
        f_x = 0;
        f_y = 0;
        f_w = finit_width;
        f_h = finit_height;
        super::view::find_min_max_boundaries();
        scale_mtof = FixedDiv(min_scale_mtof, (0.7 * FRACUNIT as f64) as fixed_t);
        if scale_mtof > max_scale_mtof
        {
            scale_mtof = min_scale_mtof;
        }
        scale_ftom = FixedDiv(FRACUNIT, scale_mtof);
        super::view::activate_new_scale();
    }
}

/// The automap's latched framebuffer pointer (test / verification accessor
/// for the fix-round-1 re-seat).
pub fn am_framebuffer() -> *mut u8
{
    unsafe { fb }
}

/// The automap's window extent as `(finit_width, finit_height)` (test /
/// verification accessor for the fix-round-1 re-seat).
pub fn am_window_dims() -> (c_int, c_int)
{
    unsafe { (finit_width, finit_height) }
}
