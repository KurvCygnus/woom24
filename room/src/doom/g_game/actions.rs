//! The deferred GA_* action machine: `do_load_level`, `do_new_game`,
//! `init_new`, `do_completed` and friends -- the actions `G_Ticker`
//! drains from `gameaction`, plus the level-exit edges fired from
//! p_switch / p_spec / p_enemy mid-demo (`exit_level`,
//! `secret_exit_level`) and the fast-monster table patcher.
//!
//! Everything here is demo-synchronization surface (F10 wave C3
//! adjudication): `init_new` reseeds the RNG ledger (`M_ClearRandom` --
//! its position relative to the `do_load_level` tail is hash-load-bearing
//! for the F9 `save_load_roundtrip` golden), `do_completed` seeds the
//! hashed intermission fields and carries the par-time workaround arms,
//! and the exit edges land on the demo stream mid-play. The bodies moved
//! verbatim from pre-split `g_game.rs`; see the module root for the
//! mapping table.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};

use crate::i_error;
use crate::doom::am_map::{AM_Stop, automapactive};
use crate::doom::d_loop::gametic;
use crate::doom::d_main::{fastparm, nomonsters, respawnparm, wipegamestate};
use crate::doom::d_mode::{commercial, exe_chex, exe_final2, exe_ultimate, shareware};
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::doomstat::{gamemode, gameversion};
use crate::doom::f_finale::F_StartFinale;
use crate::doom::m_random::M_ClearRandom;
use crate::doom::p_setup::P_SetupLevel;
use crate::doom::p_tick::leveltime;
use crate::doom::r_data::{R_FlatNumForName, R_TextureNumForName};
use crate::doom::r_sky::{skyflatnum, skytexture};
use crate::doom::s_sound::S_ResumeSound;
use crate::doom::statdump::StatCopy;
use crate::doom::violations::{self, VanillaViolation};
use crate::doom::w_wad::W_CheckNumForName;
use crate::doom::wi_stuff::WI_Start;
use crate::doom::z_zone::Z_CheckHeap;

use super::consts::{
    fixed_t, ga_completed, ga_newgame, ga_nothing, ga_screenshot, ga_victory, ga_worlddone,
    skill_t, GS_INTERMISSION, GS_LEVEL, MAX_JOY_BUTTONS, MAX_MOUSE_BUTTONS, NUMKEYS, PST_DEAD,
    PST_REBORN,
};use super::dtmc::commercial_partime;
use super::responder::deh_string;
use super::spawn::player_finish_level as G_PlayerFinishLevel;
use super::state::{
    consoleplayer, cpars, d_episode, d_map, d_skill, deathmatch, demoplayback, displayplayer,
    gameaction, gameepisode, gamemap, gameskill, gamestate, levelstarttic, netdemo, netgame, pars,
    paused, playeringame, players, respawnmonsters, secretexit, sendpause, sendsave, testcontrols,
    totalkills, totalitems, totalsecret, turbodetected, usergame, viewactive, wminfo,
};
use super::ticcmd::{
    GAMEKEYDOWN, JOYARRAY, JOYXMOVE, JOYSTRAFEMOVE, JOYYMOVE, MOUSEARRAY, MOUSEX, MOUSEY,
};

// ---------------------------------------------------------------------------
// Intra-module aliases: same-file callers keep the upstream names the
// pre-split bodies used (do_world_done / init_new -> G_DoLoadLevel,
// do_new_game -> G_InitNew).
// ---------------------------------------------------------------------------

use self::{do_load_level as G_DoLoadLevel, init_new as G_InitNew};

// ---------------------------------------------------------------------------
// G_DoLoadLevel
// ---------------------------------------------------------------------------

/// Execute the deferred `ga_loadlevel` action: load the current
/// `gameepisode`/`gamemap`, reset per-player input state, force a wipe and
/// transition into `GS_LEVEL`.
///
/// Also fixes up the Doom II / Final Doom / Chex sky textures (`SKY1`/`SKY2`
/// /`SKY3` based on `gamemap`) and resets the input latches so movement
/// keys held across the load do not produce phantom input.
///
/// Players in `PST_DEAD` are flipped to `PST_REBORN` so the per-player
/// reborn loop in `G_Ticker` will respawn them.
///
/// # Safety
/// Mutates many engine globals (`gamestate`, `wipegamestate`, `levelstarttic`,
/// the input rings, the players array). The C symbol is pinned
/// (`G_DoLoadLevel`) so the wasm export set stays byte-identical.
#[doc(alias = "G_DoLoadLevel")]
#[export_name = "G_DoLoadLevel"]
pub unsafe extern "C" fn do_load_level()
{
    skyflatnum = R_FlatNumForName(deh_string(c"F_SKY1".as_ptr()) as *mut c_char);

    // Fix sky texture for Final Doom / Chex
    if gamemode == commercial && (gameversion == exe_final2 || gameversion == exe_chex)
    {
        let skytexturename: *const c_char = if gamemap < 12
        {
            c"SKY1".as_ptr()
        }
        else if gamemap < 21
        {
            c"SKY2".as_ptr()
        }
        else
        {
            c"SKY3".as_ptr()
        };
        skytexture = R_TextureNumForName(deh_string(skytexturename) as *mut c_char);
    }

    levelstarttic = gametic;

    if wipegamestate == GS_LEVEL
    {
        wipegamestate = -1; // force a wipe
    }

    gamestate = GS_LEVEL;

    for i in 0..MAXPLAYERS
    {
        turbodetected[i] = 0;
        if playeringame[i] != 0 && players[i].playerstate == PST_DEAD
        {
            players[i].playerstate = PST_REBORN;
        }
        players[i].frags = [0; MAXPLAYERS];
    }

    P_SetupLevel(gameepisode, gamemap, 0, gameskill);
    displayplayer = consoleplayer;
    gameaction = ga_nothing;
    Z_CheckHeap();

    // Clear input state
    GAMEKEYDOWN = [0; NUMKEYS];
    JOYXMOVE = 0;
    JOYYMOVE = 0;
    JOYSTRAFEMOVE = 0;
    MOUSEX = 0;
    MOUSEY = 0;
    sendpause = 0;
    sendsave = 0;
    paused = 0;
    MOUSEARRAY = [0; MAX_MOUSE_BUTTONS + 1];
    JOYARRAY = [0; MAX_JOY_BUTTONS + 1];

    if testcontrols != 0
    {
        players[consoleplayer as usize].message = c"Press escape to quit.".as_ptr().cast_mut();
    }
}

// ---------------------------------------------------------------------------
// G_ScreenShot
// ---------------------------------------------------------------------------

/// Defer a screenshot to the next `G_Ticker` pass via `gameaction =
/// ga_screenshot`.
///
/// Render-side user action; nothing on the demo stream reads it (glue,
/// F10 wave C3 adjudication).
///
/// # Safety
/// Writes the `gameaction` global. The C symbol is pinned (`G_ScreenShot`)
/// so the wasm export set stays byte-identical.
#[doc(alias = "G_ScreenShot")]
#[export_name = "G_ScreenShot"]
pub unsafe extern "C" fn screen_shot()
{
    gameaction = ga_screenshot;
}

// ---------------------------------------------------------------------------
// G_ExitLevel / G_SecretExitLevel
// ---------------------------------------------------------------------------

/// Request a normal end-of-level transition; clears `secretexit` so the
/// next intermission picks the standard "next map" target.
///
/// # Safety
/// Writes the `secretexit` and `gameaction` globals. The C symbol is
/// pinned (`G_ExitLevel`): p_switch / p_spec / p_enemy fire it mid-demo.
#[doc(alias = "G_ExitLevel")]
#[export_name = "G_ExitLevel"]
pub unsafe extern "C" fn exit_level()
{
    secretexit = 0;
    gameaction = ga_completed;
}

/// Request a secret-exit end-of-level transition.
///
/// On Doom II the secret exit only applies when MAP31 is actually present
/// in the loaded WAD ("if no Wolf3D levels, no secret exit" - the German
/// edition retail patch removed those maps).
///
/// # Safety
/// Writes the `secretexit` and `gameaction` globals. The C symbol is
/// pinned (`G_SecretExitLevel`): fired from p_switch / p_spec mid-demo.
#[doc(alias = "G_SecretExitLevel")]
#[export_name = "G_SecretExitLevel"]
pub unsafe extern "C" fn secret_exit_level()
{
    if gamemode == commercial && W_CheckNumForName(c"map31".as_ptr()) < 0
    {
        secretexit = 0;
    }
    else
    {
        secretexit = 1;
    }
    gameaction = ga_completed;
}

// ---------------------------------------------------------------------------
// G_DoCompleted
// ---------------------------------------------------------------------------

/// Process the deferred `ga_completed` action: tear down level state, set up
/// the intermission `wbstartstruct_t`, hand off to the WI subsystem and stop
/// the automap if it was active.
///
/// Map-routing rules (mirroring vanilla):
///
/// * Chex ends after MAP05 (instead of MAP08).
/// * Doom 1: MAP08 of any episode triggers `ga_victory`; MAP09 sets
///   `didsecret` on every player.
/// * Doom II: secret-exit on MAP15 -> MAP31, on MAP31 -> MAP32; normal-exit
///   on MAP31 or MAP32 -> MAP16.
/// * Doom II: a map33 normal exit reads its par time from one int past
///   `cpars[31]` -- the first four bytes of the adjacent GAMMALVL0 string;
///   emulated explicitly per chocolate (docs/vanilla-workarounds.md #9).
///   Commercial maps outside 1..=33 have no vanilla value and abort via
///   `I_Error` instead of an out-of-bounds read.
/// * Doom 1 episode-4 par-time deliberately reads off the end of `pars[]`
///   into `cpars[]` to reproduce the vanilla overflow bug used by statcheck
///   regression tests.
///
/// # Safety
/// Mutates `wminfo`, `gamestate`, `viewactive`, `automapactive` and the
/// `players` array. The C symbol is pinned (`G_DoCompleted`) so the wasm
/// export set stays byte-identical.
#[doc(alias = "G_DoCompleted")]
#[export_name = "G_DoCompleted"]
pub unsafe extern "C" fn do_completed()
{
    gameaction = ga_nothing;

    for i in 0..MAXPLAYERS
    {
        if playeringame[i] != 0
        {
            G_PlayerFinishLevel(i as c_int);
        }
    }

    if automapactive != 0
    {
        AM_Stop();
    }

    if gamemode != commercial
    {
        if gameversion == exe_chex
        {
            if gamemap == 5
            {
                gameaction = ga_victory;
                return;
            }
        }
        else
        {
            match gamemap
            {
                8 =>
                {
                    gameaction = ga_victory;
                    return;
                }
                9 =>
                {
                    for i in 0..MAXPLAYERS
                    {
                        players[i].didsecret = 1;
                    }
                }
                _ =>
                {}
            }
        }
    }

    if gamemap == 8 && gamemode != commercial
    {
        gameaction = ga_victory;
        return;
    }
    if gamemap == 9 && gamemode != commercial
    {
        for i in 0..MAXPLAYERS
        {
            players[i].didsecret = 1;
        }
    }

    wminfo.didsecret = players[consoleplayer as usize].didsecret;
    wminfo.epsd = gameepisode - 1;
    wminfo.last = gamemap - 1;

    if gamemode == commercial
    {
        wminfo.next = if secretexit != 0
        {
            match gamemap
            {
                15 => 30,
                31 => 31,
                _ => gamemap,
            }
        }
        else
        {
            match gamemap
            {
                31 | 32 => 15,
                _ => gamemap,
            }
        };
    }
    else
    {
        wminfo.next = if secretexit != 0
        {
            8
        }
        else if gamemap == 9
        {
            match gameepisode
            {
                1 => 3,
                2 => 5,
                3 => 6,
                4 => 2,
                _ => gamemap,
            }
        }
        else
        {
            gamemap
        };
    }

    wminfo.maxkills = totalkills;
    wminfo.maxitems = totalitems;
    wminfo.maxsecret = totalsecret;
    wminfo.maxfrags = 0;

    wminfo.partime = if gamemode == commercial
    {
        match commercial_partime(gamemap)
        {
            Some(partime) => partime,
            None =>
            {
                //* Copy first: `format!` inside `i_error!` would borrow the
                //* mutable static (static_mut_refs).
                let map = gamemap;
                i_error!("G_DoCompleted: commercial map {} has no vanilla par time", map)
            }
        }
    }
    else if gameepisode < 4
    {
        35 * pars[gameepisode as usize][gamemap as usize]
    }
    else
    {
        violations::record(VanillaViolation::ParTimeOverrun);
        35 * cpars[gamemap as usize]
    };

    wminfo.pnum = consoleplayer;

    for i in 0..MAXPLAYERS
    {
        wminfo.plyr[i].in_ = playeringame[i];
        wminfo.plyr[i].skills = players[i].killcount;
        wminfo.plyr[i].sitems = players[i].itemcount;
        wminfo.plyr[i].ssecret = players[i].secretcount;
        wminfo.plyr[i].stime = leveltime;
        wminfo.plyr[i].frags = players[i].frags;
    }

    gamestate = GS_INTERMISSION;
    viewactive = 0;
    automapactive = 0;

    StatCopy(&raw mut wminfo as *mut _ as *mut crate::doom::statdump::wbstartstruct_t);
    WI_Start(&raw mut wminfo);
}

// ---------------------------------------------------------------------------
// G_WorldDone / G_DoWorldDone
// ---------------------------------------------------------------------------

/// Called by WI when the intermission screen finishes: schedule a
/// `ga_worlddone` action and, on Doom II, kick off the per-cluster finale at
/// the appropriate "end of segment" maps (6, 11, 20, 30 - and 15/31 only via
/// the secret exit).
///
/// # Safety
/// Writes `gameaction`, `players[consoleplayer].didsecret`. The C symbol is
/// pinned (`G_WorldDone`) so the wasm export set stays byte-identical.
#[doc(alias = "G_WorldDone")]
#[export_name = "G_WorldDone"]
pub unsafe extern "C" fn world_done()
{
    gameaction = ga_worlddone;

    if secretexit != 0
    {
        players[consoleplayer as usize].didsecret = 1;
    }

    if gamemode == commercial
    {
        match gamemap
        {
            15 | 31 =>
            {
                if secretexit == 0
                {
                    return;
                }
                F_StartFinale();
            }
            6 | 11 | 20 | 30 =>
            {
                F_StartFinale();
            }
            _ =>
            {}
        }
    }
}

/// Process the deferred `ga_worlddone`: enter `GS_LEVEL`, advance `gamemap`
/// to `wminfo.next + 1`, load the new level and re-enable the 3D view.
///
/// # Safety
/// Mutates `gamestate`, `gamemap`, `viewactive`, `gameaction`. The C symbol
/// is pinned (`G_DoWorldDone`) so the wasm export set stays byte-identical.
#[doc(alias = "G_DoWorldDone")]
#[export_name = "G_DoWorldDone"]
pub unsafe extern "C" fn do_world_done()
{
    gamestate = GS_LEVEL;
    gamemap = wminfo.next + 1;
    G_DoLoadLevel();
    gameaction = ga_nothing;
    viewactive = 1;
}

// ---------------------------------------------------------------------------
// G_DeferedInitNew / G_DoNewGame / G_InitNew
// ---------------------------------------------------------------------------

/// Defer a new-game start: latch `(skill, episode, map)` into `d_*` and
/// queue `ga_newgame`. `G_Ticker` will then call `G_DoNewGame` -> `G_InitNew`.
///
/// # Safety
/// Writes the `d_skill`, `d_episode`, `d_map` and `gameaction` globals.
/// The C symbol is pinned (`G_DeferedInitNew`): st_stuff / m_menu call it
/// on every new-game start.
#[doc(alias = "G_DeferedInitNew")]
#[export_name = "G_DeferedInitNew"]
pub unsafe extern "C" fn defered_init_new(skill: skill_t, episode: c_int, map: c_int)
{
    d_skill = skill;
    d_episode = episode;
    d_map = map;
    gameaction = ga_newgame;
}

/// Process `ga_newgame`: clear network / demo state, reset extra players
/// out of the game, then call `G_InitNew` with the deferred `(skill, episode,
/// map)`.
///
/// # Safety
/// Mutates many engine globals. The C symbol is pinned (`G_DoNewGame`) so
/// the wasm export set stays byte-identical.
#[doc(alias = "G_DoNewGame")]
#[export_name = "G_DoNewGame"]
pub unsafe extern "C" fn do_new_game()
{
    demoplayback = 0;
    netdemo = 0;
    netgame = 0;
    deathmatch = 0;
    playeringame[1] = 0;
    playeringame[2] = 0;
    playeringame[3] = 0;
    respawnparm = 0;
    fastparm = 0;
    nomonsters = 0;
    consoleplayer = 0;
    G_InitNew(d_skill, d_episode, d_map);
    gameaction = ga_nothing;
}

/// Initialise a new game with the given skill, episode and map.
///
/// Clamps `skill` to `sk_nightmare`, normalises `episode`/`map` for the
/// active `gamemode` (shareware caps at episode 1; pre-Ultimate caps at 3),
/// reseeds the random number generator, flips fast-monster bookkeeping for
/// nightmare or `-fast`, marks every player `PST_REBORN`, then selects the
/// sky texture.
///
/// The sky-texture selection preserves the vanilla "sky never changes in
/// Doom II" behaviour: the sky is bound here at game start rather than per
/// level, which causes Doom II's sky to be wrong on later maps unless a
/// savegame is loaded. This is intentional for demo compatibility.
///
/// Finally dispatches into `G_DoLoadLevel` to actually load the chosen map.
///
/// # Safety
/// Mutates virtually every game-loop global. The C symbol is pinned
/// (`G_InitNew`): d_main and the savegame path re-enter the engine here.
#[doc(alias = "G_InitNew")]
#[export_name = "G_InitNew"]
pub unsafe extern "C" fn init_new(skill: skill_t, episode: c_int, map: c_int)
{
    let mut skill = skill;
    let mut episode = episode;
    let mut map = map;

    if paused != 0
    {
        paused = 0;
        S_ResumeSound();
    }

    if skill > 4
    {
        // sk_nightmare = 4
        skill = 4;
    }

    if gameversion >= exe_ultimate
    {
        if episode == 0
        {
            episode = 4;
        }
    }
    else
    {
        episode = episode.clamp(1, 3);
    }

    if episode > 1 && gamemode == shareware
    {
        episode = 1;
    }

    if map < 1
    {
        map = 1;
    }
    if map > 9 && gamemode != commercial
    {
        map = 9;
    }

    M_ClearRandom();

    if skill == 4 || respawnparm != 0
    {
        // sk_nightmare = 4
        respawnmonsters = 1;
    }
    else
    {
        respawnmonsters = 0;
    }

    // Fast monsters for nightmare or fastparm
    if fastparm != 0 || (skill == 4 && gameskill != 4)
    {
        set_fast_monsters_bool(true);
    }
    else if skill != 4 && gameskill == 4
    {
        set_fast_monsters_bool(false);
    }

    // Force players to be reborn on first level load
    for i in 0..MAXPLAYERS
    {
        players[i].playerstate = PST_REBORN;
    }

    usergame = 1;
    paused = 0;
    demoplayback = 0;
    automapactive = 0;
    viewactive = 1;
    gameepisode = episode;
    gamemap = map;
    gameskill = skill;

    // Set sky texture (vanilla Doom broken behaviour for Doom II: set once at
    // game start, not per level — deliberately preserved for compatibility).
    let skytexturename: *const c_char = if gamemode == commercial
    {
        if gamemap < 12
        {
            c"SKY1".as_ptr()
        }
        else if gamemap < 21
        {
            c"SKY2".as_ptr()
        }
        else
        {
            c"SKY3".as_ptr()
        }
    }
    else
    {
        match gameepisode
        {
            1 => c"SKY1".as_ptr(),
            2 => c"SKY2".as_ptr(),
            3 => c"SKY3".as_ptr(),
            4 => c"SKY4".as_ptr(),
            _ => c"SKY1".as_ptr(),
        }
    };

    skytexture = R_TextureNumForName(deh_string(skytexturename) as *mut c_char);

    G_DoLoadLevel();
}

// ---------------------------------------------------------------------------
// Fast-monster helper for G_InitNew
// ---------------------------------------------------------------------------

/// Convenience wrapper around [`set_fast_monsters`] that takes a `bool`.
///
/// Exists to keep the `G_InitNew` call sites readable; the state-tic and
/// `mobjinfo.speed` patches themselves live in `set_fast_monsters` so the
/// arithmetic appears once.
///
/// # Safety
/// Calls into `set_fast_monsters`, which mutates the global `states` and
/// `mobjinfo` tables.
#[doc(alias = "set_fast_monsters")]
unsafe fn set_fast_monsters_bool(fast: bool)
{
    // S_SARG_RUN1 .. S_SARG_PAIN2 state range (states 134..160 in vanilla Doom)
    // mobjinfo adjustments for MT_BRUISERSHOT, MT_HEADSHOT, MT_TROOPSHOT
    // These are byte-offset operations into the info tables.
    // Delegate to a safe extern to avoid duplicating the state-offset arithmetic.
    if fast
    {
        set_fast_monsters(1);
    }
    else
    {
        set_fast_monsters(0);
    }
}

// ---------------------------------------------------------------------------
// G_SetFastMonsters — adjusts state tics and monster shot speeds
// ---------------------------------------------------------------------------

/// Toggle the "fast monsters" rule used by nightmare skill and `-fast`.
///
/// When `fast` is non-zero, halves the per-tic duration of the demon "run"
/// and "pain" states (`S_SARG_RUN1`..`S_SARG_PAIN2`) and raises Baron of Hell,
/// Cacodemon and Imp projectile speeds to 20 `FRACUNIT` per tic; when zero,
/// restores the original durations and the slower projectile speeds
/// (15/10/10).
///
/// This change is global to the `states` and `mobjinfo` tables and persists
/// across maps until inverted again - it is `G_InitNew`'s responsibility to
/// only call this when the skill flips into or out of nightmare.
///
/// # Safety
/// Mutates the shared `states` and `mobjinfo` arrays. The C symbol is
/// pinned (`G_SetFastMonsters`) so the wasm export set stays byte-identical.
#[doc(alias = "G_SetFastMonsters")]
#[export_name = "G_SetFastMonsters"]
pub unsafe extern "C" fn set_fast_monsters(fast: c_int)
{
    use crate::doom::info::{
        mobjinfo, states, MT_BRUISERSHOT, MT_HEADSHOT, MT_TROOPSHOT, S_SARG_PAIN2, S_SARG_RUN1,
    };

    for i in S_SARG_RUN1 as usize..=S_SARG_PAIN2 as usize
    {
        if fast != 0
        {
            states[i].tics >>= 1;
        }
        else
        {
            states[i].tics <<= 1;
        }
    }

    let (bruiser_spd, head_spd, troop_spd): (fixed_t, fixed_t, fixed_t) = if fast != 0
    {
        (20 * 65536, 20 * 65536, 20 * 65536)
    }
    else
    {
        (15 * 65536, 10 * 65536, 10 * 65536)
    };

    mobjinfo[MT_BRUISERSHOT as usize].speed = bruiser_spd;
    mobjinfo[MT_HEADSHOT as usize].speed = head_spd;
    mobjinfo[MT_TROOPSHOT as usize].speed = troop_spd;
}
