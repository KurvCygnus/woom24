//! The text stage: the finale's scrolling text printed
//! character-by-character over the tiling background flat.

use std::ffi::{c_char, c_int};
use std::ptr;

use super::text_tables::TEXTSPEED;
use super::{finaletext, finaleflat, FINALE_COUNT};
use crate::doom::hu_stuff::{hu_font, HU_FONTSIZE, HU_FONTSTART};
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH, I_VideoBuffer};
use crate::doom::v_video::{V_DrawPatch, V_MarkRect};
use crate::doom::w_wad::W_CacheLumpName;
use crate::doom::z_zone::PU_CACHE;

/// Tile the background flat across the screen and draw the finale text,
/// revealing characters one at a time based on `FINALE_COUNT`.
///
/// The flat is tiled as 64x64 blocks; text is rendered using the HUD font
/// starting at pixel `(10, 10)` with a line height of 11.  Characters are
/// revealed at the rate of one per `TEXTSPEED` ticks (with a 10-tick lead-in).
/// Unknown characters advance the cursor by 4 pixels.
///
/// Called from [`super::lifecycle::drawer`].  C origin: `F_TextWrite` in
/// f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle::drawer` reaches the upstream name through the root
/// shim.
#[doc(alias = "F_TextWrite")]
#[export_name = "F_TextWrite"]
pub extern "C" fn text_write() {
    unsafe {
        let src = W_CacheLumpName(finaleflat, PU_CACHE) as *mut u8;
        let mut dest = I_VideoBuffer;

        for y in 0..SCREENHEIGHT {
            let row_src = src.add(((y & 63) << 6) as usize);
            for _ in 0..(SCREENWIDTH / 64) {
                ptr::copy_nonoverlapping(row_src, dest, 64);
                dest = dest.add(64);
            }
            if SCREENWIDTH & 63 != 0 {
                let rem = (SCREENWIDTH & 63) as usize;
                ptr::copy_nonoverlapping(row_src, dest, rem);
                dest = dest.add(rem);
            }
        }

        V_MarkRect(0, 0, SCREENWIDTH, SCREENHEIGHT);

        let mut cx = 10;
        let mut cy = 10;
        let mut ch = finaletext;
        let mut count = (FINALE_COUNT as c_int - 10) / TEXTSPEED;
        if count < 0 { count = 0; }

        while count > 0 {
            count -= 1;
            let c = *ch;
            if c == 0 { break; }
            ch = ch.add(1);
            if c == b'\n' as c_char {
                cx = 10;
                cy += 11;
                continue;
            }

            let cidx = super::toupper(c as c_int) - HU_FONTSTART as c_int;
            if cidx < 0 || cidx >= HU_FONTSIZE as c_int {
                cx += 4;
                continue;
            }

            let font = hu_font[cidx as usize];
            if font.is_null() {
                cx += 4;
                continue;
            }
            let w = (*font).width as c_int;
            if cx + w > SCREENWIDTH {
                break;
            }
            V_DrawPatch(cx, cy, font);
            cx += w;
        }
    }
}
