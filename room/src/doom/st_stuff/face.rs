//! The face widget: the pain-row formula (`calc_pain_offset`, whose
//! pure core lives in `super::dtmc`) and the expression priority
//! cascade (`update_face_widget`).

use std::ffi::c_int;

use super::consts::{
    ST_DEADFACE, ST_EVILGRINCOUNT, ST_EVILGRINOFFSET, ST_GODFACE, ST_MUCHPAIN,
    ST_OUCHOFFSET, ST_RAMPAGEDELAY, ST_RAMPAGEOFFSET, ST_STRAIGHTFACECOUNT, ST_TURNCOUNT,
    ST_TURNOFFSET,
};
use super::{oldweaponsowned, plyr, st_facecount, st_faceindex, st_oldhealth, st_randomnumber};
use crate::doom::d_player::NUMWEAPONS;
use crate::doom::p_telept::mobj_t;
use crate::doom::r_main::R_PointToAngle2;
use crate::doom::tables::{ANG180, ANG45};

/// Compute the base face-array index for the current pain level.
///
/// Maps the player's clamped health (0-100) to a pain-row offset inside the
/// flat `faces` array. The result is a multiple of `ST_FACESTRIDE`. The
/// computed value is cached in a function-local static so recalculation only
/// happens when `health` changes.
///
/// Precondition: `plyr` is non-null and points to a valid `PlayerT`.
///
/// # Safety
///
/// Dereferences the global `plyr` pointer and reads/writes function-local
/// `static mut` cache slots. Caller must ensure `lifecycle::init_data` has set
/// `plyr` to a valid player and that no concurrent access to the cache occurs.
///
/// The exactness-bearing formula is extracted to
/// [`super::dtmc::pain_offset`] (baseline vectors commit f8c7e47,
/// pre-move); the `> 100 -> 100` clamp stays here because the clamped
/// value doubles as the cache key, and the `lastcalc`/`oldhealth`
/// cache slots stay at the call site per the dtmc extraction rule.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the status-bar internal callers reach the upstream name through
/// the root shim.
#[doc(alias = "ST_calcPainOffset")]
#[export_name = "ST_calcPainOffset"]
pub unsafe extern "C" fn calc_pain_offset() -> c_int {
    static mut lastcalc: c_int = 0;
    static mut oldhealth: c_int = -1;

    let health = if (*plyr).health > 100 {
        100
    } else {
        (*plyr).health
    };

    if health != oldhealth {
        lastcalc = super::dtmc::pain_offset(health);
        oldhealth = health;
    }
    lastcalc
}

/// Choose the correct face frame for this tic and update `st_faceindex` / `st_facecount`.
///
/// Priority system (highest wins):
/// 1. Dead face (priority 9).
/// 2. Evil grin on weapon pickup (priority 8).
/// 3. Ouch face on large damage (`health - st_oldhealth > ST_MUCHPAIN`) or turn
///    face on normal damage from an attacker (priority 7).
/// 4. Ouch face on large damage or rampage face when taking damage from an
///    unknown source (priority 6).
/// 5. Rampage face after `ST_RAMPAGEDELAY` tics of continuous fire (priority 5).
/// 6. God-mode / invulnerability face (priority 4).
/// 7. Idle random straight face (priority 0, selected when `st_facecount` reaches 0).
///
/// Precondition: `plyr` is non-null and `oldweaponsowned` matches the snapshot
/// from the previous call.
///
/// # Safety
///
/// Dereferences the global `plyr` pointer (and `plyr->mo`/`plyr->attacker`
/// when computing turn directions) and reads/mutates the face-widget globals
/// (`st_faceindex`, `st_facecount`, `st_oldhealth`, `oldweaponsowned`,
/// function-local `priority`/`lastattackdown`). Caller must ensure
/// `lifecycle::start`/`init_data` has been called and that no other thread
/// accesses these globals concurrently.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `ticker::update_widgets` reaches the upstream name through the
/// root shim.
#[doc(alias = "ST_updateFaceWidget")]
#[export_name = "ST_updateFaceWidget"]
pub unsafe extern "C" fn update_face_widget() {
    static mut lastattackdown: c_int = -1;
    static mut priority: c_int = 0;
    let diffang: u32;
    let i: c_int;

    if priority < 10 && (*plyr).health == 0 {
        priority = 9;
        st_faceindex = ST_DEADFACE;
        st_facecount = 1;
    }

    if priority < 9 && (*plyr).bonuscount != 0 {
        let mut doevilgrin = 0;
        for i in 0..NUMWEAPONS {
            if oldweaponsowned[i] != (*plyr).weaponowned[i] {
                doevilgrin = 1;
                oldweaponsowned[i] = (*plyr).weaponowned[i];
            }
        }
        if doevilgrin != 0 {
            priority = 8;
            st_facecount = ST_EVILGRINCOUNT;
            st_faceindex = calc_pain_offset() + ST_EVILGRINOFFSET;
        }
    }

    if priority < 8
        && (*plyr).damagecount != 0
        && !(*plyr).attacker.is_null()
        && (*plyr).attacker != (*plyr).mo
    {
        priority = 7;
        if (*plyr).health - st_oldhealth > ST_MUCHPAIN {
            st_facecount = ST_TURNCOUNT;
            st_faceindex = calc_pain_offset() + ST_OUCHOFFSET;
        } else {
            let badguyangle = R_PointToAngle2(
                (*((*plyr).mo as *mut mobj_t)).x,
                (*((*plyr).mo as *mut mobj_t)).y,
                (*((*plyr).attacker as *mut mobj_t)).x,
                (*((*plyr).attacker as *mut mobj_t)).y,
            );
            if badguyangle > (*((*plyr).mo as *mut mobj_t)).angle {
                diffang = badguyangle - (*((*plyr).mo as *mut mobj_t)).angle;
                i = if diffang > ANG180 { 1 } else { 0 };
            } else {
                diffang = (*((*plyr).mo as *mut mobj_t)).angle - badguyangle;
                i = if diffang <= ANG180 { 1 } else { 0 };
            }

            st_facecount = ST_TURNCOUNT;
            st_faceindex = calc_pain_offset();

            if diffang < ANG45 {
                st_faceindex += ST_RAMPAGEOFFSET;
            } else if i != 0 {
                st_faceindex += ST_TURNOFFSET;
            } else {
                st_faceindex += ST_TURNOFFSET + 1;
            }
        }
    }

    if priority < 7 && (*plyr).damagecount != 0 {
        if (*plyr).health - st_oldhealth > ST_MUCHPAIN {
            priority = 7;
            st_facecount = ST_TURNCOUNT;
            st_faceindex = calc_pain_offset() + ST_OUCHOFFSET;
        } else {
            priority = 6;
            st_facecount = ST_TURNCOUNT;
            st_faceindex = calc_pain_offset() + ST_RAMPAGEOFFSET;
        }
    }

    if priority < 6 {
        if (*plyr).attackdown != 0 {
            if lastattackdown == -1 {
                lastattackdown = ST_RAMPAGEDELAY;
            } else {
                lastattackdown -= 1;
                if lastattackdown == 0 {
                    priority = 5;
                    st_faceindex = calc_pain_offset() + ST_RAMPAGEOFFSET;
                    st_facecount = 1;
                    lastattackdown = 1;
                }
            }
        } else {
            lastattackdown = -1;
        }
    }

    if priority < 5 && (((*plyr).cheats & 2) != 0 || (*plyr).powers[0] != 0) {
        // CF_GODMODE || pw_invulnerability
        priority = 4;
        st_faceindex = ST_GODFACE;
        st_facecount = 1;
    }

    if st_facecount == 0 {
        st_faceindex = calc_pain_offset() + (st_randomnumber % 3);
        st_facecount = ST_STRAIGHTFACECOUNT;
        priority = 0;
    }

    st_facecount -= 1;
}
