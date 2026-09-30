//! The wall-segment entry point and its per-column raster loop:
//! `store_wall_range` (called by the `r_bsp` clipper for every visible
//! seg) and `render_seg_loop`. Distance/scale derivation, the
//! texture-pegging ladders, floor/ceiling mark decisions, sprite-clip
//! bookkeeping in the `openings` buffer, and every pixel the loop writes
//! are frame-golden surface -- moved bit-exact.

use std::ffi::{c_int, c_short};
use std::ptr;

use crate::doom::c_ffi::LinedefFlag;
use crate::doom::m_fixed::{fixed_t, FixedMul, FRACBITS};
use crate::doom::r_bsp::{backsector, curline, drawsegs, ds_p, frontsector, linedef, sidedef};
use crate::doom::r_data::{textureheight, texturetranslation, R_GetColumn};
use crate::doom::r_draw::{
    dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_x, dc_yh, dc_yl, viewheight,
};
use crate::doom::r_interp;
use crate::doom::r_main::{
    centeryfrac, colfunc, extralight, fixedcolormap, scalelight, viewangle, viewz, xtoviewangle,
    R_PointToDist, R_ScaleFromGlobalAngle,
};
use crate::doom::r_plane::{ceilingclip, ceilingplane, floorclip, floorplane, lastopening, R_CheckPlane};
use crate::doom::r_sky::skyflatnum;
use crate::doom::r_things::{negonearray, screenheightarray};
use crate::doom::tables::{self, ANG180, ANG90, ANGLETOFINESHIFT};

use super::state::{
    bottomfrac, bottomstep, bottomtexture, FINEMASK, HEIGHTBITS, HEIGHTUNIT, LIGHTLEVELS,
    LIGHTSCALESHIFT, LIGHTSEGSHIFT, markceiling, markfloor, maskedtexture, maskedtexturecol,
    MAXDRAWSEGS, MAXLIGHTSCALE, midtexture, pixhigh, pixhighstep, pixlow, pixlowstep,
    rw_bottomtexturemid, rw_centerangle, rw_distance, rw_midtexturemid, rw_normalangle, rw_angle1,
    rw_offset, rw_scale, rw_scalestep, rw_stopx, rw_toptexturemid, rw_x, segtextured, SIL_BOTH,
    SIL_BOTTOM, SIL_TOP, toptexture, topfrac, topstep, walllights, worldbottom, worldhigh, worldlow,
    worldtop,
};

/// Core per-column drawing loop for one wall segment.
///
/// Iterates from [`rw_x`] to [`rw_stopx`] (exclusive), performing the
/// following work for each column:
///
/// 1. Computes `yl` (top of wall) and `yh` (bottom of wall) from [`topfrac`]
///    and [`bottomfrac`], clamped to the current clip boundaries.
/// 2. If [`markceiling`] is set, records the ceiling span extent into
///    [`ceilingplane`].
/// 3. If [`markfloor`] is set, records the floor span extent into
///    [`floorplane`].
/// 4. If [`segtextured`] is set, computes the texture-U column and the
///    per-column colormap index.
/// 5. Draws the middle, upper, or lower texture tier as appropriate, and
///    updates [`ceilingclip`] / [`floorclip`].
/// 6. Records the masked texture column offset if [`maskedtexture`] is set.
///
/// # Safety
///
/// Reads and writes numerous `static mut` renderer globals.  All pointer
/// globals (`ceilingplane`, `floorplane`, `walllights`, `maskedtexturecol`,
/// `colfunc`) must be valid for the current frame.
#[doc(alias = "R_RenderSegLoop")]
unsafe fn render_seg_loop() {
    while rw_x < rw_stopx {
        let clip_ceil = ceilingclip[rw_x as usize] as c_int;
        let clip_floor = floorclip[rw_x as usize] as c_int;

        let mut yl = ((topfrac + HEIGHTUNIT - 1) >> HEIGHTBITS) as c_int;
        if yl < clip_ceil + 1 { yl = clip_ceil + 1; }

        if markceiling != 0 {
            let top = clip_ceil + 1;
            let mut bottom = yl - 1;
            if bottom >= clip_floor { bottom = clip_floor - 1; }
            if top <= bottom {
                (*ceilingplane).top[rw_x as usize] = top as u8;
                (*ceilingplane).bottom[rw_x as usize] = bottom as u8;
            }
        }

        let mut yh = (bottomfrac >> HEIGHTBITS) as c_int;
        if yh >= clip_floor { yh = clip_floor - 1; }

        if markfloor != 0 {
            let mut top = yh + 1;
            let bottom = clip_floor - 1;
            if top <= clip_ceil { top = clip_ceil + 1; }
            if top <= bottom {
                (*floorplane).top[rw_x as usize] = top as u8;
                (*floorplane).bottom[rw_x as usize] = bottom as u8;
            }
        }

        let texturecolumn: fixed_t;
        if segtextured != 0 {
            let angle =
                (rw_centerangle.wrapping_add(xtoviewangle[rw_x as usize])) >> ANGLETOFINESHIFT;
            texturecolumn =
                rw_offset - FixedMul(tables::finetangent[angle as usize & FINEMASK], rw_distance);

            let mut index = (rw_scale as u32) >> LIGHTSCALESHIFT;
            if index >= MAXLIGHTSCALE as u32 { index = (MAXLIGHTSCALE - 1) as u32; }
            dc_colormap = *walllights.add(index as usize);
            dc_x = rw_x;
            dc_iscale = (0xffffffff_u32 / (rw_scale as u32)) as c_int;
        }
        else { texturecolumn = 0; }

        if midtexture != 0 {
            dc_yl = yl;
            dc_yh = yh;
            dc_texturemid = rw_midtexturemid;
            dc_source = R_GetColumn(midtexture, texturecolumn >> FRACBITS);
            if let Some(func) = colfunc { func(); }
            ceilingclip[rw_x as usize] = viewheight as c_short;
            floorclip[rw_x as usize] = -1;
        } else {
            if toptexture != 0 {
                let mut mid = (pixhigh >> HEIGHTBITS) as c_int;
                pixhigh += pixhighstep;

                let clip_floor = floorclip[rw_x as usize] as c_int;
                if mid >= clip_floor { mid = clip_floor - 1; }

                if mid >= yl {
                    dc_yl = yl;
                    dc_yh = mid;
                    dc_texturemid = rw_toptexturemid;
                    dc_source = R_GetColumn(toptexture, texturecolumn >> FRACBITS);
                    if let Some(func) = colfunc { func(); }
                    ceilingclip[rw_x as usize] = mid as c_short;
                }
                else { ceilingclip[rw_x as usize] = (yl - 1) as c_short; }
            }
            else { if markceiling != 0 { ceilingclip[rw_x as usize] = (yl - 1) as c_short; } }

            if bottomtexture != 0 {
                let mut mid = ((pixlow + HEIGHTUNIT - 1) >> HEIGHTBITS) as c_int;
                pixlow += pixlowstep;

                let clip_ceil = ceilingclip[rw_x as usize] as c_int;
                if mid <= clip_ceil { mid = clip_ceil + 1; }

                if mid <= yh {
                    dc_yl = mid;
                    dc_yh = yh;
                    dc_texturemid = rw_bottomtexturemid;
                    dc_source = R_GetColumn(bottomtexture, texturecolumn >> FRACBITS);
                    if let Some(func) = colfunc { func(); }
                    floorclip[rw_x as usize] = mid as c_short;
                }
                else { floorclip[rw_x as usize] = (yh + 1) as c_short; }
            }
            else { if markfloor != 0 { floorclip[rw_x as usize] = (yh + 1) as c_short; } }

            if maskedtexture != 0 { *maskedtexturecol.add(rw_x as usize) = (texturecolumn >> FRACBITS) as c_short; }
        }

        rw_scale += rw_scalestep;
        topfrac += topstep;
        bottomfrac += bottomstep;
        rw_x += 1;
    }
}

/// Clips and draws the wall segment between screen columns `start` and `stop`.
///
/// This is the main entry point called by the BSP traversal for every visible
/// seg.  It performs all setup for the segment (distance, scale, texture
/// selection, floor/ceiling mark decisions) and then delegates pixel output to
/// `render_seg_loop`.  After the loop it saves sprite-clipping arrays into
/// the `openings` scratch buffer and advances [`ds_p`] to the next free
/// `drawseg_t` slot.
///
/// A wall range is silently ignored if the `drawseg_t` pool
/// (`drawsegs[MAXDRAWSEGS]`) is full; the C source does the same.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// `curline`, `frontsector`, and (for two-sided lines) `backsector` must be
/// valid pointers set by the BSP traversal before this call.  `ds_p` must be
/// a valid pointer into `drawsegs` or one-past-end (`&drawsegs[MAXDRAWSEGS]`);
/// the function checks for the one-past-end case internally and returns
/// silently, so callers need not guard that boundary themselves.  Reads and
/// writes numerous `static mut` renderer globals; calls [`R_CheckPlane`] and
/// `render_seg_loop`.
// FIXME: In R_StoreWallRange the Rust code computes `rw_scalestep` only when
//        `stop > start`, leaving `rw_scalestep` at its value from the
//        *previous* seg when `stop == start`.  The C source has the same
//        behaviour, so this is intentional, but it means `ds_p->scalestep`
//        may hold a stale value for single-column segs.
#[doc(alias = "R_StoreWallRange")]
#[export_name = "R_StoreWallRange"]
pub unsafe extern "C" fn store_wall_range(start: c_int, stop: c_int) {
    if ds_p == std::ptr::addr_of_mut!(drawsegs[0]).add(MAXDRAWSEGS) { return; }

    #[cfg(feature = "rangecheck")]
    {
        if start >= crate::doom::r_draw::viewwidth || start > stop {
            // Would call I_Error — skip in Rust for now
            return;
        }
    }

    sidedef = (*curline).sidedef;
    linedef = (*curline).linedef;

    (*linedef).flags |= LinedefFlag::MAPPED as c_short;

    rw_normalangle = (*curline).angle.wrapping_add(ANG90);
    let mut offsetangle = (rw_normalangle.wrapping_sub(rw_angle1) as i32).unsigned_abs();

    if offsetangle > ANG90 { offsetangle = ANG90; }

    let distangle = ANG90 - offsetangle;
    let hyp = R_PointToDist((*(*curline).v1).x, (*(*curline).v1).y);
    let sineval = tables::finesine[(distangle >> ANGLETOFINESHIFT) as usize];
    rw_distance = FixedMul(hyp, sineval);

    (*ds_p).x1 = start;
    rw_x = start;
    (*ds_p).x2 = stop;
    (*ds_p).curline = curline;
    rw_stopx = stop + 1;

    (*ds_p).scale1 = rw_scale;
    rw_scale = R_ScaleFromGlobalAngle(viewangle.wrapping_add(xtoviewangle[start as usize]));
    (*ds_p).scale1 = rw_scale;

    if stop > start {
        (*ds_p).scale2 =
            R_ScaleFromGlobalAngle(viewangle.wrapping_add(xtoviewangle[stop as usize]));
        (*ds_p).scalestep = rw_scalestep;
        rw_scalestep = ((*ds_p).scale2 - rw_scale) / (stop - start);
        (*ds_p).scalestep = rw_scalestep;
    }
    else { (*ds_p).scale2 = (*ds_p).scale1; }

    // F1 M1: all wall-span geometry reads the interpolation board's sampled
    // sector heights (live heights when the board is off/uncovered), so the
    // wall spans, silhouettes and clip windows stay mutually consistent per
    // frame and match the visplane heights sampled in R_Subsector. The back
    // sector is sampled only in the two-sided branch (null there is legal).
    let ffh = r_interp::sector_floor(frontsector as *mut crate::doom::c_ffi::sector_t);
    let fch = r_interp::sector_ceiling(frontsector as *mut crate::doom::c_ffi::sector_t);

    worldtop = fch - viewz;
    worldbottom = ffh - viewz;

    midtexture = 0;
    toptexture = 0;
    bottomtexture = 0;
    maskedtexture = 0;
    (*ds_p).maskedtexturecol = ptr::null_mut();

    if backsector.is_null() {
        // single sided line
        midtexture = *texturetranslation.add((*sidedef).midtexture as usize);
        markfloor = 1;
        markceiling = 1;

        if(*linedef).flags & (LinedefFlag::DONTPEGBOTTOM as c_short) != 0 {
            let vtop = ffh + *textureheight.add((*sidedef).midtexture as usize);
            rw_midtexturemid = vtop - viewz;
        }
        else { rw_midtexturemid = worldtop; }
        rw_midtexturemid += (*sidedef).rowoffset;

        (*ds_p).silhouette = SIL_BOTH;
        (*ds_p).sprtopclip = std::ptr::addr_of_mut!(screenheightarray[0]);
        (*ds_p).sprbottomclip = std::ptr::addr_of_mut!(negonearray[0]);
        (*ds_p).bsilheight = c_int::MAX;
        (*ds_p).tsilheight = c_int::MIN;
    } else {
        let bfh = r_interp::sector_floor(backsector as *mut crate::doom::c_ffi::sector_t);
        let bch = r_interp::sector_ceiling(backsector as *mut crate::doom::c_ffi::sector_t);

        // two sided line
        (*ds_p).sprtopclip = ptr::null_mut();
        (*ds_p).sprbottomclip = ptr::null_mut();
        (*ds_p).silhouette = 0;

        if ffh > bfh {
            (*ds_p).silhouette = SIL_BOTTOM;
            (*ds_p).bsilheight = ffh;
        } else if bfh > viewz {
            (*ds_p).silhouette = SIL_BOTTOM;
            (*ds_p).bsilheight = c_int::MAX;
        }

        if fch < bch {
            (*ds_p).silhouette |= SIL_TOP;
            (*ds_p).tsilheight = fch;
        } else if bch < viewz {
            (*ds_p).silhouette |= SIL_TOP;
            (*ds_p).tsilheight = c_int::MIN;
        }

        if bch <= ffh {
            (*ds_p).sprbottomclip = std::ptr::addr_of_mut!(negonearray[0]);
            (*ds_p).bsilheight = c_int::MAX;
            (*ds_p).silhouette |= SIL_BOTTOM;
        }

        if bfh >= fch {
            (*ds_p).sprtopclip = std::ptr::addr_of_mut!(screenheightarray[0]);
            (*ds_p).tsilheight = c_int::MIN;
            (*ds_p).silhouette |= SIL_TOP;
        }

        worldhigh = bch - viewz;
        worldlow = bfh - viewz;

        if(*frontsector).ceilingpic as c_int == skyflatnum
            && (*backsector).ceilingpic as c_int == skyflatnum
        { worldtop = worldhigh; }

        if worldlow != worldbottom
            || (*backsector).floorpic != (*frontsector).floorpic
            || (*backsector).lightlevel != (*frontsector).lightlevel
        { markfloor = 1; }
        else { markfloor = 0; }

        if worldhigh != worldtop
            || (*backsector).ceilingpic != (*frontsector).ceilingpic
            || (*backsector).lightlevel != (*frontsector).lightlevel
        { markceiling = 1; }
        else { markceiling = 0; }

        if bch <= ffh || bfh >= fch
        {
            markceiling = 1;
            markfloor = 1;
        }

        if worldhigh < worldtop {
            toptexture = *texturetranslation.add((*sidedef).toptexture as usize);
            if(*linedef).flags & (LinedefFlag::DONTPEGTOP as c_short) != 0 { rw_toptexturemid = worldtop; }
            else
            {
                let vtop = bch + *textureheight.add((*sidedef).toptexture as usize);
                rw_toptexturemid = vtop - viewz;
            }
        }

        if worldlow > worldbottom {
            bottomtexture = *texturetranslation.add((*sidedef).bottomtexture as usize);
            if(*linedef).flags & (LinedefFlag::DONTPEGBOTTOM as c_short) != 0 { rw_bottomtexturemid = worldtop; }
            else { rw_bottomtexturemid = worldlow; }
        }

        rw_toptexturemid += (*sidedef).rowoffset;
        rw_bottomtexturemid += (*sidedef).rowoffset;

        if (*sidedef).midtexture != 0 {
            maskedtexture = 1;
            (*ds_p).maskedtexturecol = lastopening.sub(rw_x as usize);
            maskedtexturecol = (*ds_p).maskedtexturecol;
            lastopening = lastopening.add((rw_stopx - rw_x) as usize);
        }
    }

    segtextured = midtexture | toptexture | bottomtexture | maskedtexture;

    if segtextured != 0 {
        let mut offsetangle = rw_normalangle.wrapping_sub(rw_angle1);
        if offsetangle > ANG180 { offsetangle = offsetangle.wrapping_neg(); }
        if offsetangle > ANG90 { offsetangle = ANG90; }

        let sineval = tables::finesine[(offsetangle >> ANGLETOFINESHIFT) as usize];
        rw_offset = FixedMul(hyp, sineval);

        if rw_normalangle.wrapping_sub(rw_angle1) < ANG180 { rw_offset = -rw_offset; }

        rw_offset += (*sidedef).textureoffset + (*curline).offset;
        rw_centerangle = ANG90.wrapping_add(viewangle).wrapping_sub(rw_normalangle);

        if fixedcolormap.is_null() {
            let mut lightnum =
                (((*frontsector).lightlevel as u32) >> LIGHTSEGSHIFT) as c_int + extralight;

            if(*(*curline).v1).y == (*(*curline).v2).y { lightnum -= 1; }
            else if(*(*curline).v1).x == (*(*curline).v2).x { lightnum += 1; }

            if lightnum < 0 { walllights = scalelight[0].as_mut_ptr(); }
            else if lightnum >= LIGHTLEVELS as c_int { walllights = scalelight[LIGHTLEVELS - 1].as_mut_ptr(); }
            else { walllights = scalelight[lightnum as usize].as_mut_ptr(); }
        }
    }

    if ffh >= viewz { markfloor = 0; }

    if fch <= viewz && (*frontsector).ceilingpic as c_int != skyflatnum { markceiling = 0; }

    worldtop >>= 4;
    worldbottom >>= 4;

    topstep = -FixedMul(rw_scalestep, worldtop);
    topfrac = (centeryfrac >> 4) - FixedMul(worldtop, rw_scale);

    bottomstep = -FixedMul(rw_scalestep, worldbottom);
    bottomfrac = (centeryfrac >> 4) - FixedMul(worldbottom, rw_scale);

    if !backsector.is_null() {
        worldhigh >>= 4;
        worldlow >>= 4;

        if worldhigh < worldtop {
            pixhigh = (centeryfrac >> 4) - FixedMul(worldhigh, rw_scale);
            pixhighstep = -FixedMul(rw_scalestep, worldhigh);
        }

        if worldlow > worldbottom {
            pixlow = (centeryfrac >> 4) - FixedMul(worldlow, rw_scale);
            pixlowstep = -FixedMul(rw_scalestep, worldlow);
        }
    }

    if markceiling != 0 { ceilingplane = R_CheckPlane(ceilingplane, rw_x, rw_stopx - 1); }
    if markfloor != 0 { floorplane = R_CheckPlane(floorplane, rw_x, rw_stopx - 1); }

    render_seg_loop();

    if (((*ds_p).silhouette & SIL_TOP) != 0 || maskedtexture != 0) && (*ds_p).sprtopclip.is_null() {
        for i in 0..(rw_stopx - start) { *lastopening.add(i as usize) = ceilingclip[(start + i) as usize]; }
        (*ds_p).sprtopclip = lastopening.sub(start as usize);
        lastopening = lastopening.add((rw_stopx - start) as usize);
    }

    if (((*ds_p).silhouette & SIL_BOTTOM) != 0 || maskedtexture != 0)
        && (*ds_p).sprbottomclip.is_null()
    {
        for i in 0..(rw_stopx - start) { *lastopening.add(i as usize) = floorclip[(start + i) as usize]; }
        (*ds_p).sprbottomclip = lastopening.sub(start as usize);
        lastopening = lastopening.add((rw_stopx - start) as usize);
    }

    if maskedtexture != 0 && ((*ds_p).silhouette & SIL_TOP) == 0 {
        (*ds_p).silhouette |= SIL_TOP;
        (*ds_p).tsilheight = c_int::MIN;
    }
    if maskedtexture != 0 && ((*ds_p).silhouette & SIL_BOTTOM) == 0 {
        (*ds_p).silhouette |= SIL_BOTTOM;
        (*ds_p).bsilheight = c_int::MAX;
    }

    ds_p = ds_p.add(1);
}
