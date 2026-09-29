//! Intermission draw primitives: the background slam, the
//! level-finished / entering overlays, the level-node placement
//! fitter, and the number/percent/time text generators.

use std::ffi::c_int;

use super::state::{background, colon, entering, finished, lnames, num, percent, sucks, wbs, wiminus, NUMCMAPS};
use super::tables::LNODES;
use super::types::WI_TITLEY;
use super::SHORT;
use crate::doom::crt::c_printf1;
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::v_video::patch_t;
use crate::doom::v_video::V_DrawPatch;

/// Draw the episode background graphic at the origin, covering the entire screen.
///
/// Called at the start of each draw function to paint the base image before
/// overlaying animations, labels, and statistics.
///
/// # Safety
///
/// Reads the global `background` patch pointer and calls `V_DrawPatch`.
/// Caller must ensure `lifecycle::load_data` has cached the patch and the video
/// backend is ready.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// every stats/map draw page reaches the upstream name through the
/// root shim.
#[doc(alias = "WI_slamBackground")]
#[export_name = "WI_slamBackground"]
pub unsafe extern "C" fn slam_background() {
    V_DrawPatch(0, 0, background);
}

/// Draw the "Finished" overlay: level name and "Finished" text at the top of the screen.
///
/// In commercial mode, does nothing for MAP33 and triggers an intentional patch
/// bounds error for map numbers above `NUMCMAPS` (preserving vanilla quirk - the
/// out-of-range patch lookup is an upstream bug retained for demo compatibility;
/// cataloged in `docs/vanilla-workarounds.md`).
///
/// # Safety
///
/// Dereferences the global `wbs` pointer plus the `lnames` array and the
/// `finished` patch pointer. Caller must ensure `lifecycle::start`/`load_data`
/// have run so these globals are valid.
///
/// Deliberately triggers a `V_DrawPatch` bounds error -- a vanilla-quirk
/// emulation, NEVER "fix" the bounds check here (cataloged workaround).
#[doc(alias = "WI_drawLF")]
pub(super) unsafe fn draw_level_finished() {
    let mut y = WI_TITLEY;

    if gamemode != d_mode::commercial || (*wbs).last < NUMCMAPS {
        V_DrawPatch(
            (SCREENWIDTH - SHORT((*(*lnames.offset((*wbs).last as isize))).width) as c_int) / 2,
            y,
            *lnames.offset((*wbs).last as isize),
        );
        y += (5 * SHORT((*(*lnames.offset((*wbs).last as isize))).height) as c_int) / 4;
        V_DrawPatch(
            (SCREENWIDTH - SHORT((*finished).width) as c_int) / 2,
            y,
            finished,
        );
    } else if (*wbs).last == NUMCMAPS {
        // MAP33 - nothing is displayed!
    } else if (*wbs).last > NUMCMAPS {
        // Deliberately trigger a V_DrawPatch error
        let tmp: patch_t = patch_t {
            width: SCREENWIDTH as i16,
            height: SCREENHEIGHT as i16,
            leftoffset: 1,
            topoffset: 1,
        };
        V_DrawPatch(0, y, &tmp as *const patch_t as *mut patch_t);
    }
}

/// Draw the "Entering" overlay: "Entering" text and the next level's name.
///
/// # Safety
///
/// Dereferences the global `wbs` pointer plus the `entering` patch and the
/// `lnames` array. Caller must ensure `lifecycle::start`/`load_data` have run
/// so these globals are valid.
#[doc(alias = "WI_drawEL")]
pub(super) unsafe fn draw_entering_level() {
    let mut y = WI_TITLEY;

    V_DrawPatch(
        (SCREENWIDTH - SHORT((*entering).width) as c_int) / 2,
        y,
        entering,
    );

    y += (5 * SHORT((*(*lnames.offset((*wbs).next as isize))).height) as c_int) / 4;

    V_DrawPatch(
        (SCREENWIDTH - SHORT((*(*lnames.offset((*wbs).next as isize))).width) as c_int) / 2,
        y,
        *lnames.offset((*wbs).next as isize),
    );
}

/// Draw the patch array `c` centred on the `n`-th level node of the current episode map.
///
/// Tries each frame in `c` (indices 0 then 1) and draws the first that fits
/// within the screen bounds. If neither fits, prints a diagnostic to stdout.
///
/// # Safety
///
/// `c` must point to an array of at least two `*mut patch_t` slots; the
/// function reads index 0 unconditionally and index 1 only if index 0 does
/// not fit on screen. Each non-null slot must point to a valid `patch_t`.
/// Dereferences the global `wbs` for the current episode and reads `LNODES`.
#[doc(alias = "WI_drawOnLnode")]
pub(super) unsafe fn draw_on_lnode(n: c_int, c: *mut *mut patch_t) {
    let mut i = 0;
    let mut fits = false;

    loop {
        let left = LNODES[(*wbs).epsd as usize][n as usize].x
            - SHORT((**c.offset(i as isize)).leftoffset) as c_int;
        let top = LNODES[(*wbs).epsd as usize][n as usize].y
            - SHORT((**c.offset(i as isize)).topoffset) as c_int;
        let right = left + SHORT((**c.offset(i as isize)).width) as c_int;
        let bottom = top + SHORT((**c.offset(i as isize)).height) as c_int;

        if left >= 0 && right < SCREENWIDTH && top >= 0 && bottom < SCREENHEIGHT {
            fits = true;
            break;
        }

        i += 1;
        if i == 2 || (*c.offset(i as isize)).is_null() {
            break;
        }
    }

    if fits && i < 2 {
        V_DrawPatch(
            LNODES[(*wbs).epsd as usize][n as usize].x,
            LNODES[(*wbs).epsd as usize][n as usize].y,
            *c.offset(i as isize),
        );
    } else {
        c_printf1(c"Could not place patch on level %d".as_ptr(), n + 1);
    }
}

/// Draw integer `n` right-justified at `(x, y)` using `digits` digit patches.
///
/// Returns the x position of the leftmost digit drawn (useful for chaining
/// displays). If `digits` is negative, the required digit count is computed
/// from `n`. The sentinel value 1994 suppresses drawing entirely (used when
/// no ammo type applies). Negative values prepend a minus sign.
///
/// # Safety
///
/// Reads the global `num` digit patches and `wiminus` patch. Caller must
/// ensure `lifecycle::load_data` has cached these patches before invoking.
#[doc(alias = "WI_drawNum")]
pub(super) unsafe fn draw_num(mut x: c_int, y: c_int, mut n: c_int, mut digits: c_int) -> c_int {
    let fontwidth = SHORT((*num[0]).width) as c_int;
    let neg = n < 0;
    if neg {
        n = -n;
    }

    if digits < 0 {
        if n == 0 {
            digits = 1;
        } else {
            digits = 0;
            let mut temp = n;
            while temp != 0 {
                temp /= 10;
                digits += 1;
            }
        }
    }

    // if non-number, do not draw it
    if n == 1994 {
        return 0;
    }

    while digits > 0 {
        digits -= 1;
        x -= fontwidth;
        V_DrawPatch(x, y, num[(n % 10) as usize]);
        n /= 10;
    }

    if neg {
        x -= 8;
        V_DrawPatch(x, y, wiminus);
    }

    x
}

/// Draw a percentage value: percent sign at `x`, then the number right-justified to the left of it.
///
/// No-op if `pct` is negative (stat not yet tallied).
///
/// # Safety
///
/// Reads the global `percent` patch and delegates to `draw_num`. Caller
/// must ensure `lifecycle::load_data` has cached the patches.
#[doc(alias = "WI_drawPercent")]
pub(super) unsafe fn draw_percent(x: c_int, y: c_int, pct: c_int) {
    if pct < 0 {
        return;
    }
    V_DrawPatch(x, y, percent);
    draw_num(x, y, pct, -1);
}

/// Draw elapsed time `t` (in seconds) right-justified at `(x, y)` in MM:SS format.
///
/// No-op if `t` is negative. If `t` exceeds the representable range (61 minutes 59 seconds),
/// draws the "SUCKS" patch instead.
///
/// # Safety
///
/// Reads the global `colon` and `sucks` patches and delegates to
/// `draw_num`. Caller must ensure `lifecycle::load_data` has cached the patches.
#[doc(alias = "WI_drawTime")]
pub(super) unsafe fn draw_time(mut x: c_int, y: c_int, t: c_int) {
    if t < 0 {
        return;
    }

    if t <= 61 * 59 {
        let mut div = 1;
        loop {
            let n = (t / div) % 60;
            x = draw_num(x, y, n, 2) - SHORT((*colon).width) as c_int;
            div *= 60;
            if div == 60 || t / div != 0 {
                V_DrawPatch(x, y, colon);
            }
            if t / div == 0 {
                break;
            }
        }
    } else {
        V_DrawPatch(x - SHORT((*sucks).width) as c_int, y, sucks);
    }
}
