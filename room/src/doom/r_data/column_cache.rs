//! The column/composite cache: the per-texture lookup tables backing
//! `R_GetColumn`, the lazy multi-patch compositor, and the byte-endian
//! helpers shared with the init pass. This is the frame-side hot half of
//! `r_data.c` -- every wall/plane/sky column passes through `get_column`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_short, c_ushort, c_void};
use std::ptr;

use crate::doom::crt::c_printf1;
use crate::doom::w_wad::W_CacheLumpNum;
use crate::i_error;
use crate::doom::z_zone::{PU_CACHE, PU_STATIC, Z_ChangeTag2, Z_Free, Z_Malloc};

use super::types::{column_t, patch_t, texture_t, texpatch_t};

// ---------------------------------------------------------------------------
// Byte-order helpers (i_swap.h SHORT / LONG)
// ---------------------------------------------------------------------------

/// Identity byte-swap for little-endian targets (mirrors `i_swap.h` `SHORT`).
///
/// On all currently supported targets `i16` is already little-endian, so this
/// is a no-op.  Present to keep the code structurally parallel to the C source.
#[doc(alias = "SHORT")]
#[inline]
pub(super) fn le_i16(x: i16) -> i16 { x }

/// Identity word-swap for little-endian targets (mirrors `i_swap.h` `LONG`).
///
/// Analogous to `SHORT`; a no-op on little-endian hosts.
#[doc(alias = "LONG")]
#[inline]
pub(super) fn le_i32(x: c_int) -> c_int { x }

// ---------------------------------------------------------------------------
// Module-private lookup-table statics
// ---------------------------------------------------------------------------

/// Array of pointers to runtime texture descriptors, length `numtextures`.
///
/// Allocated from the zone heap during `R_InitTextures`.  Private to the
/// module; `init` fills it, this file and `lookup`/`dtmc` read it.
pub(super) static mut textures: *mut *mut texture_t = ptr::null_mut();

/// Hash table of texture pointers for O(1) name lookup, length `numtextures`.
///
/// Populated by `GenerateTextureHashTable`.  Each slot is the head of a
/// linked list chained through `texture_t::next`.  Private to the module.
pub(super) static mut textures_hashtable: *mut *mut texture_t = ptr::null_mut();

/// Per-texture column width mask (`width_rounded_up_to_power_of_two - 1`).
///
/// Used by `R_GetColumn` to wrap column indices: `col & texturewidthmask[tex]`.
pub(super) static mut texturewidthmask: *mut c_int = ptr::null_mut();

/// Per-texture total byte size of the composite column buffer, length `numtextures`.
///
/// A zero entry means the texture has no multi-patch columns and no composite
/// buffer is ever allocated.
pub(super) static mut texturecompositesize: *mut c_int = ptr::null_mut();

/// Per-texture array of column-lump indices, each array has `texture.width` entries.
///
/// `texturecolumnlump[tex][col]` is the WAD lump number to use for column
/// `col` of texture `tex`, or `-1` if the column requires compositing.
pub(super) static mut texturecolumnlump: *mut *mut c_short = ptr::null_mut();

/// Per-texture array of column byte offsets, each array has `texture.width` entries.
///
/// `texturecolumnofs[tex][col]` is the byte offset within the lump (or within
/// the composite buffer) for the start of column data.
pub(super) static mut texturecolumnofs: *mut *mut c_ushort = ptr::null_mut();

/// Per-texture pointer to the composited column buffer, length `numtextures`.
///
/// Null until `R_GenerateComposite` is called for that texture.  Tagged
/// `PU_CACHE` so the zone allocator may evict it; `R_GetColumn` regenerates
/// on the next access.
pub(super) static mut texturecomposite: *mut *mut u8 = ptr::null_mut();

// ---------------------------------------------------------------------------
// R_DrawColumnInCache
// ---------------------------------------------------------------------------

/// Copy a single patch column into a pre-allocated composite texture buffer.
///
/// Iterates over the post list in `patch` (terminated by `topdelta == 0xff`)
/// and `memcpy`s each run of pixels into `cache` at the correct vertical
/// offset.  Clips posts that extend above zero or below `cacheheight`.
///
/// # Parameters
///
/// - `patch` - pointer to the first `column_t` (post) of the column.
/// - `cache` - pointer to the start of the destination column inside the
///   composite buffer (i.e., `block + colofs[x]`).
/// - `originy` - vertical origin of the owning patch within the texture.
/// - `cacheheight` - height of the texture in pixels; used for clipping.
///
/// # Safety
///
/// - `patch` must point to a valid column terminated by a `0xff` topdelta.
/// - `cache` must have at least `cacheheight` bytes of writable storage.
/// - `originy + post.topdelta` must not underflow past `i32::MIN` (safe for
///   all legal WAD data).
unsafe fn draw_column_in_cache(
    patch: *mut column_t,
    cache: *mut u8,
    originy: c_int,
    cacheheight: c_int,
) {
    let mut patch = patch;
    while (*patch).topdelta != 0xff {
        let source = (patch as *mut u8).add(3);
        let mut count = (*patch).length as c_int;
        let mut position = originy + (*patch).topdelta as c_int;

        if position < 0 {
            count += position;
            position = 0;
        }
        if position + count > cacheheight { count = cacheheight - position; }
        if count > 0 { std::ptr::copy_nonoverlapping(source, cache.add(position as usize), count as usize); }
        patch = (patch as *mut u8).add((*patch).length as usize + 4) as *mut column_t;
    }
}

// ---------------------------------------------------------------------------
// R_GenerateComposite
// ---------------------------------------------------------------------------

/// Build the composite texture buffer for texture `texnum` from its patches.
///
/// Allocates a `PU_STATIC` zone block large enough for all composited columns
/// (size pre-computed by `R_GenerateLookup`), then calls
/// `draw_column_in_cache` for every column that requires compositing
/// (`texturecolumnlump[texnum][col] < 0`).  After compositing, the block is
/// downgraded to `PU_CACHE` so the zone allocator may evict it later.
///
/// Called lazily by `R_GetColumn` the first time a composited column of a
/// given texture is needed.
///
/// # Safety
///
/// - `texnum` must be in `0..numtextures`.
/// - `R_InitTextures` and `R_GenerateLookup` must have been called first.
/// - `texturecompositesize[texnum]` must be non-zero (ensured by
///   `R_GenerateLookup` for textures with overlapping patches).
#[doc(alias = "R_GenerateComposite")]
#[export_name = "R_GenerateComposite"]
pub unsafe extern "C" fn generate_composite(texnum: c_int) {
    let texture = *textures.add(texnum as usize);

    let block = Z_Malloc(
        *texturecompositesize.add(texnum as usize),
        PU_STATIC,
        texturecomposite.add(texnum as usize) as *mut c_void,
    ) as *mut u8;

    let collump = *texturecolumnlump.add(texnum as usize);
    let colofs = *texturecolumnofs.add(texnum as usize);
    let patchcount = (*texture).patchcount as c_int;
    let patches_base = std::ptr::addr_of!((*texture).patches) as *mut texpatch_t;

    for i in 0..patchcount {
        let patch = patches_base.add(i as usize);
        let realpatch = W_CacheLumpNum((*patch).patch, PU_CACHE) as *mut patch_t;
        let x1 = (*patch).originx as c_int;
        let mut x2 = x1 + le_i16((*realpatch).width) as c_int;

        let mut x = if x1 < 0 { 0 } else { x1 };
        if x2 > (*texture).width as c_int { x2 = (*texture).width as c_int; }

        let columnofs = (realpatch as *mut u8).add(8) as *mut c_int;
        while x < x2 {
            if *collump.add(x as usize) >= 0 {
                x += 1;
                continue;
            }
            let patchcol = (realpatch as *mut u8)
                .add(le_i32(*columnofs.add((x - x1) as usize)) as usize)
                as *mut column_t;
            draw_column_in_cache(
                patchcol,
                block.add(*colofs.add(x as usize) as usize),
                (*patch).originy as c_int,
                (*texture).height as c_int,
            );
            x += 1;
        }
    }

    Z_ChangeTag2(block as *mut c_void, PU_CACHE, ptr::null(), 0);
}

// ---------------------------------------------------------------------------
// R_GenerateLookup
// ---------------------------------------------------------------------------

/// Pre-compute per-column lump/offset lookup tables for texture `texnum`.
///
/// For each column of the texture:
/// - Counts how many patches cover it.
/// - If exactly one patch covers it, records the patch lump number and byte
///   offset so `R_GetColumn` can serve data directly from the WAD cache.
/// - If multiple patches overlap, sets `collump[col] = -1` and accumulates
///   `texturecompositesize` to reserve space for the later composite buffer.
///
/// Prints a warning (and returns early) if a column has no patch coverage.
/// Calls `I_Error` if the composite buffer would exceed 64 KiB (the vanilla
/// composite-size limit, kept).
///
/// Called once per texture by `R_InitTextures` during startup.
///
/// # Safety
///
/// - `texnum` must be in `0..numtextures`.
/// - `texturecolumnlump[texnum]` and `texturecolumnofs[texnum]` must already
///   be allocated (done by `R_InitTextures` before this call).
#[doc(alias = "R_GenerateLookup")]
#[export_name = "R_GenerateLookup"]
pub unsafe extern "C" fn generate_lookup(texnum: c_int) {
    let texture = *textures.add(texnum as usize);
    let width = (*texture).width as c_int;

    *texturecomposite.add(texnum as usize) = ptr::null_mut();
    *texturecompositesize.add(texnum as usize) = 0;
    let collump = *texturecolumnlump.add(texnum as usize);
    let colofs = *texturecolumnofs.add(texnum as usize);

    let mut patchcount_ptr: *mut u8 = ptr::null_mut();
    let _patchcount_arr = Z_Malloc(
        width,
        PU_STATIC,
        &mut patchcount_ptr as *mut *mut u8 as *mut c_void,
    ) as *mut u8;
    let patchcount = patchcount_ptr; // Z_Malloc wrote the allocated pointer here

    for x in 0..width {
        *patchcount.add(x as usize) = 0;
        *collump.add(x as usize) = 0;
    }

    let patches_base = std::ptr::addr_of!((*texture).patches) as *mut texpatch_t;
    for i in 0..(*texture).patchcount as c_int {
        let patch = patches_base.add(i as usize);
        let realpatch = W_CacheLumpNum((*patch).patch, PU_CACHE) as *mut patch_t;
        let x1 = (*patch).originx as c_int;
        let mut x2 = x1 + le_i16((*realpatch).width) as c_int;

        let mut x = if x1 < 0 { 0 } else { x1 };
        if x2 > width { x2 = width; }

        let columnofs = (realpatch as *mut u8).add(8) as *mut c_int;
        while x < x2 {
            *patchcount.add(x as usize) += 1;
            *collump.add(x as usize) = (*patch).patch as c_short;
            *colofs.add(x as usize) = (le_i32(*columnofs.add((x - x1) as usize)) + 3) as c_ushort;
            x += 1;
        }
    }

    for x in 0..width {
        if *patchcount.add(x as usize) == 0 {
            c_printf1(
                c"R_GenerateLookup: column without a patch (%s)\n".as_ptr(),
                (*texture).name.as_ptr(),
            );
            Z_Free(patchcount as *mut c_void);
            return;
        }
        if *patchcount.add(x as usize) > 1 {
            *collump.add(x as usize) = -1;
            *colofs.add(x as usize) = *texturecompositesize.add(texnum as usize) as c_ushort;

            if *texturecompositesize.add(texnum as usize) > 0x10000 - (*texture).height as c_int {
                i_error!("R_GenerateLookup: texture {} is >64k", texnum);
            }
            *texturecompositesize.add(texnum as usize) += (*texture).height as c_int;
        }
    }

    Z_Free(patchcount as *mut c_void);
}

// ---------------------------------------------------------------------------
// R_GetColumn
// ---------------------------------------------------------------------------

/// Return a pointer to the pixel data for column `col` of texture `tex`.
///
/// Applies the width mask to wrap `col` into range, then looks up the result
/// in the pre-computed `texturecolumnlump` / `texturecolumnofs` tables:
/// - If `lump > 0`: data comes directly from the WAD cache (single-patch
///   column). Note: lump 0 is treated as composite even though it is a valid
///   WAD lump number — this matches the C original.
/// - Otherwise: calls `generate_composite` on the first access for that
///   texture, then returns into the composite buffer.
///
/// This is the hot-path column-fetch function; called from `r_segs.c`,
/// `r_plane.c`, and `r_things.c` on every rendered column.
///
/// # Safety
///
/// - `tex` must be in `0..numtextures`.
/// - `R_InitTextures` and `R_GenerateLookup` must have been called.
/// - The returned pointer is valid until the zone allocator evicts the
///   underlying lump or composite buffer.
#[doc(alias = "R_GetColumn")]
#[export_name = "R_GetColumn"]
pub unsafe extern "C" fn get_column(tex: c_int, col: c_int) -> *mut u8 {
    let col = col & *texturewidthmask.add(tex as usize);
    let lump = *(*texturecolumnlump.add(tex as usize)).add(col as usize) as c_int;
    let ofs = *(*texturecolumnofs.add(tex as usize)).add(col as usize) as c_int;

    if lump > 0 { return (W_CacheLumpNum(lump, PU_CACHE) as *mut u8).add(ofs as usize); }

    if (*texturecomposite.add(tex as usize)).is_null() { generate_composite(tex); }

    (*texturecomposite.add(tex as usize)).add(ofs as usize)
}
