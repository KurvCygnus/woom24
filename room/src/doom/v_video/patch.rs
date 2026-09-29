//! Patch drawing: the opaque/blit patch renderer family, the automap clip
//! callback, and the translucent (`TL`/`XLA`/shadowed) variants.

use std::ffi::c_int;
use std::ptr;

use crate::doom::i_video::SCREENWIDTH;

use super::blit::mark_rect;
use super::state::{column_t, dest_screen, patch_t, patchclip_callback, tinttable, vpatchclipfunc_t, xlatab};

/// Installs the per-patch clip callback consulted before every patch draw
/// (the automap uses it to clip patches against the map viewport).
#[doc(alias = "V_SetPatchClipCallback")]
#[export_name = "V_SetPatchClipCallback"]
pub extern "C" fn set_patch_clip_callback(func: vpatchclipfunc_t) {
    unsafe {
        patchclip_callback = func;
    }
}

/// Draws a WAD patch to the current destination buffer.
///
/// `x`/`y` are relative to the patch origin (shifted by the patch's
/// `leftoffset`/`topoffset`). The variable-length `columnofs` array is read
/// via `read_unaligned` pointer arithmetic (packed-struct idiom, kept
/// verbatim from the C port).
#[doc(alias = "V_DrawPatch")]
#[export_name = "V_DrawPatch"]
pub extern "C" fn draw_patch(x: c_int, y: c_int, patch: *mut patch_t) {
    unsafe {
        let y = y - (*patch).topoffset as c_int;
        let x = x - (*patch).leftoffset as c_int;

        if let Some(cb) = patchclip_callback {
            if cb(patch, x, y) == 0 {
                return;
            }
        }

        mark_rect(x, y, (*patch).width as c_int, (*patch).height as c_int);

        let w = (*patch).width as c_int;
        let mut desttop = dest_screen.add((y * SCREENWIDTH + x) as usize);

        let mut col = 0;
        while col < w {
            let ofs = ptr::read_unaligned((patch as *mut u8).add(8 + col as usize * 4) as *mut i32);
            let column = (patch as *mut u8).add(ofs as usize) as *mut column_t;

            let mut col_ptr = column;
            while (*col_ptr).topdelta != 0xff {
                let mut source = (col_ptr as *mut u8).add(3);
                let mut dest = desttop.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut count = (*col_ptr).length as c_int;

                while count > 0 {
                    *dest = *source;
                    dest = dest.add(SCREENWIDTH as usize);
                    source = source.add(1);
                    count -= 1;
                }

                col_ptr = (col_ptr as *mut u8).add((*col_ptr).length as usize + 4) as *mut column_t;
            }

            col += 1;
            desttop = desttop.add(1);
        }
    }
}

/// Mirrored variant of [`draw_patch`]: columns are taken right-to-left
/// (the `F_FINALE` text scroller's flipped Fireworks/Menubar patch).
#[doc(alias = "V_DrawPatchFlipped")]
#[export_name = "V_DrawPatchFlipped"]
pub extern "C" fn draw_patch_flipped(x: c_int, y: c_int, patch: *mut patch_t) {
    unsafe {
        let y = y - (*patch).topoffset as c_int;
        let x = x - (*patch).leftoffset as c_int;

        if let Some(cb) = patchclip_callback {
            if cb(patch, x, y) == 0 {
                return;
            }
        }

        mark_rect(x, y, (*patch).width as c_int, (*patch).height as c_int);

        let w = (*patch).width as c_int;
        let mut desttop = dest_screen.add((y * SCREENWIDTH + x) as usize);

        let mut col = 0;
        while col < w {
            let ofs = ptr::read_unaligned(
                (patch as *mut u8).add(8 + (w - 1 - col) as usize * 4) as *mut i32
            );
            let column = (patch as *mut u8).add(ofs as usize) as *mut column_t;

            let mut col_ptr = column;
            while (*col_ptr).topdelta != 0xff {
                let mut source = (col_ptr as *mut u8).add(3);
                let mut dest = desttop.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut count = (*col_ptr).length as c_int;

                while count > 0 {
                    *dest = *source;
                    dest = dest.add(SCREENWIDTH as usize);
                    source = source.add(1);
                    count -= 1;
                }

                col_ptr = (col_ptr as *mut u8).add((*col_ptr).length as usize + 4) as *mut column_t;
            }

            col += 1;
            desttop = desttop.add(1);
        }
    }
}

/// Identity delegation to [`draw_patch`] — matches upstream chocolate
/// behavior (the direct variant drew without the dirty-rect update in very
/// old sources). Kept as a separate symbol; do not merge the call sites.
#[doc(alias = "V_DrawPatchDirect")]
#[export_name = "V_DrawPatchDirect"]
pub extern "C" fn draw_patch_direct(x: c_int, y: c_int, patch: *mut patch_t) {
    draw_patch(x, y, patch);
}

/// Translucent (`TINTTAB`-indexed) patch draw.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[doc(alias = "V_DrawTLPatch")]
#[export_name = "V_DrawTLPatch"]
pub extern "C" fn draw_tl_patch(x: c_int, y: c_int, patch: *mut patch_t) {
    unsafe {
        let y = y - (*patch).topoffset as c_int;
        let x = x - (*patch).leftoffset as c_int;

        let w = (*patch).width as c_int;
        let mut desttop = dest_screen.add((y * SCREENWIDTH + x) as usize);

        let mut col = 0;
        while col < w {
            let ofs = ptr::read_unaligned((patch as *mut u8).add(8 + col as usize * 4) as *mut i32);
            let column = (patch as *mut u8).add(ofs as usize) as *mut column_t;

            let mut col_ptr = column;
            while (*col_ptr).topdelta != 0xff {
                let mut source = (col_ptr as *mut u8).add(3);
                let mut dest = desttop.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut count = (*col_ptr).length as c_int;

                while count > 0 {
                    let idx = ((*dest as usize) << 8) + (*source as usize);
                    *dest = *tinttable.add(idx);
                    dest = dest.add(SCREENWIDTH as usize);
                    source = source.add(1);
                    count -= 1;
                }

                col_ptr = (col_ptr as *mut u8).add((*col_ptr).length as usize + 4) as *mut column_t;
            }

            col += 1;
            desttop = desttop.add(1);
        }
    }
}

/// `XLATAB`-indexed translucent patch draw.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[doc(alias = "V_DrawXlaPatch")]
#[export_name = "V_DrawXlaPatch"]
pub extern "C" fn draw_xla_patch(x: c_int, y: c_int, patch: *mut patch_t) {
    unsafe {
        let y = y - (*patch).topoffset as c_int;
        let x = x - (*patch).leftoffset as c_int;

        if let Some(cb) = patchclip_callback {
            if cb(patch, x, y) == 0 {
                return;
            }
        }

        let w = (*patch).width as c_int;
        let mut desttop = dest_screen.add((y * SCREENWIDTH + x) as usize);

        let mut col = 0;
        while col < w {
            let ofs = ptr::read_unaligned((patch as *mut u8).add(8 + col as usize * 4) as *mut i32);
            let column = (patch as *mut u8).add(ofs as usize) as *mut column_t;

            let mut col_ptr = column;
            while (*col_ptr).topdelta != 0xff {
                let mut source = (col_ptr as *mut u8).add(3);
                let mut dest = desttop.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut count = (*col_ptr).length as c_int;

                while count > 0 {
                    let idx = (*dest as usize) + ((*source as usize) << 8);
                    *dest = *xlatab.add(idx);
                    source = source.add(1);
                    dest = dest.add(SCREENWIDTH as usize);
                    count -= 1;
                }

                col_ptr = (col_ptr as *mut u8).add((*col_ptr).length as usize + 4) as *mut column_t;
            }

            col += 1;
            desttop = desttop.add(1);
        }
    }
}

/// Alternate `TINTTAB`-indexed translucent patch draw.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[doc(alias = "V_DrawAltTLPatch")]
#[export_name = "V_DrawAltTLPatch"]
pub extern "C" fn draw_alt_tl_patch(x: c_int, y: c_int, patch: *mut patch_t) {
    unsafe {
        let y = y - (*patch).topoffset as c_int;
        let x = x - (*patch).leftoffset as c_int;

        let w = (*patch).width as c_int;
        let mut desttop = dest_screen.add((y * SCREENWIDTH + x) as usize);

        let mut col = 0;
        while col < w {
            let ofs = ptr::read_unaligned((patch as *mut u8).add(8 + col as usize * 4) as *mut i32);
            let column = (patch as *mut u8).add(ofs as usize) as *mut column_t;

            let mut col_ptr = column;
            while (*col_ptr).topdelta != 0xff {
                let mut source = (col_ptr as *mut u8).add(3);
                let mut dest = desttop.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut count = (*col_ptr).length as c_int;

                while count > 0 {
                    let idx = ((*dest as usize) << 8) + (*source as usize);
                    *dest = *tinttable.add(idx);
                    dest = dest.add(SCREENWIDTH as usize);
                    source = source.add(1);
                    count -= 1;
                }

                col_ptr = (col_ptr as *mut u8).add((*col_ptr).length as usize + 4) as *mut column_t;
            }

            col += 1;
            desttop = desttop.add(1);
        }
    }
}

/// Patch draw with a `TINTTAB`-darkened drop shadow two pixels down-right.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[doc(alias = "V_DrawShadowedPatch")]
#[export_name = "V_DrawShadowedPatch"]
pub extern "C" fn draw_shadowed_patch(x: c_int, y: c_int, patch: *mut patch_t) {
    unsafe {
        let y = y - (*patch).topoffset as c_int;
        let x = x - (*patch).leftoffset as c_int;

        let w = (*patch).width as c_int;
        let mut desttop = dest_screen.add((y * SCREENWIDTH + x) as usize);
        let mut desttop2 = dest_screen.add(((y + 2) * SCREENWIDTH + (x + 2)) as usize);

        let mut col = 0;
        while col < w {
            let ofs = ptr::read_unaligned((patch as *mut u8).add(8 + col as usize * 4) as *mut i32);
            let column = (patch as *mut u8).add(ofs as usize) as *mut column_t;

            let mut col_ptr = column;
            while (*col_ptr).topdelta != 0xff {
                let mut source = (col_ptr as *mut u8).add(3);
                let mut dest = desttop.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut dest2 = desttop2.add((*col_ptr).topdelta as usize * SCREENWIDTH as usize);
                let mut count = (*col_ptr).length as c_int;

                while count > 0 {
                    let idx = (*dest2 as usize) << 8;
                    *dest2 = *tinttable.add(idx);
                    dest2 = dest2.add(SCREENWIDTH as usize);
                    *dest = *source;
                    dest = dest.add(SCREENWIDTH as usize);
                    source = source.add(1);
                    count -= 1;
                }

                col_ptr = (col_ptr as *mut u8).add((*col_ptr).length as usize + 4) as *mut column_t;
            }

            col += 1;
            desttop = desttop.add(1);
            desttop2 = desttop2.add(1);
        }
    }
}

/// No-op clip callback used by the link anchor to create a symbol reference
/// for [`set_patch_clip_callback`]'s function-pointer parameter.
pub(super) extern "C" fn dummy_clip(_: *mut patch_t, _: c_int, _: c_int) -> c_int {
    0
}
