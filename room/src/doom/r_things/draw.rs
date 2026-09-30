//! Vissprite drawing: the masked-column walker, the vissprite rasterizer,
//! the psprite (weapon overlay) path, the pool sort, the per-sprite
//! drawseg clip, and the end-of-frame masked pass.

use std::ffi::{c_int, c_short, c_void};
use std::ptr;

use crate::doom::c_ffi::{vissprite_t, BASEYCENTER};
use crate::doom::d_player::{PspdefT, NUMPSPRITES};
#[cfg(feature = "rangecheck")]
use crate::i_error;
use crate::doom::info::*;
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACBITS, FRACUNIT};
use crate::doom::r_bsp::{drawsegs, ds_p, subsector_t};
use crate::doom::r_data::{colormaps, firstspritelump, spriteoffset, spritetopoffset, spritewidth};
use crate::doom::r_draw::{
    dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_translation, dc_x, dc_yl, dc_yh,
    viewheight, viewwidth,
};
use crate::doom::r_main::{
    basecolfunc, centerxfrac, centeryfrac, colfunc, detailshift, extralight, fixedcolormap,
    fuzzcolfunc, scalelight, transcolfunc, viewangleoffset, viewplayer, R_PointOnSegSide,
};
use crate::doom::r_segs::R_RenderMaskedSegRange;
use crate::doom::w_wad::W_CacheLumpNum;

use super::state::{
    LIGHTLEVELS, LIGHTSEGSHIFT, MAXLIGHTSCALE, MAXW, SIL_BOTTOM, SIL_TOP, mceilingclip,
    mfloorclip, negonearray, pspriteiscale, pspritescale, screenheightarray, spritedef_t,
    spritelights, sprtopscreen, spryscale, sprites, vsprsortedhead, vissprite_p, vissprites,
    FF_FRAMEMASK, FF_FULLBRIGHT, pw_invisibility,
};

#[cfg(feature = "rangecheck")]
use super::state::numsprites;

/// On-disk / in-memory header of a `patch_t` graphic lump.
/// The column offset array immediately follows in the lump data (not part of
/// this struct), accessed via pointer arithmetic in `R_DrawVisSprite`.
/// Packed to match the WAD lump layout exactly.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct patch_t {
    /// Total width of the patch in pixels.
    width: i16,
    /// Total height of the patch in pixels.
    height: i16,
    /// Horizontal draw offset from the patch origin to column 0.
    leftoffset: i16,
    /// Vertical draw offset from the patch origin to row 0.
    topoffset: i16,
}

/// Header of a single vertical post (run of opaque pixels) within a `patch_t`
/// column. The actual pixel bytes follow immediately after this struct in memory.
/// A `topdelta` value of `0xff` signals the end of the column.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct column_t {
    /// Y offset of the top of this post relative to the patch top.
    topdelta: u8,
    /// Number of pixel rows in this post.
    length: u8,
}

/// Draw one vertical column of a masked (partly transparent) sprite or
/// mid-texture using the current `colfunc`.
///
/// Iterates over the post list stored in `column`, clipping each post against
/// `mfloorclip` and `mceilingclip` before invoking `colfunc`. Used for both
/// world sprites (via `R_DrawVisSprite`) and masked mid-textures
/// (via `R_RenderMaskedSegRange`).
/// Exported as `#[no_mangle]` for C callers (called from `r_segs.rs`).
///
/// # Safety
/// - `column` must point to a valid `column_t` sequence terminated by
///   `topdelta == 0xff`.
/// - `mfloorclip`, `mceilingclip`, `dc_x`, `sprtopscreen`, and `spryscale`
///   must all be valid for the current column being drawn.
/// - `colfunc` must be set to a valid column renderer before calling.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_segs` calls the upstream name by path through the root shim.
#[doc(alias = "R_DrawMaskedColumn")]
#[export_name = "R_DrawMaskedColumn"]
pub unsafe extern "C" fn draw_masked_column(column: *mut c_void) {
    let mut column = column as *mut column_t;
    let basetexturemid = dc_texturemid;

    while (*column).topdelta != 0xff {
        // Calculate unclipped screen coordinates for post.
        let topscreen = sprtopscreen + spryscale * (*column).topdelta as c_int;
        let bottomscreen = topscreen + spryscale * (*column).length as c_int;

        dc_yl = (topscreen + FRACUNIT - 1) >> FRACBITS;
        dc_yh = (bottomscreen - 1) >> FRACBITS;

        if dc_yh >= *mfloorclip.add(dc_x as usize) as c_int { dc_yh = *mfloorclip.add(dc_x as usize) as c_int - 1; }
        if dc_yl <= *mceilingclip.add(dc_x as usize) as c_int { dc_yl = *mceilingclip.add(dc_x as usize) as c_int + 1; }

        if dc_yl <= dc_yh {
            dc_source = (column as *mut u8).add(3);
            dc_texturemid = basetexturemid - (((*column).topdelta as c_int) << FRACBITS);

            if let Some(func) = colfunc { func(); }
        }
        column = (column as *mut u8).add((*column).length as usize + 4) as *mut column_t;
    }

    dc_texturemid = basetexturemid;
}

/// Draw a fully projected vissprite to the screen.
///
/// Sets up the column renderer state (`dc_colormap`, `dc_iscale`,
/// `dc_texturemid`, `spryscale`, `sprtopscreen`) from the vissprite fields,
/// then iterates over each screen column from `vis.x1` to `vis.x2`, locating
/// the corresponding patch column and calling `R_DrawMaskedColumn`.
///
/// The column function (`colfunc`) is temporarily overridden to `fuzzcolfunc`
/// for shadow-drawn sprites (null colormap) or `transcolfunc` for translated
/// sprites, and restored to `basecolfunc` afterwards.
///
/// `mfloorclip` and `mceilingclip` must be set by the caller before this
/// function is invoked.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// - `vis` must point to a fully initialised `vissprite_t` as produced by
///   `R_ProjectSprite` or `R_DrawPSprite`.
/// - The patch lump referenced by `vis.patch + firstspritelump` must be loaded
///   in the WAD cache.
/// - `mfloorclip` and `mceilingclip` must be valid arrays of at least
///   `SCREENWIDTH` elements.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim.
#[doc(alias = "R_DrawVisSprite")]
#[export_name = "R_DrawVisSprite"]
pub unsafe extern "C" fn draw_vis_sprite(vis: *mut vissprite_t, _x1: c_int, _x2: c_int) {
    let patch = W_CacheLumpNum((*vis).patch + firstspritelump, 8) as *mut patch_t; // PU_CACHE = 8

    dc_colormap = (*vis).colormap;

    if dc_colormap.is_null() {
        // NULL colormap = shadow draw.
        colfunc = fuzzcolfunc;
    }
    else if(*vis).mobjflags & MF_TRANSLATION != 0 {
        colfunc = transcolfunc;
        dc_translation = colormaps
            .sub(256)
            .add((((*vis).mobjflags & MF_TRANSLATION) >> (MF_TRANSSHIFT - 8)) as usize);
    }

    dc_iscale = ((*vis).xiscale.abs() >> detailshift) as c_int;
    dc_texturemid = (*vis).texturemid;
    let mut frac = (*vis).startfrac;
    spryscale = (*vis).scale;
    sprtopscreen = centeryfrac - FixedMul(dc_texturemid, spryscale);

    for x in (*vis).x1..=(*vis).x2 {
        dc_x = x;
        let texturecolumn = frac >> FRACBITS;

        #[cfg(feature = "rangecheck")]
        {
            let patch_width = (*patch).width as c_int;
            if texturecolumn < 0 || texturecolumn >= patch_width { i_error!("R_DrawSpriteRange: bad texturecolumn"); }
        }

        let columnofs = (patch as *mut u8).add(8) as *mut c_int;
        let col_offset = *columnofs.add(texturecolumn as usize);
        let col = (patch as *mut u8).add(col_offset as usize) as *mut column_t;
        draw_masked_column(col as *mut c_void);

        frac += (*vis).xiscale;
    }

    colfunc = basecolfunc;
}

/// Draw one player weapon sprite (psprite) onto the screen.
///
/// Psprites are rendered in screen (HUD) space rather than world space:
/// the horizontal position is based on `psp.sx` offset from the screen
/// centre and the vertical position on `psp.sy` relative to `BASEYCENTER`.
/// A temporary stack-allocated `vissprite_t` (`avis`) is filled and passed
/// directly to `R_DrawVisSprite`; it is never inserted into the vissprite pool.
///
/// If the player carries the partial-invisibility power-up at a sufficient
/// level the weapon is drawn with `fuzzcolfunc` (shadow effect).
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// - `psp` must be a valid, non-null `PspdefT` pointer whose `state` field
///   points to a valid `State`.
/// - `sprites`, `pspritescale`, `pspriteiscale`, `spritelights`,
///   `mfloorclip`, and `mceilingclip` must all be valid for the current frame.
/// - `viewplayer` must point to a fully initialised player structure.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim.
#[doc(alias = "R_DrawPSprite")]
#[export_name = "R_DrawPSprite"]
pub unsafe extern "C" fn draw_psprite(psp: *mut PspdefT) {
    let psp = &*psp;

    // Decide which patch to use.
    if psp.state.is_null() { return; }

    let state = psp.state as *mut State;

    #[cfg(feature = "rangecheck")]
    {
        let sprite = (*state).sprite;
        if sprite as u32 >= numsprites as u32 { i_error!("R_ProjectSprite: invalid sprite number {}", sprite); }
    }

    let sprdef = &*(sprites as *mut spritedef_t).add((*state).sprite as usize);
    #[cfg(feature = "rangecheck")]
    {
        if ((*state).frame & FF_FRAMEMASK) >= sprdef.numframes {
            i_error!(
                "R_ProjectSprite [PSPRITE]: invalid sprite frame {} : {}",
                (*state).sprite,
                (*state).frame
            );
        }
    }
    let sprframe = &*sprdef
        .spriteframes
        .add(((*state).frame & FF_FRAMEMASK) as usize);

    let lump = sprframe.lump[0] as c_int;
    let flip = sprframe.flip[0] as c_int;

    // F1 M1: the weapon overlay reads the interpolation board's sampled
    // screen position when uncapped rendering is active. The flash slot
    // samples the weapon pair (vanilla copies sx/sy from the weapon slot
    // every tic), and a sprite state change snaps (Woof! p_pspr.c:1221), so
    // the weapon never slides between two different frames.
    let (sx, sy) = crate::doom::r_interp::sample_psp(psp as *const PspdefT as *mut PspdefT);

    // Calculate edges of the shape.
    let mut tx = sx - 160 * FRACUNIT;
    tx -= *spriteoffset.add(lump as usize);
    let x1 = (centerxfrac + FixedMul(tx, pspritescale)) >> FRACBITS;

    // Off the right side.
    if x1 > viewwidth { return; }

    tx += *spritewidth.add(lump as usize);
    let x2 = ((centerxfrac + FixedMul(tx, pspritescale)) >> FRACBITS) - 1;

    // Off the left side.
    if x2 < 0 { return; }

    // Store information in a vissprite.
    let mut avis: vissprite_t = unsafe { std::mem::zeroed() };
    let vis = &mut avis;
    vis.mobjflags = 0;
    vis.texturemid =
        (BASEYCENTER << FRACBITS) + FRACUNIT / 2 - (sy - *spritetopoffset.add(lump as usize));
    vis.x1 = if x1 < 0 { 0 } else { x1 };
    vis.x2 = if x2 >= viewwidth { viewwidth - 1 } else { x2 };
    vis.scale = pspritescale << detailshift;

    if flip != 0 {
        vis.xiscale = -pspriteiscale;
        vis.startfrac = *spritewidth.add(lump as usize) - 1;
    } else {
        vis.xiscale = pspriteiscale;
        vis.startfrac = 0;
    }

    if vis.x1 > x1 { vis.startfrac += vis.xiscale * (vis.x1 - x1); }

    vis.patch = lump;

    let player = &*viewplayer;
    if player.powers[pw_invisibility] > 4 * 32 || player.powers[pw_invisibility] & 8 != 0 {
        // Shadow draw.
        vis.colormap = ptr::null_mut();
    } else if !fixedcolormap.is_null() {
        // Fixed color.
        vis.colormap = fixedcolormap;
    } else if (*state).frame & FF_FULLBRIGHT != 0 {
        // Full bright.
        vis.colormap = colormaps;
    } else {
        // Local light.
        vis.colormap = *spritelights.add(MAXLIGHTSCALE - 1);
    }

    draw_vis_sprite(vis, vis.x1, vis.x2);
}

/// Draw all active player weapon sprites (psprites) for the current frame.
///
/// Determines the lighting level from the sector the player is standing in,
/// sets `mfloorclip` and `mceilingclip` to their full-screen defaults
/// (`screenheightarray` and `negonearray` respectively), then iterates over
/// `viewplayer.psprites` calling `R_DrawPSprite` for each slot with a
/// non-null state.
///
/// Only called from `R_DrawMasked` when `viewangleoffset == 0` (i.e., the
/// player is not looking through a camera or mirror).
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// - `viewplayer` must point to a fully initialised player structure with a
///   valid `mo -> subsector -> sector` chain.
/// - `scalelight`, `extralight`, `screenheightarray`, and `negonearray` must
///   all be valid before this function is called.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim.
#[doc(alias = "R_DrawPlayerSprites")]
#[export_name = "R_DrawPlayerSprites"]
pub unsafe extern "C" fn draw_player_sprites() {
    let mo = (*viewplayer).mo as *mut crate::doom::c_ffi::mobj_t;
    let sub = (*mo).subsector as *mut subsector_t;
    let sec = (*sub).sector;
    let lightnum = ((*sec).lightlevel >> LIGHTSEGSHIFT as i16) as c_int + extralight;

    if lightnum < 0 { spritelights = scalelight[0].as_mut_ptr(); }
    else if lightnum >= LIGHTLEVELS as c_int { spritelights = scalelight[LIGHTLEVELS - 1].as_mut_ptr(); }
    else { spritelights = scalelight[lightnum as usize].as_mut_ptr(); }

    // Clip to screen bounds.
    mfloorclip = std::ptr::addr_of_mut!(screenheightarray[0]);
    mceilingclip = std::ptr::addr_of_mut!(negonearray[0]);

    // Add all active psprites.
    let psp = (*viewplayer).psprites.as_ptr() as *mut PspdefT;
    for i in 0..NUMPSPRITES {
        let psp_i = psp.add(i);
        if !(*psp_i).state.is_null() { draw_psprite(psp_i); }
    }
}

/// Sort the vissprite pool into a back-to-front doubly-linked list for the
/// painter's algorithm.
///
/// Builds an `unsorted` circular list from the pool entries, then repeatedly
/// extracts the entry with the smallest `scale` value (farthest away) and
/// appends it to `vsprsortedhead`. This is an O(n^2) selection sort, matching
/// the original C implementation exactly.
///
/// After this call, `vsprsortedhead.next` is the farthest sprite and
/// `vsprsortedhead.prev` is the closest; traversing `next` links draws
/// sprites back-to-front.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// Must be called after all `R_ProjectSprite` calls for the frame and before
/// `R_DrawMasked` iterates `vsprsortedhead`. Modifies the `next`/`prev`
/// pointers of every vissprite in the pool.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim.
#[doc(alias = "R_SortVisSprites")]
#[export_name = "R_SortVisSprites"]
pub unsafe extern "C" fn sort_vis_sprites() {
    let count = vissprite_p.offset_from(std::ptr::addr_of_mut!(vissprites[0]));

    let mut unsorted: vissprite_t = unsafe { std::mem::zeroed() };
    unsorted.next = &mut unsorted;
    unsorted.prev = &mut unsorted;

    if count == 0 { return; }

    for i in 0..count {
        let ds = std::ptr::addr_of_mut!(vissprites[0]).add(i as usize);
        (*ds).next = ds.add(1);
        (*ds).prev = ds.sub(1);
    }

    vissprites[0].prev = &mut unsorted;
    unsorted.next = std::ptr::addr_of_mut!(vissprites[0]);
    (*vissprite_p.sub(1)).next = &mut unsorted;
    unsorted.prev = vissprite_p.sub(1);

    // Pull the vissprites out by scale.
    vsprsortedhead.next = &raw mut vsprsortedhead;
    vsprsortedhead.prev = &raw mut vsprsortedhead;

    for _ in 0..count {
        let mut bestscale = c_int::MAX;
        let mut best = unsorted.next;

        let mut ds = unsorted.next;
        while ds != &mut unsorted {
            if (*ds).scale < bestscale {
                bestscale = (*ds).scale;
                best = ds;
            }
            ds = (*ds).next;
        }

        (*(*best).next).prev = (*best).prev;
        (*(*best).prev).next = (*best).next;
        (*best).next = &raw mut vsprsortedhead;
        (*best).prev = vsprsortedhead.prev;
        (*vsprsortedhead.prev).next = best;
        vsprsortedhead.prev = best;
    }
}

/// Clip and draw one sorted vissprite against the drawseg silhouette list.
///
/// Scans drawsegs from back to front to build per-column floor (`CLIPBOT`) and
/// ceiling (`CLIPTOP`) clip arrays. For each drawseg that overlaps the sprite's
/// horizontal span:
/// - If the seg is behind the sprite and has a masked mid-texture,
///   `R_RenderMaskedSegRange` is called immediately (mid-textures must be
///   drawn in depth order relative to sprites).
/// - If the seg is in front of the sprite, its silhouette (`SIL_BOTTOM`,
///   `SIL_TOP`, or both) is used to tighten `CLIPBOT`/`CLIPTOP` for those
///   columns not yet set.
///
/// After all drawsegs are processed, unset clip values default to `viewheight`
/// (bottom) and `-1` (top), then `R_DrawVisSprite` is called.
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// - `spr` must be a valid `vissprite_t` pointer from the sorted list.
/// - `ds_p` and `drawsegs` must be valid for the current frame.
/// - `R_SortVisSprites` must have been called this frame before any calls to
///   this function.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach it through the root shim. The function-local
/// `static mut CLIPBOT`/`CLIPTOP` scratch arrays stay inline (per-frame
/// scratch shared across invocations; do not convert to stack arrays).
#[doc(alias = "R_DrawSprite")]
#[export_name = "R_DrawSprite"]
pub unsafe extern "C" fn draw_sprite(spr: *mut vissprite_t) {
    let spr = &*spr;

    static mut CLIPBOT: [c_short; MAXW] = [0; MAXW];
    static mut CLIPTOP: [c_short; MAXW] = [0; MAXW];

    for x in spr.x1..=spr.x2 {
        CLIPBOT[x as usize] = -2;
        CLIPTOP[x as usize] = -2;
    }

    // Scan drawsegs from end to start for obscuring segs.
    let mut ds = ds_p.sub(1);
    let drawsegs_base = std::ptr::addr_of_mut!(drawsegs[0]);
    while ds >= drawsegs_base {
        // Determine if the drawseg obscures the sprite.
        if (*ds).x1 > spr.x2
            || (*ds).x2 < spr.x1
            || ((*ds).silhouette == 0 && (*ds).maskedtexturecol.is_null())
        {
            // Does not cover sprite.
            ds = ds.sub(1);
            continue;
        }

        let r1 = if (*ds).x1 < spr.x1 { spr.x1 } else { (*ds).x1 };
        let r2 = if (*ds).x2 > spr.x2 { spr.x2 } else { (*ds).x2 };

        let (lowscale, scale): (fixed_t, fixed_t);
        if (*ds).scale1 > (*ds).scale2 {
            lowscale = (*ds).scale2;
            scale = (*ds).scale1;
        } else {
            lowscale = (*ds).scale1;
            scale = (*ds).scale2;
        }

        if scale < spr.scale
            || (lowscale < spr.scale && R_PointOnSegSide(spr.gx, spr.gy, (*ds).curline) == 0)
        {
            // Masked mid texture?
            if !(*ds).maskedtexturecol.is_null() { R_RenderMaskedSegRange(ds, r1, r2); }
            // Seg is behind sprite.
            ds = ds.sub(1);
            continue;
        }

        // Clip this piece of the sprite.
        let mut silhouette = (*ds).silhouette;

        if spr.gz >= (*ds).bsilheight { silhouette &= !SIL_BOTTOM; }
        if spr.gzt <= (*ds).tsilheight { silhouette &= !SIL_TOP; }

        if silhouette == SIL_BOTTOM {
            for x in r1..=r2 { if CLIPBOT[x as usize] == -2 { CLIPBOT[x as usize] = *(*ds).sprbottomclip.add(x as usize); } }
        }
        else if silhouette == SIL_TOP {
            for x in r1..=r2 { if CLIPTOP[x as usize] == -2 { CLIPTOP[x as usize] = *(*ds).sprtopclip.add(x as usize); } }
        }
        else if silhouette == (SIL_TOP | SIL_BOTTOM) {
            for x in r1..=r2 {
                if CLIPBOT[x as usize] == -2 { CLIPBOT[x as usize] = *(*ds).sprbottomclip.add(x as usize); }
                if CLIPTOP[x as usize] == -2 { CLIPTOP[x as usize] = *(*ds).sprtopclip.add(x as usize); }
            }
        }

        ds = ds.sub(1);
    }

    // All clipping has been performed, so draw the sprite.
    // Check for unclipped columns.
    for x in spr.x1..=spr.x2 {
        if CLIPBOT[x as usize] == -2 { CLIPBOT[x as usize] = viewheight as c_short; }
        if CLIPTOP[x as usize] == -2 { CLIPTOP[x as usize] = -1; }
    }

    mfloorclip = std::ptr::addr_of_mut!(CLIPBOT[0]);
    mceilingclip = std::ptr::addr_of_mut!(CLIPTOP[0]);
    draw_vis_sprite(
        spr as *const vissprite_t as *mut vissprite_t,
        spr.x1,
        spr.x2,
    );
}

/// Top-level masked rendering pass called at the end of each frame.
///
/// Performs four steps in order:
/// 1. Sorts the vissprite pool back-to-front via `R_SortVisSprites`.
/// 2. Draws each sorted vissprite with `R_DrawSprite` (which also interleaves
///    any masked mid-textures that are depth-behind the sprite).
/// 3. Draws any remaining masked mid-textures that were not rendered in step 2.
/// 4. Draws the player weapon overlay via `R_DrawPlayerSprites` (skipped when
///    `viewangleoffset != 0`, i.e., in demo playback camera modes).
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
/// Must be called after `R_RenderBSPNode` has filled the drawseg list and
/// the vissprite pool for the current frame. All renderer globals (`ds_p`,
/// `drawsegs`, `viewangleoffset`, etc.) must be valid.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_main` calls the upstream name through the root shim.
#[doc(alias = "R_DrawMasked")]
#[export_name = "R_DrawMasked"]
pub unsafe extern "C" fn draw_masked() {
    sort_vis_sprites();

    if vissprite_p > std::ptr::addr_of_mut!(vissprites[0]) {
        // Draw all vissprites back to front.
        let mut spr = vsprsortedhead.next;
        while spr != &raw mut vsprsortedhead {
            draw_sprite(spr);
            spr = (*spr).next;
        }
    }

    // Render any remaining masked mid textures.
    let mut ds = ds_p.sub(1);
    let drawsegs_base = std::ptr::addr_of_mut!(drawsegs[0]);
    while ds >= drawsegs_base {
        if !(*ds).maskedtexturecol.is_null() { R_RenderMaskedSegRange(ds, (*ds).x1, (*ds).x2); }
        ds = ds.sub(1);
    }

    // Draw the psprites on top of everything.
    if viewangleoffset == 0 { draw_player_sprites(); }
}
