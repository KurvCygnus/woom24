//! The menu pages: the item callbacks (episode/skill/options/sound/
//! read-this/quit/detail/screen-size) and the per-page draw helpers,
//! including the pinned `quit_response` (the function-address compare's
//! anchor) and the gametic-keyed quit-sound/quip picks (NOT RNG --
//! matches the dstrings corrected record).

use std::ffi::{c_char, c_int};
use std::ptr;

use super::consts::{detail, messages, mousesens, music_vol_idx, nightmare, scrnsize, sfx_vol_idx, LINEHEIGHT};
use super::message::start_message;
use super::state::{currentMenu, detailLevel, epi, itemOn, mouseSensitivity, screenSize, showMessages};
use super::state::endstring;
use super::tables::{OptionsDef, quitsounds, quitsounds2, ReadDef1, ReadDef2, SoundDef};
use crate::c_write;
use crate::doom::d_loop::gametic;
use crate::doom::d_mode;
use crate::doom::d_player::M_Menu_SetPlayerMessage;
use crate::doom::doomstat::gameversion;
use crate::doom::dstrings::{doom1_endmsg, doom2_endmsg};
use crate::doom::g_game::{demoplayback, netgame, usergame, G_DeferedInitNew};
use crate::{i_error};
use crate::doom::i_system::I_Quit;
use crate::doom::i_timer::I_WaitVBL;
use crate::doom::m_controls::key_menu_confirm;
use crate::doom::r_main::R_SetViewSize;
use crate::doom::s_sound::{S_SetMusicVolume, S_SetSfxVolume, S_StartSound};
use crate::doom::sounds::Sfx;
use crate::doom::v_video::{patch_t, V_DrawPatchDirect};
use crate::doom::w_wad::W_CacheLumpName;

/// Draw the first "Read This" help screen and position the skull cursor.
///
/// Selects the correct WAD lump (`HELP`, `HELP1`, or `HELP2`) and skull
/// position based on `gameversion` and `gamemode`.
#[doc(alias = "M_DrawReadThis1")]
pub(super) extern "C" fn draw_read_this1() {
    unsafe {
        super::state::inhelpscreens = 1;

        let lumpname: *const c_char;
        let skullx: i16;
        let skully: i16;

        match gameversion {
            d_mode::exe_doom_1_666
            | d_mode::exe_doom_1_7
            | d_mode::exe_doom_1_8
            | d_mode::exe_doom_1_9
            | d_mode::exe_hacx => {
                if crate::doom::doomstat::gamemode == d_mode::commercial {
                    lumpname = c"HELP".as_ptr();
                    skullx = 330;
                    skully = 165;
                } else {
                    lumpname = c"HELP2".as_ptr();
                    skullx = 280;
                    skully = 185;
                }
            }
            d_mode::exe_ultimate | d_mode::exe_chex => {
                lumpname = c"HELP1".as_ptr();
                skullx = 280;
                skully = 185;
            }
            d_mode::exe_final | d_mode::exe_final2 => {
                lumpname = c"HELP".as_ptr();
                skullx = 330;
                skully = 165;
            }
            _ => {
                i_error!("Unhandled game version");
            }
        }

        V_DrawPatchDirect(0, 0, W_CacheLumpName(lumpname, 0) as *mut patch_t);
        ReadDef1.x = skullx;
        ReadDef1.y = skully;
    }
}

/// Draw the second "Read This" help screen (`HELP1` lump).
#[doc(alias = "M_DrawReadThis2")]
pub(super) extern "C" fn draw_read_this2() {
    unsafe {
        super::state::inhelpscreens = 1;
        V_DrawPatchDirect(0, 0, W_CacheLumpName(c"HELP1".as_ptr(), 0) as *mut patch_t);
    }
}

/// Draw the Sound Volume menu page: title patch and two thermometer sliders.
#[doc(alias = "M_DrawSound")]
pub(super) extern "C" fn draw_sound() {
    use crate::doom::s_sound::{musicVolume, sfxVolume};
    unsafe {
        V_DrawPatchDirect(
            60,
            38,
            W_CacheLumpName(c"M_SVOL".as_ptr(), 0) as *mut patch_t,
        );

        draw_thermo(
            SoundDef.x as c_int,
            SoundDef.y as c_int + LINEHEIGHT * (sfx_vol_idx as c_int + 1),
            16,
            sfxVolume,
        );

        draw_thermo(
            SoundDef.x as c_int,
            SoundDef.y as c_int + LINEHEIGHT * (music_vol_idx as c_int + 1),
            16,
            musicVolume,
        );
    }
}

/// No-op activation callback for the Sound Volume menu item (navigation handled elsewhere).
#[doc(alias = "M_Sound")]
pub(super) extern "C" fn sound_item(_choice: c_int) {}

/// Adjust the SFX volume slider; `choice` 0 decrements, 1 increments (range 0-15).
#[doc(alias = "M_SfxVol")]
pub(super) extern "C" fn sfx_vol(choice: c_int) {
    use crate::doom::s_sound::sfxVolume;
    unsafe {
        match choice {
            0 if sfxVolume > 0 => {
                sfxVolume -= 1;
            }
            1 if sfxVolume < 15 => {
                sfxVolume += 1;
            }
            _ => {}
        }
        S_SetSfxVolume(sfxVolume * 8);
    }
}

/// Adjust the music volume slider; `choice` 0 decrements, 1 increments (range 0-15).
#[doc(alias = "M_MusicVol")]
pub(super) extern "C" fn music_vol(choice: c_int) {
    use crate::doom::s_sound::musicVolume;
    unsafe {
        match choice {
            0 if musicVolume > 0 => {
                musicVolume -= 1;
            }
            1 if musicVolume < 15 => {
                musicVolume += 1;
            }
            _ => {}
        }
        S_SetMusicVolume(musicVolume * 8);
    }
}

/// Draw the "DOOM" title graphic at the top of the main menu.
#[doc(alias = "M_DrawMainMenu")]
pub(super) extern "C" fn draw_main_menu() {
    V_DrawPatchDirect(
        94,
        2,
        W_CacheLumpName(c"M_DOOM".as_ptr(), 0) as *mut patch_t,
    );
}

/// Draw the "New Game" and "Skill Level" title patches above the skill menu.
#[doc(alias = "M_DrawNewGame")]
pub(super) extern "C" fn draw_new_game() {
    V_DrawPatchDirect(
        96,
        14,
        W_CacheLumpName(c"M_NEWG".as_ptr(), 0) as *mut patch_t,
    );
    V_DrawPatchDirect(
        54,
        38,
        W_CacheLumpName(c"M_SKILL".as_ptr(), 0) as *mut patch_t,
    );
}

/// Handle "New Game" activation: navigate to the episode or skill menu as appropriate.
///
/// Shows an error message if in a network game (except demo playback).
/// Skips the episode menu for Doom II (commercial) and Chex Quest.
#[doc(alias = "M_NewGame")]
pub(super) extern "C" fn new_game(_choice: c_int) {
    unsafe {
        if netgame != 0 && demoplayback == 0 {
            start_message(
                c"you can't start a new game\nwhile in a network game.\n\npress a key.".as_ptr()
                    as *mut c_char,
                None,
                0,
            );
            return;
        }
        if crate::doom::doomstat::gamemode == d_mode::commercial || gameversion == d_mode::exe_chex {
            super::lifecycle::setup_next_menu(&raw mut super::tables::NewDef);
        } else {
            super::lifecycle::setup_next_menu(&raw mut super::tables::EpiDef);
        }
    }
}

/// Draw the "Which Episode?" title patch above the episode menu.
#[doc(alias = "M_DrawEpisode")]
pub(super) extern "C" fn draw_episode() {
    V_DrawPatchDirect(
        54,
        38,
        W_CacheLumpName(c"M_EPISOD".as_ptr(), 0) as *mut patch_t,
    );
}

/// Confirmation callback for the Nightmare skill prompt; starts the game if `key` confirms.
#[doc(alias = "M_VerifyNightmare")]
pub(super) extern "C" fn verify_nightmare(key: c_int) {
    unsafe {
        if key != key_menu_confirm {
            return;
        }
        G_DeferedInitNew(4, epi + 1, 1);
        super::lifecycle::clear_menus();
    }
}

/// Start a new game at the given skill level, or prompt for confirmation on Nightmare.
#[doc(alias = "M_ChooseSkill")]
pub(super) extern "C" fn choose_skill(choice: c_int) {
    if choice as usize == nightmare {
        start_message(
            c"are you sure? this skill level\nisn't even remotely fair.\n\npress y or n.".as_ptr()
                as *mut c_char,
            Some(verify_nightmare),
            1,
        );
        return;
    }
    unsafe {
        G_DeferedInitNew(choice, epi + 1, 1);
        super::lifecycle::clear_menus();
    }
}

/// Store the chosen episode index and navigate to the skill menu.
///
/// Shows a shareware-restriction message if episode > 0 in shareware mode.
/// Clamps episode 4 to episode 0 in registered mode (which only has three episodes).
#[doc(alias = "M_Episode")]
pub(super) extern "C" fn episode(choice: c_int) {
    unsafe {
        if crate::doom::doomstat::gamemode == d_mode::shareware && choice != 0 {
            start_message(
                c"this is the shareware version of doom.\n\nyou need to order the entire trilogy.\n\npress a key.".as_ptr()
                    as *mut c_char,
                None,
                0,
            );
            super::lifecycle::setup_next_menu(&raw mut ReadDef1);
            return;
        }
        if crate::doom::doomstat::gamemode == d_mode::registered && choice > 2 {
            epi = 0;
        } else {
            epi = choice;
        }
        super::lifecycle::setup_next_menu(&raw mut super::tables::NewDef);
    }
}

/// Draw the Options menu page: title patch, detail/message toggles, and thermometer sliders.
#[doc(alias = "M_DrawOptions")]
pub(super) extern "C" fn draw_options() {
    unsafe {
        V_DrawPatchDirect(
            108,
            15,
            W_CacheLumpName(c"M_OPTTTL".as_ptr(), 0) as *mut patch_t,
        );

        let detail_names: [*const c_char; 2] = [c"M_GDHIGH".as_ptr(), c"M_GDLOW".as_ptr()];
        let msg_names: [*const c_char; 2] = [c"M_MSGOFF".as_ptr(), c"M_MSGON".as_ptr()];

        V_DrawPatchDirect(
            OptionsDef.x as c_int + 175,
            OptionsDef.y as c_int + LINEHEIGHT * detail as c_int,
            W_CacheLumpName(detail_names[detailLevel as usize], 0) as *mut patch_t,
        );

        V_DrawPatchDirect(
            OptionsDef.x as c_int + 120,
            OptionsDef.y as c_int + LINEHEIGHT * messages as c_int,
            W_CacheLumpName(msg_names[showMessages as usize], 0) as *mut patch_t,
        );

        draw_thermo(
            OptionsDef.x as c_int,
            OptionsDef.y as c_int + LINEHEIGHT * (mousesens as c_int + 1),
            10,
            mouseSensitivity,
        );

        draw_thermo(
            OptionsDef.x as c_int,
            OptionsDef.y as c_int + LINEHEIGHT * (scrnsize as c_int + 1),
            9,
            screenSize,
        );
    }
}

/// No-op activation callback for the Options menu item (navigation handled by `responder`).
#[doc(alias = "M_Options")]
pub(super) extern "C" fn options_item(_choice: c_int) {}

/// Toggle the `showMessages` setting and display a confirmation HUD message.
#[doc(alias = "M_ChangeMessages")]
pub(super) extern "C" fn change_messages(_choice: c_int) {
    use crate::doom::hu_stuff::message_dontfuckwithme;
    unsafe {
        showMessages = 1 - showMessages;
        if showMessages == 0 {
            M_Menu_SetPlayerMessage(c"Messages OFF".as_ptr());
        } else {
            M_Menu_SetPlayerMessage(c"Messages ON".as_ptr());
        }
        message_dontfuckwithme = 1;
    }
}

/// Confirmation callback for "End Game"; clears menus if the player confirms.
#[doc(alias = "M_EndGameResponse")]
pub(super) extern "C" fn end_game_response(key: c_int) {
    unsafe {
        if key != key_menu_confirm {
            return;
        }
        (*currentMenu).lastOn = itemOn;
    }
    super::lifecycle::clear_menus();
}

/// Prompt the player to confirm ending the current game (suppressed if not in-game or in a network game).
#[doc(alias = "M_EndGame")]
pub(super) extern "C" fn end_game(_choice: c_int) {
    unsafe {
        if usergame == 0 {
            S_StartSound(ptr::null_mut(), Sfx::Oof as c_int);
            return;
        }
        if netgame != 0 {
            start_message(
                c"you can't end a netgame!\n\npress a key."
                    .as_ptr()
                    .cast_mut(),
                None,
                0,
            );
            return;
        }
        start_message(
            c"are you sure you want to end the game?\n\npress y or n."
                .as_ptr()
                .cast_mut(),
            Some(end_game_response),
            1,
        );
    }
}

/// No-op placeholder for the "Read This" main-menu item (navigation is in `responder`).
#[doc(alias = "M_ReadThis")]
pub(super) extern "C" fn read_this(_choice: c_int) {}

/// Handle "done" from the first help screen: advance to page 2 or finish, depending on game version.
#[doc(alias = "M_ReadThis2")]
pub(super) extern "C" fn read_this2(_choice: c_int) {
    unsafe {
        if gameversion <= d_mode::exe_doom_1_9 && crate::doom::doomstat::gamemode != d_mode::commercial {
            super::lifecycle::setup_next_menu(&raw mut ReadDef2);
        } else {
            finish_read_this(0);
        }
    }
}

/// No-op: closing the last help screen returns to the previous menu via `responder`'s back key.
#[doc(alias = "M_FinishReadThis")]
pub(super) extern "C" fn finish_read_this(_choice: c_int) {}

/// Confirmation callback for the quit prompt; plays a quit sound and exits if the player confirms.
///
/// The quit-sound pick is gametic-keyed (`(gametic >> 2) & 7`) -- the
/// demo-visible clock, NOT RNG (report §8.4).
///
/// The pre-move export symbol is kept with `#[export_name]` below (the
/// ONE brief-mandated pin addition): `responder`'s window-close
/// detection compares `messageRoutine`'s address against THIS function
/// item to detect an open quit dialog -- the compare rides the root
/// shim (`super::M_QuitResponse`), so set-site (`quit_doom`) and
/// compare-site must keep referencing the same item.
#[doc(alias = "M_QuitResponse")]
#[export_name = "M_QuitResponse"]
pub extern "C" fn quit_response(key: c_int) {
    unsafe {
        if key != key_menu_confirm {
            return;
        }
        if netgame == 0 {
            if crate::doom::doomstat::gamemode == d_mode::commercial {
                S_StartSound(ptr::null_mut(), quitsounds2[((gametic >> 2) & 7) as usize]);
            } else {
                S_StartSound(ptr::null_mut(), quitsounds[((gametic >> 2) & 7) as usize]);
            }
            I_WaitVBL(105);
        }
        I_Quit();
    }
}

/// Select a game-specific quit quip from `doom1_endmsg` or `doom2_endmsg`, keyed by `gametic`.
#[doc(alias = "M_SelectEndMessage")]
fn select_end_message() -> *const c_char {
    unsafe {
        if super::logical_gamemission() == d_mode::doom {
            doom1_endmsg[(gametic as usize) & 7].0
        } else {
            doom2_endmsg[(gametic as usize) & 7].0
        }
    }
}

/// Show the quit confirmation dialog with a game-specific quip and a y/n prompt.
#[doc(alias = "M_QuitDOOM")]
pub(super) extern "C" fn quit_doom(_choice: c_int) {
    unsafe {
        let msg = select_end_message();
        let msg_str = std::ffi::CStr::from_ptr(msg).to_string_lossy();
        c_write!(endstring, "{}\n\n(press y to quit to dos.)", msg_str);
        start_message(
            std::ptr::addr_of_mut!(endstring[0]),
            Some(quit_response),
            1,
        );
    }
}

/// Adjust mouse sensitivity; `choice` 0 decrements, 1 increments (range 0-9).
#[doc(alias = "M_ChangeSensitivity")]
pub(super) extern "C" fn change_sensitivity(choice: c_int) {
    unsafe {
        match choice {
            0 if mouseSensitivity > 0 => {
                mouseSensitivity -= 1;
            }
            1 if mouseSensitivity < 9 => {
                mouseSensitivity += 1;
            }
            _ => {}
        }
    }
}

/// Toggle graphics detail between high (0) and low (1) and display a HUD confirmation.
#[doc(alias = "M_ChangeDetail")]
pub(super) extern "C" fn change_detail(_choice: c_int) {
    unsafe {
        detailLevel = 1 - detailLevel;
        R_SetViewSize(super::state::screenblocks, detailLevel);
        if detailLevel == 0 {
            M_Menu_SetPlayerMessage(c"High detail".as_ptr());
        } else {
            M_Menu_SetPlayerMessage(c"Low detail".as_ptr());
        }
    }
}

/// Adjust the screen-size slider; `choice` 0 shrinks, 1 enlarges (range 0-8).
///
/// Updates both `screenSize` and `screenblocks`, then calls `R_SetViewSize`.
#[doc(alias = "M_SizeDisplay")]
pub(super) extern "C" fn size_display(choice: c_int) {
    unsafe {
        match choice {
            0 if screenSize > 0 => {
                super::state::screenblocks -= 1;
                screenSize -= 1;
            }
            1 if screenSize < 8 => {
                super::state::screenblocks += 1;
                screenSize += 1;
            }
            _ => {}
        }
        R_SetViewSize(super::state::screenblocks, detailLevel);
    }
}

/// Draw a thermometer slider at `(x, y)` with `thermWidth` cells and a filled dot at `thermDot`.
///
/// Renders left cap, `thermWidth` middle pieces, right cap, then the movable dot.
#[doc(alias = "M_DrawThermo")]
fn draw_thermo(x: c_int, y: c_int, thermWidth: c_int, thermDot: c_int) {
    let mut xx = x;
    V_DrawPatchDirect(
        xx,
        y,
        W_CacheLumpName(c"M_THERML".as_ptr(), 0) as *mut patch_t,
    );
    xx += 8;
    for _ in 0..thermWidth {
        V_DrawPatchDirect(
            xx,
            y,
            W_CacheLumpName(c"M_THERMM".as_ptr(), 0) as *mut patch_t,
        );
        xx += 8;
    }
    V_DrawPatchDirect(
        xx,
        y,
        W_CacheLumpName(c"M_THERMR".as_ptr(), 0) as *mut patch_t,
    );

    V_DrawPatchDirect(
        (x + 8) + thermDot * 8,
        y,
        W_CacheLumpName(c"M_THERMO".as_ptr(), 0) as *mut patch_t,
    );
}

/// No-op: draw callback for an empty (non-selected) grid cell (unused in this port).
#[doc(alias = "M_DrawEmptyCell")]
pub(super) fn draw_empty_cell(_menu: *mut super::types::menu_t, _item: c_int) {}

/// No-op: draw callback for the selected grid cell (unused in this port).
#[doc(alias = "M_DrawSelCell")]
pub(super) fn draw_sel_cell(_menu: *mut super::types::menu_t, _item: c_int) {}

