//! Blit primitives: dirty-rect marking, rectangular copies/fills, the box
//! and line drawers, buffer selection, and the `video_cfg` retarget hook.

use std::ffi::c_int;
use std::ptr;

use crate::doom::i_video::{I_VideoBuffer, SCREENHEIGHT, SCREENWIDTH};
use crate::doom::m_bbox::M_AddToBox;

use super::state::{dest_screen, dirtybox};

/// Marks a rectangle of the destination buffer dirty (grown into
/// [`dirtybox`]) so the present step blits it. Only tracks when the
/// destination is the primary framebuffer.
#[doc(alias = "V_MarkRect")]
#[export_name = "V_MarkRect"]
pub extern "C" fn mark_rect(x: c_int, y: c_int, width: c_int, height: c_int) {
    unsafe {
        if dest_screen == I_VideoBuffer {
            M_AddToBox(std::ptr::addr_of_mut!(dirtybox[0]), x, y);
            M_AddToBox(
                std::ptr::addr_of_mut!(dirtybox[0]),
                x + width - 1,
                y + height - 1,
            );
        }
    }
}

/// Copies a `width x height` block between two buffers, both addressed as
/// `SCREENWIDTH`-strided rows, marking the destination dirty.
#[doc(alias = "V_CopyRect")]
#[export_name = "V_CopyRect"]
pub extern "C" fn copy_rect(
    srcx: c_int,
    srcy: c_int,
    source: *mut u8,
    width: c_int,
    height: c_int,
    destx: c_int,
    desty: c_int,
) {
    unsafe {
        mark_rect(destx, desty, width, height);

        let mut src = source.add((SCREENWIDTH * srcy + srcx) as usize);
        let mut dest = dest_screen.add((SCREENWIDTH * desty + destx) as usize);

        let mut h = height;
        while h > 0 {
            ptr::copy_nonoverlapping(src, dest, width as usize);
            src = src.add(SCREENWIDTH as usize);
            dest = dest.add(SCREENWIDTH as usize);
            h -= 1;
        }
    }
}

/// Blits a `width x height` block from `src` to the destination buffer at
/// `(x, y)`, marking the area dirty (the wipe melt's captured start frame).
#[doc(alias = "V_DrawBlock")]
#[export_name = "V_DrawBlock"]
pub extern "C" fn draw_block(x: c_int, y: c_int, width: c_int, height: c_int, src: *mut u8) {
    unsafe {
        mark_rect(x, y, width, height);

        let mut dest = dest_screen.add((y * SCREENWIDTH + x) as usize);
        let mut source = src;
        let mut h = height;

        while h > 0 {
            ptr::copy_nonoverlapping(source, dest, width as usize);
            source = source.add(width as usize);
            dest = dest.add(SCREENWIDTH as usize);
            h -= 1;
        }
    }
}

/// Fills a `w x h` rectangle in the primary framebuffer with palette entry
/// `c` (the mouse-speed HUD background).
#[doc(alias = "V_DrawFilledBox")]
#[export_name = "V_DrawFilledBox"]
pub extern "C" fn draw_filled_box(x: c_int, y: c_int, w: c_int, h: c_int, c: c_int) {
    unsafe {
        let mut buf = I_VideoBuffer.add((SCREENWIDTH * y + x) as usize);

        let mut y1 = 0;
        while y1 < h {
            let mut buf1 = buf;
            let mut x1 = 0;
            while x1 < w {
                *buf1 = c as u8;
                buf1 = buf1.add(1);
                x1 += 1;
            }
            buf = buf.add(SCREENWIDTH as usize);
            y1 += 1;
        }
    }
}

/// Draws a horizontal run of `c`-colored pixels into the primary framebuffer.
#[doc(alias = "V_DrawHorizLine")]
#[export_name = "V_DrawHorizLine"]
pub extern "C" fn draw_horiz_line(x: c_int, y: c_int, w: c_int, c: c_int) {
    unsafe {
        let mut buf = I_VideoBuffer.add((SCREENWIDTH * y + x) as usize);
        let mut x1 = 0;
        while x1 < w {
            *buf = c as u8;
            buf = buf.add(1);
            x1 += 1;
        }
    }
}

/// Draws a vertical run of `c`-colored pixels into the primary framebuffer.
#[doc(alias = "V_DrawVertLine")]
#[export_name = "V_DrawVertLine"]
pub extern "C" fn draw_vert_line(x: c_int, y: c_int, h: c_int, c: c_int) {
    unsafe {
        let mut buf = I_VideoBuffer.add((SCREENWIDTH * y + x) as usize);
        let mut y1 = 0;
        while y1 < h {
            *buf = c as u8;
            buf = buf.add(SCREENWIDTH as usize);
            y1 += 1;
        }
    }
}

/// Draws an unfilled `w x h` box outline in palette color `c`.
#[doc(alias = "V_DrawBox")]
#[export_name = "V_DrawBox"]
pub extern "C" fn draw_box(x: c_int, y: c_int, w: c_int, h: c_int, c: c_int) {
    draw_horiz_line(x, y, w, c);
    draw_horiz_line(x, y + h - 1, w, c);
    draw_vert_line(x, y, h, c);
    draw_vert_line(x + w - 1, y, h, c);
}

/// Copies a full `SCREENWIDTH * SCREENHEIGHT` raw screen over the
/// destination buffer (intermission screen fetch).
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[doc(alias = "V_DrawRawScreen")]
#[export_name = "V_DrawRawScreen"]
pub extern "C" fn draw_raw_screen(raw: *mut u8) {
    unsafe {
        ptr::copy_nonoverlapping(raw, dest_screen, (SCREENWIDTH * SCREENHEIGHT) as usize);
    }
}

/// Video init hook; a no-op in this port (boot calls it, `d_main/boot.rs`).
#[doc(alias = "V_Init")]
#[export_name = "V_Init"]
pub extern "C" fn init() {
    // no-op
}

/// Points the `V_*` drawing primitives at `buffer` (off-screen draws: the
/// r_draw background buffer, the status-bar refresh).
#[doc(alias = "V_UseBuffer")]
#[export_name = "V_UseBuffer"]
pub extern "C" fn use_buffer(buffer: *mut u8) {
    unsafe {
        dest_screen = buffer;
    }
}

/// Returns the `V_*` drawing primitives to the primary framebuffer.
#[doc(alias = "V_RestoreBuffer")]
#[export_name = "V_RestoreBuffer"]
pub extern "C" fn restore_buffer() {
    unsafe {
        dest_screen = I_VideoBuffer;
    }
}

/// Retarget `dest_screen` after a `video_cfg` framebuffer swap: if the V_*
/// layer was pointed at the freed primary framebuffer (its pre-swap pointer
/// is `old_primary`), follow the new one; a `V_UseBuffer`-selected
/// off-screen buffer survives untouched.
pub(crate) unsafe fn retarget_after_framebuffer_swap(old_primary: *mut u8)
{
    if dest_screen == old_primary
    {
        dest_screen = I_VideoBuffer;
    }
}
