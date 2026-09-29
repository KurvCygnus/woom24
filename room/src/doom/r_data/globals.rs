//! The 20 `#[no_mangle]` public statics of `r_data` (`r_data.c` globals
//! exported through `r_state.h` and, for three of them, declared locally
//! in `p_spec.c`): one data home, upstream names + C linkage retained.
//! `p_spec` writes the two translation tables under the freeze zone;
//! `c_tests/r_data_c.rs` reads seven of them through the module-root
//! paths held by the re-export block in `mod.rs`.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_int;
use std::ptr;

/// WAD lump index of the first flat (the lump after `F_START`).
///
/// Exported as `extern int firstflat` in `r_state.h`.  Used by `r_plane.c`
/// to compute lump numbers from flat indices: `lump = firstflat + flatnum`.
#[no_mangle]
pub static mut firstflat: c_int = 0;

/// WAD lump index of the last flat (the lump before `F_END`).
///
/// Not in `r_state.h`; declared locally in `p_spec.c`.  Together with
/// `firstflat` it defines the flat lump range in the WAD.
#[no_mangle]
pub static mut lastflat: c_int = 0;

/// Total number of flat lumps (`lastflat - firstflat + 1`).
///
/// Declared locally in `p_spec.c` (not in `r_state.h`) as `extern int numflats`
/// for animation bounds checking.
#[no_mangle]
pub static mut numflats: c_int = 0;

/// WAD lump index of the first patch lump (the lump after the patch namespace marker).
///
/// Initialized but not currently used by the renderer; present to mirror the C globals.
#[no_mangle]
pub static mut firstpatch: c_int = 0;

/// WAD lump index of the last patch lump.
#[no_mangle]
pub static mut lastpatch: c_int = 0;

/// Total number of patch lumps.
#[no_mangle]
pub static mut numpatches: c_int = 0;

/// WAD lump index of the first sprite lump (the lump after `S_START`).
///
/// Exported via `r_state.h`.  Used by `r_things.c` and `f_finale.c` to
/// convert sprite-relative lump indices to absolute WAD lump numbers:
/// `lump = firstspritelump + relative`.
#[no_mangle]
pub static mut firstspritelump: c_int = 0;

/// WAD lump index of the last sprite lump (the lump before `S_END`).
///
/// Exported via `r_state.h`.  Used by `r_things.c` when scanning the WAD for
/// sprite frames.
#[no_mangle]
pub static mut lastspritelump: c_int = 0;

/// Total number of sprite lumps (`lastspritelump - firstspritelump + 1`).
///
/// Exported via `r_state.h`.  Bounds the `spritewidth`, `spriteoffset`, and
/// `spritetopoffset` arrays.
#[no_mangle]
pub static mut numspritelumps: c_int = 0;

/// Total number of wall textures loaded from TEXTURE1 and TEXTURE2.
///
/// Exported as `#[no_mangle]`.  Bounds all per-texture arrays and the hash
/// table.
#[no_mangle]
pub static mut numtextures: c_int = 0;

/// Per-texture height in 16.16 fixed-point units, length `numtextures`.
///
/// Exported via `r_state.h` as `fixed_t *textureheight`.  Used by `r_segs.c`
/// and `p_floor.c` for texture-pegging calculations.
#[no_mangle]
pub static mut textureheight: *mut c_int = ptr::null_mut();

/// Flat animation translation table, length `numflats + 1`.
///
/// Exported via `r_state.h` as `int *flattranslation`.  `p_spec.c` updates
/// entries during animated flat processing; `r_plane.c` uses
/// `flattranslation[pl->picnum]` when fetching the actual lump to draw.
#[no_mangle]
pub static mut flattranslation: *mut c_int = ptr::null_mut();

/// Texture animation translation table, length `numtextures + 1`.
///
/// Exported via `r_state.h` as `int *texturetranslation`.  `p_spec.c`
/// updates entries for animated textures; `r_segs.c` applies the translation
/// before calling `R_GetColumn`.
#[no_mangle]
pub static mut texturetranslation: *mut c_int = ptr::null_mut();

/// Per-sprite-lump width in 16.16 fixed-point units, length `numspritelumps`.
///
/// Exported via `r_state.h` as `fixed_t *spritewidth`.  Used by `r_things.c`
/// to compute screen-space sprite extents.
#[no_mangle]
pub static mut spritewidth: *mut c_int = ptr::null_mut();

/// Per-sprite-lump horizontal draw offset in 16.16 fixed-point units, length `numspritelumps`.
///
/// Exported via `r_state.h` as `fixed_t *spriteoffset`.  Used by `r_things.c`
/// to position sprites relative to the thing's world coordinates.
#[no_mangle]
pub static mut spriteoffset: *mut c_int = ptr::null_mut();

/// Per-sprite-lump vertical draw offset in 16.16 fixed-point units, length `numspritelumps`.
///
/// Exported via `r_state.h` as `fixed_t *spritetopoffset`.  Used by
/// `r_things.c` to compute the top screen row of each sprite.
#[no_mangle]
pub static mut spritetopoffset: *mut c_int = ptr::null_mut();

/// Pointer to the COLORMAP lump data: 34 colormaps of 256 bytes each.
///
/// Exported via `r_state.h` as `lighttable_t *colormaps`.  The renderer
/// indexes this as `colormaps + light_level * 256` to obtain a 256-entry
/// palette remapping table.  Used by `r_main.c`, `r_draw.c`, `r_plane.c`,
/// and `r_things.c`.
#[no_mangle]
pub static mut colormaps: *mut u8 = ptr::null_mut();

/// Total bytes of flat data touched during `R_PrecacheLevel`, for diagnostics.
///
/// Exported as `#[no_mangle]`.  Corresponds to `flatmemory` in `r_data.c`.
/// Write-only diagnostic in this port (zero readers in tree) -- kept for
/// symbol parity.
#[no_mangle]
pub static mut flatmemory: c_int = 0;

/// Total bytes of texture patch data touched during `R_PrecacheLevel`, for diagnostics.
///
/// Exported as `#[no_mangle]`.  Corresponds to `texturememory` in `r_data.c`.
/// Write-only diagnostic in this port (zero readers in tree) -- kept for
/// symbol parity.
#[no_mangle]
pub static mut texturememory: c_int = 0;

/// Total bytes of sprite lump data touched during `R_PrecacheLevel`, for diagnostics.
///
/// Exported as `#[no_mangle]`.  Corresponds to `spritememory` in `r_data.c`.
/// Write-only diagnostic in this port (zero readers in tree) -- kept for
/// symbol parity.
#[no_mangle]
pub static mut spritememory: c_int = 0;
