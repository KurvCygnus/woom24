//! The art-screen stage: per-episode full-screen art patches and the
//! episode 3 bunny scroll with its END* pistol-shot overlay.

use std::ffi::c_char;
use std::ffi::c_int;
use std::ptr;

use super::DEH_String;
use crate::DEH_snprintf;
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::g_game::gameepisode;
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH, I_VideoBuffer};
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::v_video::{column_t, patch_t, V_DrawPatch, V_MarkRect};
use crate::doom::w_wad::W_CacheLumpName;
use crate::doom::z_zone::{PU_CACHE, PU_LEVEL};

/// The last `END*` animation frame shown during the bunny scroll.
///
/// Prevents a pistol sound from firing on every redraw of the same frame.
/// C origin: `laststage` (static local) in `F_BunnyScroll` in f_finale.c.
static mut LAST_STAGE: c_int = 0;

/// Draw a single column `col` of `patch` to column `x` of the video buffer,
/// stretching vertically according to the patch's column offsets.
///
/// Used by [`bunny_scroll`] to implement the horizontal scroll effect.
/// The patch column is read as a standard Doom post-format column
/// (topdelta / length / pixel data).  C origin: `F_DrawPatchCol` in
/// f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `artscreen::bunny_scroll` reaches the upstream name through the
/// root shim.
#[doc(alias = "F_DrawPatchCol")]
#[export_name = "F_DrawPatchCol"]
pub extern "C" fn draw_patch_col(x: c_int, patch: *mut patch_t, col: c_int) {
    unsafe {
        let patch_ptr = patch as *mut u8;
        let ofs = ptr::read_unaligned(patch_ptr.add(8 + col as usize * 4) as *mut i32);
        let mut column = patch_ptr.add(ofs as usize) as *mut column_t;
        let desttop = I_VideoBuffer.add(x as usize);

        while (*column).topdelta != 0xff {
            let mut source = (column as *mut u8).add(3);
            let mut dest = desttop.add((*column).topdelta as usize * SCREENWIDTH as usize);
            let mut count = (*column).length as c_int;
            while count > 0 {
                *dest = *source;
                dest = dest.add(SCREENWIDTH as usize);
                source = source.add(1);
                count -= 1;
            }
            column = (column as *mut u8).add((*column).length as usize + 4) as *mut column_t;
        }
    }
}

/// Draw the episode-3 bunny-scroll art-screen.
///
/// Horizontally scrolls two 320-wide patches (`PFUB2` then `PFUB1`) to the
/// left, starting after tick 230.  After tick 1130 an animated `END*` patch
/// sequence is overlaid in the centre of the screen, one new frame every 5
/// ticks (up to frame 6), with a pistol sound on each new frame.
///
/// Called by [`art_screen_drawer`].  C origin: `F_BunnyScroll` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `art_screen_drawer` reaches the upstream name through the root
/// shim.
#[doc(alias = "F_BunnyScroll")]
#[export_name = "F_BunnyScroll"]
pub extern "C" fn bunny_scroll() {
    unsafe {
        let p1 = W_CacheLumpName(DEH_String(c"PFUB2".as_ptr().cast_mut()), PU_LEVEL) as *mut patch_t;
        let p2 = W_CacheLumpName(DEH_String(c"PFUB1".as_ptr().cast_mut()), PU_LEVEL) as *mut patch_t;

        V_MarkRect(0, 0, SCREENWIDTH, SCREENHEIGHT);

        let mut scrolled = 320 - ((super::FINALE_COUNT as c_int - 230) / 2);
        scrolled = scrolled.clamp(0, 320);

        for x in 0..SCREENWIDTH {
            if x + scrolled < 320 {
                draw_patch_col(x, p1, x + scrolled);
            } else {
                draw_patch_col(x, p2, x + scrolled - 320);
            }
        }

        if super::FINALE_COUNT < 1130 { return; }
        if super::FINALE_COUNT < 1180 {
            V_DrawPatch(
                (SCREENWIDTH - 13 * 8) / 2,
                (SCREENHEIGHT - 8 * 8) / 2,
                W_CacheLumpName(DEH_String(c"END0".as_ptr().cast_mut()), PU_CACHE) as *mut patch_t,
            );
            LAST_STAGE = 0;
            return;
        }

        let mut stage = ((super::FINALE_COUNT as c_int) - 1180) / 5;
        if stage > 6 { stage = 6; }
        if stage > LAST_STAGE {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
            LAST_STAGE = stage;
        }

        let mut namebuf: [c_char; 10] = [0; 10];
        DEH_snprintf!(namebuf, "END{}", stage);
        V_DrawPatch(
            (SCREENWIDTH - 13 * 8) / 2,
            (SCREENHEIGHT - 8 * 8) / 2,
            W_CacheLumpName(namebuf.as_mut_ptr(), PU_CACHE) as *mut patch_t,
        );
    }
}

/// Draw the art-screen for the current episode (non-Cast, non-Text stage).
///
/// Episode 3 delegates to [`bunny_scroll`].  Episodes 1 (retail: CREDIT,
/// otherwise HELP2), 2 (VICTORY2), and 4 (ENDPIC) draw a full-screen patch.
/// Other episode numbers are silently ignored.
///
/// Called from [`super::lifecycle::drawer`].  C origin:
/// `F_ArtScreenDrawer` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle::drawer` reaches the upstream name through the root
/// shim.
#[doc(alias = "F_ArtScreenDrawer")]
#[export_name = "F_ArtScreenDrawer"]
pub extern "C" fn art_screen_drawer() {
    unsafe {
        if gameepisode == 3 {
            bunny_scroll();
            return;
        }

        let lumpname = match gameepisode {
            1 => {
                if gamemode == d_mode::retail {
                    c"CREDIT".as_ptr().cast_mut()
                } else {
                    c"HELP2".as_ptr().cast_mut()
                }
            }
            2 => c"VICTORY2".as_ptr().cast_mut(),
            4 => c"ENDPIC".as_ptr().cast_mut(),
            _ => return,
        };

        V_DrawPatch(
            0,
            0,
            W_CacheLumpName(DEH_String(lumpname), PU_CACHE) as *mut patch_t,
        );
    }
}
