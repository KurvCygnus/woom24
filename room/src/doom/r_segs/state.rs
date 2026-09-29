//! The seg-rasterizer's mutable frame state: the eleven private constants
//! and the 33 `#[no_mangle]` statics of `r_segs.c` (exported through
//! `c_ffi.rs`'s re-export block for `c_tests/r_segs_c.rs`). One data home;
//! upstream names + C linkage retained. `r_bsp` writes `rw_angle1` and
//! calls `R_StoreWallRange`; `r_main` reads `rw_distance` /
//! `rw_normalangle` / `walllights` (its unit tests write the latter two
//! directly) -- every root path is held by the re-export block in
//! `mod.rs`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_short, c_uchar};
use std::ptr;

use crate::doom::m_fixed::{angle_t, fixed_t};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of fractional bits used for the pixel-height accumulator.
///
/// The top/bottom screen-y values are maintained as 12.20 fixed-point
/// values internally; `>> HEIGHTBITS` converts them to integer screen rows.
pub(super) const HEIGHTBITS: u32 = 12;

/// One unit in the pixel-height accumulator (`1 << HEIGHTBITS`).
///
/// Added to `topfrac` before the right-shift to achieve ceiling (upward)
/// rounding when computing `yl`.
pub(super) const HEIGHTUNIT: c_int = 1 << HEIGHTBITS;

/// Silhouette flag: this drawseg occludes sprites below its bottom edge.
pub(super) const SIL_BOTTOM: c_int = 1;

/// Silhouette flag: this drawseg occludes sprites above its top edge.
pub(super) const SIL_TOP: c_int = 2;

/// Silhouette flag: this drawseg occludes sprites on both sides.
pub(super) const SIL_BOTH: c_int = 3;

/// Number of distinct light levels used by the colormap tables.
pub(super) const LIGHTLEVELS: usize = 16;

/// Shift applied to a sector's `lightlevel` to derive a scale-light table index.
pub(super) const LIGHTSEGSHIFT: u32 = 4;

/// Number of scale-based light entries per light level in `scalelight`.
pub(super) const MAXLIGHTSCALE: usize = 48;

/// Shift applied to a column scale value to derive an index into `scalelight[n]`.
pub(super) const LIGHTSCALESHIFT: u32 = 12;

/// Mask for the fine-angle table (8192 entries, indices 0..=8191).
pub(super) const FINEMASK: usize = 0x1FFF;

/// Maximum number of drawsegs that can be stored per frame.
pub(super) const MAXDRAWSEGS: usize = 256;

// ---------------------------------------------------------------------------
// Globals defined by this module
// ---------------------------------------------------------------------------

/// Non-zero when the current seg has at least one visible texture.
///
/// Exported as `#[no_mangle]` for C callers.  Set by `R_StoreWallRange`
/// as the OR of `midtexture | toptexture | bottomtexture | maskedtexture`.
/// Controls whether `R_RenderSegLoop` computes texture-U and lighting.
#[no_mangle]
pub static mut segtextured: c_int = 0;

/// Non-zero when the floor plane must be extended for this seg.
///
/// Exported as `#[no_mangle]` for C callers.  Set by `R_StoreWallRange`.
/// A floor mark is needed when the back sector has a different floor flat,
/// height, or light level, or when the seg is single-sided.
#[no_mangle]
pub static mut markfloor: c_int = 0;

/// Non-zero when the ceiling plane must be extended for this seg.
///
/// Exported as `#[no_mangle]` for C callers.  Set by `R_StoreWallRange`.
/// Analogous to [`markfloor`] for the ceiling.
#[no_mangle]
pub static mut markceiling: c_int = 0;

/// Non-zero when the seg has a transparent mid-texture on a two-sided line.
///
/// Exported as `#[no_mangle]` for C callers.  When set, `R_StoreWallRange`
/// reserves a column slice in the `openings` scratch buffer and
/// `R_RenderMaskedSegRange` later draws it after all opaque geometry.
#[no_mangle]
pub static mut maskedtexture: c_int = 0;

/// Translated texture number for the upper wall texture (0 = none).
///
/// Exported as `#[no_mangle]` for C callers.  Set by `R_StoreWallRange`
/// from `sidedef->toptexture` via `texturetranslation`.
#[no_mangle]
pub static mut toptexture: c_int = 0;

/// Translated texture number for the lower wall texture (0 = none).
///
/// Exported as `#[no_mangle]` for C callers.  Set by `R_StoreWallRange`
/// from `sidedef->bottomtexture` via `texturetranslation`.
#[no_mangle]
pub static mut bottomtexture: c_int = 0;

/// Translated texture number for the middle wall texture (0 = none).
///
/// Exported as `#[no_mangle]` for C callers.  Non-zero only for single-sided
/// lines (or masked mid-textures, tracked separately via [`maskedtexture`]).
#[no_mangle]
pub static mut midtexture: c_int = 0;

/// Normal angle of the current seg's linedef (perpendicular to the wall).
///
/// Exported as `#[no_mangle]` for C callers.  Computed as
/// `curline->angle + ANG90` by `R_StoreWallRange`.
#[no_mangle]
pub static mut rw_normalangle: angle_t = 0;

/// Angle from the viewpoint to the left endpoint of the current seg.
///
/// Exported as `#[no_mangle]` for C callers.  Written by the BSP clipper
/// (`R_ClipPassWallSegment` / `R_ClipSolidWallSegment` in `r_bsp`) before
/// calling `R_StoreWallRange`.
#[no_mangle]
pub static mut rw_angle1: angle_t = 0;

/// Screen column where rendering of the current seg starts (inclusive).
///
/// Exported as `#[no_mangle]` for C callers.  Also used as the loop variable
/// inside `R_RenderSegLoop`.
#[no_mangle]
pub static mut rw_x: c_int = 0;

/// Screen column where rendering of the current seg stops (exclusive).
///
/// Exported as `#[no_mangle]` for C callers.  Set to `stop + 1` in
/// `R_StoreWallRange`.
#[no_mangle]
pub static mut rw_stopx: c_int = 0;

/// Angle used to compute the texture-U coordinate for each column.
///
/// Exported as `#[no_mangle]` for C callers.  Set to
/// `ANG90 + viewangle - rw_normalangle` by `R_StoreWallRange`.
#[no_mangle]
pub static mut rw_centerangle: angle_t = 0;

/// Texture horizontal offset for the current seg (fixed-point pixels).
///
/// Exported as `#[no_mangle]` for C callers.  Combines the linedef's
/// `textureoffset`, the seg's own `offset`, and a view-angle correction.
#[no_mangle]
pub static mut rw_offset: fixed_t = 0;

/// Perpendicular distance from the viewpoint to the wall (fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Used with `finesine` to derive
/// the scale at each column.
#[no_mangle]
pub static mut rw_distance: fixed_t = 0;

/// Projection scale at the current column.
///
/// Exported as `#[no_mangle]` for C callers.  Larger values mean the wall is
/// closer.  Stepped by [`rw_scalestep`] across the seg.
#[no_mangle]
pub static mut rw_scale: fixed_t = 0;

/// Per-column increment for [`rw_scale`].
///
/// Exported as `#[no_mangle]` for C callers.  Computed as
/// `(scale2 - scale1) / (stop - start)`.
#[no_mangle]
pub static mut rw_scalestep: fixed_t = 0;

/// Texture vertical midpoint for the middle wall texture (fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Controls where the texture
/// origin sits relative to the column; affected by `DONTPEGBOTTOM`.
#[no_mangle]
pub static mut rw_midtexturemid: fixed_t = 0;

/// Texture vertical midpoint for the upper wall texture (fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Affected by `DONTPEGTOP`.
#[no_mangle]
pub static mut rw_toptexturemid: fixed_t = 0;

/// Texture vertical midpoint for the lower wall texture (fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Affected by `DONTPEGBOTTOM`.
#[no_mangle]
pub static mut rw_bottomtexturemid: fixed_t = 0;

/// Front sector ceiling height minus `viewz`, in world units (not fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Used as the top boundary for
/// wall-texture rendering.  Shifted right by 4 before the seg loop.
#[no_mangle]
pub static mut worldtop: c_int = 0;

/// Front sector floor height minus `viewz`, in world units (not fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Used as the bottom boundary for
/// wall-texture rendering.  Shifted right by 4 before the seg loop.
#[no_mangle]
pub static mut worldbottom: c_int = 0;

/// Back sector ceiling height minus `viewz`, in world units (not fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Meaningful only for two-sided
/// lines; used to determine if an upper texture is needed.
#[no_mangle]
pub static mut worldhigh: c_int = 0;

/// Back sector floor height minus `viewz`, in world units (not fixed-point).
///
/// Exported as `#[no_mangle]` for C callers.  Meaningful only for two-sided
/// lines; used to determine if a lower texture is needed.
#[no_mangle]
pub static mut worldlow: c_int = 0;

/// Screen-y of the top of the upper texture at the current column (scaled).
///
/// Exported as `#[no_mangle]` for C callers.  Stepped by [`pixhighstep`]
/// each column in `R_RenderSegLoop`.
#[no_mangle]
pub static mut pixhigh: fixed_t = 0;

/// Screen-y of the bottom of the lower texture at the current column (scaled).
///
/// Exported as `#[no_mangle]` for C callers.  Stepped by [`pixlowstep`] each
/// column in `R_RenderSegLoop`.
#[no_mangle]
pub static mut pixlow: fixed_t = 0;

/// Per-column step for [`pixhigh`].
///
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut pixhighstep: fixed_t = 0;

/// Per-column step for [`pixlow`].
///
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut pixlowstep: fixed_t = 0;

/// Fractional screen-y of the top of the wall at the current column.
///
/// Exported as `#[no_mangle]` for C callers.  Maintained in 12.20 format;
/// `>> HEIGHTBITS` yields the integer screen row.
#[no_mangle]
pub static mut topfrac: fixed_t = 0;

/// Per-column step for [`topfrac`].
///
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut topstep: fixed_t = 0;

/// Fractional screen-y of the bottom of the wall at the current column.
///
/// Exported as `#[no_mangle]` for C callers.  Maintained in 12.20 format;
/// `>> HEIGHTBITS` yields the integer screen row.
#[no_mangle]
pub static mut bottomfrac: fixed_t = 0;

/// Per-column step for [`bottomfrac`].
///
/// Exported as `#[no_mangle]` for C callers.
#[no_mangle]
pub static mut bottomstep: fixed_t = 0;

/// Pointer to the scale-based light table row for the current seg.
///
/// Exported as `#[no_mangle]` for C callers.  Points into
/// `scalelight[lightnum]`; indexed by `rw_scale >> LIGHTSCALESHIFT` to pick
/// the per-column colormap.
#[no_mangle]
pub static mut walllights: *mut *mut c_uchar = ptr::null_mut();

/// Per-column texture-U coordinate buffer for masked mid-textures.
///
/// Exported as `#[no_mangle]` for C callers.  Points into the `openings`
/// scratch buffer (in `r_plane`); written by `R_RenderSegLoop` and consumed by
/// `R_RenderMaskedSegRange`.  A value of `c_short::MAX` (SHRT_MAX) means the
/// column has already been drawn or is not present.
#[no_mangle]
pub static mut maskedtexturecol: *mut c_short = ptr::null_mut();
