//! The cooperative netgame stats page: the sequential
//! kills/items/secrets/frags count-up state machine and its draw.

use std::ffi::c_int;
use std::ptr;

use super::anim::{draw_animated_back, init_animated_back, update_animated_back};
use super::drawutil::{draw_level_finished, draw_num, draw_percent, slam_background};
use super::state::{
    acceleratestage, bcnt, cnt_frags, cnt_items, cnt_kills, cnt_pause, cnt_secret, dofrags,
    frags, items, kills, me, ng_state, p, percent, plrs, secret, star, state, wbs,
};
use super::types::{stateenum_t, NG_SPACINGX, NG_STATSY, WI_SPACINGY};
use super::SHORT;
use crate::doom::d_mode;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::doomstat::gamemode;
use crate::doom::i_timer::TICRATE;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::v_video::V_DrawPatch;

/// Initialise the cooperative netgame stats phase: zero per-player counters and check if frags exist.
///
/// # Safety
///
/// Mutates the netgame-stats globals (`state`, `ng_state`, `cnt_pause`,
/// `cnt_kills`, `cnt_items`, `cnt_secret`, `cnt_frags`, `dofrags`) and
/// reads `playeringame`. Caller must ensure `lifecycle::init_variables` has run.
#[doc(alias = "WI_initNetgameStats")]
pub(super) unsafe fn init_netgame_stats() {
    state = stateenum_t::StatCount;
    acceleratestage = 0;
    ng_state = 1;
    cnt_pause = TICRATE;

    for i in 0..MAXPLAYERS {
        if crate::doom::g_game::playeringame[i] == 0 {
            continue;
        }
        cnt_kills[i] = 0;
        cnt_items[i] = 0;
        cnt_secret[i] = 0;
        cnt_frags[i] = 0;
        dofrags += super::stats_dm::frag_sum(i as c_int);
    }

    dofrags = if dofrags != 0 { 1 } else { 0 };

    init_animated_back();
}

/// Tick the netgame stats phase: sequentially count up kills, items, secrets, and frags.
///
/// States 2/4/6/8 are counting states; odd states are pauses between categories.
///
/// # Safety
///
/// Mutates the netgame-stats globals (`cnt_kills`, `cnt_items`,
/// `cnt_secret`, `cnt_frags`, `ng_state`, `cnt_pause`, `acceleratestage`)
/// and reads `plrs`, `wbs`, `playeringame`. Triggers sound effects via
/// `S_StartSound`. Caller must ensure `init_netgame_stats` has run.
#[doc(alias = "WI_updateNetgameStats")]
pub(super) unsafe fn update_netgame_stats() {
    update_animated_back();

    if acceleratestage != 0 && ng_state != 10 {
        acceleratestage = 0;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] == 0 {
                continue;
            }
            cnt_kills[i] = ((*plrs.offset(i as isize)).skills * 100) / (*wbs).maxkills;
            cnt_items[i] = ((*plrs.offset(i as isize)).sitems * 100) / (*wbs).maxitems;
            cnt_secret[i] = ((*plrs.offset(i as isize)).ssecret * 100) / (*wbs).maxsecret;
            if dofrags != 0 {
                cnt_frags[i] = super::stats_dm::frag_sum(i as c_int);
            }
        }
        S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
        ng_state = 10;
    }

    if ng_state == 2 {
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let mut stillticking = false;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] == 0 {
                continue;
            }
            cnt_kills[i] += 2;
            let target = ((*plrs.offset(i as isize)).skills * 100) / (*wbs).maxkills;
            if cnt_kills[i] >= target {
                cnt_kills[i] = target;
            } else {
                stillticking = true;
            }
        }
        if !stillticking {
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            ng_state += 1;
        }
    } else if ng_state == 4 {
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let mut stillticking = false;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] == 0 {
                continue;
            }
            cnt_items[i] += 2;
            let target = ((*plrs.offset(i as isize)).sitems * 100) / (*wbs).maxitems;
            if cnt_items[i] >= target {
                cnt_items[i] = target;
            } else {
                stillticking = true;
            }
        }
        if !stillticking {
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            ng_state += 1;
        }
    } else if ng_state == 6 {
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let mut stillticking = false;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] == 0 {
                continue;
            }
            cnt_secret[i] += 2;
            let target = ((*plrs.offset(i as isize)).ssecret * 100) / (*wbs).maxsecret;
            if cnt_secret[i] >= target {
                cnt_secret[i] = target;
            } else {
                stillticking = true;
            }
        }
        if !stillticking {
            S_StartSound(ptr::null_mut(), Sfx::Barexp as c_int);
            ng_state += 1 + 2 * if dofrags == 0 { 1 } else { 0 };
        }
    } else if ng_state == 8 {
        if bcnt & 3 == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
        }
        let mut stillticking = false;
        for i in 0..MAXPLAYERS {
            if crate::doom::g_game::playeringame[i] == 0 {
                continue;
            }
            cnt_frags[i] += 1;
            let fsum = super::stats_dm::frag_sum(i as c_int);
            if cnt_frags[i] >= fsum {
                cnt_frags[i] = fsum;
            } else {
                stillticking = true;
            }
        }
        if !stillticking {
            S_StartSound(ptr::null_mut(), Sfx::Pldeth as c_int);
            ng_state += 1;
        }
    } else if ng_state == 10 {
        if acceleratestage != 0 {
            S_StartSound(ptr::null_mut(), Sfx::Sgcock as c_int);
            if gamemode == d_mode::commercial {
                super::lifecycle::init_no_state();
            } else {
                super::lifecycle::init_show_next_loc();
            }
        }
    } else if ng_state & 1 != 0 {
        cnt_pause -= 1;
        if cnt_pause == 0 {
            ng_state += 1;
            cnt_pause = TICRATE;
        }
    }
}

/// Draw the cooperative netgame stats page: background, column headers, and per-player rows.
///
/// # Safety
///
/// Reads the netgame-stats globals plus the column-header patches
/// (`kills`, `items`, `secret`, `frags`, `percent`, `p`, `star`) and
/// `playeringame`. Caller must ensure `lifecycle::load_data` cached the
/// patches and the netgame stats phase has been initialised.
#[doc(alias = "WI_drawNetgameStats")]
pub(super) unsafe fn draw_netgame_stats() {
    let pwidth = SHORT((*percent).width) as c_int;

    slam_background();
    draw_animated_back();
    draw_level_finished();

    let ng_statsx = 32 + SHORT((*star).width) as c_int / 2 + 32 * if dofrags == 0 { 1 } else { 0 };

    V_DrawPatch(
        ng_statsx + NG_SPACINGX - SHORT((*kills).width) as c_int,
        NG_STATSY,
        kills,
    );
    V_DrawPatch(
        ng_statsx + 2 * NG_SPACINGX - SHORT((*items).width) as c_int,
        NG_STATSY,
        items,
    );
    V_DrawPatch(
        ng_statsx + 3 * NG_SPACINGX - SHORT((*secret).width) as c_int,
        NG_STATSY,
        secret,
    );
    if dofrags != 0 {
        V_DrawPatch(
            ng_statsx + 4 * NG_SPACINGX - SHORT((*frags).width) as c_int,
            NG_STATSY,
            frags,
        );
    }

    let mut y = NG_STATSY + SHORT((*kills).height) as c_int;

    for i in 0..MAXPLAYERS {
        if crate::doom::g_game::playeringame[i] == 0 {
            continue;
        }
        let mut x = ng_statsx;
        V_DrawPatch(x - SHORT((*p[i]).width) as c_int, y, p[i]);
        if i as c_int == me {
            V_DrawPatch(x - SHORT((*p[i]).width) as c_int, y, star);
        }
        x += NG_SPACINGX;
        draw_percent(x - pwidth, y + 10, cnt_kills[i]);
        x += NG_SPACINGX;
        draw_percent(x - pwidth, y + 10, cnt_items[i]);
        x += NG_SPACINGX;
        draw_percent(x - pwidth, y + 10, cnt_secret[i]);
        x += NG_SPACINGX;
        if dofrags != 0 {
            draw_num(x, y + 10, cnt_frags[i], -1);
        }
        y += WI_SPACINGY;
    }
}
