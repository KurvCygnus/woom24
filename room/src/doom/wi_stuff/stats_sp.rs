//! The single-player stats page: the kill/item/secret/par count-up
//! state machine and its draw.

use std::ffi::c_int;
use std::ptr;

use super::anim::{draw_animated_back, init_animated_back, update_animated_back};
use super::drawutil::{draw_percent, draw_time, slam_background};
use super::state::{
    acceleratestage, bcnt, cnt_items, cnt_kills, cnt_par, cnt_pause, cnt_secret, cnt_time, items,
    kills, me, num, par, plrs, sp_secret, sp_state, state, timepatch, wbs,
};
use super::types::{stateenum_t, SP_STATSX, SP_STATSY, SP_TIMEX, SP_TIMEY};
use super::SHORT;
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::i_timer::TICRATE;
use crate::doom::i_video::SCREENWIDTH;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::v_video::V_DrawPatch;

/// Initialise the single-player stats phase: set all display values to -1 (not yet drawn).
///
/// # Safety
///
/// Mutates the single-player stats globals (`state`, `sp_state`,
/// `cnt_kills[0]`, `cnt_items[0]`, `cnt_secret[0]`, `cnt_time`, `cnt_par`,
/// `cnt_pause`, `acceleratestage`) and resets the animated background.
/// Caller must ensure `lifecycle::init_variables` has run.
#[doc(alias = "WI_initStats")]
pub(super) unsafe fn init_stats() {
    state = stateenum_t::StatCount;
    acceleratestage = 0;
    sp_state = 1;
    cnt_kills[0] = -1;
    cnt_items[0] = -1;
    cnt_secret[0] = -1;
    cnt_time = -1;
    cnt_par = -1;
    cnt_pause = TICRATE;

    init_animated_back();
}

/// Tick the single-player stats phase: sequentially count up kills, items, secrets, then time.
///
/// States 2/4/6 count up kill/item/secret percentages; state 8 counts time and par
/// simultaneously; state 10 waits for the player to accelerate.
///
/// # Safety
///
/// Mutates the single-player stats counters (`cnt_kills[0]`, `cnt_items[0]`,
/// `cnt_secret[0]`, `cnt_time`, `cnt_par`, `sp_state`, `cnt_pause`,
/// `acceleratestage`) and reads `plrs`, `wbs`. Triggers sound effects via
/// `S_StartSound`. Caller must ensure `init_stats` has run.
#[doc(alias = "WI_updateStats")]
pub(super) unsafe fn update_stats() {
    update_animated_back();

    if acceleratestage != 0 && sp_state != 10 {
        acceleratestage = 0;
        cnt_kills[0] = ((*plrs.offset(me as isize)).skills * 100) / (*wbs).maxkills;
        cnt_items[0] = ((*plrs.offset(me as isize)).sitems * 100) / (*wbs).maxitems;
        cnt_secret[0] = ((*plrs.offset(me as isize)).ssecret * 100) / (*wbs).maxsecret;
        cnt_time = (*plrs.offset(me as isize)).stime / TICRATE;
        cnt_par = (*wbs).partime / TICRATE;
        S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
        sp_state = 10;
    }

    if sp_state == 2 {
        cnt_kills[0] += 2;
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let target = ((*plrs.offset(me as isize)).skills * 100) / (*wbs).maxkills;
        if cnt_kills[0] >= target {
            cnt_kills[0] = target;
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            sp_state += 1;
        }
    } else if sp_state == 4 {
        cnt_items[0] += 2;
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let target = ((*plrs.offset(me as isize)).sitems * 100) / (*wbs).maxitems;
        if cnt_items[0] >= target {
            cnt_items[0] = target;
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            sp_state += 1;
        }
    } else if sp_state == 6 {
        cnt_secret[0] += 2;
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let target = ((*plrs.offset(me as isize)).ssecret * 100) / (*wbs).maxsecret;
        if cnt_secret[0] >= target {
            cnt_secret[0] = target;
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            sp_state += 1;
        }
    } else if sp_state == 8 {
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        cnt_time += 3;
        let target_time = (*plrs.offset(me as isize)).stime / TICRATE;
        if cnt_time >= target_time {
            cnt_time = target_time;
        }
        cnt_par += 3;
        let target_par = (*wbs).partime / TICRATE;
        if cnt_par >= target_par {
            cnt_par = target_par;
            if cnt_time >= target_time {
                S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
                sp_state += 1;
            }
        }
    } else if sp_state == 10 {
        if acceleratestage != 0 {
            S_StartSound(ptr::null_mut(), Sfx::Sgcock as c_int);
            if gamemode == d_mode::commercial {
                super::lifecycle::init_no_state();
            } else {
                super::lifecycle::init_show_next_loc();
            }
        }
    } else if sp_state & 1 != 0 {
        cnt_pause -= 1;
        if cnt_pause == 0 {
            sp_state += 1;
            cnt_pause = TICRATE;
        }
    }
}

/// Draw the single-player stats page: background, level name, kill/item/secret/time/par values.
///
/// # Safety
///
/// Reads the single-player stats counters plus the patches `num`, `kills`,
/// `items`, `sp_secret`, `timepatch`, `par`, and dereferences `wbs` to
/// decide whether to draw the par-time row. Caller must ensure
/// `lifecycle::load_data` cached the patches and the stats phase has
/// been initialised.
#[doc(alias = "WI_drawStats")]
pub(super) unsafe fn draw_stats() {
    let lh = (3 * SHORT((*num[0]).height) as c_int) / 2;

    slam_background();
    draw_animated_back();
    super::drawutil::draw_level_finished();

    V_DrawPatch(SP_STATSX, SP_STATSY, kills);
    draw_percent(SCREENWIDTH - SP_STATSX, SP_STATSY, cnt_kills[0]);

    V_DrawPatch(SP_STATSX, SP_STATSY + lh, items);
    draw_percent(SCREENWIDTH - SP_STATSX, SP_STATSY + lh, cnt_items[0]);

    V_DrawPatch(SP_STATSX, SP_STATSY + 2 * lh, sp_secret);
    draw_percent(SCREENWIDTH - SP_STATSX, SP_STATSY + 2 * lh, cnt_secret[0]);

    V_DrawPatch(SP_TIMEX, SP_TIMEY(), timepatch);
    draw_time(SCREENWIDTH / 2 - SP_TIMEX, SP_TIMEY(), cnt_time);

    if (*wbs).epsd < 3 {
        V_DrawPatch(SCREENWIDTH / 2 + SP_TIMEX, SP_TIMEY(), par);
        draw_time(SCREENWIDTH - SP_TIMEX, SP_TIMEY(), cnt_par);
    }
}
