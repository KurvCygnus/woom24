//! The per-tic status-bar advance: `ticker` (the RNG-ledger dtmc
//! surface -- exactly one `M_Random` draw per tic) and the widget
//! data refresh it drives.

use std::ffi::c_int;

use super::plyr;
use super::{
    keyboxes, st_armson, st_clock, st_fragson, st_fragscount, st_msgcounter, st_chat,
    st_notdeathmatch, st_oldchat, st_oldhealth, st_randomnumber, st_statusbaron, w_ready,
};
use crate::doom::d_items::weaponinfo;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::g_game::{consoleplayer, deathmatch};
use crate::doom::m_random::M_Random;

/// Refresh all status-bar widget data pointers and counters from the player state.
///
/// Called once per tic by `ticker`. Updates the ready-ammo pointer (using a
/// sentinel value of 1994 for weapons with no ammo type), key-slot indices,
/// visibility flags for the arms panel and frag counter, the frag total, and
/// the face widget via `super::face::update_face_widget`. Also decrements
/// `st_msgcounter` and restores `st_chat` when it expires.
///
/// # Safety
///
/// Dereferences the global `plyr` pointer and mutates the widget globals
/// (`w_ready`, `keyboxes`, `st_notdeathmatch`, `st_armson`, `st_fragson`,
/// `st_fragscount`, `st_chat`, `st_msgcounter`). Caller must ensure
/// `lifecycle::create_widgets` has run and that `plyr` is valid.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `ticker` reaches the upstream name directly.
#[doc(alias = "ST_updateWidgets")]
#[export_name = "ST_updateWidgets"]
pub unsafe extern "C" fn update_widgets() {
    static mut largeammo: c_int = 1994;

    if weaponinfo[(*plyr).readyweapon as usize].ammo == 5 {
        // am_noammo
        w_ready.num = &raw mut largeammo as *mut c_int;
    } else {
        w_ready.num = (*plyr)
            .ammo
            .as_mut_ptr()
            .add(weaponinfo[(*plyr).readyweapon as usize].ammo as usize);
    }
    w_ready.data = (*plyr).readyweapon;

    for i in 0..3 {
        keyboxes[i] = if (*plyr).cards[i] != 0 {
            i as c_int
        } else {
            -1
        };
        if (*plyr).cards[i + 3] != 0 {
            keyboxes[i] = (i + 3) as c_int;
        }
    }

    super::face::update_face_widget();

    st_notdeathmatch = if deathmatch == 0 { 1 } else { 0 };
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
    st_fragscount = 0;

    for i in 0..MAXPLAYERS {
        if i != consoleplayer as usize {
            st_fragscount += (*plyr).frags[i];
        } else {
            st_fragscount -= (*plyr).frags[i];
        }
    }

    st_msgcounter -= 1;
    if st_msgcounter == 0 {
        st_chat = st_oldchat;
    }
}

/// Advance the status bar by one game tic.
///
/// Increments the internal clock, samples a new random number for face-idle
/// variation, updates all widget data via `update_widgets`, and records the
/// current health for the ouch-face comparison next tic.
/// Called once per tic by `G_Ticker` in `g_game.c`.
///
/// # Safety
///
/// Mutates `st_clock`, `st_randomnumber`, and `st_oldhealth`, and indirectly
/// touches every widget global via `update_widgets`. Caller must ensure
/// `lifecycle::start` has been called and that `plyr` is valid.
///
/// ## dtmc (whole-body)
///
/// This body IS the demo-synchronization surface: the single
/// `M_Random()` draw advances `rndindex` (state-hash word 2) exactly
/// once per tic, and its position between the `st_clock` increment
/// and the `update_widgets` fan-out is the observable pinned by the
/// ledger vector in `super::dtmc`. Nothing here is extractable --
/// keep whole, keep order.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs:218` imports the upstream name through the
/// root shim.
#[doc(alias = "ST_Ticker")]
#[export_name = "ST_Ticker"]
pub unsafe extern "C" fn ticker() {
    st_clock = st_clock.wrapping_add(1);
    st_randomnumber = M_Random();
    update_widgets();
    st_oldhealth = (*plyr).health;
}
