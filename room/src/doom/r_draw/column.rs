//! The column renderers: opaque wall-texture columns, the low-detail
//! doubled variants, the fuzz (partial-invisibility) pair, and the
//! color-translated pair. One vertical strip of the frame per call.

use std::ffi::c_int;

use crate::doom::c_ffi::FUZZTABLE;
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::m_fixed::FRACBITS;

use super::state::{
    columnofs, dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_translation, dc_x, dc_yl,
    dc_yh, fuzzoffset, fuzzpos, viewheight, ylookup,
};

// ---------------------------------------------------------------------------
// External symbols
// ---------------------------------------------------------------------------

extern "C" {
    /// Raw linear framebuffer written to the display; `screens[0]` in C terms.
    static mut colormaps: *mut u8;
    /// Screen-space Y coordinate of the view center, used to compute texture fractions.
    static mut centery: c_int;
}

/// Draws a single vertical column of opaque wall texture into the framebuffer.
///
/// Reads the column context from the `dc_*` globals and writes `dc_yh - dc_yl + 1`
/// pixels into `screens[0]`, applying `dc_colormap` for lighting. The texture is
/// sampled at a 128-texel-high virtual column; `dc_iscale` steps through it in
/// 16.16 fixed-point per screen row.
///
/// Does nothing if `dc_yh < dc_yl` (empty column segment).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads the `dc_*` statics and writes through [`ylookup`]/[`columnofs`];
/// the caller must have rebuilt the LUTs for the active framebuffer and set
/// in-bounds `dc_*` values (`debug_assert!`ed in debug builds).
#[doc(alias = "R_DrawColumn")]
#[export_name = "R_DrawColumn"]
pub extern "C" fn draw_column() {
    unsafe {
        let count = dc_yh - dc_yl;

        // Zero length, column does not exceed a pixel.
        if count < 0 {
            return;
        }

        // Runtime raster stride (F1 M2): hoisted out of the inner loop.
        let stride = SCREENWIDTH as usize;

        debug_assert!(
            (dc_x as u32) < (SCREENWIDTH as u32) && dc_yl >= 0 && dc_yh < SCREENHEIGHT,
            "R_DrawColumn: {} to {} at {}",
            dc_yl as c_int,
            dc_yh as c_int,
            dc_x as c_int
        );

        // Framebuffer destination address.
        let mut dest = ylookup[dc_yl as usize].add(columnofs[dc_x as usize] as usize);

        // Determine scaling, which is the only mapping to be done.
        let fracstep = dc_iscale;
        let mut frac = dc_texturemid + (dc_yl - centery) * fracstep;

        // Inner loop that does the actual texture mapping.
        let mut count = count;
        loop {
            *dest = *dc_colormap.add(*dc_source.add(((frac >> FRACBITS) & 127) as usize) as usize);
            dest = dest.add(stride);
            frac += fracstep;
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

/// Low-detail variant of [`draw_column`] that writes each pixel to two adjacent
/// screen columns, producing blocky 2x-wide columns for the low-resolution detail mode.
///
/// Uses `dc_x * 2` and `dc_x * 2 + 1` as the destination columns. All other
/// column-context globals (`dc_*`) have the same meaning as in `R_DrawColumn`.
///
/// Does nothing if `dc_yh < dc_yl`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Same contract as [`draw_column`]; `dc_x * 2 + 1` must stay in bounds,
/// which the `MAXWIDTH`-sized [`columnofs`] LUT guarantees.
#[doc(alias = "R_DrawColumnLow")]
#[export_name = "R_DrawColumnLow"]
pub extern "C" fn draw_column_low() {
    unsafe {
        let count = dc_yh - dc_yl;

        // Zero length.
        if count < 0 {
            return;
        }

        // Blocky mode, need to multiply by 2.
        let x = dc_x << 1;

        // Runtime raster stride (F1 M2): hoisted out of the inner loop.
        let stride = SCREENWIDTH as usize;

        debug_assert!(
            (dc_x as u32) < (SCREENWIDTH as u32) && dc_yl >= 0 && dc_yh < SCREENHEIGHT,
            "R_DrawColumnLow: {} to {} at {}",
            dc_yl as c_int,
            dc_yh as c_int,
            dc_x as c_int
        );

        let mut dest = ylookup[dc_yl as usize].add(columnofs[x as usize] as usize);
        let mut dest2 = ylookup[dc_yl as usize].add(columnofs[(x + 1) as usize] as usize);

        let fracstep = dc_iscale;
        let mut frac = dc_texturemid + (dc_yl - centery) * fracstep;

        let mut count = count;
        loop {
            let pix =
                *dc_colormap.add(*dc_source.add(((frac >> FRACBITS) & 127) as usize) as usize);
            *dest = pix;
            *dest2 = pix;
            dest = dest.add(stride);
            dest2 = dest2.add(stride);
            frac += fracstep;
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

/// Draws a partial-invisibility (spectre/fuzz) column using the fuzz effect.
///
/// For each row the function reads a neighboring pixel from the framebuffer
/// (offset by `fuzzoffset[fuzzpos]` bytes) and re-indexes it through colormap 6,
/// producing a smeared dark image. `fuzzpos` advances and wraps modulo `FUZZTABLE`.
///
/// The top and bottom rows are clamped: `dc_yl` is raised to 1 and `dc_yh` is
/// lowered to `viewheight - 2` to avoid reading outside the viewport.
///
/// Does nothing if the clamped range is empty (`dc_yh < dc_yl`).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads and writes framebuffer memory one stride row away from `dest`;
/// `fuzzpos` advances across pixels and frames (frame-golden visible).
#[doc(alias = "R_DrawFuzzColumn")]
#[export_name = "R_DrawFuzzColumn"]
pub extern "C" fn draw_fuzz_column() {
    unsafe {
        // Adjust borders. Low...
        if dc_yl == 0 {
            dc_yl = 1;
        }

        // .. and high.
        if dc_yh == viewheight - 1 {
            dc_yh = viewheight - 2;
        }

        let count = dc_yh - dc_yl;

        // Zero length.
        if count < 0 {
            return;
        }

        // Runtime raster stride (F1 M2): fuzzoffset entries are +/-1
        // direction units (crispy r_draw.c:409 scales by SCREENWIDTH here).
        let stride = SCREENWIDTH as isize;

        debug_assert!(
            (dc_x as u32) < (SCREENWIDTH as u32) && dc_yl >= 0 && dc_yh < SCREENHEIGHT,
            "R_DrawFuzzColumn: {} to {} at {}",
            dc_yl as c_int,
            dc_yh as c_int,
            dc_x as c_int
        );

        let mut dest = ylookup[dc_yl as usize].add(columnofs[dc_x as usize] as usize);

        let mut count = count;
        loop {
            // Lookup framebuffer, and retrieve a pixel that is either one
            // column left or right of the current one.  Add index from
            // colormap to index.
            let offset = stride * fuzzoffset[fuzzpos as usize] as isize;
            let src_pix = *dest.offset(offset);
            *dest = *colormaps.add(6 * 256 + src_pix as usize);

            // Clamp table lookup index.
            fuzzpos += 1;
            if fuzzpos == FUZZTABLE as c_int {
                fuzzpos = 0;
            }

            dest = dest.offset(stride);
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

/// Low-detail variant of [`draw_fuzz_column`] that writes each fuzz pixel to
/// two adjacent screen columns (`dc_x * 2` and `dc_x * 2 + 1`).
///
/// Both pixels receive the same fuzz-sampled value derived from column `dc_x * 2`'s
/// neighbor; the fuzz table position advances once per row pair.
/// Border clamping and early-exit behavior are identical to `R_DrawFuzzColumn`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Same contract as [`draw_fuzz_column`], doubled to two columns.
#[doc(alias = "R_DrawFuzzColumnLow")]
#[export_name = "R_DrawFuzzColumnLow"]
pub extern "C" fn draw_fuzz_column_low() {
    unsafe {
        // Adjust borders. Low...
        if dc_yl == 0 {
            dc_yl = 1;
        }

        // .. and high.
        if dc_yh == viewheight - 1 {
            dc_yh = viewheight - 2;
        }

        let count = dc_yh - dc_yl;

        // Zero length.
        if count < 0 {
            return;
        }

        // low detail mode, need to multiply by 2
        let x = dc_x << 1;

        // Runtime raster stride (F1 M2): fuzzoffset entries are +/-1
        // direction units (crispy r_draw.c:409 scales by SCREENWIDTH here).
        let stride = SCREENWIDTH as isize;

        debug_assert!(
            (x as u32) < (SCREENWIDTH as u32) && dc_yl >= 0 && dc_yh < SCREENHEIGHT,
            "R_DrawFuzzColumnLow: {} to {} at {}",
            dc_yl as c_int,
            dc_yh as c_int,
            dc_x as c_int
        );

        let mut dest = ylookup[dc_yl as usize].add(columnofs[x as usize] as usize);
        let mut dest2 = ylookup[dc_yl as usize].add(columnofs[(x + 1) as usize] as usize);

        let mut count = count;
        loop {
            let offset = stride * fuzzoffset[fuzzpos as usize] as isize;
            let src_pix = *dest.offset(offset);
            let pix = *colormaps.add(6 * 256 + src_pix as usize);
            *dest = pix;
            *dest2 = pix;

            // Clamp table lookup index.
            fuzzpos += 1;
            if fuzzpos == FUZZTABLE as c_int {
                fuzzpos = 0;
            }

            dest = dest.offset(stride);
            dest2 = dest2.offset(stride);
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

/// Draws a color-translated column; used for player sprites rendered in non-green colors.
///
/// The pixel pipeline is: `dc_colormap[ dc_translation[ dc_source[frac] ] ]`.
/// `dc_translation` maps the green palette ramp to another color ramp, allowing one
/// set of player sprites to appear in multiple colors (gray, brown, red).
///
/// Does nothing if `dc_yh < dc_yl`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads the `dc_*` statics including [`dc_translation`]; the caller must
/// have set a valid 256-byte translation table.
#[doc(alias = "R_DrawTranslatedColumn")]
#[export_name = "R_DrawTranslatedColumn"]
pub extern "C" fn draw_translated_column() {
    unsafe {
        let count = dc_yh - dc_yl;
        if count < 0 {
            return;
        }

        // Runtime raster stride (F1 M2): hoisted out of the inner loop.
        let stride = SCREENWIDTH as usize;

        debug_assert!(
            (dc_x as u32) < (SCREENWIDTH as u32) && dc_yl >= 0 && dc_yh < SCREENHEIGHT,
            "R_DrawTranslatedColumn: {} to {} at {}",
            dc_yl as c_int,
            dc_yh as c_int,
            dc_x as c_int
        );

        let mut dest = ylookup[dc_yl as usize].add(columnofs[dc_x as usize] as usize);

        let fracstep = dc_iscale;
        let mut frac = dc_texturemid + (dc_yl - centery) * fracstep;

        let mut count = count;
        loop {
            let src_idx = *dc_source.add((frac >> FRACBITS) as usize) as usize;
            let trans_idx = *dc_translation.add(src_idx) as usize;
            *dest = *dc_colormap.add(trans_idx);
            dest = dest.add(stride);
            frac += fracstep;
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

/// Low-detail variant of [`draw_translated_column`] that writes each translated
/// pixel to two adjacent screen columns (`dc_x * 2` and `dc_x * 2 + 1`).
///
/// The pixel pipeline and border behavior are identical to `R_DrawTranslatedColumn`.
/// Does nothing if `dc_yh < dc_yl`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Same contract as [`draw_translated_column`], doubled to two columns.
#[doc(alias = "R_DrawTranslatedColumnLow")]
#[export_name = "R_DrawTranslatedColumnLow"]
pub extern "C" fn draw_translated_column_low() {
    unsafe {
        let count = dc_yh - dc_yl;
        if count < 0 {
            return;
        }

        // low detail, need to scale by 2
        let x = dc_x << 1;

        // Runtime raster stride (F1 M2): hoisted out of the inner loop.
        let stride = SCREENWIDTH as usize;

        debug_assert!(
            (x as u32) < (SCREENWIDTH as u32) && dc_yl >= 0 && dc_yh < SCREENHEIGHT,
            "R_DrawTranslatedColumnLow: {} to {} at {}",
            dc_yl as c_int,
            dc_yh as c_int,
            x
        );

        let mut dest = ylookup[dc_yl as usize].add(columnofs[x as usize] as usize);
        let mut dest2 = ylookup[dc_yl as usize].add(columnofs[(x + 1) as usize] as usize);

        let fracstep = dc_iscale;
        let mut frac = dc_texturemid + (dc_yl - centery) * fracstep;

        let mut count = count;
        loop {
            let src_idx = *dc_source.add((frac >> FRACBITS) as usize) as usize;
            let trans_idx = *dc_translation.add(src_idx) as usize;
            let pix = *dc_colormap.add(trans_idx);
            *dest = pix;
            *dest2 = pix;
            dest = dest.add(stride);
            dest2 = dest2.add(stride);
            frac += fracstep;
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_int;
    use std::ptr;
    use std::sync::Mutex;

    use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
    use crate::doom::r_draw::{
        columnofs, dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_translation, dc_x, dc_yl,
        dc_yh, fuzzpos, R_DrawColumn, R_DrawColumnLow, R_DrawFuzzColumn,
        R_DrawTranslatedColumn, viewheight, ylookup,
    };

    extern "C" {
        static mut colormaps: *mut u8;
        static mut centery: c_int;
    }

    /// Serialises all tests that touch the shared mutable renderer globals.
    static LOCK: Mutex<()> = Mutex::new(());

    /// Verifies that `R_DrawColumn` writes the correct colormap-indexed texture samples
    /// for each row in a small column segment.
    #[test]
    fn draw_column_basic() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut source = vec![0u8; 128];
            for i in 0..128 {
                source[i] = i as u8;
            }
            let mut colormap = vec![0u8; 256];
            for i in 0..256 {
                colormap[i] = (i ^ 0x55) as u8;
            }

            // Set up ylookup and columnofs for a full-screen buffer
            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            dc_colormap = colormap.as_mut_ptr();
            dc_source = source.as_mut_ptr();
            dc_x = 10;
            dc_yl = 20;
            dc_yh = 30;
            dc_iscale = 0x10000; // 1.0 in fixed point
            dc_texturemid = 0;
            centery = 0;

            R_DrawColumn();

            // Verify each pixel
            for row in 20..=30 {
                let src_idx = row & 127;
                let expected = colormap[source[src_idx as usize] as usize];
                let actual = framebuffer[(row * SCREENWIDTH + 10) as usize];
                assert_eq!(actual, expected, "pixel mismatch at row {row}");
            }
        }
    }

    /// Verifies that `R_DrawColumn` leaves the framebuffer untouched when `dc_yh < dc_yl`.
    #[test]
    fn draw_column_negative_count_returns_early() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0xABu8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            dc_x = 5;
            dc_yl = 10;
            dc_yh = 5; // count = -5
            dc_colormap = ptr::null_mut();
            dc_source = ptr::null_mut();

            R_DrawColumn();

            // Framebuffer should be untouched
            for i in 0..framebuffer.len() {
                assert_eq!(framebuffer[i], 0xAB, "framebuffer[{i}] was modified");
            }
        }
    }

    /// Verifies that `R_DrawColumnLow` writes the same pixel value to both adjacent
    /// screen columns (`dc_x * 2` and `dc_x * 2 + 1`).
    #[test]
    fn draw_column_low_doubles() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut source = vec![0u8; 128];
            source[0] = 42;
            let mut colormap = vec![0u8; 256];
            colormap[42] = 99;

            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            dc_colormap = colormap.as_mut_ptr();
            dc_source = source.as_mut_ptr();
            dc_x = 5;
            dc_yl = 10;
            dc_yh = 10;
            dc_iscale = 0;
            dc_texturemid = 0;
            centery = 0;

            R_DrawColumnLow();

            let x = dc_x << 1;
            assert_eq!(framebuffer[(10 * SCREENWIDTH + x) as usize], 99);
            assert_eq!(framebuffer[(10 * SCREENWIDTH + x + 1) as usize], 99);
        }
    }

    /// Verifies that `R_DrawTranslatedColumn` applies `dc_translation` then `dc_colormap`
    /// to produce the expected output pixel.
    #[test]
    fn draw_translated_column_maps_colors() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut source = vec![0u8; 256];
            source[0] = 5; // source pixel at frac=0
            let mut translation = vec![0u8; 256];
            translation[5] = 7; // map color 5 → 7
            let mut colormap = vec![0u8; 256];
            colormap[7] = 99;

            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            dc_colormap = colormap.as_mut_ptr();
            dc_source = source.as_mut_ptr();
            dc_translation = translation.as_mut_ptr();
            dc_x = 15;
            dc_yl = 25;
            dc_yh = 25;
            dc_iscale = 0;
            dc_texturemid = 0;
            centery = 0;

            R_DrawTranslatedColumn();

            assert_eq!(framebuffer[(25 * SCREENWIDTH + 15) as usize], 99);
        }
    }

    /// Verifies that `R_DrawFuzzColumn` reads a neighboring framebuffer pixel via `fuzzoffset`,
    /// indexes it through colormap 6, and advances `fuzzpos`.
    #[test]
    fn draw_fuzz_column_reads_adjacent() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            // Pre-fill the framebuffer so the fuzz effect has something to read.
            // fuzzoffset[0] = FUZZOFF = SCREENWIDTH, so it reads one row below.
            framebuffer[((25 + 1) * SCREENWIDTH + 10) as usize] = 3;

            // Set up colormaps: colormaps[6*256 + 3] = 77
            let mut colormaps_buf = vec![0u8; 32 * 256];
            colormaps_buf[6 * 256 + 3] = 77;

            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            colormaps = colormaps_buf.as_mut_ptr();
            dc_x = 10;
            dc_yl = 25;
            dc_yh = 25;
            dc_iscale = 0;
            dc_texturemid = 0;
            centery = 0;
            fuzzpos = 0;
            viewheight = SCREENHEIGHT;

            R_DrawFuzzColumn();

            // fuzzpos should have advanced
            assert_eq!(fuzzpos, 1);
            // The pixel at (10, 25) should now be colormaps[6*256 + framebuffer[(10, 26)]]
            assert_eq!(framebuffer[(25 * SCREENWIDTH + 10) as usize], 77);
        }
    }

    /// Verifies that `R_DrawFuzzColumn` clamps `dc_yl` to 1 and `dc_yh` to `viewheight - 2`
    /// to avoid reading outside the viewport.
    #[test]
    fn draw_fuzz_column_clamps_borders() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut colormaps_buf = vec![0u8; 32 * 256];

            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            colormaps = colormaps_buf.as_mut_ptr();
            dc_x = 0;
            dc_yl = 0; // will be clamped to 1
            dc_yh = SCREENHEIGHT - 1; // will be clamped to SCREENHEIGHT - 2
            dc_iscale = 0;
            dc_texturemid = 0;
            centery = 0;
            fuzzpos = 0;
            viewheight = SCREENHEIGHT;

            R_DrawFuzzColumn();

            // After clamping: dc_yl = 1, dc_yh = SCREENHEIGHT - 2
            assert_eq!(dc_yl, 1);
            assert_eq!(dc_yh, SCREENHEIGHT - 2);
        }
    }
}
