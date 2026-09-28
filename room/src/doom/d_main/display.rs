//! The display pipeline: `display` (upstream `D_Display`) -- the
//! per-frame state-diff drawer with its blocking wipe loop -- and the
//! `grab_mouse_callback` seam.
//!
//! The wipe-loop body is golden-adjacent (the F9 video goldens hash
//! frames that pass through it): the `I_GetTime`/`I_Sleep`/
//! `refresh_fraction` ordering and the `board_active` gate moved
//! VERBATIM from the pre-split file -- never "fix" them here.

use std::ffi::c_int;

use crate::types::Boolean;
use crate::doom::am_map::{automapactive, AM_Drawer};
use crate::doom::d_loop::{gametic, NetUpdate};
use crate::doom::d_player::{players, MAXPLAYERS};
use crate::doom::f_finale::F_Drawer;
use crate::doom::f_wipe::{wipe_EndScreen, wipe_ScreenWipe, wipe_StartScreen};
use crate::doom::g_game::{
    demoplayback, gamestate, nodrawers, paused, testcontrols, testcontrols_mousespeed, viewactive,
    displayplayer,
};
use crate::doom::hu_stuff::HU_Drawer;
use crate::doom::i_timer::{I_GetTime, I_Sleep};
use crate::doom::i_video::{
    I_FinishUpdate, I_SetPalette, I_UpdateNoBlit, SCREENHEIGHT, SCREENWIDTH,
};
use crate::doom::m_menu::{inhelpscreens, menuactive, M_Drawer};
use crate::doom::r_draw::{
    scaledviewwidth, viewheight, viewwindowx, viewwindowy, R_DrawViewBorder, R_FillBackScreen,
};
use crate::doom::r_interp;
use crate::doom::r_main::{setsizeneeded, R_ExecuteSetViewSize, R_RenderPlayerView};
use crate::doom::st_stuff::ST_Drawer;
use crate::doom::v_video::{
    patch_t, V_DrawMouseSpeedBox, V_DrawPatchDirect,
};
use crate::doom::wi_stuff::WI_Drawer;
use crate::doom::w_wad::W_CacheLumpName;
use crate::doom::z_zone::PU_CACHE;

use super::compat::deh_string as DEH_String;
use super::consts::{byte, GS_DEMOSCREEN, GS_FINALE, GS_INTERMISSION, GS_LEVEL};
use super::sequencing::page_drawer as D_PageDrawer;
use super::state::{advancedemo, wipegamestate};

// D_Display static local state — these must persist across frames.
/// Cached `viewactive` value from the previous frame.
static mut D_DISP_VIEWACTIVE: c_int = 0;
/// Cached `menuactive` value from the previous frame.
static mut D_DISP_MENUACTIVE: c_int = 0;
/// Cached `inhelpscreens` value from the previous frame.
static mut D_DISP_INHELPSCREENS: c_int = 0;
/// Non-zero when the view fills the full screen (no status bar visible).
static mut D_DISP_FULLSCREEN: c_int = 0;
/// Game state from the previous frame; -1 forces border and palette redraws.
static mut D_DISP_OLD_GAMESTATE: c_int = -1;
/// Remaining frames for which the view border must be redrawn.
static mut D_DISP_BORDERDRAWCOUNT: c_int = 0;

extern "C" {
    // d_loop.rs — drone not yet exported as pub static:
    /// Whether this instance is a drone (spectator) player; from `d_loop.rs`.
    static mut drone: c_int;
}

/// Draw current display, possibly wiping it from the previous frame.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// wasm shell's diag re-present and the sprite regression tests import
/// the upstream name through the root shim.
#[doc(alias = "D_Display")]
#[export_name = "D_Display"]
pub extern "C" fn display() {
    unsafe {
        if nodrawers != 0 { return; }

        let mut redrawsbar = false;

        // Change the view size if needed
        if setsizeneeded.is_truthy() {
            R_ExecuteSetViewSize();
            D_DISP_OLD_GAMESTATE = -1; // force background redraw
            D_DISP_BORDERDRAWCOUNT = 3;
        }

        // Save the current screen if about to wipe
        let wipe = gamestate != wipegamestate;
        if wipe { wipe_StartScreen(0, 0, SCREENWIDTH, SCREENHEIGHT); }

        // Buffered drawing based on game state
        if gamestate == GS_LEVEL {
            if gametic == 0 {
                // Skip level rendering
            }
            else {
                if automapactive != 0 { AM_Drawer(); }
                if wipe || (viewheight != 200 && D_DISP_FULLSCREEN != 0) { redrawsbar = true; }
                if D_DISP_INHELPSCREENS != 0 && inhelpscreens == 0 { redrawsbar = true; }
                ST_Drawer(Boolean::from(viewheight == 200), Boolean::from(redrawsbar));
                D_DISP_FULLSCREEN = (viewheight == 200) as c_int;
            }
        }
        else if gamestate == GS_INTERMISSION { WI_Drawer(); }
        else if gamestate == GS_FINALE { F_Drawer(); }
        else if gamestate == GS_DEMOSCREEN { D_PageDrawer(); }

        // Draw buffered stuff to screen
        I_UpdateNoBlit();

        // Draw the view directly
        if gamestate == GS_LEVEL && automapactive == 0 && gametic != 0 {
            let dp = displayplayer as usize;
            if dp < MAXPLAYERS { R_RenderPlayerView(std::ptr::addr_of_mut!(players[dp])); }
            HU_Drawer();
        }

        // Set palette on state change (non-level states)
        if gamestate != GS_LEVEL && gamestate != D_DISP_OLD_GAMESTATE { I_SetPalette(W_CacheLumpName(DEH_String(c"PLAYPAL".as_ptr()), PU_CACHE) as *mut byte); }

        // See if the border needs to be initially drawn
        if gamestate == GS_LEVEL && D_DISP_OLD_GAMESTATE != GS_LEVEL {
            D_DISP_VIEWACTIVE = 0;
            R_FillBackScreen();
        }

        // See if the border needs to be updated
        if gamestate == GS_LEVEL && automapactive == 0 && scaledviewwidth != 320 {
            if menuactive != 0 || D_DISP_MENUACTIVE != 0 || D_DISP_VIEWACTIVE == 0 { D_DISP_BORDERDRAWCOUNT = 3; }
            if D_DISP_BORDERDRAWCOUNT != 0 {
                R_DrawViewBorder();
                D_DISP_BORDERDRAWCOUNT -= 1;
            }
        }

        // Test controls: show mouse speed box
        if testcontrols != 0 { V_DrawMouseSpeedBox(testcontrols_mousespeed); }

        // Update static state trackers
        D_DISP_MENUACTIVE = menuactive;
        D_DISP_VIEWACTIVE = viewactive;
        D_DISP_INHELPSCREENS = inhelpscreens;
        D_DISP_OLD_GAMESTATE = gamestate;
        wipegamestate = gamestate;

        // Draw pause pic
        if paused != 0 {
            let y = if automapactive != 0 { 4 } else { viewwindowy + 4 };
            V_DrawPatchDirect(
                viewwindowx + (scaledviewwidth - 68) / 2,
                y,
                W_CacheLumpName(DEH_String(c"M_PAUSE".as_ptr()), PU_CACHE) as *mut patch_t,
            );
        }

        // Menus go directly to the screen
        M_Drawer();
        NetUpdate();

        // Normal update
        if !wipe {
            I_FinishUpdate();
            return;
        }

        // Wipe update
        wipe_EndScreen(0, 0, SCREENWIDTH, SCREENHEIGHT);

        let mut wipestart = I_GetTime() - 1;
        loop {
            let mut nowtime;
            let mut tics;
            loop {
                nowtime = I_GetTime();
                tics = nowtime - wipestart;
                if tics <= 0 { I_Sleep(1); } else { break; }
            }

            wipestart = nowtime;
            // F1 M1: refresh the interpolation fraction once per wipe
            // iteration — the wipe presents many frames per simulated tic
            // (Woof! re-samples per wipe pass, `d_main.c:396-401`).
            // Fix round 1 (review Important 3/4): the poll itself is gated
            // on board activity, not just the samples — the legacy
            // `doomgeneric_Tick` path never arms the board, so it must not
            // pay the extra `I_GetTimeMS` polls either. That keeps the
            // tick/clock-poll pattern (and the demo golden baselines built
            // on it) exactly at the c7bda6c baseline, and the code now
            // matches the gate this comment always claimed.
            if r_interp::board_active() { r_interp::refresh_fraction(); }
            let done = wipe_ScreenWipe(
                1, // wipe_Melt
                0,
                0,
                SCREENWIDTH,
                SCREENHEIGHT,
                tics,
            ) != 0;

            I_UpdateNoBlit();
            M_Drawer();
            I_FinishUpdate();

            if done { break; }
        }
    }
}

/// Return `TRUE` when the window should grab (capture) the mouse pointer.
///
/// The mouse is released for drone players, while the menu is open, or while a demo is playing.
///
/// Address-taken only (`doom_loop`'s inner `grab_cb` forwards it to
/// `I_SetGrabMouseCallback`), never linker-resolved -- therefore no pin.
#[doc(alias = "D_GrabMouseCallback")]
pub(super) extern "C" fn grab_mouse_callback() -> Boolean {
    unsafe {
        // Drone players don't need mouse focus
        if drone != 0 { return Boolean::FALSE; }

        // When menu is active or game is paused, release the mouse
        if menuactive != 0 || paused != 0 { return Boolean::FALSE; }

        // Only grab mouse when playing levels (but not demos)
        let demoplayback_val: c_int = demoplayback;
        let advancedemo_val: c_int = advancedemo;
        Boolean::from(gamestate == GS_LEVEL && demoplayback_val == 0 && advancedemo_val == 0)
    }
}
