//! View-size machinery: the deferred resize request, the LUT-rebuilding
//! executor, the texture-mapping table build, and the light-table
//! precomputation.

use std::ffi::{c_int, c_short};

use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH};
use crate::doom::m_fixed::{FixedDiv, FixedMul, FRACBITS, FRACUNIT};
use crate::doom::r_data::colormaps;
use crate::doom::r_draw::{
    scaledviewwidth, viewheight, viewwidth, R_DrawColumn, R_DrawColumnLow, R_DrawFuzzColumn,
    R_DrawFuzzColumnLow, R_DrawSpan, R_DrawSpanLow, R_DrawTranslatedColumn,
    R_DrawTranslatedColumnLow, R_InitBuffer,
};
use crate::doom::r_things::{pspriteiscale, pspritescale, screenheightarray};
use crate::doom::tables::{self, ANGLETOFINESHIFT};
use crate::doom::tables::ANG90;
use crate::types::Boolean;

use super::state::{
    basecolfunc, centerx, centerxfrac, centery, centeryfrac, clipangle, colfunc, detailshift,
    fuzzcolfunc, projection, scalelight, setblocks, setdetail, setsizeneeded, spanfunc,
    transcolfunc, xtoviewangle, viewangletox, zlight, DISTMAP, LIGHTLEVELS, LIGHTSCALESHIFT,
    LIGHTZSHIFT, MAXLIGHTSCALE, MAXLIGHTZ, NUMCOLORMAPS,
};

/// Number of fine-angle steps spanning the horizontal field of view (90 degrees
/// expressed in fine-angle units; `FINEANGLES / 4 = 2048`).
pub(super) const FIELDOFVIEW: c_int = 2048;

/// Schedule a viewport size change for the next frame.
///
/// Sets [`setsizeneeded`], [`setblocks`], and [`setdetail`] so that
/// [`execute_set_view_size`] will apply them at the start of the next rendered
/// frame. Safe to call mid-frame because the actual resize is deferred.
///
/// Equivalent to `R_SetViewSize` in `r_main.c`.
///
/// # Safety
/// Writes three renderer globals; safe to call from any context as long as
/// no other thread reads those globals concurrently (single-threaded engine).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `m_menu` and `video_cfg` reach the upstream name through the root shim.
#[doc(alias = "R_SetViewSize")]
#[export_name = "R_SetViewSize"]
pub unsafe extern "C" fn set_view_size(blocks: c_int, detail: c_int) {
    setsizeneeded = Boolean::TRUE;
    setblocks = blocks;
    setdetail = detail;
}

/// Apply a pending viewport size change.
///
/// Computes all viewport dimension globals ([`viewwidth`], [`viewheight`],
/// [`scaledviewwidth`], [`centerx`], [`centery`], [`centerxfrac`],
/// [`centeryfrac`], [`projection`], [`detailshift`]), selects the appropriate
/// column/span draw function pointers, rebuilds the texture-mapping tables,
/// and recomputes both the `scalelight` and `yslope`/`distscale` plane
/// tables.
///
/// Only called when [`setsizeneeded`] is `TRUE`; normally invoked once per
/// frame from the game loop before rendering begins.
///
/// Equivalent to `R_ExecuteSetViewSize` in `r_main.c`.
///
/// # Safety
/// Writes a large number of renderer globals and calls several initialisation
/// helpers. Must not be called while a frame render is in progress.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main`, `g_game/savegentry`, and the `video_raster` sweep reach the
/// upstream name through the root shim.
#[doc(alias = "R_ExecuteSetViewSize")]
#[export_name = "R_ExecuteSetViewSize"]
pub unsafe extern "C" fn execute_set_view_size() {
    setsizeneeded = Boolean::FALSE;

    if setblocks == 11 {
        scaledviewwidth = SCREENWIDTH;
        viewheight = SCREENHEIGHT;
    } else {
        scaledviewwidth = setblocks * 32;
        viewheight = (setblocks * 168 / 10) & !7;
    }

    detailshift = setdetail;
    viewwidth = scaledviewwidth >> detailshift;

    centery = viewheight / 2;
    centerx = viewwidth / 2;
    centerxfrac = centerx << FRACBITS;
    centeryfrac = centery << FRACBITS;
    projection = centerxfrac;

    if detailshift == 0 {
        colfunc = Some(R_DrawColumn);
        basecolfunc = Some(R_DrawColumn);
        fuzzcolfunc = Some(R_DrawFuzzColumn);
        transcolfunc = Some(R_DrawTranslatedColumn);
        spanfunc = Some(R_DrawSpan);
    } else {
        colfunc = Some(R_DrawColumnLow);
        basecolfunc = Some(R_DrawColumnLow);
        fuzzcolfunc = Some(R_DrawFuzzColumnLow);
        transcolfunc = Some(R_DrawTranslatedColumnLow);
        spanfunc = Some(R_DrawSpanLow);
    }

    R_InitBuffer(scaledviewwidth, viewheight);
    init_texture_mapping();

    // psprite scales
    pspritescale = FRACUNIT * viewwidth / SCREENWIDTH;
    pspriteiscale = FRACUNIT * SCREENWIDTH / viewwidth;

    // thing clipping
    for i in 0..viewwidth as usize {
        screenheightarray[i] = viewheight as c_short;
    }

    // planes
    for i in 0..viewheight as usize {
        let mut dy = ((i as c_int - viewheight / 2) << FRACBITS) + FRACUNIT / 2;
        dy = dy.wrapping_abs();
        crate::doom::r_plane::yslope[i] = FixedDiv((viewwidth << detailshift) / 2 * FRACUNIT, dy);
    }

    for i in 0..viewwidth as usize {
        let cosadj = (*tables::finecosine
            .0
            .add((xtoviewangle[i] >> ANGLETOFINESHIFT) as usize))
        .wrapping_abs();
        crate::doom::r_plane::distscale[i] = FixedDiv(FRACUNIT, cosadj);
    }

    // Calculate the light levels to use for each level / scale combination.
    for i in 0..LIGHTLEVELS {
        let startmap = (((LIGHTLEVELS - 1 - i) * 2) * NUMCOLORMAPS / LIGHTLEVELS) as c_int;
        for j in 0..MAXLIGHTSCALE {
            let mut level = startmap
                - (j as c_int * SCREENWIDTH)
                    / (viewwidth << detailshift)
                    / DISTMAP as c_int;

            if level < 0 {
                level = 0;
            }
            if level >= NUMCOLORMAPS as c_int {
                level = NUMCOLORMAPS as c_int - 1;
            }

            scalelight[i][j] = colormaps.add(level as usize * 256);
        }
    }
}

/// Build the [`viewangletox`] and [`xtoviewangle`] lookup tables for the
/// current viewport geometry, then set [`clipangle`].
///
/// The focal length is derived from [`centerxfrac`] and the `finetangent`
/// table so that `FIELDOFVIEW` fine-angle steps span exactly [`viewwidth`]
/// pixels. After building both tables the function removes the sentinel
/// `-1`/`viewwidth+1` values from [`viewangletox`] (fencepost cleanup).
///
/// Called by [`execute_set_view_size`] whenever the viewport is resized.
///
/// # Safety
/// Reads and writes numerous renderer globals ([`viewwidth`],
/// [`centerxfrac`], [`clipangle`], etc.). Must be called after [`viewwidth`]
/// and [`centerxfrac`] have been set by [`execute_set_view_size`].
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach the upstream name through the root shim.
#[doc(alias = "R_InitTextureMapping")]
#[export_name = "R_InitTextureMapping"]
pub unsafe extern "C" fn init_texture_mapping() {
    // Use tangent table to generate viewangletox:
    //  viewangletox will give the next greatest x after the view angle.
    //
    // Calc focallength so FIELDOFVIEW angles covers SCREENWIDTH.
    let focallength = FixedDiv(
        centerxfrac,
        tables::finetangent[tables::FINEANGLES / 4 + FIELDOFVIEW as usize / 2],
    );

    for i in 0..tables::FINEANGLES / 2 {
        let t: c_int;
        if tables::finetangent[i] > FRACUNIT * 2 {
            t = -1;
        } else if tables::finetangent[i] < -FRACUNIT * 2 {
            t = viewwidth + 1;
        } else {
            let mut tt = FixedMul(tables::finetangent[i], focallength);
            tt = (centerxfrac - tt + FRACUNIT - 1) >> FRACBITS;
            if tt < -1 {
                t = -1;
            } else if tt > viewwidth + 1 {
                t = viewwidth + 1;
            } else {
                t = tt;
            }
        }
        viewangletox[i] = t;
    }

    // Scan viewangletox[] to generate xtoviewangle[]:
    //  xtoviewangle will give the smallest view angle that maps to x.
    for x in 0..=viewwidth as usize {
        let mut i = 0usize;
        while viewangletox[i] > x as c_int {
            i += 1;
        }
        xtoviewangle[x] = ((i as u32) << ANGLETOFINESHIFT).wrapping_sub(ANG90);
    }

    // Take out the fencepost cases from viewangletox.
    for i in 0..tables::FINEANGLES / 2 {
        let mut t = FixedMul(tables::finetangent[i], focallength);
        t = centerx - t;

        if viewangletox[i] == -1 {
            viewangletox[i] = 0;
        } else if viewangletox[i] == viewwidth + 1 {
            viewangletox[i] = viewwidth;
        }
    }

    clipangle = xtoviewangle[0];
}

/// Precompute the Z-distance-based light table [`zlight`].
///
/// For each combination of sector light level and Z-distance bucket, computes
/// a pointer into the master `colormaps` array. The Z-distance buckets are
/// shifted by `LIGHTZSHIFT` before lookup. Only the distance-based
/// [`zlight`] table is built here; the scale-based [`scalelight`] table
/// depends on `viewwidth` and is rebuilt in [`execute_set_view_size`].
///
/// Equivalent to `R_InitLightTables` in `r_main.c`.
///
/// # Safety
/// Reads `colormaps` from `r_data`; that pointer must be non-null and point
/// to `NUMCOLORMAPS * 256` valid bytes. Called during startup by
/// [`crate::doom::r_main::frame::init`].
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// [`crate::doom::r_main::frame::init`] reaches it through the root shim.
#[doc(alias = "R_InitLightTables")]
#[export_name = "R_InitLightTables"]
pub unsafe extern "C" fn init_light_tables() {
    for i in 0..LIGHTLEVELS {
        let startmap = (((LIGHTLEVELS - 1 - i) * 2) * NUMCOLORMAPS / LIGHTLEVELS) as c_int;
        for j in 0..MAXLIGHTZ {
            let mut scale = FixedDiv(
                (SCREENWIDTH / 2) * FRACUNIT,
                ((j + 1) << LIGHTZSHIFT) as c_int,
            );
            scale >>= LIGHTSCALESHIFT;
            let mut level = startmap - scale / DISTMAP as c_int;

            if level < 0 {
                level = 0;
            }
            if level >= NUMCOLORMAPS as c_int {
                level = NUMCOLORMAPS as c_int - 1;
            }

            zlight[i][j] = colormaps.add(level as usize * 256);
        }
    }
}
