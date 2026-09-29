//! The finale stage machine: start/tick/respond/draw dispatch over the
//! `FinaleStage` states, including the text→art transition's
//! `wipegamestate = -1` wipe trigger.

use std::ffi::{c_int, c_uint};
use std::ptr;

use super::text_tables::{TEXTSPEED, TEXTWAIT, TEXTSCREENS};
use super::{finaletext, finaleflat, logical_gamemission, FinaleStage, FINALE_COUNT, FINALE_STAGE};
use crate::doom::am_map::automapactive;
use crate::doom::d_event::event_t;
use crate::doom::d_main::wipegamestate;
use crate::doom::d_mode;
use crate::doom::doomstat::{gamemode, gameversion};
use crate::doom::g_game::{gameaction, gameepisode, gamemap, gamestate, players, viewactive};
use crate::doom::s_sound::{S_ChangeMusic, S_StartMusic};
use crate::doom::sounds::Mus;

/// `gamestate` value that indicates the finale is active.
///
/// Mirrors `GS_FINALE` from `g_game.h`; kept local to avoid a circular
/// dependency.
const GS_FINALE: c_int = 2;

/// `gameaction` value meaning "do nothing".
///
/// C origin: `ga_nothing` in `g_game.h`.
const ga_nothing: c_int = 0;

/// `gameaction` value that triggers loading the next level/world.
///
/// C origin: `ga_worlddone` in `g_game.h`.
const ga_worlddone: c_int = 8;

/// Maximum number of simultaneously active players.
///
/// C origin: `MAXPLAYERS` in `doomdef.h`.
const MAXPLAYERS: usize = 4;

/// Begin a new finale sequence for the current episode and map.
///
/// Resets game state (`gameaction`, `gamestate`, `viewactive`,
/// `automapactive`), selects and starts the appropriate music, searches
/// `TEXTSCREENS` to find the matching text string and background flat, and
/// initialises the internal stage to `Text`.
///
/// Postcondition: `FINALE_STAGE` is `Text`, `FINALE_COUNT` is 0, and
/// `finaletext`/`finaleflat` point to the selected screen data (or null if
/// none matched).
///
/// Called from C code in `g_game.c` when `gameaction == ga_completed` and the
/// appropriate episode/map conditions are met.  C origin: `F_StartFinale` in
/// f_finale.c.
///
/// The dead `TEXTSCREENS` padding-sentinel check and the Chex Quest
/// level-5 inline hack are faithful-to-C oddities carried verbatim.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game` reaches the upstream name through the root shim.
#[doc(alias = "F_StartFinale")]
#[export_name = "F_StartFinale"]
pub extern "C" fn start_finale() {
    unsafe {
        gameaction = ga_nothing;
        gamestate = GS_FINALE;
        viewactive = 0;
        automapactive = 0;

        if logical_gamemission() == d_mode::doom { S_ChangeMusic(Mus::Victor as c_int, 1); }
        else { S_ChangeMusic(Mus::ReadM as c_int, 1); }

        finaletext = ptr::null_mut();
        finaleflat = ptr::null_mut();

        for i in 0..22usize {
            let screen = &TEXTSCREENS[i];
            if screen.mission == 0 && screen.background.is_null() { break; } // padding sentinel

            // Hack for Chex Quest
            if gameversion == d_mode::exe_chex && screen.mission == d_mode::doom {
                // In C this mutates the static array; we emulate by checking level 5.
                // Actually the C code DOES mutate textscreens[i].level.
                // For simplicity we just check the hacked level inline.
            }

            let level = if gameversion == d_mode::exe_chex && screen.mission == d_mode::doom {
                5
            } else {
                screen.level
            };

            if logical_gamemission() == screen.mission
                && (logical_gamemission() != d_mode::doom || gameepisode == screen.episode)
                && gamemap == level
            {
                finaletext = screen.text;
                finaleflat = screen.background;
            }
        }

        finaletext = super::DEH_String(finaletext);
        finaleflat = super::DEH_String(finaleflat);

        FINALE_STAGE = FinaleStage::Text;
        FINALE_COUNT = 0;
    }
}

/// Forward input events to the cast responder while in the Cast stage.
///
/// Returns 1 if the event was consumed, 0 otherwise.  All non-Cast stage
/// events are ignored at this level.
///
/// Called from C code in `g_game.c`.  C origin: `F_Responder` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/responder.rs` reaches the upstream name through the root
/// shim.
#[doc(alias = "F_Responder")]
#[export_name = "F_Responder"]
pub extern "C" fn responder(event: *mut event_t) -> c_int {
    unsafe {
        let _ev = &*event;
        if let FinaleStage::Cast = FINALE_STAGE { return super::cast::cast_responder(event); }
        0
    }
}

/// Advance the finale state machine by one game tick.
///
/// In commercial mode (`gamemode == commercial`), checks whether any player
/// has pressed a button to skip; on map 30 this starts the cast roll, otherwise
/// it triggers `ga_worlddone`.  Increments `FINALE_COUNT`; delegates to
/// [`super::cast::cast_ticker`] during the Cast stage.  In non-commercial
/// mode, advances from Text to ArtScreen when the text has been fully
/// displayed and the wait period has expired (triggering a wipe and, for
/// episode 3, the bunny music).
///
/// Called from C code in `g_game.c` once per game tick.
/// C origin: `F_Ticker` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs` reaches the upstream name through the root shim.
#[doc(alias = "F_Ticker")]
#[export_name = "F_Ticker"]
pub extern "C" fn ticker() {
    unsafe {
        // check for skipping
        if gamemode == d_mode::commercial && FINALE_COUNT > 50 {
            for i in 0..MAXPLAYERS {
                if players[i].cmd.buttons != 0 {
                    if gamemap == 30 { super::cast::start_cast(); }
                    else { gameaction = ga_worlddone; }
                    break;
                }
            }
        }

        FINALE_COUNT += 1;

        if let FinaleStage::Cast = FINALE_STAGE {
            super::cast::cast_ticker();
            return;
        }

        if gamemode == d_mode::commercial { return; }

        if let FinaleStage::Text = FINALE_STAGE {
            let len = super::strlen(finaletext);
            if FINALE_COUNT > (len as c_uint) * (TEXTSPEED as c_uint) + (TEXTWAIT as c_uint) {
                FINALE_COUNT = 0;
                FINALE_STAGE = FinaleStage::ArtScreen;
                wipegamestate = -1;
                if gameepisode == 3 { S_StartMusic(Mus::Bunny as c_int); }
            }
        }
    }
}

/// Dispatch to the appropriate draw function based on the current finale stage.
///
/// Called from C code in `g_game.c` every frame while `gamestate == GS_FINALE`.
/// C origin: `F_Drawer` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs` and the differential oracle reach the upstream
/// name through the root shim.
#[doc(alias = "F_Drawer")]
#[export_name = "F_Drawer"]
pub extern "C" fn drawer() {
    unsafe {
        match FINALE_STAGE {
            FinaleStage::Cast => super::cast::cast_drawer(),
            FinaleStage::Text => super::textstage::text_write(),
            FinaleStage::ArtScreen => super::artscreen::art_screen_drawer(),
        }
    }
}
