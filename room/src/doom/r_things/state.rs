//! Data home for sprite rendering: the shared constants, the `spritedef_t`
//! vocabulary, all 13 `#[no_mangle]` statics, and the private vissprite
//! pool (`project`/`draw` share it via `super::`).

use std::ffi::{c_char, c_int, c_short, c_void};
use std::ptr;

use crate::doom::c_ffi::{spriteframe_t, vissprite_t};
use crate::doom::m_fixed::fixed_t;

/// Compile-time array cap for per-column tables (F1 M2, boom
/// `MAX_SCREENWIDTH` shape); the live raster width is the `SCREENWIDTH`
/// static re-exported from `i_video`.
pub(super) const MAXW: usize = crate::doom::video_cfg::MAX_SCREENWIDTH as usize;

/// Maximum number of visible sprites that can be projected in a single frame.
/// Sprites beyond this limit are silently dropped into `overflowsprite`.
pub(super) const MAXVISSPRITES: usize = 128;

/// Number of distinct light level bands used by the scale-light table.
pub(super) const LIGHTLEVELS: usize = 16;

/// Right-shift applied to a sector's light level to obtain a band index.
pub(super) const LIGHTSEGSHIFT: u32 = 4;

/// Maximum scale-light index; caps the luminosity lookup so the table is not
/// over-indexed for very close sprites.
pub(super) const MAXLIGHTSCALE: usize = 48;

/// Right-shift applied to a sprite's screen scale to produce a `scalelight` index.
pub(super) const LIGHTSCALESHIFT: u32 = 12;

/// Bitmask that isolates the frame index bits from a state frame field.
/// The upper bit (`FF_FULLBRIGHT`) is stripped so that only the 0-based
/// animation frame number remains.
pub(super) const FF_FRAMEMASK: c_int = 0x7fff;

/// Flag bit in a state frame field indicating the sprite should be drawn
/// at full brightness regardless of sector lighting.
pub(super) const FF_FULLBRIGHT: c_int = 0x8000;

/// Index into the player `powers` array for the partial-invisibility power-up.
/// Used when deciding whether to draw the player weapon with a fuzz (shadow)
/// column function.
pub(super) const pw_invisibility: usize = 2;

/// Silhouette flag: the drawseg has a valid bottom silhouette that can clip
/// sprites from below.
pub(super) const SIL_BOTTOM: c_int = 1;

/// Silhouette flag: the drawseg has a valid top silhouette that can clip
/// sprites from above.
pub(super) const SIL_TOP: c_int = 2;

/// Sprite definition table entry for one sprite name (e.g., "TROO").
/// Points to an array of `spriteframe_t` records, one per animation frame.
/// Mirrors `spritedef_t` from `r_local.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct spritedef_t {
    /// Number of animation frames defined for this sprite.
    pub(super) numframes: c_int,
    /// Pointer to the heap-allocated array of frame descriptors.
    pub(super) spriteframes: *mut spriteframe_t,
}

// ---------------------------------------------------------------------------
// Globals defined by this module
// ---------------------------------------------------------------------------

/// Scale applied to player weapon (psprite) columns this frame.
#[no_mangle]
pub static mut pspritescale: fixed_t = 0;

/// Inverse of pspritescale.
#[no_mangle]
pub static mut pspriteiscale: fixed_t = 0;

/// Pointer to the active light-table array for sprites this frame.
#[no_mangle]
pub static mut spritelights: *mut *mut u8 = ptr::null_mut();

/// Clipping array initialised to -1 for psprite bottom clipping.
/// Sized to the compile-time cap (boom `negonearray[MAX_SCREENWIDTH]`);
/// only `[0..viewwidth]` is initialised and read.
#[no_mangle]
pub static mut negonearray: [c_short; MAXW] = [0; MAXW];

/// Clipping array initialised to viewheight for psprite top clipping.
/// Sized to the compile-time cap; see [`negonearray`].
#[no_mangle]
pub static mut screenheightarray: [c_short; MAXW] = [0; MAXW];

/// Pointer to the sprite definition table.
#[no_mangle]
pub static mut sprites: *mut c_void = ptr::null_mut();

/// Total number of sprite names found in the WAD.
#[no_mangle]
pub static mut numsprites: c_int = 0;

/// Temporary frame-building array used during R_InitSprites.
#[no_mangle]
pub static mut sprtemp: [spriteframe_t; 29] = [spriteframe_t {
    rotate: 0,
    lump: [0; 8],
    flip: [0; 8],
}; 29];

/// Highest frame index seen for the current sprite during R_InitSprites.
#[no_mangle]
pub static mut maxframe: c_int = 0;

/// Name of the sprite currently being processed by R_InitSprites.
#[no_mangle]
pub static mut spritename: *mut c_char = ptr::null_mut();

// ---------------------------------------------------------------------------
// Module-local state
// ---------------------------------------------------------------------------

/// Fixed-size pool of projected sprite records for the current frame.
/// Accessed sequentially via `vissprite_p`; up to `MAXVISSPRITES` entries.
pub(super) static mut vissprites: [vissprite_t; MAXVISSPRITES] = unsafe { std::mem::zeroed() };

// ---------------------------------------------------------------------------
// Module-local state (also referenced by r_segs.rs via extern "C")
// ---------------------------------------------------------------------------

/// Pointer to the next free slot in `vissprites`; advanced by `R_NewVisSprite`.
pub(super) static mut vissprite_p: *mut vissprite_t = ptr::null_mut();

/// Unused counter retained for ABI compatibility with the C original.
pub(super) static mut newvissprite: c_int = 0;

/// Sentinel vissprite returned when the pool is exhausted so callers always
/// receive a valid (though discarded) write target.
pub(super) static mut overflowsprite: vissprite_t = unsafe { std::mem::zeroed() };

/// Mutable pointers used by masked column drawing (read by r_segs.rs).
#[no_mangle]
pub static mut mfloorclip: *mut c_short = ptr::null_mut();

/// Mutable pointers used by masked column drawing (read by r_segs.rs).
#[no_mangle]
pub static mut mceilingclip: *mut c_short = ptr::null_mut();

/// Current sprite Y scale (read by r_segs.rs).
#[no_mangle]
pub static mut spryscale: fixed_t = 0;

/// Screen Y coordinate of sprite top (read by r_segs.rs).
#[no_mangle]
pub static mut sprtopscreen: fixed_t = 0;

/// Dummy head node of the doubly-linked sorted vissprite list built by
/// `R_SortVisSprites`; iterated by `R_DrawMasked`.
pub(super) static mut vsprsortedhead: vissprite_t = unsafe { std::mem::zeroed() };
