//! Data home for the raster state: every `#[no_mangle]` static the column
//! and span renderers read and write, plus the `MAXWIDTH`/`MAXHEIGHT`
//! LUT-sizing constants.

use std::ffi::c_int;
use std::ptr;

use crate::doom::c_ffi::{FUZZOFF, FUZZTABLE};

/// Maximum framebuffer width supported by the lookup tables.
///
/// Sized to the `video_cfg` validation cap (the Boom `MAX_SCREENWIDTH`
/// shape): every per-column array here and in `r_main`/`r_plane`/`r_things`
/// is statically capped, and `VideoConfig::validated` rejects anything past
/// it, so all indexing through these LUTs is provably in-bounds.
pub(super) const MAXWIDTH: usize = crate::doom::video_cfg::MAX_SCREENWIDTH as usize;
/// Maximum framebuffer height supported by the lookup tables.
pub(super) const MAXHEIGHT: usize = crate::doom::video_cfg::MAX_SCREENHEIGHT as usize;

/// Base address of the view image written by the renderer (typically points into `I_VideoBuffer`).
#[no_mangle]
pub static mut viewimage: *mut u8 = ptr::null_mut();

/// Width of the current render viewport in pixels.
#[no_mangle]
pub static mut viewwidth: c_int = 0;

/// Width of the viewport scaled for the current detail mode (equals `viewwidth` in high-detail).
#[no_mangle]
pub static mut scaledviewwidth: c_int = 0;

/// Height of the current render viewport in pixels.
#[no_mangle]
pub static mut viewheight: c_int = 0;

/// X pixel offset from the left edge of the framebuffer to the left edge of the viewport.
#[no_mangle]
pub static mut viewwindowx: c_int = 0;

/// Y pixel offset from the top of the framebuffer to the top of the viewport.
#[no_mangle]
pub static mut viewwindowy: c_int = 0;

/// Per-row pointer LUT: `ylookup[y]` points to the first byte of row `y` in the framebuffer.
/// Avoids a multiply by `SCREENWIDTH` in the inner rendering loops.
#[no_mangle]
pub static mut ylookup: [*mut u8; MAXHEIGHT] = [ptr::null_mut(); MAXHEIGHT];

/// Per-column byte offset LUT: `columnofs[x]` is the byte offset within a row for column `x`.
/// Accounts for `viewwindowx` so that sub-window rendering works without extra arithmetic.
#[no_mangle]
pub static mut columnofs: [c_int; MAXWIDTH] = [0; MAXWIDTH];

/// Color-translation tables for the three non-green player colors (gray, brown, red).
/// Each table remaps the 16-entry green palette ramp (indices `0x70`-`0x7f`) to another ramp.
///
/// Dead-but-exported (zero consumers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut translations: [[u8; 256]; 3] = [[0; 256]; 3];

/// Current colormap (light-level lookup table) used by the column renderer.
/// Points into `colormaps`; index `colormap[p]` converts a palette index to a lit palette index.
#[no_mangle]
pub static mut dc_colormap: *mut u8 = ptr::null_mut();

/// Screen-space X coordinate of the column being drawn (0 = left edge of viewport).
#[no_mangle]
pub static mut dc_x: c_int = 0;

/// Topmost screen-space Y coordinate of the column segment to draw (inclusive).
#[no_mangle]
pub static mut dc_yl: c_int = 0;

/// Bottommost screen-space Y coordinate of the column segment to draw (inclusive).
#[no_mangle]
pub static mut dc_yh: c_int = 0;

/// Inverse texture scale in 16.16 fixed-point: the amount added to the texture fraction
/// per screen row, equal to `textureheight / columnheight`.
#[no_mangle]
pub static mut dc_iscale: c_int = 0;

/// Texture mid-point fraction in 16.16 fixed-point, corresponding to the true center of
/// the wall post; used together with `dc_iscale` to compute the starting texture row.
#[no_mangle]
pub static mut dc_texturemid: c_int = 0;

/// First pixel in a column (possibly virtual).
#[no_mangle]
pub static mut dc_source: *mut u8 = ptr::null_mut();

/// Just for profiling.
///
/// Dead-but-exported (zero consumers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut dccount: c_int = 0;

/// Pre-computed table of per-row direction units used by the fuzz/spectre effect.
/// Each entry is `+FUZZOFF` (+1, one row down) or `-FUZZOFF` (-1, one row up); the
/// column renderers multiply by the runtime `SCREENWIDTH` stride (crispy keeps the
/// same +/-1 table and scales at the use site, `r_draw.c:409`), giving the
/// smeared, semi-transparent look of partial-invisibility at any raster width.
#[no_mangle]
pub static mut fuzzoffset: [c_int; FUZZTABLE] = [
    FUZZOFF, -FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF,
    FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, -FUZZOFF, -FUZZOFF,
    -FUZZOFF, FUZZOFF, -FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF,
    -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, -FUZZOFF, -FUZZOFF,
    -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF,
];

/// Current position within `fuzzoffset`; wraps back to 0 when it reaches `FUZZTABLE`.
#[no_mangle]
pub static mut fuzzpos: c_int = 0;

/// Pointer to the 256-byte color-translation table currently active for
/// `R_DrawTranslatedColumn`. Used to remap the player-sprite green ramp to
/// another color set.
#[no_mangle]
pub static mut dc_translation: *mut u8 = ptr::null_mut();

/// Heap-allocated block of 3 × 256 bytes holding the gray, brown, and red translation tables
/// built by `R_InitTranslationTables`.
#[no_mangle]
pub static mut translationtables: *mut u8 = ptr::null_mut();

/// Screen-space Y row of the span being drawn.
#[no_mangle]
pub static mut ds_y: c_int = 0;

/// Leftmost screen-space X coordinate of the span (inclusive).
#[no_mangle]
pub static mut ds_x1: c_int = 0;

/// Rightmost screen-space X coordinate of the span (inclusive).
#[no_mangle]
pub static mut ds_x2: c_int = 0;

/// Colormap used by the span renderer; points into `colormaps` for the appropriate light level.
#[no_mangle]
pub static mut ds_colormap: *mut u8 = ptr::null_mut();

/// Starting texture U (X) fraction in 16.16 fixed-point for the leftmost pixel of the span.
#[no_mangle]
pub static mut ds_xfrac: c_int = 0;

/// Starting texture V (Y) fraction in 16.16 fixed-point for the leftmost pixel of the span.
#[no_mangle]
pub static mut ds_yfrac: c_int = 0;

/// Per-pixel increment of `ds_xfrac` in 16.16 fixed-point along the span.
#[no_mangle]
pub static mut ds_xstep: c_int = 0;

/// Per-pixel increment of `ds_yfrac` in 16.16 fixed-point along the span.
#[no_mangle]
pub static mut ds_ystep: c_int = 0;

/// Start of a 64×64 tile image.
#[no_mangle]
pub static mut ds_source: *mut u8 = ptr::null_mut();

/// Just for profiling.
///
/// Dead-but-exported (zero consumers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut dscount: c_int = 0;

#[cfg(test)]
mod tests {
    use std::ffi::c_int;

    use crate::doom::c_ffi::{FUZZOFF, FUZZTABLE};
    use crate::doom::r_draw::fuzzoffset;

    /// Checks that `fuzzoffset` exactly matches the table from the C source (`r_draw.c`).
    #[test]
    fn fuzzoffset_exact_values() {
        let expected: [c_int; FUZZTABLE] = [
            FUZZOFF, -FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF,
            -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF,
            -FUZZOFF, -FUZZOFF, -FUZZOFF, FUZZOFF, -FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF,
            FUZZOFF, -FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, -FUZZOFF, FUZZOFF,
            FUZZOFF, -FUZZOFF, -FUZZOFF, -FUZZOFF, -FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF, FUZZOFF,
            -FUZZOFF, FUZZOFF, FUZZOFF, -FUZZOFF, FUZZOFF,
        ];
        unsafe {
            for (i, (&got, &want)) in fuzzoffset.iter().zip(expected.iter()).enumerate() {
                assert_eq!(
                    got, want,
                    "fuzzoffset[{i}] mismatch: got {got}, want {want}"
                );
            }
        }
    }

    /// Checks that the `fuzzoffset` table contains exactly 29 positive and 21 negative entries.
    #[test]
    fn fuzzoffset_positive_count() {
        unsafe {
            let pos = fuzzoffset.iter().filter(|&&v| v > 0).count();
            let neg = fuzzoffset.iter().filter(|&&v| v < 0).count();
            assert_eq!(
                pos, 29,
                "expected 29 positive fuzzoffset entries, got {pos}"
            );
            assert_eq!(
                neg, 21,
                "expected 21 negative fuzzoffset entries, got {neg}"
            );
        }
    }
}
