//! The framebuffer state: the exported video statics, the `s_Fb` layout
//! descriptor, and the buffer lifecycle (`init_graphics`, the F1 M2
//! `reinit_video_buffer`, `shutdown_graphics`). `s_Fb` follows the
//! doomgeneric present buffer's dimensions (`dg_res_x` / `dg_res_y`) --
//! keep that coupling.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_float, c_int, c_void};
use std::mem;
use std::ptr;

use crate::doom::i_input::I_InitInput;
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};
use crate::doom::video_cfg::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::z_zone::{PU_STATIC, Z_Free, Z_Malloc};
use crate::i_error;

/// Bit-field descriptor for one colour channel inside the
/// framebuffer pixel layout. Mirrors `struct FB_BitField` in
/// `i_video.c`.
pub(super) struct FbBitField {
    /// Bit position of the channel's least-significant bit within a pixel.
    pub(super) offset: u32,
    /// Number of bits the channel occupies.
    pub(super) length: u32,
}

/// Framebuffer layout descriptor populated by `init_graphics`.
/// Mirrors `struct FB_ScreenInfo` in `i_video.c`.
pub(super) struct FbScreenInfo {
    /// Visible framebuffer width in pixels.
    pub(super) xres: u32,
    /// Visible framebuffer height in pixels.
    pub(super) yres: u32,
    /// Allocated framebuffer width (>= `xres`).
    pub(super) xres_virtual: u32,
    /// Allocated framebuffer height (>= `yres`).
    pub(super) yres_virtual: u32,
    /// Pixel size in bits. Only 16 and 32 are supported.
    pub(super) bits_per_pixel: u32,
    /// Red channel position and width.
    pub(super) red: FbBitField,
    /// Green channel position and width.
    pub(super) green: FbBitField,
    /// Blue channel position and width.
    pub(super) blue: FbBitField,
    /// Alpha / transparency channel position and width.
    pub(super) transp: FbBitField,
}

/// Pointer to the palette-indexed Doom screen buffer. Allocated in
/// the zone heap (`PU_STATIC`) during `init_graphics`. Mirrors
/// the C global `I_VideoBuffer`; extern-declared by `f_wipe` and
/// `r_draw`, so the `#[no_mangle]` symbol must not move.
#[no_mangle]
pub static mut I_VideoBuffer: *mut u8 = ptr::null_mut();

/// Non-zero when the screen is visible; the C source sets this to
/// `true` after init and never clears it. Mirrors `screenvisible`.
#[no_mangle]
pub static mut screenvisible: c_int = 0;

/// Non-zero if the engine is running as a screensaver. Always 0 in
/// this port. Mirrors `screensaver_mode`.
#[no_mangle]
pub static mut screensaver_mode: c_int = 0;

/// Gamma-correction level index into `gammatable`. Mirrors `usegamma`.
/// Bound via `m_config` (in the C source).
#[no_mangle]
pub static mut usegamma: c_int = 0;

/// DOS-style mouse acceleration factor. Movement above
/// `mouse_threshold` is multiplied by this. Mirrors `mouse_acceleration`.
#[no_mangle]
pub static mut mouse_acceleration: c_float = 2.0;

/// Mouse-movement threshold above which acceleration kicks in.
/// Mirrors `mouse_threshold`.
#[no_mangle]
pub static mut mouse_threshold: c_int = 10;

/// Integer upscaling factor used by `finish_update` to enlarge the
/// raster-sized Doom buffer to fit the framebuffer. Picked automatically
/// in `init_graphics`, overridden with `-scaling <n>`, and set by the
/// `video_cfg` reconfiguration thereafter (F1 M2).
#[no_mangle]
pub static mut fb_scaling: c_int = 1;

/// Non-zero to enable mouse input. Always 0 in this port unless set
/// by config. Mirrors `usemouse`.
#[no_mangle]
pub static mut usemouse: c_int = 0;

/// Active framebuffer layout. Initialised by `init_graphics`,
/// consumed by `cmap_to_framebuffer` / `finish_update`. Mirrors the C file
/// static `s_Fb`.
pub(super) static mut s_Fb: FbScreenInfo = FbScreenInfo {
    xres: 0,
    yres: 0,
    xres_virtual: 0,
    yres_virtual: 0,
    bits_per_pixel: 0,
    red: FbBitField {
        offset: 0,
        length: 0,
    },
    green: FbBitField {
        offset: 0,
        length: 0,
    },
    blue: FbBitField {
        offset: 0,
        length: 0,
    },
    transp: FbBitField {
        offset: 0,
        length: 0,
    },
};

/// Initialise the video subsystem: pick the framebuffer pixel layout
/// (`-gfxmode rgba8888|rgb565`, default rgba8888), compute the
/// integer scaling factor (`-scaling <n>` or auto-fit), allocate the
/// raster-sized palette buffer (dimensions from `video_cfg`; 320x200 at
/// the default config), mark the screen visible, and start input.
///
/// Mirrors `I_InitGraphics` from `i_video.c`. The Rust version
/// reads `-scaling` digits manually (no `atoi`) and skips the C
/// `printf` diagnostics.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` and the link anchor import the upstream name through
/// the root shim.
///
/// # Safety
///
/// Touches a number of mutable statics (`s_Fb`, `I_VideoBuffer`,
/// `fb_scaling`, `screenvisible`) and calls back into the zone
/// allocator. Must be called exactly once at startup before any
/// other I_* graphics function.
#[doc(alias = "I_InitGraphics")]
#[export_name = "I_InitGraphics"]
pub unsafe extern "C" fn init_graphics() {
    s_Fb = mem::zeroed::<FbScreenInfo>();
    s_Fb.xres = crate::doom::doomgeneric::dg_res_x() as u32;
    s_Fb.yres = crate::doom::doomgeneric::dg_res_y() as u32;
    s_Fb.xres_virtual = s_Fb.xres;
    s_Fb.yres_virtual = s_Fb.yres;

    // Default to rgba8888
    s_Fb.bits_per_pixel = 32;
    s_Fb.blue.length = 8;
    s_Fb.green.length = 8;
    s_Fb.red.length = 8;
    s_Fb.transp.length = 8;
    s_Fb.blue.offset = 0;
    s_Fb.green.offset = 8;
    s_Fb.red.offset = 16;
    s_Fb.transp.offset = 24;

    // Check for -gfxmode arg
    let gfxmodeparm = M_CheckParmWithArgs(c"-gfxmode".as_ptr().cast_mut(), 1);
    if gfxmodeparm != 0 {
        let mode = *myargv.add((gfxmodeparm + 1) as usize);
        if !mode.is_null() {
            let mode_str = std::ffi::CStr::from_ptr(mode);
            if mode_str.to_bytes() == b"rgba8888" {
                s_Fb.bits_per_pixel = 32;
                s_Fb.blue.length = 8;
                s_Fb.green.length = 8;
                s_Fb.red.length = 8;
                s_Fb.transp.length = 8;
                s_Fb.blue.offset = 0;
                s_Fb.green.offset = 8;
                s_Fb.red.offset = 16;
                s_Fb.transp.offset = 24;
            } else if mode_str.to_bytes() == b"rgb565" {
                s_Fb.bits_per_pixel = 16;
                s_Fb.blue.length = 5;
                s_Fb.green.length = 6;
                s_Fb.red.length = 5;
                s_Fb.transp.length = 0;
                s_Fb.blue.offset = 11;
                s_Fb.green.offset = 5;
                s_Fb.red.offset = 0;
                s_Fb.transp.offset = 16;
            } else {
                i_error!(
                    "Unknown gfxmode value: {}\n",
                    std::ffi::CStr::from_ptr(mode).to_string_lossy()
                );
            }
        }
    }

    // Auto-scaling factor
    let scale_parm = M_CheckParmWithArgs(c"-scaling".as_ptr().cast_mut(), 1);
    if scale_parm != 0 {
        let val = *myargv.add((scale_parm + 1) as usize);
        if !val.is_null() {
            // Simple atoi-like conversion
            let mut n: c_int = 0;
            let mut p = val;
            while *p >= b'0' as c_char && *p <= b'9' as c_char {
                n = n * 10 + (*p as c_int - b'0' as c_int);
                p = p.add(1);
            }
            fb_scaling = n;
        }
    } else {
        fb_scaling = (s_Fb.xres / SCREENWIDTH as u32) as c_int;
        let y_scale = (s_Fb.yres / SCREENHEIGHT as u32) as c_int;
        if y_scale < fb_scaling {
            fb_scaling = y_scale;
        }
    }

    // Allocate video buffer
    I_VideoBuffer = Z_Malloc(SCREENWIDTH * SCREENHEIGHT, PU_STATIC, ptr::null_mut()) as *mut u8;

    screenvisible = 1;

    I_InitInput();
}

/// Re-create the palette frame buffer and the present geometry for a new
/// raster size (F1 M2 `video_cfg` reconfiguration).
///
/// This is the runtime analogue of the allocation path in `init_graphics`:
/// frees the previous `I_VideoBuffer` (zone `PU_STATIC` block), allocates the
/// new one, and retargets the framebuffer descriptor and `fb_scaling` to the
/// doomgeneric present buffer's current dimensions.
///
/// The doomgeneric present buffer itself must already have been resized (see
/// `doomgeneric::realloc_screen_buffer`), because `s_Fb` follows its
/// dimensions. Failure to allocate the palette buffer follows the established
/// degradation contract: `I_Error` (native) / the log-channel banner (web).
///
/// Name kept (already house style; F1 M2 Rust-only, no C symbol).
///
/// # Safety
///
/// Frees and replaces the global `I_VideoBuffer`; callers must ensure no
/// frame render is in flight and must schedule a view rebuild (the
/// `video_cfg::apply` path does both).
pub unsafe fn reinit_video_buffer(width: c_int, height: c_int, fb_scaling_new: c_int) {
    if !I_VideoBuffer.is_null() {
        Z_Free(I_VideoBuffer as *mut c_void);
    }

    I_VideoBuffer = Z_Malloc(width * height, PU_STATIC, ptr::null_mut()) as *mut u8;
    if I_VideoBuffer.is_null() {
        i_error!(
            "video_cfg: failed to allocate {}x{} frame buffer\n",
            width,
            height
        );
    }

    fb_scaling = fb_scaling_new;
    s_Fb.xres = crate::doom::doomgeneric::dg_res_x() as u32;
    s_Fb.yres = crate::doom::doomgeneric::dg_res_y() as u32;
    s_Fb.xres_virtual = s_Fb.xres;
    s_Fb.yres_virtual = s_Fb.yres;
}

/// Release the palette-indexed screen buffer. Mirrors
/// `I_ShutdownGraphics` from `i_video.c`. The Rust version also
/// nulls `I_VideoBuffer` so a subsequent stray dereference faults
/// immediately rather than touching freed zone memory.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` and the link anchor import the upstream name through
/// the root shim.
///
/// # Safety
///
/// Calls into the zone allocator. After return, `I_VideoBuffer` is
/// null - callers must not access it without calling
/// `init_graphics` again.
#[doc(alias = "I_ShutdownGraphics")]
#[export_name = "I_ShutdownGraphics"]
pub unsafe extern "C" fn shutdown_graphics() {
    Z_Free(I_VideoBuffer as *mut c_void);
    I_VideoBuffer = ptr::null_mut();
}
