//! The attract-loop sequencer: `page_ticker` (upstream `D_PageTicker`),
//! `page_drawer` (upstream `D_PageDrawer`), `advance_demo` (upstream
//! `D_AdvanceDemo`), `do_advance_demo` (upstream `D_DoAdvanceDemo`) and
//! `start_title` (upstream `D_StartTitle`).
//!
//! Whole-body demo-synchronization surface (F10 wave C4 adjudication):
//! WHEN the attract pages rotate and WHAT state each arm latches
//! determine `gamestate`/`pagename`/music at every demo-golden
//! checkpoint. The pure page-count core lives in [`super::dtmc`].

use std::ffi::c_int;

use crate::doom::d_mode;
use crate::doom::d_player::{consoleplayer, players, MAXPLAYERS};
use crate::doom::doomstat::{gamemode, gameversion};
use crate::doom::g_game::{
    gameaction, gamestate, paused, usergame, G_DeferedPlayDemo,
};
use crate::doom::i_timer::TICRATE;
use crate::doom::s_sound::S_StartMusic;
use crate::doom::sounds::Mus;
use crate::doom::v_video::{patch_t, V_DrawPatch};
use crate::doom::w_wad::{W_CacheLumpName, W_CheckNumForName};
use crate::doom::z_zone::PU_CACHE;

use self::advance_demo as D_AdvanceDemo;
use super::compat::eq_ci as c_str_eq;
use super::consts::{ga_nothing, GS_DEMOSCREEN};
use super::dtmc::attract_sequence_count;
use super::state::{advancedemo, bfgedition, demosequence, pagename, pagetic};

/// Handle timing for demo projection.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` imports the upstream name through the root shim.
#[doc(alias = "D_PageTicker")]
#[export_name = "D_PageTicker"]
pub extern "C" fn page_ticker() {
    unsafe {
        pagetic -= 1;
        if pagetic < 0 { D_AdvanceDemo(); }
    }
}

/// Draw the current demo page.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// root shim routes `display`'s `GS_DEMOSCREEN` arm to it.
#[doc(alias = "D_PageDrawer")]
#[export_name = "D_PageDrawer"]
pub extern "C" fn page_drawer() {
    unsafe { V_DrawPatch(0, 0, W_CacheLumpName(pagename, PU_CACHE) as *mut patch_t); }
}

/// Called after each demo or intro demosequence finishes.
///
/// One-statement latch (`advancedemo = 1`) polled by `d_net`'s
/// `run_tic` every tic -- the flag edge is on the per-tic path. The
/// pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/demo.rs` and `d_net` import the upstream name through the
/// root shim.
#[doc(alias = "D_AdvanceDemo")]
#[export_name = "D_AdvanceDemo"]
pub extern "C" fn advance_demo() {
    unsafe { advancedemo = 1; }
}

/// Cycle through the demo sequences.
///
/// Writer of the attract-loop state machine (`playerstate`,
/// `advancedemo`, `usergame`, `paused`, `gameaction`, `gamestate`,
/// `pagename`, `pagetic`, `demosequence`, music). The pre-move export
/// symbol is kept with `#[export_name]` below; `d_net` extern-declares
/// and calls the upstream name every tic (`loop_table.rs`).
#[doc(alias = "D_DoAdvanceDemo")]
#[export_name = "D_DoAdvanceDemo"]
pub extern "C" fn do_advance_demo() {
    unsafe {
        // Set player state to live (PST_LIVE = 1)
        let cp = consoleplayer;
        if cp >= 0 && (cp as usize) < MAXPLAYERS { players[cp as usize].playerstate = 1; }

        advancedemo = 0;
        usergame = 0;
        paused = 0;
        gameaction = ga_nothing;

        // Demo sequence: 7 for ultimate/final, 6 for others
        let max_seq = attract_sequence_count(gameversion);
        demosequence = (demosequence + 1) % max_seq;

        match demosequence
        {
            0 => {
                if gamemode == d_mode::commercial { pagetic = TICRATE * 11; } else { pagetic = 170; }
                gamestate = GS_DEMOSCREEN;
                pagename = c"TITLEPIC".as_ptr().cast_mut();
                if gamemode == d_mode::commercial { S_StartMusic(Mus::Dm2ttl as c_int); }
                else { S_StartMusic(Mus::Intro as c_int); }
            }
            1 => { G_DeferedPlayDemo(c"demo1".as_ptr()); }
            2 => {
                pagetic = 200;
                gamestate = GS_DEMOSCREEN;
                pagename = c"CREDIT".as_ptr().cast_mut();
            }
            3 => { G_DeferedPlayDemo(c"demo2".as_ptr()); }
            4 => {
                gamestate = GS_DEMOSCREEN;
                if gamemode == d_mode::commercial {
                    pagetic = TICRATE * 11;
                    pagename = c"TITLEPIC".as_ptr().cast_mut();
                    S_StartMusic(Mus::Dm2ttl as c_int);
                }
                else {
                    pagetic = 200;
                    if gamemode == d_mode::retail { pagename = c"CREDIT".as_ptr().cast_mut(); }
                    else { pagename = c"HELP2".as_ptr().cast_mut(); }
                }
            }
            5 => { G_DeferedPlayDemo(c"demo3".as_ptr()); }
            6 => {
                // THE DEFINITIVE DOOM Special Edition demo
                G_DeferedPlayDemo(c"demo4".as_ptr());
            }
            _ => {}
        }

        // BFG Edition workaround: TITLEPIC missing, use INTERPIC
        if bfgedition != 0 && c_str_eq(pagename, c"TITLEPIC".as_ptr()) && W_CheckNumForName(c"titlepic".as_ptr()) < 0 { pagename = c"INTERPIC".as_ptr().cast_mut(); }
    }
}

/// Start the title screen demo sequence.
///
/// `demosequence = -1` pins the attract cycle's phase-0 start, hence
/// the golden window's opening state. The pre-move export symbol is
/// kept with `#[export_name]` below; `boot::doom_main` calls it through
/// the in-module alias.
#[doc(alias = "D_StartTitle")]
#[export_name = "D_StartTitle"]
pub extern "C" fn start_title() {
    unsafe {
        gameaction = ga_nothing;
        demosequence = -1;
        D_AdvanceDemo();
    }
}
