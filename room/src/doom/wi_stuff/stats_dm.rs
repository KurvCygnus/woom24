//! The deathmatch stats page: the frag-matrix count-up state machine
//! and its draw, plus the shared frag-total helper.

use std::ffi::c_int;
use std::ptr;

use super::anim::{draw_animated_back, init_animated_back, update_animated_back};
use super::drawutil::{draw_level_finished, draw_num, slam_background};
use super::state::{
    acceleratestage, bcnt, cnt_pause, dm_frags, dm_state, dm_totals, killers, me, num, p, plrs,
    star, state, total, victims, bstar,
};
use super::types::{stateenum_t, DM_KILLERSX, DM_KILLERSY, DM_MATRIXX, DM_MATRIXY, DM_SPACINGX, DM_TOTALSX, DM_VICTIMSX, DM_VICTIMSY, WI_SPACINGY};
use super::SHORT;
use crate::doom::d_mode;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::doomstat::gamemode;
use crate::doom::i_timer::TICRATE;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::v_video::V_DrawPatch;

/// Compute the net frag total for `playernum`: sum of frags against other active players
/// minus self-frags.
///
/// # Safety
///
/// Reads the global `playeringame` array and indexes into `plrs[playernum]`
/// (a pointer into `wbs.plyr`). Caller must ensure `lifecycle::init_variables`
/// has set `plrs` to a valid array and that `playernum` is within
/// `0..MAXPLAYERS`.
#[doc(alias = "WI_fragSum")]
pub(super) unsafe fn frag_sum(playernum: c_int) -> c_int {
    let mut sum = 0;
    for i in 0..MAXPLAYERS {
        if crate::doom::g_game::playeringame[i] != 0 && i as c_int != playernum {
            sum += (*plrs.offset(playernum as isize)).frags[i];
        }
    }
    sum -= (*plrs.offset(playernum as isize)).frags[playernum as usize];
    sum
}

/// Initialise the deathmatch stats phase: zero all counters and start counting up.
///
/// # Safety
///
/// Mutates the intermission state globals (`state`, `acceleratestage`,
/// `dm_state`, `cnt_pause`, `dm_frags`, `dm_totals`) and reads
/// `playeringame`. Caller must ensure `lifecycle::init_variables` has been invoked.
#[doc(alias = "WI_initDeathmatchStats")]
pub(super) unsafe fn init_deathmatch_stats() {
    state = stateenum_t::StatCount;
    acceleratestage = 0;
    dm_state = 1;
    cnt_pause = TICRATE;

    for i in 0..MAXPLAYERS {
        if crate::doom::g_game::playeringame[i] != 0 {
            for j in 0..MAXPLAYERS {
                if crate::doom::g_game::playeringame[j] != 0 {
                    dm_frags[i][j] = 0;
                }
            }
            dm_totals[i] = 0;
        }
    }

    init_animated_back();
}

/// Tick the deathmatch stats phase: count frag values up toward the actual totals.
///
/// State machine: odd states are pauses, state 2 ticks frags, state 4 waits for acceleration.
///
/// # Safety
///
/// Mutates the deathmatch-stats globals (`dm_frags`, `dm_totals`,
/// `dm_state`, `cnt_pause`, `acceleratestage`) and reads `plrs`,
/// `playeringame`, `gamemode`. Triggers sound effects via `S_StartSound`.
/// Caller must ensure `init_deathmatch_stats` has run and the sound
/// subsystem is initialised.
#[doc(alias = "WI_updateDeathmatchStats")]
pub(super) unsafe fn update_deathmatch_stats() {
    update_animated_back();

    if acceleratestage != 0 && dm_state != 4 {
        acceleratestage = 0;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] != 0 {
                for j in 0..MAXPLAYERS {
                    if crate::doom::g_game::playeringame[j] != 0 {
                        dm_frags[i][j] = (*plrs.offset(i as isize)).frags[j];
                    }
                }
                dm_totals[i] = frag_sum(i as c_int);
            }
        }
        S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
        dm_state = 4;
    }

    if dm_state == 2 {
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let mut stillticking = false;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] != 0 {
                for j in 0..MAXPLAYERS {
                    if crate::doom::g_game::playeringame[j] != 0
                        && dm_frags[i][j] != (*plrs.offset(i as isize)).frags[j]
                    {
                        if (*plrs.offset(i as isize)).frags[j] < 0 {
                            dm_frags[i][j] -= 1;
                        } else {
                            dm_frags[i][j] += 1;
                        }
                        if dm_frags[i][j] > 99 {
                            dm_frags[i][j] = 99;
                        }
                        if dm_frags[i][j] < -99 {
                            dm_frags[i][j] = -99;
                        }
                        stillticking = true;
                    }
                }
                dm_totals[i] = frag_sum(i as c_int);
                if dm_totals[i] > 99 {
                    dm_totals[i] = 99;
                }
                if dm_totals[i] < -99 {
                    dm_totals[i] = -99;
                }
            }
        }
        if !stillticking {
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            dm_state += 1;
        }
    } else if dm_state == 4 {
        if acceleratestage != 0 {
            S_StartSound(ptr::null_mut(), Sfx::Slop as c_int);
            if gamemode == d_mode::commercial {
                super::lifecycle::init_no_state();
            } else {
                super::lifecycle::init_show_next_loc();
            }
        }
    } else if dm_state & 1 != 0 {
        cnt_pause -= 1;
        if cnt_pause == 0 {
            dm_state += 1;
            cnt_pause = TICRATE;
        }
    }
}

/// Draw the deathmatch stats page: background, level name, frag matrix, and totals column.
///
/// # Safety
///
/// Reads the deathmatch-stats globals plus the `total`/`killers`/`victims`/
/// `p`/`star`/`bstar` patches and `playeringame`. Caller must ensure
/// `lifecycle::load_data` cached the patches and the deathmatch stats phase
/// has been initialised.
#[doc(alias = "WI_drawDeathmatchStats")]
pub(super) unsafe fn draw_deathmatch_stats() {
    slam_background();
    draw_animated_back();
    draw_level_finished();

    V_DrawPatch(
        DM_TOTALSX - SHORT((*total).width) as c_int / 2,
        DM_MATRIXY - WI_SPACINGY + 10,
        total,
    );
    V_DrawPatch(DM_KILLERSX, DM_KILLERSY, killers);
    V_DrawPatch(DM_VICTIMSX, DM_VICTIMSY, victims);

    let mut x = DM_MATRIXX + DM_SPACINGX;
    let mut y = DM_MATRIXY;

    for i in 0..MAXPLAYERS {
        if crate::doom::g_game::playeringame[i] != 0 {
            V_DrawPatch(
                x - SHORT((*p[i]).width) as c_int / 2,
                DM_MATRIXY - WI_SPACINGY,
                p[i],
            );
            V_DrawPatch(DM_MATRIXX - SHORT((*p[i]).width) as c_int / 2, y, p[i]);
            if i as c_int == me {
                V_DrawPatch(
                    x - SHORT((*p[i]).width) as c_int / 2,
                    DM_MATRIXY - WI_SPACINGY,
                    bstar,
                );
                V_DrawPatch(DM_MATRIXX - SHORT((*p[i]).width) as c_int / 2, y, star);
            }
        }
        x += DM_SPACINGX;
        y += WI_SPACINGY;
    }

    let mut y = DM_MATRIXY + 10;
    let w = SHORT((*num[0]).width) as c_int;

    for i in 0..MAXPLAYERS {
        let mut x = DM_MATRIXX + DM_SPACINGX;
        if crate::doom::g_game::playeringame[i] != 0 {
            for j in 0..MAXPLAYERS {
                if crate::doom::g_game::playeringame[j] != 0 {
                    draw_num(x + w, y, dm_frags[i][j], 2);
                }
                x += DM_SPACINGX;
            }
            draw_num(DM_TOTALSX + w, y, dm_totals[i], 2);
        }
        y += WI_SPACINGY;
    }
}
