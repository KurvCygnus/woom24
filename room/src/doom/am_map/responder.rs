//! The automap input surface: the key/mouse/joystick responder (pan,
//! zoom, toggle, follow, grid, marks, iddt cheat) and the `const` cheat
//! sequence constructor shared with `state::cheat_amap`.

use std::ffi::{c_char, c_int};

use super::consts::{F_PANINC, M_ZOOMIN, M_ZOOMOUT};
use super::state::{
    automapactive, cheat_amap, cheating, followplayer, f_oldloc, grid, markpointnum, m_paninc,
    mtof_zoommul, ftom_zoommul, plr,
};
use super::view::{ftom, min_out_window_scale, restore_scale_and_loc, save_scale_and_loc};
use super::DEH_String;
use crate::c_write;
use crate::doom::d_event::event_t;
use crate::doom::g_game::{deathmatch, viewactive};
use crate::doom::m_cheat::cht_CheckCheat;
use crate::doom::m_controls::{
    key_map_clearmark, key_map_east, key_map_follow, key_map_grid, key_map_mark, key_map_maxzoom,
    key_map_north, key_map_south, key_map_toggle, key_map_west, key_map_zoomin, key_map_zoomout,
};
use crate::doom::m_fixed::FRACUNIT;

/// Compile-time helper that copies a byte slice into a fixed-length `c_char`
/// array, padding with zeros.
///
/// Used to initialise [`cheat_amap`]'s `sequence` field in a `const` context.
pub(super) const fn make_cheat_seq(seq: &[u8]) -> [c_char; 25] {
    let mut arr = [0i8; 25];
    let mut i = 0;
    while i < seq.len() {
        arr[i] = seq[i] as c_char;
        i += 1;
    }
    arr
}

/// Process a keyboard/mouse event for the automap.
///
/// When the automap is closed: opens it on `key_map_toggle`.
/// When the automap is open and a key-down event arrives: handles pan, zoom,
/// toggle, follow mode, grid toggle, mark placement/clear, and the `iddt` cheat.
/// On key-up: stops ongoing pan and zoom.
///
/// Returns 1 if the event was consumed, 0 if it should be forwarded.
///
/// Called from C code in `g_game.c`.  C origin: `AM_Responder` in am_map.c.
///
/// # Safety
///
/// `ev` must be a valid pointer to an `event_t`; mutable statics are accessed.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/responder.rs` and the differential oracle
/// (`c2rust-intermediate/src/am_map.rs:23`) reach the upstream name
/// through the root shim.
///
/// The iddt branch is render-only fidelity: it cycles the `cheating`
/// counter and forces `rc = 0` -- never any simulation state (report
/// §5.4).
#[doc(alias = "AM_Responder")]
#[export_name = "AM_Responder"]
pub unsafe extern "C" fn responder(ev: *mut event_t) -> c_int {
    let mut rc: c_int = 0;
    static mut bigstate: c_int = 0;

    if automapactive == 0 {
        if (*ev).type_ == 0 && (*ev).data1 == key_map_toggle {
            // ev_keydown
            super::lifecycle::start();
            viewactive = 0;
            rc = 1;
        }
    } else if (*ev).type_ == 0 {
        // ev_keydown
        rc = 1;
        let key = (*ev).data1;

        if key == key_map_east {
            if followplayer == 0 {
                m_paninc.x = ftom(F_PANINC);
            } else {
                rc = 0;
            }
        } else if key == key_map_west {
            if followplayer == 0 {
                m_paninc.x = -ftom(F_PANINC);
            } else {
                rc = 0;
            }
        } else if key == key_map_north {
            if followplayer == 0 {
                m_paninc.y = ftom(F_PANINC);
            } else {
                rc = 0;
            }
        } else if key == key_map_south {
            if followplayer == 0 {
                m_paninc.y = -ftom(F_PANINC);
            } else {
                rc = 0;
            }
        } else if key == key_map_zoomout {
            mtof_zoommul = M_ZOOMOUT;
            ftom_zoommul = M_ZOOMIN;
        } else if key == key_map_zoomin {
            mtof_zoommul = M_ZOOMIN;
            ftom_zoommul = M_ZOOMOUT;
        } else if key == key_map_toggle {
            bigstate = 0;
            viewactive = 1;
            super::lifecycle::stop();
        } else if key == key_map_maxzoom {
            bigstate = !bigstate;
            if bigstate != 0 {
                save_scale_and_loc();
                min_out_window_scale();
            } else {
                restore_scale_and_loc();
            }
        } else if key == key_map_follow {
            followplayer = !followplayer;
            f_oldloc.x = c_int::MAX;
            if followplayer != 0 {
                (*plr).message = DEH_String(c"Follow Mode ON".as_ptr().cast_mut());
            } else {
                (*plr).message = DEH_String(c"Follow Mode OFF".as_ptr().cast_mut());
            }
        } else if key == key_map_grid {
            grid = !grid;
            if grid != 0 {
                (*plr).message = DEH_String(c"Grid ON".as_ptr().cast_mut());
            } else {
                (*plr).message = DEH_String(c"Grid OFF".as_ptr().cast_mut());
            }
        } else if key == key_map_mark {
            static mut AM_MARK_MSG: [c_char; 20] = [0; 20];
            c_write!(AM_MARK_MSG, "Marked Spot {}", markpointnum as c_int);
            (*plr).message = std::ptr::addr_of_mut!(AM_MARK_MSG[0]);
            super::lifecycle::add_mark();
        } else if key == key_map_clearmark {
            super::lifecycle::clear_marks();
            (*plr).message = DEH_String(c"All Marks Cleared".as_ptr().cast_mut());
        } else {
            rc = 0;
        }

        if deathmatch == 0 && cht_CheckCheat(&raw mut cheat_amap, (*ev).data2 as c_char) != 0 {
            rc = 0;
            cheating = (cheating + 1) % 3;
        }
    } else if (*ev).type_ == 1 {
        // ev_keyup
        rc = 0;
        let key = (*ev).data1;

        if key == key_map_east || key == key_map_west {
            if followplayer == 0 {
                m_paninc.x = 0;
            }
        } else if key == key_map_north || key == key_map_south {
            if followplayer == 0 {
                m_paninc.y = 0;
            }
        } else if key == key_map_zoomout || key == key_map_zoomin {
            mtof_zoommul = FRACUNIT;
            ftom_zoommul = FRACUNIT;
        }
    }

    rc
}

