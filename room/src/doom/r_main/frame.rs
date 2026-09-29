//! The per-frame render sequence: boot-time init, view-frame preparation
//! (including the interpolation-board camera sample), and the top-level
//! render entry point.

use std::ptr;

use crate::doom::crt::c_printf;
use crate::doom::d_loop::NetUpdate;
use crate::doom::d_player::PlayerT;
use crate::doom::m_menu::{detailLevel, screenblocks};
use crate::doom::p_setup::numnodes;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_bsp::{R_ClearClipSegs, R_ClearDrawSegs, R_RenderBSPNode};
use crate::doom::r_data::{colormaps, R_InitData};
use crate::doom::r_draw::R_InitTranslationTables;
use crate::doom::r_plane::{R_ClearPlanes, R_DrawPlanes, R_InitPlanes};
use crate::doom::r_segs::walllights;
use crate::doom::r_things::{R_ClearSprites, R_DrawMasked};
use crate::doom::tables::{self, ANGLETOFINESHIFT};

use super::geometry::{init_point_to_angle, init_tables};
use super::state::{
    extralight, fixedcolormap, framecount, lighttable_t, scalelightfixed, sscount, validcount,
    viewangle, viewangleoffset, viewcos, viewplayer, viewsin, viewx, viewy, viewz, MAXLIGHTSCALE,
};
use super::viewsize::{init_light_tables, set_view_size};

/// One-time renderer initialisation called at engine startup.
///
/// Calls, in order: `R_InitData`, `R_InitPointToAngle`, `R_InitTables`,
/// `R_SetViewSize`, `R_InitPlanes`, `R_InitLightTables`, `R_InitSkyMap`,
/// `R_InitTranslationTables`. Prints a `.` to stdout after each step as a
/// startup progress indicator.
///
/// Equivalent to `R_Init` in `r_main.c`.
///
/// # Safety
/// Initialises global renderer state; must be called exactly once before any
/// frame is rendered. Calls multiple unsafe initialisers internally.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` reaches the upstream name through the root shim.
#[doc(alias = "R_Init")]
#[export_name = "R_Init"]
pub unsafe extern "C" fn init() {
    R_InitData();
    c_printf(c".".as_ptr());
    init_point_to_angle();
    c_printf(c".".as_ptr());
    init_tables();
    c_printf(c".".as_ptr());
    set_view_size(screenblocks, detailLevel);
    R_InitPlanes();
    c_printf(c".".as_ptr());
    init_light_tables();
    c_printf(c".".as_ptr());
    crate::doom::r_sky::R_InitSkyMap();
    R_InitTranslationTables();
    c_printf(c".".as_ptr());

    framecount = 0;
}

/// Prepare all per-frame view globals from the given player's current state.
///
/// Sets [`viewplayer`], [`viewx`], [`viewy`], [`viewz`], [`viewangle`],
/// [`viewsin`], [`viewcos`], [`extralight`], and [`fixedcolormap`]. Also
/// resets [`sscount`] and increments both [`framecount`] and [`validcount`].
///
/// When the player has a fixed colormap powerup, fills [`scalelightfixed`]
/// with the fixed colormap pointer and redirects [`walllights`] to it so
/// that wall-lighting lookup code requires no special-case handling.
///
/// Equivalent to `R_SetupFrame` in `r_main.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to a [`PlayerT`] whose `mo`
/// mobj pointer is also valid. Reads and writes numerous renderer globals.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// freeze-zone callers reach the upstream name through the root shim.
#[doc(alias = "R_SetupFrame")]
#[export_name = "R_SetupFrame"]
pub unsafe extern "C" fn setup_frame(player: *mut PlayerT) {
    viewplayer = player;
    let mo = (*player).mo as *mut mobj_t;
    // F1 M1: the camera reads the interpolation board's sampled quad when
    // uncapped rendering is active; the sample degrades to the live
    // simulation values under the guard set (first tic, paused, spawn,
    // teleport), so the vanilla path is unchanged when sampling is off.
    let cam = crate::doom::r_interp::sample_camera(
        player,
        mo as *mut crate::doom::c_ffi::mobj_t,
    );
    viewx = cam.x;
    viewy = cam.y;
    viewangle = cam.angle.wrapping_add(viewangleoffset as u32);
    extralight = (*player).extralight;

    viewz = cam.z;

    viewsin = tables::finesine[(viewangle >> ANGLETOFINESHIFT) as usize];
    viewcos = *tables::finecosine
        .0
        .add((viewangle >> ANGLETOFINESHIFT) as usize);

    sscount = 0;

    if (*player).fixedcolormap != 0 {
        fixedcolormap = colormaps
            .add((*player).fixedcolormap as usize * 256 * std::mem::size_of::<lighttable_t>());

        for i in 0..MAXLIGHTSCALE {
            scalelightfixed[i] = fixedcolormap;
        }
        walllights = std::ptr::addr_of_mut!(scalelightfixed[0]);
    } else {
        fixedcolormap = ptr::null_mut();
    }

    framecount += 1;
    validcount += 1;
}

/// Top-level per-frame render entry point.
///
/// Calls [`setup_frame`] to prepare view globals, clears all renderer
/// buffers (`R_ClearClipSegs`, `R_ClearDrawSegs`, `R_ClearPlanes`,
/// `R_ClearSprites`), traverses the BSP tree via `R_RenderBSPNode`, then
/// draws floors/ceilings (`R_DrawPlanes`) and masked objects
/// (`R_DrawMasked`). `NetUpdate` is called between phases to keep network
/// and demo state responsive on slow machines.
///
/// Equivalent to `R_RenderPlayerView` (`R_RenderView`) in `r_main.c`.
///
/// # Safety
/// `player` must be a valid, non-null pointer to a fully-initialised
/// [`PlayerT`]. All renderer globals and map data must have been loaded and
/// initialised prior to this call.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` reaches the upstream name through the root shim.
#[doc(alias = "R_RenderPlayerView")]
#[export_name = "R_RenderPlayerView"]
pub unsafe extern "C" fn render_player_view(player: *mut PlayerT) {
    setup_frame(player);

    R_ClearClipSegs();
    R_ClearDrawSegs();
    R_ClearPlanes();
    R_ClearSprites();

    NetUpdate();
    R_RenderBSPNode(numnodes - 1);
    NetUpdate();
    R_DrawPlanes();
    NetUpdate();
    R_DrawMasked();
    NetUpdate();
}
