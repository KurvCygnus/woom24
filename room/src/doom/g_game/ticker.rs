//! The per-tic gameplay dispatcher (`ticker`, upstream `G_Ticker`) and
//! its turbo-message scratch buffer.
//!
//! `G_Ticker` drains the `gameaction` queue (the GA_* machine), routes
//! per-player ticcmds through the demo read/write hooks, decodes the
//! `BT_SPECIAL` buttons, updates the `consistancy` ring (the
//! net-consistency cookie reads `rndindex`, NOT `prndindex`) and advances
//! the active sub-state -- demo-synchronization surface whole-body (F10
//! wave C3 adjudication). The body moved verbatim from pre-split
//! `g_game.rs`; see the module root for the mapping table.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};

use crate::doom::am_map::AM_Ticker;
use crate::doom::crt::c_snprintf1;
use crate::doom::d_loop::{gametic, ticdup};
use crate::doom::d_main::D_PageTicker;
use crate::doom::d_net::netcmds;
use crate::doom::d_player::{TiccmdT, MAXPLAYERS};
use crate::doom::f_finale::{F_StartFinale, F_Ticker};
use crate::doom::hu_stuff::{player_names, HU_Ticker};
use crate::doom::i_system::I_Error;
use crate::doom::m_misc::{M_snprintf_clamp, M_StringCopy};
use crate::doom::m_random::rndindex;
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::P_Ticker;
use crate::doom::s_sound::{S_PauseSound, S_ResumeSound};
use crate::doom::st_stuff::ST_Ticker;
use crate::doom::v_video::V_ScreenShot;
use crate::doom::wi_stuff::{WI_End, WI_Ticker};

use super::actions::{
    do_completed as G_DoCompleted, do_load_level as G_DoLoadLevel, do_new_game as G_DoNewGame,
    do_world_done as G_DoWorldDone,
};
use super::demo::{
    do_play_demo as G_DoPlayDemo, read_demo_ticcmd as G_ReadDemoTiccmd,
    write_demo_ticcmd as G_WriteDemoTiccmd,
};
use super::responder::deh_string;
use super::savegentry::{do_load_game as G_DoLoadGame, do_save_game as G_DoSaveGame};
use super::spawn::do_reborn as G_DoReborn;

use super::consts::{
    ga_completed, ga_loadgame, ga_loadlevel, ga_newgame, ga_nothing, ga_playdemo, ga_savegame,
    ga_screenshot, ga_victory, ga_worlddone, BTS_PAUSE, BTS_SAVEGAME, BTS_SAVEMASK, BTS_SAVESHIFT,
    BT_SPECIAL, BT_SPECIALMASK, GS_DEMOSCREEN, GS_FINALE, GS_INTERMISSION, GS_LEVEL, PST_REBORN,
};
use super::state::{
    consoleplayer, consistancy, demoplayback, demorecording, gameaction, gamestate, netdemo,
    netgame, oldgamestate, paused, playeringame, players, turbodetected, SAVEDESCRIPTION,
    SAVEGAMESLOT,
};

// ---------------------------------------------------------------------------
// Cross-subfile upstream-name shims (intra-module): the ticker drives the
// GA_* machine and the demo read/write hooks by their upstream names,
// exactly as the pre-split body did (the alias imports at the top).
// ---------------------------------------------------------------------------

/// Buffer for turbo-cheat message (static local in `G_Ticker`).
static mut TURBOMESSAGE: [c_char; 80] = [0; 80];

/// Advance the gameplay state machine by exactly one tic (1/35 s).
///
/// The tic dispatches in three phases, in order:
///
/// 1. **Reborn pass** - every player in `PST_REBORN` is respawned via
///    `G_DoReborn`.
/// 2. **Gameaction drain** - the `gameaction` queue is processed until empty,
///    dispatching to `G_DoLoadLevel`, `G_DoNewGame`, `G_DoLoadGame`,
///    `G_DoSaveGame`, `G_DoPlayDemo`, `G_DoCompleted`, `F_StartFinale`,
///    `G_DoWorldDone` or a screenshot grab.
/// 3. **Per-player ticcmd pass** - net commands are copied into each player
///    slot, demo I/O runs, turbo banners are emitted (every ~4 seconds,
///    offset per player), and the consistency-check ring is updated when
///    netgame / non-netdemo / `gametic % ticdup == 0`.
///
/// Special buttons (pause toggle, savegame request) are then decoded; finally
/// the active state ticker runs - `P_Ticker` / `ST_Ticker` / `AM_Ticker` /
/// `HU_Ticker` for `GS_LEVEL`, `WI_Ticker` for `GS_INTERMISSION`,
/// `F_Ticker` for `GS_FINALE`, `D_PageTicker` for `GS_DEMOSCREEN`.
///
/// # Safety
/// Mutates virtually every game-loop global; intended to be called at most
/// once per tic from the engine main loop. The C symbol is pinned
/// (`G_Ticker`): `d_net`'s extern block links it by symbol and its
/// `run_tic` calls it every tic.
#[doc(alias = "G_Ticker")]
#[export_name = "G_Ticker"]
pub unsafe extern "C" fn ticker()
{
    use crate::doom::c_ffi::BACKUPTICS;

    // Player reborns
    for i in 0..MAXPLAYERS { if playeringame[i] != 0 && players[i].playerstate == PST_REBORN { G_DoReborn(i as c_int); } }

    // Process pending game actions
    while gameaction != ga_nothing
    {
        match gameaction
        {
            ga_loadlevel => G_DoLoadLevel(),
            ga_newgame => G_DoNewGame(),
            ga_loadgame => G_DoLoadGame(),
            ga_savegame => G_DoSaveGame(),
            ga_playdemo => G_DoPlayDemo(),
            ga_completed => G_DoCompleted(),
            ga_victory => F_StartFinale(),
            ga_worlddone => G_DoWorldDone(),
            ga_screenshot =>
            {
                V_ScreenShot(c"DOOM%02i.%s".as_ptr().cast_mut());
                players[consoleplayer as usize].message =
                    deh_string(c"screen shot".as_ptr()) as *mut c_char;
                gameaction = ga_nothing;
            }
            _ => {}
        }
    }

    // Get commands, check consistency, build new consistency check
    let buf = ((gametic / ticdup) as usize) % BACKUPTICS;

    for i in 0..MAXPLAYERS
    {
        if playeringame[i] != 0
        {
            let cmd = &mut players[i].cmd as *mut TiccmdT;

            // Copy net command into player command
            std::ptr::copy_nonoverlapping(netcmds.add(i), cmd, 1);

            if demoplayback != 0 { G_ReadDemoTiccmd(cmd); }
            if demorecording != 0 { G_WriteDemoTiccmd(cmd); }

            // Turbo detection
            if (*cmd).forwardmove > 0x32 { turbodetected[i] = 1; }

            //* The nested `M_snprintf_clamp(..., c_snprintf1(...))` looks
            //* wrong and is parity-correct -- vanilla's nested snprintf had
            //* the same shape. Never "fix" it (pre-split g_game.rs:1546-1555).
            if(gametic & 31) == 0 &&
                ((gametic >> 5) % MAXPLAYERS as c_int) == i as c_int &&
                turbodetected[i] != 0
                {
                    M_snprintf_clamp(
                        std::ptr::addr_of_mut!(TURBOMESSAGE[0]),
                        80,
                        c_snprintf1(
                            std::ptr::addr_of_mut!(TURBOMESSAGE[0]),
                            80,
                            c"%s is turbo!".as_ptr(),
                            player_names[i],
                        ),
                    );
                    players[consoleplayer as usize].message = std::ptr::addr_of_mut!(TURBOMESSAGE[0]);
                    turbodetected[i] = 0;
                }

            if netgame != 0 && netdemo == 0 && (gametic % ticdup) == 0
            {
                if gametic > BACKUPTICS as c_int && consistancy[i][buf] != (*cmd).consistancy { I_Error(c"consistency failure (%i should be %i)".as_ptr()); }
                let mo = players[i].mo as *mut mobj_t;
                if !mo.is_null() { consistancy[i][buf] = (*mo).x as u8; }
                else { consistancy[i][buf] = rndindex as u8; }
            }
        }
    }

    // Check for special buttons
    for i in 0..MAXPLAYERS
    {
        if playeringame[i] != 0
        {
            let buttons = players[i].cmd.buttons;
            if buttons & BT_SPECIAL != 0
            {
                match buttons & BT_SPECIALMASK
                {
                    BTS_PAUSE =>
                    {
                        paused ^= 1;
                        if paused != 0 { S_PauseSound(); }
                        else { S_ResumeSound(); }
                    }
                    BTS_SAVEGAME =>
                    {
                        if SAVEDESCRIPTION[0] == 0
                        {
                            M_StringCopy(
                                std::ptr::addr_of_mut!(SAVEDESCRIPTION[0]),
                                c"NET GAME".as_ptr(),
                                32
                            );
                        }
                        SAVEGAMESLOT = ((buttons & BTS_SAVEMASK) >> BTS_SAVESHIFT) as c_int;
                        gameaction = ga_savegame;
                    }
                    _ => {}
                }
            }
        }
    }

    // Check if intermission screen just ended
    if oldgamestate == GS_INTERMISSION && gamestate != GS_INTERMISSION { WI_End(); }
    oldgamestate = gamestate;

    // Main game-state dispatch
    match gamestate
    {
        GS_LEVEL =>
        {
            P_Ticker();
            ST_Ticker();
            AM_Ticker();
            HU_Ticker();
        }
        GS_INTERMISSION => { WI_Ticker(); }
        GS_FINALE => { F_Ticker(); }
        GS_DEMOSCREEN => { D_PageTicker(); }
        _ => {}
    }
}
