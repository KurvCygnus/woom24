//! The texture/patch data vocabulary of `r_data` (`r_data.c` internal
//! types plus the `r_local.h` graphics headers). `pub(super)` visibility:
//! every consumer lives inside this module directory; the old `pub`
//! `patch_t` / `post_t` / `column_t` had zero external consumers
//! (`v_video` and `r_things` define their own copies -- deliberate
//! mirrors, never unify across modules), so the graduation tightens
//! them to the module.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_short};

/// On-disk patch entry inside a `maptexture_t` WAD record (packed, C layout).
///
/// Corresponds to `mappatch_t` in `r_data.c`.  `stepdir` and `colormap` are
/// present in the WAD format but unused by the renderer.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(super) struct mappatch_t {
    /// Horizontal origin of the patch within the texture, in pixels.
    pub originx: i16,
    /// Vertical origin of the patch within the texture, in pixels.
    pub originy: i16,
    /// Index into the PNAMES patch directory.
    pub patch: i16,
    /// Unused animation field (always 1 in practice).
    pub stepdir: i16,
    /// Unused colormap field (always 0 in practice).
    pub colormap: i16,
}

/// On-disk texture definition as stored in TEXTURE1 / TEXTURE2 WAD lumps (packed, C layout).
///
/// Corresponds to `maptexture_t` in `r_data.c`.  The `patches` field is a
/// C flexible-array member: the struct is followed by `patchcount - 1`
/// additional `mappatch_t` entries in memory.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(super) struct maptexture_t {
    /// Texture name, up to 8 ASCII characters, NUL-padded.
    pub name: [c_char; 8],
    /// Non-zero if the texture has transparent holes (unused by the renderer).
    pub masked: c_int,
    /// Texture width in pixels.
    pub width: i16,
    /// Texture height in pixels.
    pub height: i16,
    /// Obsolete field present in the WAD format; ignored.
    pub obsolete: c_int,
    /// Number of `mappatch_t` entries that follow this struct in memory.
    pub patchcount: i16,
    /// First patch entry; additional patches follow contiguously in WAD data.
    pub patches: mappatch_t,
}

/// Runtime patch descriptor stored inside a `texture_t` (C layout).
///
/// Corresponds to `texpatch_t` in `r_data.c`.  `patch` is a WAD lump number
/// (resolved from the PNAMES index during `R_InitTextures`).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct texpatch_t {
    /// Horizontal origin of the patch within the texture, in pixels.
    pub originx: i16,
    /// Vertical origin of the patch within the texture, in pixels.
    pub originy: i16,
    /// WAD lump number of the patch graphic.
    pub patch: c_int,
}

/// Runtime texture descriptor held in the `textures` pointer array (C layout).
///
/// Corresponds to `texture_t` in `r_data.c`.  The `patches` field is a C
/// flexible-array member: the struct is allocated with room for
/// `patchcount - 1` additional `texpatch_t` entries immediately after.
/// The `next` pointer links entries that hash to the same bucket in
/// `textures_hashtable`.
#[repr(C)]
pub(super) struct texture_t {
    /// Texture name, up to 8 ASCII characters, NUL-padded.
    pub name: [c_char; 8],
    /// Texture width in pixels.
    pub width: i16,
    /// Texture height in pixels.
    pub height: i16,
    /// Index of this texture in the `textures` array; set by `GenerateTextureHashTable`.
    pub index: c_int,
    /// Next entry in the hash-table chain for this bucket, or null.
    pub next: *mut texture_t,
    /// Number of `texpatch_t` entries that follow this struct in memory.
    pub patchcount: i16,
    /// First patch descriptor; additional patches follow contiguously.
    pub patches: texpatch_t,
}

/// Patch graphic header, matching the WAD `patch_t` struct (packed, C layout).
///
/// Corresponds to `patch_t` in `r_local.h`.  Immediately after the four
/// header fields, `width` 32-bit column offsets follow (the `columnofs[]`
/// array), then the column data itself.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(super) struct patch_t {
    /// Width of the patch in pixels.
    pub width: i16,
    /// Height of the patch in pixels.
    pub height: i16,
    /// Horizontal draw offset from the patch origin, in pixels.
    pub leftoffset: i16,
    /// Vertical draw offset from the patch origin, in pixels.
    pub topoffset: i16,
}

/// Single run of opaque pixels within a column (a "post"), packed C layout.
///
/// Corresponds to `post_t` in `r_local.h`.  A column is a sequence of posts
/// terminated by a `topdelta` value of `0xff`.  The actual pixel data follows
/// immediately after the two-byte header (one byte of padding before the
/// pixels and one byte of padding after).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(super) struct post_t {
    /// Row offset from the top of the texture column where this post begins.
    pub topdelta: u8,
    /// Number of pixels in this post.
    pub length: u8,
}

/// Alias for `post_t`; used interchangeably in the C source as `column_t`.
pub(super) type column_t = post_t;

/// Per-rotation frame data for one sprite animation frame (C layout).
///
/// Corresponds to `spriteframe_t` in `r_local.h`.  `lump[r]` is the WAD
/// lump offset from `firstspritelump` for rotation `r`; `flip[r]` is non-zero
/// if that rotation should be drawn mirrored.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct spriteframe_t {
    /// Non-zero if the sprite has per-rotation variants; 0 for a single view.
    pub rotate: c_int,
    /// Lump offsets (from `firstspritelump`) for each of the 8 rotations.
    pub lump: [c_short; 8],
    /// Mirror flags for each rotation (non-zero = draw flipped).
    pub flip: [u8; 8],
}

/// Sprite definition holding all animation frames for one sprite class (C layout).
///
/// Corresponds to `spritedef_t` in `r_local.h`.  `spriteframes` points to a
/// heap-allocated array of `numframes` `spriteframe_t` entries.
#[repr(C)]
pub(super) struct spritedef_t {
    /// Number of animation frames.
    pub numframes: c_int,
    /// Heap-allocated array of frame descriptors, length `numframes`.
    pub spriteframes: *mut spriteframe_t,
}
