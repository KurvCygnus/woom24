//! The span rasterizer: `map_plane` (one horizontal flat span, with the
//! per-row step cache ladder), `make_spans` (the close/open transition
//! walk), and `draw_planes` (the end-of-frame visplane iteration with the
//! sky special case and the pad-sentinel boundary handling).

use std::ffi::c_int;

use crate::doom::i_video::SCREENWIDTH;
use crate::doom::m_fixed::FixedMul;
use crate::doom::r_data::{colormaps, firstflat, flattranslation, R_GetColumn};
use crate::doom::r_draw::{
    dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_x, dc_yh, dc_yl, ds_colormap, ds_source,
    ds_x1, ds_x2, ds_xfrac, ds_xstep, ds_y, ds_yfrac, ds_ystep,
};
use crate::doom::r_main::{
    colfunc, detailshift, extralight, fixedcolormap, spanfunc, viewangle, viewx, viewy, viewz,
    xtoviewangle, zlight,
};
use crate::doom::r_sky::{skytexture, skytexturemid};
use crate::doom::r_things::pspriteiscale;
use crate::doom::tables::{self, ANGLETOFINESHIFT, FINEMASK};
use crate::doom::w_wad::{W_CacheLumpNum, W_ReleaseLumpNum};
use crate::doom::z_zone::PU_STATIC;

use super::state::{
    basexscale, baseyscale, cacheddistance, cachedheight, cachedxstep, cachedystep, distscale,
    finecosine, lastvisplane, lighttable_t, planeheight, planezlight, spanstart, visplanes, yslope,
    ANGLETOSKYSHIFT, LIGHTLEVELS, LIGHTSEGSHIFT, LIGHTZSHIFT, MAXLIGHTZ,
};

/// Draws one horizontal span of a floor or ceiling flat.
///
/// Called by [`make_spans`] with the screen row `y` and the inclusive column
/// range `[x1, x2]`.  Sets up all `ds_*` globals required by `spanfunc`
/// and then calls it.
///
/// Uses globals: `planeheight`, `basexscale`, `baseyscale`, `viewx`, `viewy`,
/// `viewangle`, `xtoviewangle`, `distscale`, `yslope`, `planezlight`,
/// `fixedcolormap`, `spanfunc`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads and writes numerous `static mut` renderer globals.  All pointers
/// (`planezlight`, `spanfunc`, `fixedcolormap`) must be valid when called.
/// The range-check feature gate guards against out-of-bounds `x1`/`x2`/`y`
/// values; without it the caller is responsible for passing valid coordinates.
#[doc(alias = "R_MapPlane")]
#[export_name = "R_MapPlane"]
pub extern "C" fn map_plane(y: c_int, x1: c_int, x2: c_int) {
    unsafe {
        #[cfg(feature = "rangecheck")]
        {
            if x2 < x1
                || x1 < 0
                || x2 >= crate::doom::r_draw::viewwidth
                || y > crate::doom::r_draw::viewheight
            {
                // Would call I_Error — skip in Rust for now
                return;
            }
        }

        let y_usize = y as usize;

        let distance = if planeheight != cachedheight[y_usize] {
            cachedheight[y_usize] = planeheight;
            let d = FixedMul(planeheight, yslope[y_usize]);
            cacheddistance[y_usize] = d;
            cachedxstep[y_usize] = FixedMul(d, basexscale);
            cachedystep[y_usize] = FixedMul(d, baseyscale);
            d
        } else {
            cacheddistance[y_usize]
        };

        ds_xstep = cachedxstep[y_usize];
        ds_ystep = cachedystep[y_usize];

        let length = FixedMul(distance, distscale[x1 as usize]);
        let angle = viewangle.wrapping_add(xtoviewangle[x1 as usize]) >> ANGLETOFINESHIFT;
        ds_xfrac = viewx.wrapping_add(FixedMul(finecosine(angle as usize), length));
        ds_yfrac = (-viewy).wrapping_sub(FixedMul(
            tables::finesine[angle as usize & FINEMASK as usize],
            length,
        ));

        if !fixedcolormap.is_null() { ds_colormap = fixedcolormap; }
        else
        {
            let mut index = (distance >> LIGHTZSHIFT) as usize;
            if index >= MAXLIGHTZ { index = MAXLIGHTZ - 1; }
            ds_colormap = *planezlight.add(index) as *mut u8;
        }

        ds_y = y;
        ds_x1 = x1;
        ds_x2 = x2;

        if let Some(func) = spanfunc { func(); }
    }
}

/// Closes and opens horizontal spans as the per-column clip bounds change.
///
/// Called once per column `x` during [`draw_planes`].  `t1`/`b1` are the
/// top/bottom clip values for the previous column; `t2`/`b2` are those for
/// the current column.  Any row that was open in the previous column but
/// closed in the current one is flushed to [`map_plane`].  Any row that
/// opens in the current column but was closed in the previous one has its
/// start recorded in `spanstart`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads and writes the `static mut` globals [`spanstart`].  Calls
/// [`map_plane`], which itself writes additional globals.  All indices
/// derived from `t1`/`b1`/`t2`/`b2` must be valid screen rows
/// (`0 <= row < SCREENHEIGHT`); the caller (the `draw_planes` loop) is
/// responsible for keeping them in range.
#[doc(alias = "R_MakeSpans")]
#[export_name = "R_MakeSpans"]
pub extern "C" fn make_spans(x: c_int, t1: c_int, b1: c_int, t2: c_int, b2: c_int) {
    unsafe {
        let mut t1 = t1;
        let mut b1 = b1;
        let mut t2 = t2;
        let mut b2 = b2;

        while t1 < t2 && t1 <= b1 {
            map_plane(t1, spanstart[t1 as usize], x - 1);
            t1 += 1;
        }
        while b1 > b2 && b1 >= t1 {
            map_plane(b1, spanstart[b1 as usize], x - 1);
            b1 -= 1;
        }

        while t2 < t1 && t2 <= b2 {
            spanstart[t2 as usize] = x;
            t2 += 1;
        }
        while b2 > b1 && b2 >= t2 {
            spanstart[b2 as usize] = x;
            b2 -= 1;
        }
    }
}

/// Rasterizes all accumulated visplanes into horizontal pixel spans.
///
/// Called once per frame after BSP traversal is complete.  Iterates over every
/// visplane in the pool up to [`lastvisplane`].  Sky flats are drawn as
/// vertical columns via `colfunc`; regular flats are drawn as horizontal
/// spans via [`make_spans`] / [`map_plane`] / `spanfunc`.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Reads and writes numerous `static mut` renderer globals.  All function
/// pointers (`colfunc`, `spanfunc`) and data pointers (`fixedcolormap`,
/// `planezlight`, `zlight`) must be valid.  The `W_CacheLumpNum` /
/// `W_ReleaseLumpNum` calls must have access to a properly initialised WAD
/// lump directory.
#[doc(alias = "R_DrawPlanes")]
#[export_name = "R_DrawPlanes"]
pub extern "C" fn draw_planes() {
    unsafe {
        let mut pl = std::ptr::addr_of_mut!(visplanes[0]);
        let end = lastvisplane;

        while pl < end {
            if (*pl).minx > (*pl).maxx {
                pl = pl.add(1);
                continue;
            }

            // Sky flat
            if (*pl).picnum == crate::doom::r_sky::skyflatnum {
                dc_iscale = pspriteiscale >> detailshift;
                // Sky is always drawn full bright,
                // i.e. colormaps[0] is used.
                // Because of this hack, sky is not affected
                // by INVUL inverse mapping.
                dc_colormap = colormaps;
                dc_texturemid = skytexturemid;

                for x in (*pl).minx..=(*pl).maxx {
                    dc_yl = (*pl).top[x as usize] as c_int;
                    dc_yh = (*pl).bottom[x as usize] as c_int;

                    if dc_yl <= dc_yh {
                        let angle =
                            viewangle.wrapping_add(xtoviewangle[x as usize]) >> ANGLETOSKYSHIFT;
                        dc_x = x;
                        dc_source = R_GetColumn(skytexture, angle as c_int);
                        if let Some(func) = colfunc { func(); }
                    }
                }
                pl = pl.add(1);
                continue;
            }

            // Regular flat
            let flat_idx = *flattranslation.offset((*pl).picnum as isize);
            let lumpnum = firstflat + flat_idx;
            ds_source = W_CacheLumpNum(lumpnum, PU_STATIC) as *mut u8;

            planeheight = ((*pl).height.wrapping_sub(viewz)).abs();
            let mut light = ((*pl).lightlevel >> LIGHTSEGSHIFT) as c_int + extralight;

            if light >= LIGHTLEVELS as c_int { light = LIGHTLEVELS as c_int - 1; }
            if light < 0 { light = 0; }

            // Set planezlight — in the full implementation this would be
            // a pointer into zlight[light]
            let light_usize = light as usize;
            if light_usize < LIGHTLEVELS { planezlight = zlight[light_usize].as_ptr() as *const *const lighttable_t; }

            // Set sentinel values at the visplane boundaries. The C code does
            // pl->top[pl->maxx+1] = 0xff and pl->top[pl->minx-1] = 0xff, which
            // relies on padding bytes when at screen edges.
            if(*pl).minx > 0 { (*pl).top[((*pl).minx - 1) as usize] = 0xFF; }
            else { (*pl).pad1 = 0xFF; }
            if(*pl).maxx < SCREENWIDTH as c_int - 1 { (*pl).top[((*pl).maxx + 1) as usize] = 0xFF; }
            else { (*pl).pad2 = 0xFF; }

            let stop = (*pl).maxx + 1;
            for x in (*pl).minx..=stop {
                let prev_x = x - 1;
                let t_top = if prev_x < 0 { (*pl).pad1 } else { (*pl).top[prev_x as usize] };
                let t_bottom = if prev_x < 0 { (*pl).pad3 } else { (*pl).bottom[prev_x as usize] };
                let b_top = if x == SCREENWIDTH as c_int { (*pl).pad2 } else { (*pl).top[x as usize] };
                let b_bottom = if x == SCREENWIDTH as c_int { (*pl).pad4 } else { (*pl).bottom[x as usize] };
                make_spans(
                    x,
                    t_top as c_int,
                    t_bottom as c_int,
                    b_top as c_int,
                    b_bottom as c_int,
                );
            }

            W_ReleaseLumpNum(lumpnum);
            pl = pl.add(1);
        }
    }
}
