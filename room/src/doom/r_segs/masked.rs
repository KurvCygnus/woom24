//! The deferred mid-texture pass: `render_masked_seg_range`, called by
//! `r_things` during the sprite pass (after all opaque walls are drawn)
//! for each drawseg that reserved a masked-texture column slice.

use std::ffi::{c_int, c_short, c_void};

use crate::doom::c_ffi::LinedefFlag;
use crate::doom::m_fixed::FixedMul;
use crate::doom::r_bsp::{backsector, curline, drawseg_t, frontsector};
use crate::doom::r_data::{textureheight, texturetranslation, R_GetColumn};
use crate::doom::r_draw::{dc_colormap, dc_iscale, dc_texturemid, dc_x};
use crate::doom::r_interp;
use crate::doom::r_main::{centeryfrac, extralight, fixedcolormap, scalelight, viewz};
use crate::doom::r_things::R_DrawMaskedColumn;

use super::state::{
    LIGHTLEVELS, LIGHTSCALESHIFT, LIGHTSEGSHIFT, MAXLIGHTSCALE, maskedtexturecol, rw_scalestep,
    walllights,
};

/// Draws the transparent mid-texture for a two-sided seg.
///
/// Called during the sprite-rendering pass (after all opaque walls are drawn)
/// for each [`drawseg_t`] that has a masked mid-texture.  Iterates columns
/// `[x1, x2]`; for each column where `maskedtexturecol` is not `SHRT_MAX`,
/// fetches the correct texture column, sets up lighting, and calls
/// [`R_DrawMaskedColumn`].
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// `ds` must be a valid, non-null pointer to a [`drawseg_t`] that was filled
/// in by `R_StoreWallRange` during the current frame.  All pointer fields
/// within `ds` (`curline`, `maskedtexturecol`, `sprbottomclip`,
/// `sprtopclip`) must still be valid.  Reads and writes numerous
/// `static mut` renderer globals.
#[doc(alias = "R_RenderMaskedSegRange")]
#[export_name = "R_RenderMaskedSegRange"]
pub unsafe extern "C" fn render_masked_seg_range(ds: *mut drawseg_t, x1: c_int, x2: c_int) {
    let texnum: c_int = *texturetranslation.add((*(*(*ds).curline).sidedef).midtexture as usize);

    curline = (*ds).curline;
    frontsector = (*curline).frontsector;
    backsector = (*curline).backsector;

    let mut lightnum: c_int =
        (((*frontsector).lightlevel as u32) >> LIGHTSEGSHIFT) as c_int + extralight;

    if(*(*curline).v1).y == (*(*curline).v2).y { lightnum -= 1; }
    else if(*(*curline).v1).x == (*(*curline).v2).x { lightnum += 1; }

    if lightnum < 0 { walllights = scalelight[0].as_mut_ptr(); }
    else if lightnum >= LIGHTLEVELS as c_int { walllights = scalelight[LIGHTLEVELS - 1].as_mut_ptr(); }
    else { walllights = scalelight[lightnum as usize].as_mut_ptr(); }

    maskedtexturecol = (*ds).maskedtexturecol;
    rw_scalestep = (*ds).scalestep;
    crate::doom::r_things::spryscale = (*ds).scale1 + (x1 - (*ds).x1) * rw_scalestep;
    crate::doom::r_things::mfloorclip = (*ds).sprbottomclip;
    crate::doom::r_things::mceilingclip = (*ds).sprtopclip;

    if (*(*curline).linedef).flags & (LinedefFlag::DONTPEGBOTTOM as c_short) != 0 {
        // F1 M1: heights sampled through the interpolation board.
        dc_texturemid = {
            let ffh = r_interp::sector_floor(frontsector as *mut crate::doom::c_ffi::sector_t);
            let bfh = r_interp::sector_floor(backsector as *mut crate::doom::c_ffi::sector_t);
            if ffh > bfh { ffh } else { bfh }
        };
        dc_texturemid = dc_texturemid + *textureheight.add(texnum as usize) - viewz;
    } else {
        dc_texturemid = {
            let fch = r_interp::sector_ceiling(frontsector as *mut crate::doom::c_ffi::sector_t);
            let bch = r_interp::sector_ceiling(backsector as *mut crate::doom::c_ffi::sector_t);
            if fch < bch { fch } else { bch }
        };
        dc_texturemid -= viewz;
    }
    dc_texturemid += (*(*curline).sidedef).rowoffset;

    if !fixedcolormap.is_null() { dc_colormap = fixedcolormap; }

    dc_x = x1;
    while dc_x <= x2 {
        if *maskedtexturecol.add(dc_x as usize) != c_short::MAX {
            if fixedcolormap.is_null() {
                let mut index = (crate::doom::r_things::spryscale as u32) >> LIGHTSCALESHIFT;
                if index >= MAXLIGHTSCALE as u32 { index = (MAXLIGHTSCALE - 1) as u32; }
                dc_colormap = *walllights.add(index as usize);
            }

            crate::doom::r_things::sprtopscreen =
                centeryfrac - FixedMul(dc_texturemid, crate::doom::r_things::spryscale);
            dc_iscale = (0xffffffff_u32 / (crate::doom::r_things::spryscale as u32)) as c_int;

            let raw = R_GetColumn(texnum, *maskedtexturecol.add(dc_x as usize) as c_int);
            let col = raw.sub(3);
            R_DrawMaskedColumn(col as *mut c_void);

            *maskedtexturecol.add(dc_x as usize) = c_short::MAX;
        }
        crate::doom::r_things::spryscale += rw_scalestep;
        dc_x += 1;
    }
}
