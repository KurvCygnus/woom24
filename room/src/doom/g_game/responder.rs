//! The gameplay event responder (`responder`, upstream `G_Responder`)
//! plus the `deh_string` identity seam shared by the `actions` / `ticker`
//! / `savegentry` callers.
//!
//! `G_Responder`'s latches (key states, mouse/joystick axes, the
//! weapon-cycle direction) feed `build_ticcmd`, and the demo-window
//! menu-popup / spy-mode arms sequence the demo session, so the function
//! is demo-synchronization surface whole-body (F10 wave C3
//! adjudication). The body moved verbatim from pre-split `g_game.rs`;
//! see the module root for the mapping table.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int, c_uint};

use crate::doom::am_map::AM_Responder;
use crate::doom::d_event::event_t;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::f_finale::F_Responder;
use crate::doom::hu_stuff::HU_Responder;
use crate::doom::m_controls::{key_nextweapon, key_pause, key_prevweapon, key_spy};
use crate::doom::m_menu::{mouseSensitivity, M_StartControlPanel};
use crate::doom::st_stuff::ST_Responder;

use super::consts::{boolean, ga_nothing, GS_DEMOSCREEN, GS_FINALE, GS_LEVEL, NUMKEYS};
use super::state::{
    consoleplayer, deathmatch, demoplayback, displayplayer, gameaction, gamestate, playeringame,
    sendpause, singledemo, testcontrols, testcontrols_mousespeed,
};
use super::ticcmd::{
    set_joy_buttons, set_mouse_buttons, GAMEKEYDOWN, JOYXMOVE, JOYYMOVE, JOYSTRAFEMOVE, MOUSEX,
    MOUSEY, NEXT_WEAPON,
};

// ---------------------------------------------------------------------------
// DEH_String identity (no dehacked support)
// ---------------------------------------------------------------------------

/// Stand-in for the Dehacked string-substitution macro from `deh_str.h`.
///
/// This port does not implement Dehacked patches, so the lookup is the
/// identity function. Kept as a wrapper to make the original C call sites
/// translate cleanly and to leave a single seam where Dehacked support could
/// later be added. Shared by the sky-texture names (`actions`), the
/// screen-shot / game-saved messages (`ticker` / `savegentry`).
///
/// # Safety
/// Caller must ensure `s` is a valid NUL-terminated C string for the
/// lifetime of the returned pointer.
#[doc(alias = "DEH_String")]
#[inline]
pub(super) unsafe fn deh_string(s: *const c_char) -> *const c_char { s }

// ---------------------------------------------------------------------------
// G_Responder
// ---------------------------------------------------------------------------

/// Handle one input event during gameplay or the demo loop.
///
/// Returns non-zero ("event consumed") when the event was handled here and
/// should not propagate further; zero lets later responders (menu, console)
/// see it. Mirrors `G_Responder` in `g_game.c`:
///
/// * Spy-mode (`key_spy`) cycles `displayplayer` even during demo playback.
/// * During the demo loop / playback, any key, mouse-click or joystick
///   button press pops the main menu.
/// * In `GS_LEVEL`, defers to HU / ST / AM responders in order.
/// * In `GS_FINALE`, defers to `F_Responder`.
/// * Otherwise routes by `event_t::type_`: keydown updates `GAMEKEYDOWN`
///   (with `key_pause` setting `sendpause`), keyup clears it, mouse and
///   joystick events latch axes and buttons.
///
/// # Safety
/// Dereferences `ev` and mutates many static input globals; safe under the
/// single-threaded engine convention. The C symbol is pinned
/// (`G_Responder`) so the wasm export set stays byte-identical.
#[doc(alias = "G_Responder")]
#[export_name = "G_Responder"]
pub unsafe extern "C" fn responder(ev: *mut event_t) -> boolean
{
    let ev = &*ev;

    // Spy mode changes even during demo
    if gamestate == GS_LEVEL &&
        ev.type_ == 0 && // ev_keydown
        ev.data1 == key_spy &&
        (singledemo != 0 || deathmatch == 0)
        {
            loop
            {
                displayplayer += 1;
                if displayplayer == MAXPLAYERS as c_int
                {
                    displayplayer = 0;
                }
                if playeringame[displayplayer as usize] != 0 || displayplayer == consoleplayer
                {
                    break;
                }
            }
            return 1;
        }

    // Any key pops up menu if in demos
    if gameaction == ga_nothing &&
        singledemo == 0 &&
        (demoplayback != 0 || gamestate == GS_DEMOSCREEN)
        {
            if ev.type_ == 0 || // ev_keydown
                (ev.type_ == 2 && ev.data1 != 0) || // ev_mouse with buttons
                (ev.type_ == 3 && ev.data1 != 0)    // ev_joystick with buttons
                {
                    M_StartControlPanel();
                    return 1;
                }
            return 0;
        }

    if gamestate == GS_LEVEL
    {
        if HU_Responder(ev as *const event_t as *mut event_t) != 0 { return 1; }
        if ST_Responder(ev as *const event_t as *mut event_t) != 0 { return 1; }
        if AM_Responder(ev as *const event_t as *mut event_t) != 0 { return 1; }
    }

    if gamestate == GS_FINALE && F_Responder(ev as *const event_t as *mut event_t) != 0 { return 1; }

    if testcontrols != 0 && ev.type_ == 2
    {
        // ev_mouse
        testcontrols_mousespeed = ev.data2.abs();
    }

    // Prev/next weapon keys
    if ev.type_ == 0 && ev.data1 == key_prevweapon { NEXT_WEAPON = -1; }
    else if ev.type_ == 0 && ev.data1 == key_nextweapon { NEXT_WEAPON = 1; }

    match ev.type_
    {
        0 =>
        {
            // ev_keydown
            if ev.data1 == key_pause { sendpause = 1; }
            else if (ev.data1 as usize) < NUMKEYS { GAMEKEYDOWN[ev.data1 as usize] = 1; }
            return 1;
        }
        1 =>
        {
            // ev_keyup
            if (ev.data1 as usize) < NUMKEYS { GAMEKEYDOWN[ev.data1 as usize] = 0; }
            return 0;
        }
        2 =>
        {
            // ev_mouse
            set_mouse_buttons(ev.data1 as c_uint);
            MOUSEX = ev.data2 * (mouseSensitivity + 5) / 10;
            MOUSEY = ev.data3 * (mouseSensitivity + 5) / 10;
            return 1;
        }
        3 =>
        {
            // ev_joystick
            set_joy_buttons(ev.data1 as c_uint);
            JOYXMOVE = ev.data2;
            JOYYMOVE = ev.data3;
            JOYSTRAFEMOVE = ev.data4;
            return 1;
        }
        _ => {}
    }
    0
}
