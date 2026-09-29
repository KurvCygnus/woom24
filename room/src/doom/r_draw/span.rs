//! The span renderers: one horizontal row segment of a floor/ceiling flat
//! per call, high- and low-detail variants.

use std::ffi::c_int;

use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};

use super::state::{columnofs, ds_colormap, ds_source, ds_x1, ds_x2, ds_xfrac, ds_xstep, ds_y,
                   ds_yfrac, ds_ystep, ylookup};

/// Draws a single horizontal floor or ceiling span into the framebuffer.
///
/// Samples a 64×64 flat texture tile stored at `ds_source`, walking through it
/// in u/v (x/y) texture space with `ds_xstep` / `ds_ystep` increments. Position
/// and step are packed into 32-bit words (upper 16 bits = X, lower 16 bits = Y)
/// to avoid separate fixed-point additions.
///
/// Writes pixels to `screens[0]` from column `ds_x1` to `ds_x2` inclusive on row `ds_y`.
/// Does not check for zero-length spans; the caller must ensure `ds_x2 >= ds_x1`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads the `ds_*` statics and writes through [`ylookup`]/[`columnofs`];
/// the caller must pass in-bounds span extents (`debug_assert!`ed).
#[doc(alias = "R_DrawSpan")]
#[export_name = "R_DrawSpan"]
pub extern "C" fn draw_span() {
    unsafe {
        let mut position: u32;

        // Pack position and step variables into a single 32-bit integer,
        // with x in the top 16 bits and y in the bottom 16 bits.  For
        // each 16-bit part, the top 6 bits are the integer part and the
        // bottom 10 bits are the fractional part of the pixel position.
        position = ((ds_xfrac << 10) as u32 & 0xffff0000) | ((ds_yfrac >> 6) as u32 & 0x0000ffff);
        let step: u32 =
            ((ds_xstep << 10) as u32 & 0xffff0000) | ((ds_ystep >> 6) as u32 & 0x0000ffff);

        let mut dest = ylookup[ds_y as usize].add(columnofs[ds_x1 as usize] as usize);

        // We do not check for zero spans here?
        let mut count = ds_x2 - ds_x1;

        debug_assert!(
            ds_x2 >= ds_x1
                && ds_x1 >= 0
                && ds_x2 < SCREENWIDTH
                && (ds_y as u32) <= (SCREENHEIGHT as u32),
            "R_DrawSpan: {} to {} at {}",
            ds_x1 as c_int,
            ds_x2 as c_int,
            ds_y as c_int
        );

        loop {
            // Calculate current texture index in u,v.
            let ytemp = ((position >> 4) & 0x0fc0) as usize;
            let xtemp = (position >> 26) as usize;
            let spot = xtemp | ytemp;

            // Lookup pixel from flat texture tile, re-index using light/colormap.
            *dest = *ds_colormap.add(*ds_source.add(spot) as usize);
            dest = dest.add(1);

            position = position.wrapping_add(step);
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

/// Low-detail variant of [`draw_span`] that writes each sampled texel to two
/// consecutive framebuffer bytes, producing blocky 2x-wide pixels.
///
/// The logical span coordinates (`ds_x1`, `ds_x2`) are doubled to address the
/// correct pixels; the texture-space walk is unchanged.
///
/// # FIXME
/// In the original C (`r_draw.c`) `ds_x1` and `ds_x2` are mutated in-place
/// (`ds_x1 <<= 1; ds_x2 <<= 1`), leaving the globals modified after the call.
/// This Rust port uses a local `ds_x1_low` and leaves the globals unchanged,
/// so callers that read `ds_x1`/`ds_x2` after `R_DrawSpanLow` see different
/// values than they would after the C version.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Same contract as [`draw_span`]; the local `ds_x1_low` doubling is the
/// documented C divergence above -- do not "fix" it during refactors.
#[doc(alias = "R_DrawSpanLow")]
#[export_name = "R_DrawSpanLow"]
pub extern "C" fn draw_span_low() {
    unsafe {
        let mut position: u32;

        position = ((ds_xfrac << 10) as u32 & 0xffff0000) | ((ds_yfrac >> 6) as u32 & 0x0000ffff);
        let step: u32 =
            ((ds_xstep << 10) as u32 & 0xffff0000) | ((ds_ystep >> 6) as u32 & 0x0000ffff);

        let mut count = ds_x2 - ds_x1;

        // Blocky mode, need to multiply by 2.
        let ds_x1_low = ds_x1 << 1;

        debug_assert!(
            ds_x2 >= ds_x1
                && ds_x1 >= 0
                && ds_x2 < SCREENWIDTH
                && (ds_y as u32) <= (SCREENHEIGHT as u32),
            "R_DrawSpanLow: {} to {} at {}",
            ds_x1 as c_int,
            ds_x2 as c_int,
            ds_y as c_int
        );

        let mut dest = ylookup[ds_y as usize].add(columnofs[ds_x1_low as usize] as usize);

        loop {
            let ytemp = ((position >> 4) & 0x0fc0) as usize;
            let xtemp = (position >> 26) as usize;
            let spot = xtemp | ytemp;

            // Lowres/blocky mode does it twice,
            // while scale is adjusted appropriately.
            let pix = *ds_colormap.add(*ds_source.add(spot) as usize);
            *dest = pix;
            dest = dest.add(1);
            *dest = pix;
            dest = dest.add(1);

            position = position.wrapping_add(step);
            if count == 0 {
                break;
            }
            count -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
    use crate::doom::r_draw::{
        columnofs, ds_colormap, ds_source, ds_x1, ds_x2, ds_xfrac, ds_xstep, ds_y, ds_yfrac,
        ds_ystep, R_DrawSpan, R_DrawSpanLow, ylookup,
    };

    /// Serialises all tests that touch the shared mutable renderer globals.
    static LOCK: Mutex<()> = Mutex::new(());

    /// Verifies that `R_DrawSpan` samples and colormap-indexes five consecutive texels
    /// across a short horizontal span.
    #[test]
    fn draw_span_basic() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut source = vec![0u8; 64 * 64];
            for i in 0..(64 * 64) {
                source[i] = (i % 256) as u8;
            }
            let mut colormap = vec![0u8; 256];
            for i in 0..256 {
                colormap[i] = (i ^ 0xAA) as u8;
            }

            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            ds_colormap = colormap.as_mut_ptr();
            ds_source = source.as_mut_ptr();
            ds_y = 50;
            ds_x1 = 10;
            ds_x2 = 14;
            ds_xfrac = 0;
            ds_yfrac = 0;
            ds_xstep = 0x10000; // 1.0 in fixed point
            ds_ystep = 0;

            R_DrawSpan();

            // With xfrac=0, yfrac=0, xstep=1.0 (0x10000), ystep=0:
            // position = 0
            // step = (0x10000 << 10) & 0xffff0000 = 0x0400_0000
            // After each pixel, position increases by 0x0400_0000,
            // so xtemp (position >> 26) increments by 1 each time.
            // ds_x2 - ds_x1 = 4, so we draw 5 pixels (10..=14).
            let base = (50 * SCREENWIDTH + 10) as usize;
            assert_eq!(framebuffer[base], colormap[source[0] as usize]);
            assert_eq!(framebuffer[base + 1], colormap[source[1] as usize]);
            assert_eq!(framebuffer[base + 2], colormap[source[2] as usize]);
            assert_eq!(framebuffer[base + 3], colormap[source[3] as usize]);
            assert_eq!(framebuffer[base + 4], colormap[source[4] as usize]);
        }
    }

    /// Verifies that `R_DrawSpanLow` writes the same texel to two consecutive framebuffer bytes.
    #[test]
    fn draw_span_low_doubles() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            let mut framebuffer = vec![0u8; (SCREENWIDTH * SCREENHEIGHT) as usize];
            let mut source = vec![0u8; 64 * 64];
            source[0] = 77;
            let mut colormap = vec![0u8; 256];
            colormap[77] = 88;

            for i in 0..SCREENHEIGHT {
                ylookup[i as usize] = framebuffer.as_mut_ptr().add((i * SCREENWIDTH) as usize);
            }
            for i in 0..SCREENWIDTH {
                columnofs[i as usize] = i;
            }

            ds_colormap = colormap.as_mut_ptr();
            ds_source = source.as_mut_ptr();
            ds_y = 60;
            ds_x1 = 5;
            ds_x2 = 5;
            ds_xfrac = 0;
            ds_yfrac = 0;
            ds_xstep = 0;
            ds_ystep = 0;

            R_DrawSpanLow();

            // ds_x2 - ds_x1 = 0, so 1 iteration. Each iteration writes 2 pixels.
            // ds_x1_low = 10
            let base = (60 * SCREENWIDTH + 10) as usize;
            assert_eq!(framebuffer[base], 88);
            assert_eq!(framebuffer[base + 1], 88);
        }
    }
}
