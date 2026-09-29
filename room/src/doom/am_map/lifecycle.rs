//! The automap lifecycle: variable init, WAD pic load/unload, mark
//! management, and the public start/stop/ticker entry points --
//! including the load-bearing misordered AM_MSGEXITED event (carried
//! VERBATIM; see the module Deterministic Aspects).

use std::ffi::{c_char, c_int};

use super::consts::{AM_MSGENTERED, AM_MSGEXITED};
use super::state::{
    amclock, automapactive, f_h, f_oldloc, f_w, finit_height, finit_width, fb, followplayer,
    lightlev, marknums, markpointnum, markpoints, m_h, m_paninc, m_w, m_x, m_y, old_m_h, old_m_w,
    old_m_x, old_m_y, plr, stopped,
};
use super::view::{change_window_loc, change_window_scale, do_follow_player, ftom, level_init};
use crate::c_write;
use crate::doom::c_ffi::mobj_t;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::d_event::event_t;
use crate::doom::g_game::{consoleplayer, gameepisode, gamemap, playeringame, players};
use crate::doom::i_video::{SCREENHEIGHT, SCREENWIDTH, I_VideoBuffer};
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::st_stuff::ST_Responder;
use crate::doom::v_video::patch_t;
use crate::doom::w_wad::{W_CacheLumpName, W_ReleaseLumpName};
use crate::doom::z_zone::PU_STATIC;

/// Place a mark at the current viewport centre, advancing the mark slot index.
///
/// Marks wrap around after [`super::consts::AM_NUMMARKPOINTS`] entries.
/// C origin: `AM_addMark` in am_map.c.
///
/// # Safety
///
/// Reads and writes multiple mutable statics; must only be called with the
/// automap active.
#[doc(alias = "AM_addMark")]
pub(super) unsafe fn add_mark() {
    markpoints[markpointnum as usize].x = m_x + m_w / 2;
    markpoints[markpointnum as usize].y = m_y + m_h / 2;
    markpointnum = (markpointnum + 1) % super::consts::AM_NUMMARKPOINTS as c_int;
}

/// Initialise all automap variables for a new session.
///
/// Sets `automapactive = 1`, assigns the framebuffer pointer, resets clocks
/// and pan/zoom multipliers, selects the tracked player, centres the viewport
/// on that player, and sends `AM_MSGENTERED` to the status bar.
///
/// C origin: `AM_initVariables` in am_map.c.
///
/// # Safety
///
/// Reads and writes multiple mutable statics; `players` and `playeringame`
/// must be valid.
#[doc(alias = "AM_initVariables")]
pub(super) unsafe fn init_variables() {
    automapactive = 1;
    fb = I_VideoBuffer;

    // F1 M2: the automap window tracks the live raster (finit_height keeps
    // the 32-row status-bar reservation). Crispy moved the same pair out of
    // static initializers when SCREENWIDTH became runtime.
    finit_width = SCREENWIDTH;
    finit_height = SCREENHEIGHT - 32;

    f_oldloc.x = c_int::MAX;
    amclock = 0;
    lightlev = 0;

    m_paninc.x = 0;
    m_paninc.y = 0;
    super::state::ftom_zoommul = FRACUNIT;
    super::state::mtof_zoommul = FRACUNIT;

    m_w = ftom(f_w);
    m_h = ftom(f_h);

    if playeringame[consoleplayer as usize] != 0 {
        plr = std::ptr::addr_of_mut!(players[0]).add(consoleplayer as usize);
    } else {
        plr = std::ptr::addr_of_mut!(players[0]);
        for pnum in 0..MAXPLAYERS {
            if playeringame[pnum] != 0 {
                plr = std::ptr::addr_of_mut!(players[0]).add(pnum);
                break;
            }
        }
    }

    m_x = (*((*plr).mo as *mut mobj_t)).x - m_w / 2;
    m_y = (*((*plr).mo as *mut mobj_t)).y - m_h / 2;
    change_window_loc();

    old_m_x = m_x;
    old_m_y = m_y;
    old_m_w = m_w;
    old_m_h = m_h;

    // ! Load-bearing vanilla fidelity: this event is WELL-FORMED for the
    // ! status bar (type_ = 1 = ev_keyup, data1 = AM_MSGENTERED), so
    // ! st_stuff's `ev.type_ == 1` branch DOES take it -- the exact
    // ! opposite of `stop`'s misordered AM_MSGEXITED event below. Both
    // ! halves move VERBATIM (vendor am_map.c:541-549); never "fix"
    // ! either side.
    let st_notify = event_t {
        type_: 1, // ev_keyup
        data1: AM_MSGENTERED,
        data2: 0,
        data3: 0,
        data4: 0,
    };
    ST_Responder(&st_notify as *const _ as *mut event_t);
}

/// Load the ten `AMMNUM0`-`AMMNUM9` mark-point glyph patches from the WAD.
///
/// Cached as `PU_STATIC` so they remain resident while the automap is open.
/// C origin: `AM_loadPics` in am_map.c.
///
/// # Safety
///
/// `W_CacheLumpName` must succeed; WAD must be loaded.
#[doc(alias = "AM_loadPics")]
pub(super) unsafe fn load_pics() {
    let mut namebuf: [c_char; 9] = [0; 9];
    for i in 0..10i32 {
        c_write!(namebuf, "AMMNUM{}", i);
        marknums[i as usize] = W_CacheLumpName(namebuf.as_mut_ptr(), PU_STATIC) as *mut patch_t;
    }
}

/// Release the ten `AMMNUM*` mark-point glyph patches back to the WAD cache.
///
/// Called by [`stop`] when the automap closes.
/// C origin: `AM_unloadPics` in am_map.c.
///
/// # Safety
///
/// WAD must be loaded; patch lumps must have been loaded by [`load_pics`].
#[doc(alias = "AM_unloadPics")]
pub(super) unsafe fn unload_pics() {
    let mut namebuf: [c_char; 9] = [0; 9];
    for i in 0..10i32 {
        c_write!(namebuf, "AMMNUM{}", i);
        W_ReleaseLumpName(namebuf.as_mut_ptr());
    }
}

/// Reset all mark points: set each `x` field to `-1` and reset the slot index.
///
/// C origin: `AM_clearMarks` in am_map.c.
///
/// # Safety
///
/// Writes mutable statics; safe as long as the automap is active.
#[doc(alias = "AM_clearMarks")]
pub(super) unsafe fn clear_marks() {
    for i in 0..super::consts::AM_NUMMARKPOINTS {
        markpoints[i].x = -1;
    }
    markpointnum = 0;
}

/// Deactivate the automap, release patch resources, and notify the status bar.
///
/// Sets `automapactive = 0`, sends `AM_MSGEXITED` to `ST_Responder`, and
/// unloads the mark-point patches.  Sets `stopped = 1` so that a subsequent
/// [`start`] will not call `stop` again.
///
/// Called by C code in `g_game.c` and `am_map.c`.
/// C origin: `AM_Stop` in am_map.c.
///
/// # Safety
///
/// Must be called only when the automap was previously started; WAD must be
/// loaded.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/actions.rs`, `p_inter/damage.rs` (mid-simulation
/// telefrag-class path), and the differential oracle
/// (`c2rust-intermediate/src/p_inter.rs:24`) reach the upstream name
/// through the root shim. The st_notify event below is the CATALOGED
/// vanilla-fidelity quirk (vendor am_map.c:541-549) -- see the `// !`
/// note at the construction site.
#[doc(alias = "AM_Stop")]
#[export_name = "AM_Stop"]
pub unsafe extern "C" fn stop() {
    // ! CATALOGED vanilla fidelity (vendor am_map.c:543): the event is
    // ! MISORDERED -- type_ = 0 (ev_keydown!) with ev_keyup landing in
    // ! data1 and AM_MSGEXITED in data2 -- so st_stuff's
    // ! `ev.type_ == 1` branch NEVER takes it and the event falls into
    // ! the keydown/cheat path instead. init_variables' well-formed
    // ! AM_MSGENTERED (type_ = 1) DOES match. Both halves move VERBATIM;
    // ! any "cleanup" of either side changes the st_gamestate transition
    // ! behavior.
    let st_notify = event_t {
        type_: 0, // ev_keydown
        data1: 1, // ev_keyup
        data2: AM_MSGEXITED,
        data3: 0,
        data4: 0,
    };
    unload_pics();
    automapactive = 0;
    ST_Responder(&st_notify as *const _ as *mut event_t);
    stopped = 1;
}

/// Activate the automap.
///
/// If the automap is already open (`stopped == 0`), closes it first.
/// Re-runs `level_init` whenever the level or episode changes, then calls
/// `init_variables` and `load_pics`.
///
/// Called by C code in `g_game.c` and by [`super::responder::responder`]
/// when the toggle key is pressed.  C origin: `AM_Start` in am_map.c.
///
/// # Safety
///
/// The game must be in an active level with valid map data loaded.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/actions.rs` and `responder` reach the upstream name through
/// the root shim.
#[doc(alias = "AM_Start")]
#[export_name = "AM_Start"]
pub unsafe extern "C" fn start() {
    static mut lastlevel: c_int = -1;
    static mut lastepisode: c_int = -1;

    if stopped == 0 {
        stop();
    }
    stopped = 0;
    if lastlevel != gamemap || lastepisode != gameepisode {
        level_init();
        lastlevel = gamemap;
        lastepisode = gameepisode;
    }
    init_variables();
    load_pics();
}

/// Advance the automap state by one game tick.
///
/// Returns immediately if `automapactive == 0`.  Otherwise: increments
/// `amclock`, updates the follow-player position, applies zoom, and pans the
/// viewport.
///
/// Called from C code in `g_game.c` once per game tick.
/// C origin: `AM_Ticker` in am_map.c.
///
/// # Safety
///
/// Reads and writes mutable statics; `plr` must be valid while the automap is
/// active.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` reaches the upstream name through the root shim.
#[doc(alias = "AM_Ticker")]
#[export_name = "AM_Ticker"]
pub unsafe extern "C" fn ticker() {
    if automapactive == 0 {
        return;
    }

    amclock += 1;

    if followplayer != 0 {
        do_follow_player();
    }

    if super::state::ftom_zoommul != FRACUNIT {
        change_window_scale();
    }

    if m_paninc.x != 0 || m_paninc.y != 0 {
        change_window_loc();
    }

    // AM_updateLightLev();
}
