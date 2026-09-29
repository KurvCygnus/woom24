//! The present path: palette-index to framebuffer pixel converters and
//! the per-frame / per-tic raster hooks that drive them.

use std::ffi::c_int;

use super::buffer::{fb_scaling, s_Fb, I_VideoBuffer};
use super::palette::COLORS;
use crate::doom::i_input::I_GetEvent;
use crate::doom::video_cfg::{SCREENHEIGHT, SCREENWIDTH};

extern "C" {
    /// Platform hook called once per frame after `finish_update` has
    /// filled `DG_ScreenBuffer`. Implemented by the host front-end
    /// (GPU / headless / Wayland backend).
    fn DG_DrawFrame();
}

/// Convert `in_pixels` palette indices at `inp` into framebuffer
/// pixels at `out`, picking 16-bpp (delegates to `cmap_to_rgb565`)
/// or 32-bpp packing based on `s_Fb.bits_per_pixel`. Honours
/// `fb_scaling` by writing each pixel that many times.
///
/// # Safety
///
/// `inp` must be readable for `in_pixels` bytes. `out` must be
/// writable for `in_pixels * fb_scaling * (bpp/8)` bytes. The
/// caller is responsible for keeping `s_Fb` and `COLORS` valid.
unsafe fn cmap_to_framebuffer(out: *mut u8, inp: *mut u8, in_pixels: c_int) {
    let bpp = s_Fb.bits_per_pixel;

    if bpp == 16 {
        cmap_to_rgb565(out, inp, in_pixels);
        return;
    }

    // FIXME: C i_video.c calls I_Error for any bpp other than 16/32.
    // This port silently treats any non-16 value as 32-bpp, which
    // would write garbage rather than crashing if `bits_per_pixel`
    // is corrupted.
    if bpp != 32 {
        // For safety, default to 32bpp path
    }

    let mut out_ptr = out;
    let mut inp_ptr = inp;

    for _ in 0..in_pixels {
        let idx = *inp_ptr as usize;
        let c = COLORS[idx];

        let pix = ((c.r as u32) << s_Fb.red.offset)
            | ((c.g as u32) << s_Fb.green.offset)
            | ((c.b as u32) << s_Fb.blue.offset);

        for _ in 0..fb_scaling {
            (out_ptr as *mut u32).write_unaligned(pix);
            out_ptr = out_ptr.add(4);
        }

        inp_ptr = inp_ptr.add(1);
    }
}

/// Convert `in_pixels` palette indices at `inp` into RGB565
/// little-endian pixels at `out`. Each output pixel is written
/// `fb_scaling` times.
///
/// # Safety
///
/// Same preconditions as `cmap_to_framebuffer` (with 2-byte pixels).
unsafe fn cmap_to_rgb565(out: *mut u8, inp: *mut u8, in_pixels: c_int) {
    let mut out_ptr = out as *mut u16;
    let mut inp_ptr = inp;

    for _ in 0..in_pixels {
        let idx = *inp_ptr as usize;
        let c = COLORS[idx];

        let p = (((c.r as u16) & 0xF8) << 8) | (((c.g as u16) & 0xFC) << 3) | ((c.b as u16) >> 3);

        for _ in 0..fb_scaling {
            *out_ptr = p;
            out_ptr = out_ptr.add(1);
        }

        inp_ptr = inp_ptr.add(1);
    }
}

/// Per-frame "start" hook. No-op in this port (the C source also
/// does nothing here for the generic backend).
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers besides the link anchor,
/// kept for wasm symbol-set parity).
///
/// # Safety
///
/// Trivially safe; declared `unsafe extern "C"` to match the
/// engine's expected signature.
#[doc(alias = "I_StartFrame")]
#[export_name = "I_StartFrame"]
pub unsafe extern "C" fn start_frame() {}

/// Per-tic "start" hook. Pumps one input event via `I_GetEvent`.
/// Mirrors `I_StartTic` from `i_video.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop/mod.rs` extern-declares it.
///
/// # Safety
///
/// Forwards to `I_GetEvent`, which dereferences input-queue globals.
#[doc(alias = "I_StartTic")]
#[export_name = "I_StartTic"]
pub unsafe extern "C" fn start_tic() {
    I_GetEvent();
}

/// Stub that originally allowed an SDL "no blit" intermediate stage.
/// Always a no-op for the generic backend.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_UpdateNoBlit")]
#[export_name = "I_UpdateNoBlit"]
pub unsafe extern "C" fn update_no_blit() {}

/// Blit and present one frame.
///
/// Converts the raster-sized palette buffer at `I_VideoBuffer` into the
/// final framebuffer at `DG_ScreenBuffer`, centring it inside the
/// configured `s_Fb.xres x s_Fb.yres` window with `x_offset` and
/// `y_offset` padding bytes per row, repeating each Doom scanline
/// `fb_scaling` times to upscale. After the conversion, the platform
/// callback `DG_DrawFrame` is invoked to present.
///
/// The padding math saturates at zero (`.max(0)`) so a framebuffer
/// smaller than the scaled image still produces a valid pointer
/// arithmetic - corresponding C code performs unsigned subtraction
/// and would wrap around in that case.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Reads `SCREENWIDTH * SCREENHEIGHT` bytes from `I_VideoBuffer` and
/// writes a region of `DG_ScreenBuffer` whose extent depends on the
/// framebuffer geometry. Caller must ensure both pointers are valid
/// and the framebuffer is large enough for the configured layout.
#[doc(alias = "I_FinishUpdate")]
#[export_name = "I_FinishUpdate"]
pub unsafe extern "C" fn finish_update() {
    let y_offset = (((s_Fb.yres as i32 - (SCREENHEIGHT as c_int * fb_scaling))
        * (s_Fb.bits_per_pixel as c_int / 8))
        / 2)
    .max(0) as u32;
    let x_offset = (((s_Fb.xres as i32 - (SCREENWIDTH as c_int * fb_scaling))
        * (s_Fb.bits_per_pixel as c_int / 8))
        / 2)
    .max(0) as u32;
    let x_offset_end = ((s_Fb.xres as i32 - (SCREENWIDTH as c_int * fb_scaling))
        * (s_Fb.bits_per_pixel as c_int / 8)
        - x_offset as i32)
        .max(0) as u32;

    let mut line_in = I_VideoBuffer;
    let screen_ptr = crate::doom::doomgeneric::DG_ScreenBuffer as *mut u8;
    let mut line_out = screen_ptr.add(y_offset as usize + x_offset as usize);

    for _ in 0..SCREENHEIGHT as usize {
        for _i in 0..fb_scaling {
            line_out = line_out.add(x_offset as usize);

            cmap_to_framebuffer(line_out, line_in, SCREENWIDTH as c_int);

            line_out = line_out.add(
                (SCREENWIDTH as c_int * fb_scaling * (s_Fb.bits_per_pixel as c_int / 8)) as usize
                    + x_offset_end as usize,
            );
        }
        line_in = line_in.add(SCREENWIDTH as usize);
    }

    DG_DrawFrame();
}

/// Copy the entire palette-indexed screen buffer into `scr`. Used by
/// the wipe/melt screen-transition code. Mirrors `I_ReadScreen`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `f_wipe/mod.rs` extern-declares it.
///
/// # Safety
///
/// `scr` must be writable for `SCREENWIDTH * SCREENHEIGHT` bytes.
/// `I_VideoBuffer` must be initialised.
#[doc(alias = "I_ReadScreen")]
#[export_name = "I_ReadScreen"]
pub unsafe extern "C" fn read_screen(scr: *mut u8) {
    std::ptr::copy(I_VideoBuffer, scr, (SCREENWIDTH * SCREENHEIGHT) as usize);
}
