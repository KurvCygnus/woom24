//! The intermission lifecycle: respond/end/tick/draw, the stats
//! dispatch, the accelerate poll, and the WAD load/unload walker
//! (including the episode-1 anim-8 aliasing hack).

use std::ffi::{c_char, c_int};
use std::ptr;

use super::anim::{draw_animated_back, init_animated_back, update_animated_back};
use super::drawutil::{draw_entering_level, draw_on_lnode, slam_background};
use super::state::{
    acceleratestage, bcnt, bstar, bp, background, cnt, colon, entering, finished, firstrefresh,
    frags, items, kills, killers, lnames, me, NUMCMAPS, num, p, par, percent, plrs, secret,
    snl_pointeron, sp_secret, splat, star, state, sucks, timepatch, total, victims, wbs, wiminus,
    yah,
};
use super::stats_dm::{init_deathmatch_stats, update_deathmatch_stats};
use super::stats_ng::init_netgame_stats;
use super::stats_sp::init_stats;
use super::types::{stateenum_t, wbstartstruct_t, SHOWNEXTLOCDELAY, NUMMAPS};
use super::DEH_String;
use crate::c_write;
use crate::DEH_snprintf;
use crate::doom::d_event::event_t;
use crate::doom::d_mode;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::doomstat::gamemode;
use crate::doom::g_game::{deathmatch, netgame, playeringame, players, G_WorldDone};
use crate::doom::m_misc::M_StringCopy;
use crate::doom::s_sound::S_ChangeMusic;
use crate::doom::sounds::Mus;
use crate::doom::v_video::patch_t;
use crate::doom::w_wad::{W_CacheLumpName, W_CheckNumForName, W_ReleaseLumpName};
use crate::doom::z_zone::{Z_Malloc, PU_STATIC};

/// Function pointer type for the load/unload callback used by `load_unload_data`.
///
/// The callback receives the lump name and a pointer to the patch pointer slot.
type LoadCallback = unsafe extern "C" fn(*mut c_char, *mut *mut patch_t);

/// Intermission event responder - always returns 0 (all input goes through `check_for_accelerate`).
///
/// Called by `G_Responder` in `g_game.c`.
///
/// # Safety
///
/// `_ev` is not dereferenced. Caller may pass any pointer (including null).
/// The function is `unsafe extern "C"` for ABI compatibility with the C
/// responder signature only.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/responder.rs` reaches the upstream name through the root
/// shim.
#[doc(alias = "WI_Responder")]
#[export_name = "WI_Responder"]
pub unsafe extern "C" fn responder(_ev: *mut event_t) -> c_int {
    0
}

/// Tear down the intermission subsystem after the screen is dismissed.
///
/// Releases all WAD patches loaded by `load_data`. Called by `G_WorldDone`
/// before the game transitions to the next level.
///
/// # Safety
///
/// Delegates to `unload_data`, which nulls every cached patch pointer.
/// Caller must ensure no draw functions are still executing.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` reaches the upstream name through the root shim.
#[doc(alias = "WI_End")]
#[export_name = "WI_End"]
pub unsafe extern "C" fn end() {
    unload_data();
}

/// Enter the `NoState` phase: count down 10 tics then call `G_WorldDone`.
///
/// # Safety
///
/// Mutates the intermission state globals `state`, `acceleratestage`, and
/// `cnt`. Caller must ensure `start` has been invoked.
#[doc(alias = "WI_initNoState")]
pub(super) unsafe fn init_no_state() {
    state = stateenum_t::NoState;
    acceleratestage = 0;
    cnt = 10;
}

/// Tick the `NoState` phase; advances animations and calls `G_WorldDone` when `cnt` reaches 0.
///
/// # Safety
///
/// Mutates `cnt` and runs `update_animated_back` (which dereferences `wbs`
/// and the animation state). Caller must ensure `start` has been invoked.
#[doc(alias = "WI_updateNoState")]
pub(super) unsafe fn update_no_state() {
    update_animated_back();
    cnt -= 1;
    if cnt == 0 {
        G_WorldDone();
    }
}

/// Enter the `ShowNextLoc` phase: show the episode map with a blinking "you are here" pointer.
///
/// # Safety
///
/// Mutates `state`, `acceleratestage`, `cnt`, and resets the animated
/// background via `init_animated_back` (which dereferences `wbs`). Caller
/// must ensure `start` has been invoked.
#[doc(alias = "WI_initShowNextLoc")]
pub(super) unsafe fn init_show_next_loc() {
    state = stateenum_t::ShowNextLoc;
    acceleratestage = 0;
    cnt = SHOWNEXTLOCDELAY * crate::doom::i_timer::TICRATE;
    init_animated_back();
}

/// Tick the `ShowNextLoc` phase; blinks the pointer and transitions to `NoState` when done.
///
/// # Safety
///
/// Mutates `cnt` and `snl_pointeron` and may transition to `NoState` via
/// `init_no_state`. Caller must ensure `start` has been invoked.
#[doc(alias = "WI_updateShowNextLoc")]
pub(super) unsafe fn update_show_next_loc() {
    update_animated_back();
    cnt -= 1;
    if cnt == 0 || acceleratestage != 0 {
        init_no_state();
    } else {
        snl_pointeron = (cnt & 31) < 20;
    }
}

/// Draw the `ShowNextLoc` phase: episode map with completed-level splats and optional pointer.
///
/// # Safety
///
/// Dereferences the global `wbs` pointer and the `splat`/`yah` patch arrays,
/// and delegates to `slam_background`, `draw_animated_back`, `draw_entering_level`,
/// and `draw_on_lnode`. Caller must ensure `start`/`load_data` have run.
#[doc(alias = "WI_drawShowNextLoc")]
pub(super) unsafe fn draw_show_next_loc() {
    slam_background();
    draw_animated_back();

    if gamemode != d_mode::commercial {
        if (*wbs).epsd > 2 {
            draw_entering_level();
            return;
        }

        let last = if (*wbs).last == 8 {
            (*wbs).next - 1
        } else {
            (*wbs).last
        };

        for i in 0..=last {
            draw_on_lnode(i, std::ptr::addr_of_mut!(splat) as *mut *mut patch_t);
        }

        if (*wbs).didsecret != 0 {
            draw_on_lnode(8, std::ptr::addr_of_mut!(splat) as *mut *mut patch_t);
        }

        if snl_pointeron {
            draw_on_lnode((*wbs).next, std::ptr::addr_of_mut!(yah) as *mut *mut patch_t);
        }
    }

    if gamemode != d_mode::commercial || (*wbs).next != 30 {
        draw_entering_level();
    }
}

/// Draw the `NoState` phase: same as `ShowNextLoc` but with the pointer always visible.
///
/// # Safety
///
/// Mutates `snl_pointeron` and delegates to `draw_show_next_loc`; same
/// invariants apply (intermission must be started and patches loaded).
#[doc(alias = "WI_drawNoState")]
pub(super) unsafe fn draw_no_state() {
    snl_pointeron = true;
    draw_show_next_loc();
}

/// Poll all active players' attack and use buttons and set `acceleratestage` if any are newly pressed.
///
/// This is the mechanism by which the player can skip the count-up animation by
/// pressing fire or use during the intermission screen.
///
/// # Safety
///
/// Reads `playeringame` and mutates each active player's `attackdown`/
/// `usedown` flags via the global `players` array, and may set
/// `acceleratestage`. Caller must ensure the player array is initialised.
#[doc(alias = "WI_checkForAccelerate")]
pub(super) unsafe fn check_for_accelerate() {
    for i in 0..MAXPLAYERS {
        if playeringame[i] != 0 {
            let player = players.as_mut_ptr().add(i);
            if (*player).cmd.buttons & 1 != 0 {
                // BT_ATTACK
                if (*player).attackdown == 0 {
                    acceleratestage = 1;
                }
                (*player).attackdown = 1;
            } else {
                (*player).attackdown = 0;
            }
            if (*player).cmd.buttons & 2 != 0 {
                // BT_USE
                if (*player).usedown == 0 {
                    acceleratestage = 1;
                }
                (*player).usedown = 1;
            } else {
                (*player).usedown = 0;
            }
        }
    }
}

/// Advance the intermission screen by one game tic.
///
/// Increments `bcnt`, starts the intermission music on the first tic,
/// checks for acceleration input, and dispatches to the appropriate state
/// update function. Called by `G_Ticker` in `g_game.c`.
///
/// # Safety
///
/// Mutates `bcnt` and `state` (indirectly via the update functions) and
/// reads `gamemode`/`deathmatch`/`netgame`. Triggers `S_ChangeMusic` and
/// invokes `check_for_accelerate` and one of the per-state updaters, each
/// with their own state-global dependencies. Caller must ensure `start`
/// has been invoked and the sound subsystem is initialised.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` reaches the upstream name through the root shim.
#[doc(alias = "WI_Ticker")]
#[export_name = "WI_Ticker"]
pub unsafe extern "C" fn ticker() {
    bcnt += 1;

    if bcnt == 1 {
        if gamemode == d_mode::commercial {
            S_ChangeMusic(Mus::Dm2int as c_int, 1);
        } else {
            S_ChangeMusic(Mus::Inter as c_int, 1);
        }
    }

    check_for_accelerate();

    match state {
        stateenum_t::StatCount => {
            if deathmatch != 0 {
                update_deathmatch_stats();
            } else if netgame != 0 {
                super::stats_ng::update_netgame_stats();
            } else {
                super::stats_sp::update_stats();
            }
        }
        stateenum_t::ShowNextLoc => {
            update_show_next_loc();
        }
        stateenum_t::NoState => {
            update_no_state();
        }
    }
}

/// Draw the intermission screen for the current frame.
///
/// Dispatches to the appropriate draw function based on `state` and `deathmatch`/`netgame` flags.
/// Called by `D_Display` in `d_main.c` every frame while `gamestate == GS_INTERMISSION`.
///
/// # Safety
///
/// Reads the global `state`, `deathmatch`, and `netgame` flags and
/// dispatches to one of the per-phase draw functions, each of which
/// dereferences the intermission globals (`wbs`, patch pointers, etc.).
/// Caller must ensure `start` has been invoked.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` and the differential oracle reach the upstream
/// name through the root shim.
#[doc(alias = "WI_Drawer")]
#[export_name = "WI_Drawer"]
pub unsafe extern "C" fn drawer() {
    match state {
        stateenum_t::StatCount => {
            if deathmatch != 0 {
                super::stats_dm::draw_deathmatch_stats();
            } else if netgame != 0 {
                super::stats_ng::draw_netgame_stats();
            } else {
                super::stats_sp::draw_stats();
            }
        }
        stateenum_t::ShowNextLoc => {
            draw_show_next_loc();
        }
        stateenum_t::NoState => {
            draw_no_state();
        }
    }
}

/// Walk every intermission lump name and invoke `callback` for each.
///
/// Shared by `load_data` and `unload_data`. Handles both episode (Doom 1)
/// and commercial (Doom 2) map lists, animation frame patches, and all UI
/// patches (numbers, labels, background).
///
/// # Safety
///
/// `callback` is invoked as an `unsafe extern "C"` function and is given raw
/// pointers into the static patch globals (`lnames`, `yah`, `splat`, `num`,
/// `percent`, `finished`, `entering`, etc.) and the per-episode animation
/// state. Caller must pass a callback that respects those pointers' validity
/// and only reads/writes the single slot supplied. Dereferences `wbs` and
/// the global `gamemode`; the WAD subsystem must be initialised.
///
/// The episode-1 anim-8 aliasing hack (`(*st).p[i]` shares episode 1
/// slot-4 patches) moves VERBATIM -- never "fix" the aliasing.
#[doc(alias = "WI_loadUnloadData")]
unsafe fn load_unload_data(callback: LoadCallback) {
    let mut name: [c_char; 9] = [0; 9];

    if gamemode == d_mode::commercial {
        for i in 0..NUMCMAPS {
            DEH_snprintf!(name, "CWILV{:02}", i);
            callback(name.as_mut_ptr(), lnames.offset(i as isize));
        }
    } else {
        for i in 0..NUMMAPS as c_int {
            c_write!(name, "WILV{}{}", (*wbs).epsd, i);
            callback(name.as_mut_ptr(), lnames.offset(i as isize));
        }

        callback(DEH_String(c"WIURH0".as_ptr().cast_mut()), &mut yah[0]);
        callback(DEH_String(c"WIURH1".as_ptr().cast_mut()), &mut yah[1]);
        callback(DEH_String(c"WISPLAT".as_ptr().cast_mut()), &mut splat[0]);

        if (*wbs).epsd < 3 {
            let epsd = (*wbs).epsd as usize;
            for j in 0..super::tables::NUMANIMS[epsd] as usize {
                let cfg = super::tables::anim_config(epsd, j);
                let st = super::tables::anim_state_ptr(epsd, j);
                for i in 0..cfg.nanims as usize {
                    if (*wbs).epsd != 1 || j != 8 {
                        c_write!(name, "WIA{}{:02}{:02}", (*wbs).epsd, j, i);
                        callback(name.as_mut_ptr(), &mut (*st).p[i]);
                    } else {
                        (*st).p[i] = (*super::tables::anim_state_ptr(1, 4)).p[i];
                    }
                }
            }
        }
    }

    callback(DEH_String(c"WIMINUS".as_ptr().cast_mut()), &mut wiminus);

    for i in 0..10i32 {
        DEH_snprintf!(name, "WINUM{}", i);
        callback(name.as_mut_ptr(), &mut num[i as usize]);
    }

    callback(DEH_String(c"WIPCNT".as_ptr().cast_mut()), &mut percent);
    callback(DEH_String(c"WIF".as_ptr().cast_mut()), &mut finished);
    callback(DEH_String(c"WIENTER".as_ptr().cast_mut()), &mut entering);
    callback(DEH_String(c"WIOSTK".as_ptr().cast_mut()), &mut kills);
    callback(DEH_String(c"WIOSTS".as_ptr().cast_mut()), &mut secret);
    callback(DEH_String(c"WISCRT2".as_ptr().cast_mut()), &mut sp_secret);

    if W_CheckNumForName(DEH_String(c"WIOBJ".as_ptr().cast_mut())) >= 0 {
        if netgame != 0 && deathmatch == 0 {
            callback(DEH_String(c"WIOBJ".as_ptr().cast_mut()), &mut items);
        } else {
            callback(DEH_String(c"WIOSTI".as_ptr().cast_mut()), &mut items);
        }
    } else {
        callback(DEH_String(c"WIOSTI".as_ptr().cast_mut()), &mut items);
    }

    callback(DEH_String(c"WIFRGS".as_ptr().cast_mut()), &mut frags);
    callback(DEH_String(c"WICOLON".as_ptr().cast_mut()), &mut colon);
    callback(DEH_String(c"WITIME".as_ptr().cast_mut()), &mut timepatch);
    callback(DEH_String(c"WISUCKS".as_ptr().cast_mut()), &mut sucks);
    callback(DEH_String(c"WIPAR".as_ptr().cast_mut()), &mut par);
    callback(DEH_String(c"WIKILRS".as_ptr().cast_mut()), &mut killers);
    callback(DEH_String(c"WIVCTMS".as_ptr().cast_mut()), &mut victims);
    callback(DEH_String(c"WIMSTT".as_ptr().cast_mut()), &mut total);

    for i in 0..MAXPLAYERS {
        DEH_snprintf!(name, "STPB{}", i);
        callback(name.as_mut_ptr(), &mut p[i]);
        DEH_snprintf!(name, "WIBP{}", i + 1);
        callback(name.as_mut_ptr(), &mut bp[i]);
    }

    if gamemode == d_mode::commercial || (gamemode == d_mode::retail && (*wbs).epsd == 3) {
        M_StringCopy(
            name.as_mut_ptr(),
            DEH_String(c"INTERPIC".as_ptr().cast_mut()),
            name.len(),
        );
    } else {
        DEH_snprintf!(name, "WIMAP{}", (*wbs).epsd);
    }

    callback(name.as_mut_ptr(), &mut background);
}

/// Load callback: cache the named lump at `PU_STATIC` priority and store the pointer.
///
/// # Safety
///
/// Caller must ensure `name` is a valid NUL-terminated C-string pointer
/// recognised by `W_CacheLumpName`, and `variable` is a valid, non-null,
/// properly aligned pointer to a `*mut patch_t` slot the function may
/// overwrite.
#[doc(alias = "WI_loadCallback")]
unsafe extern "C" fn load_callback(name: *mut c_char, variable: *mut *mut patch_t) {
    *variable = W_CacheLumpName(name, PU_STATIC) as *mut patch_t;
}

/// Allocate the `lnames` pointer array and cache all intermission WAD patches.
///
/// Must be called before the first `drawer` call. `lnames` is allocated from
/// the zone heap at `PU_STATIC` with a size matching either `NUMCMAPS`
/// (commercial) or `NUMMAPS` (episode).  Also loads the `star`/`bstar`
/// patches directly.
///
/// # Safety
///
/// Allocates `lnames` via `Z_Malloc` and mutates every cached patch global
/// (`star`, `bstar`, and everything touched by `load_unload_data`). Caller
/// must ensure the WAD and zone-memory subsystems are initialised and that
/// `wbs` is valid (via `init_variables`).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `start` reaches the upstream name directly.
#[doc(alias = "WI_loadData")]
#[export_name = "WI_loadData"]
pub unsafe extern "C" fn load_data() {
    if gamemode == d_mode::commercial {
        NUMCMAPS = 32;
        lnames = Z_Malloc(
            (std::mem::size_of::<*mut patch_t>() * NUMCMAPS as usize) as c_int,
            PU_STATIC,
            ptr::null_mut(),
        ) as *mut *mut patch_t;
    } else {
        lnames = Z_Malloc(
            (std::mem::size_of::<*mut patch_t>() * NUMMAPS) as c_int,
            PU_STATIC,
            ptr::null_mut(),
        ) as *mut *mut patch_t;
    }

    load_unload_data(load_callback);

    star = W_CacheLumpName(DEH_String(c"STFST01".as_ptr().cast_mut()), PU_STATIC) as *mut patch_t;
    bstar = W_CacheLumpName(DEH_String(c"STFDEAD0".as_ptr().cast_mut()), PU_STATIC) as *mut patch_t;
}

/// Unload callback: release the named lump from the WAD cache and null the pointer.
///
/// # Safety
///
/// Caller must ensure `name` is a valid NUL-terminated C-string pointer
/// previously cached via `W_CacheLumpName`, and `variable` is a valid,
/// non-null, properly aligned pointer to a `*mut patch_t` slot that this
/// function may overwrite with null.
#[doc(alias = "WI_unloadCallback")]
unsafe extern "C" fn unload_callback(name: *mut c_char, variable: *mut *mut patch_t) {
    W_ReleaseLumpName(name);
    *variable = ptr::null_mut();
}

/// Release all intermission WAD patches loaded by `load_data`.
///
/// Called indirectly by `end` at the close of the intermission screen.
///
/// # Safety
///
/// Nulls every cached patch pointer in the intermission globals and
/// releases their WAD lumps. Caller must ensure no draw or update
/// functions are still executing.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `end` reaches the upstream name directly.
#[doc(alias = "WI_unloadData")]
#[export_name = "WI_unloadData"]
pub unsafe extern "C" fn unload_data() {
    load_unload_data(unload_callback);
}

/// Initialise all intermission globals from the level-start record `wbstartstruct`.
///
/// Clamps all max-count fields to a minimum of 1 to avoid division-by-zero in
/// percentage calculations. Adjusts `wbs.epsd` downward by 3 for non-retail
/// builds that were given an out-of-range episode number.
///
/// # Safety
///
/// Caller must ensure `wbstartstruct` is a valid, non-null, properly aligned
/// pointer to an initialised `wbstartstruct_t` that remains live for the
/// duration of the intermission. Mutates the global state pointers `wbs`,
/// `plrs` and the counter globals (`acceleratestage`, `cnt`, `bcnt`,
/// `firstrefresh`, `me`).
#[doc(alias = "WI_initVariables")]
unsafe fn init_variables(wbstartstruct: *mut wbstartstruct_t) {
    wbs = wbstartstruct;
    plrs = (*wbs).plyr.as_mut_ptr();

    acceleratestage = 0;
    cnt = 0;
    bcnt = 0;
    firstrefresh = 1;
    me = (*wbs).pnum;

    if (*wbs).maxkills == 0 {
        (*wbs).maxkills = 1;
    }
    if (*wbs).maxitems == 0 {
        (*wbs).maxitems = 1;
    }
    if (*wbs).maxsecret == 0 {
        (*wbs).maxsecret = 1;
    }

    if gamemode != d_mode::retail && (*wbs).epsd > 2 {
        (*wbs).epsd -= 3;
    }
}

/// Start the intermission screen for a newly completed level.
///
/// Initialises all state variables, loads WAD patches, and enters the appropriate
/// stats phase (deathmatch, netgame, or single-player). Called from `G_WorldDone`
/// in `g_game.c`.
///
/// # Safety
///
/// Caller must ensure `wbstartstruct` is a valid, non-null, properly aligned
/// pointer to an initialised `wbstartstruct_t` whose lifetime spans the
/// intermission. Mutates every intermission global via `init_variables`,
/// `load_data`, and the per-mode `init_*Stats` helpers; the WAD, zone,
/// and sound subsystems must be initialised.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/actions.rs` reaches the upstream name through the root shim
/// (the `&raw mut wminfo` ABI handoff).
#[doc(alias = "WI_Start")]
#[export_name = "WI_Start"]
pub unsafe extern "C" fn start(wbstartstruct: *mut wbstartstruct_t) {
    init_variables(wbstartstruct);
    load_data();

    if deathmatch != 0 {
        init_deathmatch_stats();
    } else if netgame != 0 {
        init_netgame_stats();
    } else {
        init_stats();
    }
}
