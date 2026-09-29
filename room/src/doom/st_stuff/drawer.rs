//! The per-frame drawing side: background refresh, the st_lib widget
//! fan-out, full/differential redraw, the `ST_Drawer` entry point,
//! and the house-native `force_full_redraw` re-arm latch.

use std::ffi::c_int;

use super::consts::{ST_FX, ST_HEIGHT, ST_WIDTH, ST_X, ST_Y};
use super::{
    faceback, sbar, st_armson, st_firsttime, st_fragson, st_statusbaron, w_ammo, w_arms,
    w_armsbg, w_armor, w_faces, w_frags, w_health, w_keyboxes, w_maxammo, w_ready,
};
use crate::doom::am_map::automapactive;
use crate::doom::d_player::NUMAMMO;
use crate::doom::g_game::deathmatch;
use crate::doom::st_lib::{
    STlib_updateBinIcon, STlib_updateMultIcon, STlib_updateNum, STlib_updatePercent,
};
use crate::doom::v_video::{V_CopyRect, V_DrawPatch, V_RestoreBuffer, V_UseBuffer};
use crate::types::Boolean;

/// Blit the status bar background into the backing buffer, then copy it to screen.
///
/// Only runs when `st_statusbaron` is set. In a network game the face-background
/// patch (`STFB#`) is drawn on top of the bar before the copy.
///
/// # Safety
///
/// Reads the global status-bar visibility flag `st_statusbaron` and the cached
/// patches `sbar`/`faceback`, and writes to the backing screen buffer
/// `st_backing_screen`. Caller must ensure `lifecycle::init` has loaded the
/// patches and allocated the buffer, and that the video subsystem is ready.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `full_redraw` reaches the upstream name directly.
#[doc(alias = "ST_refreshBackground")]
#[export_name = "ST_refreshBackground"]
pub unsafe extern "C" fn refresh_background() {
    if st_statusbaron != 0 {
        V_UseBuffer(super::st_backing_screen);
        V_DrawPatch(ST_X, 0, sbar);
        if crate::doom::g_game::netgame != 0 {
            V_DrawPatch(ST_FX, 0, faceback);
        }
        V_RestoreBuffer();
        V_CopyRect(ST_X, 0, super::st_backing_screen, ST_WIDTH, ST_HEIGHT, ST_X, ST_Y);
    }
}

/// Drive all st_lib widget update calls for a single frame.
///
/// `refresh` is passed through to each widget: non-zero forces a full redraw,
/// zero redraws only widgets whose value changed since last frame.
/// Updates `st_armson` and `st_fragson` visibility flags before iterating.
///
/// # Safety
///
/// Mutates `st_armson`/`st_fragson` and reads every `w_*` widget global,
/// passing them to the `STlib_update*` family. Caller must ensure
/// `lifecycle::create_widgets` has been called so the widget pointers refer
/// to live player/state values.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `full_redraw`/`diff_redraw` reach the upstream name directly.
#[doc(alias = "ST_drawWidgets")]
#[export_name = "ST_drawWidgets"]
pub unsafe extern "C" fn draw_widgets(refresh: c_int) {
    st_armson = if st_statusbaron != 0 && deathmatch == 0 {
        1
    } else {
        0
    };
    st_fragson = if deathmatch != 0 && st_statusbaron != 0 {
        1
    } else {
        0
    };

    STlib_updateNum(&raw mut w_ready, refresh);

    for i in 0..NUMAMMO {
        STlib_updateNum(std::ptr::addr_of_mut!(w_ammo[0]).add(i), refresh);
        STlib_updateNum(std::ptr::addr_of_mut!(w_maxammo[0]).add(i), refresh);
    }

    STlib_updatePercent(&raw mut w_health, refresh);
    STlib_updatePercent(&raw mut w_armor, refresh);
    STlib_updateBinIcon(&raw mut w_armsbg, refresh);

    for i in 0..6 {
        STlib_updateMultIcon(std::ptr::addr_of_mut!(w_arms[0]).add(i), refresh);
    }

    STlib_updateMultIcon(&raw mut w_faces, refresh);

    for i in 0..3 {
        STlib_updateMultIcon(std::ptr::addr_of_mut!(w_keyboxes[0]).add(i), refresh);
    }

    STlib_updateNum(&raw mut w_frags, refresh);
}

/// Perform a full status-bar redraw: clear `st_firsttime`, refresh the background, then redraw all widgets.
///
/// # Safety
///
/// Mutates `st_firsttime` and delegates to `refresh_background` and
/// `draw_widgets`, which require an initialised status bar and live
/// `plyr` pointer.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `drawer` reaches the upstream name directly.
#[doc(alias = "ST_doRefresh")]
#[export_name = "ST_doRefresh"]
pub unsafe extern "C" fn full_redraw() {
    st_firsttime = 0;
    refresh_background();
    draw_widgets(1);
}

/// Perform a differential redraw: only redraw widgets whose value changed.
///
/// # Safety
///
/// Delegates to `draw_widgets`; same invariants apply (widgets must have
/// been created and `plyr` must be valid).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `drawer` reaches the upstream name directly.
#[doc(alias = "ST_diffDraw")]
#[export_name = "ST_diffDraw"]
pub unsafe extern "C" fn diff_redraw() {
    draw_widgets(0);
}

/// Force the next `drawer` call to perform a full status-bar redraw.
///
/// The copied-in status-bar background lives in the primary framebuffer, and
/// the `st_firsttime` one-shot latch fires exactly once; a runtime
/// framebuffer swap (`video_cfg::apply`) therefore leaves the bar rendering
/// widgets over uninitialized pixels until the next level start. A caller
/// that re-creates the framebuffer re-arms the latch so the next `drawer`
/// re-copies the background from `st_backing_screen`, which survives the
/// swap (raster-independent `ST_WIDTH x ST_HEIGHT` allocation).
///
/// House-native (no C origin): the latch semantics are documented
/// cross-module with `video_cfg.rs:305-312`; do not "modernize" the
/// one-shot latch.
pub fn force_full_redraw() {
    unsafe {
        st_firsttime = 1;
    }
}

/// Draw the status bar for the current frame.
///
/// `fullscreen` indicates that the view fills the entire screen (no bar),
/// `refresh` forces a complete redraw regardless of dirty state. When
/// `fullscreen` is false or the automap is active, the bar is shown; otherwise
/// it is hidden.
///
/// Called once per frame by the main render loop (`D_Display` in `d_main.c`).
///
/// # Safety
///
/// Mutates `st_statusbaron` and `st_firsttime` and dispatches to
/// `super::palette::apply_palette`, `full_redraw`, and `diff_redraw`. Caller
/// must ensure the status-bar subsystem has been started (`lifecycle::start`)
/// and the video backend is ready.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs:99` imports the upstream name through the root
/// shim and the differential oracle declares the symbol directly.
#[doc(alias = "ST_Drawer")]
#[export_name = "ST_Drawer"]
pub unsafe extern "C" fn drawer(fullscreen: Boolean, refresh: Boolean) {
    st_statusbaron = if fullscreen.is_false() || automapactive != 0 {
        1
    } else {
        0
    };
    st_firsttime = if st_firsttime != 0 || refresh.is_truthy() {
        1
    } else {
        0
    };
    super::palette::apply_palette();
    if st_firsttime != 0 {
        full_redraw();
    } else {
        diff_redraw();
    }
}
