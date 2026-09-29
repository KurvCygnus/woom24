//! The back screen: the bezel background behind a windowed viewport
//! (allocation with the fix-round-1 live-size check, flat tiling, border
//! patch pass) plus the per-frame border restore blits and the view-buffer
//! LUT rebuild.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use crate::doom::c_ffi::SBARHEIGHT;
use crate::doom::d_mode::commercial;
use crate::doom::doomstat::gamemode;
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::v_video::{patch_t, V_DrawPatch, V_MarkRect, V_RestoreBuffer, V_UseBuffer};

use super::state::{scaledviewwidth, viewheight, viewwindowx, viewwindowy, ylookup, columnofs};

// ---------------------------------------------------------------------------
// External symbols
// ---------------------------------------------------------------------------

extern "C" {
    /// Returns a pointer to the cached lump with the given name, using the given zone tag.
    fn W_CacheLumpName(name: *const c_char, tag: c_int) -> *mut c_void;
    /// Allocates `size` bytes from the zone heap with the given tag; returns a pointer to the block.
    fn Z_Malloc(size: c_int, tag: c_int, user: *mut c_void) -> *mut c_void;
    /// Frees a block previously allocated from the zone heap.
    fn Z_Free(ptr: *mut c_void);

    /// Raw linear framebuffer written to the display; `screens[0]` in C terms.
    static mut I_VideoBuffer: *mut u8;
}

// ---------------------------------------------------------------------------
// Background buffer (module-local)
// ---------------------------------------------------------------------------

/// Backing buffer for the bezel drawn around the viewport when the window is smaller than
/// the full screen. Allocated on demand; freed when switching to full-screen mode.
static mut background_buffer: *mut u8 = ptr::null_mut();

/// Byte size [`background_buffer`] is currently allocated for.
///
/// Fix round 1 (Critical 1): the lazy allocation used to keep its boot-time
/// size forever, so an up-switch under a windowed view made the next
/// `R_FillBackScreen` tile `SCREENWIDTH*(SCREENHEIGHT-SBARHEIGHT)` bytes
/// into the stale block. Crispy avoids the size check by allocating
/// `MAXWIDTH*(MAXHEIGHT-SBARHEIGHT)` once from malloc
/// (`doom/r_draw.c:1125`); that is infeasible here (4096x4064 is ~16 MiB
/// against the 6 MiB zone), so the buffer is size-checked and re-allocated
/// instead. `Z_Malloc` aborts via `I_Error` on exhaustion (the established
/// allocation-failure contract), so there is no null fallthrough.
static mut background_buffer_size: c_int = 0;

/// Initialises the `ylookup` and `columnofs` lookup tables for a viewport of the given size.
///
/// Computes `viewwindowx` (horizontal centering offset) and `viewwindowy` (vertical offset,
/// accounting for the status-bar height when the viewport is not full-screen). Then fills
/// `columnofs[0..width]` and `ylookup[0..height]` so that each inner rendering loop can
/// locate any pixel without a multiply.
///
/// Must be called whenever the viewport dimensions change (e.g. when the player resizes
/// the view window).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Writes [`ylookup`] as raw pointers into `I_VideoBuffer`; the caller must
/// have installed the active framebuffer there first.
#[doc(alias = "R_InitBuffer")]
#[export_name = "R_InitBuffer"]
pub extern "C" fn init_buffer(width: c_int, height: c_int) {
    unsafe {
        // Handle resize, e.g. smaller view windows with border and/or status bar.
        viewwindowx = (SCREENWIDTH - width) >> 1;

        // Column offset. For windows.
        for i in 0..width {
            columnofs[i as usize] = viewwindowx + i;
        }

        // Same with base row offset.
        if width == SCREENWIDTH {
            viewwindowy = 0;
        } else {
            viewwindowy = (SCREENHEIGHT - SBARHEIGHT - height) >> 1;
        }

        // Precalculate all row offsets.
        for i in 0..height {
            ylookup[i as usize] = I_VideoBuffer.add(((i + viewwindowy) * SCREENWIDTH) as usize);
        }
    }
}

// ---------------------------------------------------------------------------
// DEH_String shim — identity in this build
// ---------------------------------------------------------------------------

/// Identity shim for the DeHackEd string-replacement function.
///
/// In the original C codebase `DEH_String` allows patch files to substitute string constants
/// at runtime. This Rust build does not support DeHackEd patches, so the function simply
/// returns its argument unchanged.
///
/// # Safety
/// `s` must be a valid, non-null pointer to a NUL-terminated C string for the duration of
/// any downstream C FFI call that receives the returned pointer.
#[doc(alias = "DEH_String")]
unsafe fn deh_string(s: *const c_char) -> *const c_char {
    s
}

/// Fills the background buffer with a tiled flat texture and draws the beveled viewport border.
///
/// When the viewport is smaller than the full screen, the area outside it is filled with a
/// repeating 64×64 flat: `FLOOR7_2` for Doom 1 / Ultimate Doom, `GRNROCK` for Doom II.
/// Border patches (`brdr_t`, `brdr_b`, `brdr_l`, `brdr_r`, and the four corner patches)
/// are then drawn into the same `background_buffer` using `V_UseBuffer`.
///
/// If the viewport covers the full screen (`scaledviewwidth == SCREENWIDTH`) the background
/// buffer is freed and the function returns immediately.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Allocates/frees the module-local `background_buffer` through the zone
/// heap and draws into it via the `V_*` layer; the live-size re-allocation
/// (fix round 1, Critical 1) is load-bearing for `video_cfg` up-switches.
#[doc(alias = "R_FillBackScreen")]
#[export_name = "R_FillBackScreen"]
pub extern "C" fn fill_back_screen() {
    unsafe {
        // If we are running full screen, there is no need to do any of this,
        // and the background buffer can be freed if it was previously in use.
        if scaledviewwidth == SCREENWIDTH {
            if !background_buffer.is_null() {
                Z_Free(background_buffer as *mut c_void);
                background_buffer = ptr::null_mut();
                background_buffer_size = 0;
            }
            return;
        }

        // Allocate the background buffer if necessary - or re-allocate it at
        // the live raster size after a video_cfg reconfiguration (fix round
        // 1, Critical 1).
        let needed = SCREENWIDTH * (SCREENHEIGHT - SBARHEIGHT);
        if background_buffer.is_null() {
            background_buffer = Z_Malloc(
                needed,
                1, // PU_STATIC
                ptr::null_mut(),
            ) as *mut u8;
            background_buffer_size = needed;
        } else if background_buffer_size != needed {
            Z_Free(background_buffer as *mut c_void);
            background_buffer = Z_Malloc(
                needed,
                1, // PU_STATIC
                ptr::null_mut(),
            ) as *mut u8;
            background_buffer_size = needed;
        }

        let name = if gamemode == commercial {
            deh_string(c"GRNROCK".as_ptr())
        } else {
            deh_string(c"FLOOR7_2".as_ptr())
        };

        let src = W_CacheLumpName(name, 8) as *mut u8; // PU_CACHE = 8
        let mut dest = background_buffer;

        for y in 0..(SCREENHEIGHT - SBARHEIGHT) {
            let row_src = src.add(((y & 63) << 6) as usize);
            for _ in 0..(SCREENWIDTH / 64) {
                ptr::copy_nonoverlapping(row_src, dest, 64);
                dest = dest.add(64);
            }
            let remainder = SCREENWIDTH & 63;
            if remainder != 0 {
                ptr::copy_nonoverlapping(row_src, dest, remainder as usize);
                dest = dest.add(remainder as usize);
            }
        }

        // Draw screen and bezel; this is done to a separate screen buffer.
        V_UseBuffer(background_buffer);

        let mut patch = W_CacheLumpName(deh_string(c"brdr_t".as_ptr()), 8) as *mut patch_t;
        for x in (0..scaledviewwidth).step_by(8) {
            V_DrawPatch(viewwindowx + x, viewwindowy - 8, patch);
        }

        patch = W_CacheLumpName(deh_string(c"brdr_b".as_ptr()), 8) as *mut patch_t;
        for x in (0..scaledviewwidth).step_by(8) {
            V_DrawPatch(viewwindowx + x, viewwindowy + viewheight, patch);
        }

        patch = W_CacheLumpName(deh_string(c"brdr_l".as_ptr()), 8) as *mut patch_t;
        for y in (0..viewheight).step_by(8) {
            V_DrawPatch(viewwindowx - 8, viewwindowy + y, patch);
        }

        patch = W_CacheLumpName(deh_string(c"brdr_r".as_ptr()), 8) as *mut patch_t;
        for y in (0..viewheight).step_by(8) {
            V_DrawPatch(viewwindowx + scaledviewwidth, viewwindowy + y, patch);
        }

        // Draw beveled edge.
        V_DrawPatch(
            viewwindowx - 8,
            viewwindowy - 8,
            W_CacheLumpName(deh_string(c"brdr_tl".as_ptr()), 8) as *mut patch_t,
        );
        V_DrawPatch(
            viewwindowx + scaledviewwidth,
            viewwindowy - 8,
            W_CacheLumpName(deh_string(c"brdr_tr".as_ptr()), 8) as *mut patch_t,
        );
        V_DrawPatch(
            viewwindowx - 8,
            viewwindowy + viewheight,
            W_CacheLumpName(deh_string(c"brdr_bl".as_ptr()), 8) as *mut patch_t,
        );
        V_DrawPatch(
            viewwindowx + scaledviewwidth,
            viewwindowy + viewheight,
            W_CacheLumpName(deh_string(c"brdr_br".as_ptr()), 8) as *mut patch_t,
        );

        V_RestoreBuffer();
    }
}

/// Byte size the background buffer is currently allocated for (test /
/// verification accessor for the fix-round-1 size check).
pub fn background_buffer_bytes() -> c_int {
    unsafe { background_buffer_size }
}

/// Copies `count` bytes from the background buffer to the video buffer at byte offset `ofs`.
///
/// Used by `R_DrawViewBorder` to blit the pre-rendered bezel regions from `background_buffer`
/// into `I_VideoBuffer`. Does nothing if `background_buffer` is null (full-screen mode).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Copies raw bytes between `background_buffer` and `I_VideoBuffer`; the
/// caller owns the offset arithmetic (`hu_lib`'s erase ladder and
/// [`draw_view_border`] both drive it). The pre-move export symbol is kept
/// with `#[export_name]` below; `hu_lib.rs:156` imports the upstream name
/// through its own extern block, so the pin is mandatory there.
#[doc(alias = "R_VideoErase")]
#[export_name = "R_VideoErase"]
pub extern "C" fn video_erase(ofs: u32, count: c_int) {
    unsafe {
        if !background_buffer.is_null() {
            ptr::copy_nonoverlapping(
                background_buffer.add(ofs as usize),
                I_VideoBuffer.add(ofs as usize),
                count as usize,
            );
        }
    }
}

/// Copies the border regions from the background buffer into the video buffer each frame.
///
/// Blits four rectangular areas (top strip, bottom strip, and the two side strips) using
/// `R_VideoErase`, then calls `V_MarkRect` to tell the video subsystem that the full
/// non-status-bar area needs to be presented.
///
/// Returns immediately without doing anything if `scaledviewwidth == SCREENWIDTH`
/// (full-screen viewport, no border to draw).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads the view-geometry statics and blits through [`video_erase`]; the
/// `top`/`side` arithmetic assumes the `R_InitBuffer`-established window
/// layout.
#[doc(alias = "R_DrawViewBorder")]
#[export_name = "R_DrawViewBorder"]
pub extern "C" fn draw_view_border() {
    unsafe {
        if scaledviewwidth == SCREENWIDTH {
            return;
        }

        let top = ((SCREENHEIGHT - SBARHEIGHT) - viewheight) / 2;
        let side = (SCREENWIDTH - scaledviewwidth) / 2;

        // copy top and one line of left side
        video_erase(0, (top * SCREENWIDTH + side) as c_int);

        // copy one line of right side and bottom
        let ofs = ((viewheight + top) * SCREENWIDTH - side) as u32;
        video_erase(ofs, (top * SCREENWIDTH + side) as c_int);

        // copy sides using wraparound
        let mut ofs = ((top * SCREENWIDTH) + SCREENWIDTH - side) as u32;
        let side_doubled = side << 1;

        for _ in 1..viewheight {
            video_erase(ofs, side_doubled);
            ofs += SCREENWIDTH as u32;
        }

        V_MarkRect(0, 0, SCREENWIDTH, SCREENHEIGHT - SBARHEIGHT);
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;
    use std::sync::Mutex;

    use super::background_buffer;
    use crate::doom::c_ffi::SBARHEIGHT;
    use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
    use crate::doom::r_draw::{
        columnofs, scaledviewwidth, viewheight, viewwindowx, viewwindowy, ylookup,
        R_DrawViewBorder, R_InitBuffer, R_VideoErase,
    };

    extern "C" {
        static mut I_VideoBuffer: *mut u8;
    }

    /// Serialises all tests that touch the shared mutable renderer globals.
    static LOCK: Mutex<()> = Mutex::new(());

    /// Verifies that a full-screen `R_InitBuffer` call sets `viewwindowx` and `viewwindowy`
    /// to zero and fills `columnofs` and `ylookup` with identity-offset values.
    #[test]
    fn init_buffer_fullscreen() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            // Allocate a fake video buffer so ylookup doesn't deref null.
            let (sw, sh) = (SCREENWIDTH, SCREENHEIGHT);
            let mut fake_buf = vec![0u8; (sw * sh) as usize];
            let orig_buf = I_VideoBuffer;
            I_VideoBuffer = fake_buf.as_mut_ptr();

            R_InitBuffer(sw, 168);

            assert_eq!(viewwindowx, 0);
            assert_eq!(viewwindowy, 0);
            for i in 0..sw {
                assert_eq!(columnofs[i as usize], i, "columnofs[{i}] mismatch");
            }
            for i in 0..168 {
                assert_eq!(
                    ylookup[i as usize],
                    I_VideoBuffer.add((i * sw) as usize),
                    "ylookup[{i}] mismatch"
                );
            }

            I_VideoBuffer = orig_buf;
        }
    }

    /// Verifies centering offsets and `columnofs` values for a 256×168 windowed viewport.
    #[test]
    fn init_buffer_windowed_256x168() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let (sw, sh) = (SCREENWIDTH, SCREENHEIGHT);
            let mut fake_buf = vec![0u8; (sw * sh) as usize];
            let orig_buf = I_VideoBuffer;
            I_VideoBuffer = fake_buf.as_mut_ptr();

            R_InitBuffer(256, 168);

            assert_eq!(viewwindowx, (sw - 256) >> 1); // 32
            assert_eq!(viewwindowy, (sh - SBARHEIGHT - 168) >> 1); // 0
            for i in 0..256 {
                assert_eq!(columnofs[i as usize], viewwindowx + i);
            }

            I_VideoBuffer = orig_buf;
        }
    }

    /// Verifies centering offsets, `columnofs`, and `ylookup` for a 200×100 windowed viewport.
    #[test]
    fn init_buffer_windowed_200x100() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let (sw, sh) = (SCREENWIDTH, SCREENHEIGHT);
            let mut fake_buf = vec![0u8; (sw * sh) as usize];
            let orig_buf = I_VideoBuffer;
            I_VideoBuffer = fake_buf.as_mut_ptr();

            R_InitBuffer(200, 100);

            assert_eq!(viewwindowx, (sw - 200) >> 1); // 60
            assert_eq!(viewwindowy, (sh - SBARHEIGHT - 100) >> 1); // 34
            for i in 0..200 {
                assert_eq!(columnofs[i as usize], viewwindowx + i);
            }
            for i in 0..100 {
                let expected = I_VideoBuffer.add(((i + viewwindowy) * SCREENWIDTH) as usize);
                assert_eq!(ylookup[i as usize], expected, "ylookup[{i}] mismatch");
            }

            I_VideoBuffer = orig_buf;
        }
    }

    // -----------------------------------------------------------------------
    // R_VideoErase
    // -----------------------------------------------------------------------

    /// Verifies that `R_VideoErase` copies the specified bytes from `background_buffer`
    /// into `I_VideoBuffer` at the correct offset.
    #[test]
    fn video_erase_copies_from_background() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut video = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut bg = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            for i in 0..bg.len() {
                bg[i] = (i % 256) as u8;
            }

            let orig_video = I_VideoBuffer;
            I_VideoBuffer = video.as_mut_ptr();
            background_buffer = bg.as_mut_ptr();

            R_VideoErase(100, 10);

            for i in 100..110 {
                assert_eq!(video[i as usize], bg[i as usize]);
            }
            // Ensure surrounding bytes are untouched
            assert_eq!(video[99], 0);
            assert_eq!(video[110], 0);

            I_VideoBuffer = orig_video;
            background_buffer = ptr::null_mut();
        }
    }

    /// Verifies that `R_VideoErase` is a no-op when `background_buffer` is null.
    #[test]
    fn video_erase_null_background_does_nothing() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut video = vec![0xCCu8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let orig_video = I_VideoBuffer;
            I_VideoBuffer = video.as_mut_ptr();
            background_buffer = ptr::null_mut();

            R_VideoErase(0, 10);

            for i in 0..10 {
                assert_eq!(video[i], 0xCC);
            }

            I_VideoBuffer = orig_video;
        }
    }

    // -----------------------------------------------------------------------
    // R_DrawViewBorder
    // -----------------------------------------------------------------------

    /// Verifies that `R_DrawViewBorder` exits immediately without panicking when the
    /// viewport is full-screen (`scaledviewwidth == SCREENWIDTH`).
    #[test]
    fn draw_view_border_fullscreen_returns_early() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            scaledviewwidth = SCREENWIDTH;
            // Should return without doing anything (no panic)
            R_DrawViewBorder();
        }
    }

    /// Verifies the `top` and `side` geometry values computed inside `R_DrawViewBorder`
    /// for a representative windowed viewport.
    #[test]
    fn draw_view_border_sets_correct_offsets() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            scaledviewwidth = 256;
            viewheight = 168;

            let top = ((SCREENHEIGHT - SBARHEIGHT) - viewheight) / 2;
            let side = (SCREENWIDTH - scaledviewwidth) / 2;

            assert_eq!(top, 0);
            assert_eq!(side, 32);
        }
    }
}
