//! The menu input surface: the event responder (navigation, F-key
//! shortcuts, save-name typing, modal-message dispatch, and the
//! window-close quit-dialog detection by function address) and the
//! control-panel opener.

use std::ffi::{c_char, c_int};
use std::ptr;

use super::consts::{
    sfx_vol, EV_JOYSTICK, EV_KEYDOWN, EV_MOUSE, EV_QUIT, KEY_BACKSPACE, KEY_ENTER, KEY_ESCAPE,
};
use super::state::{
    currentMenu, itemOn, menuactive, messageLastMenuActive, messageNeedsInput, messageRoutine,
    messageToPrint, saveCharIndex, saveOldString, saveSlot, saveStringEnter, savegamestrings,
};
use super::tables::{ReadDef1, ReadDef2, SoundDef};
use super::text::{is_null_key, string_width};
use crate::types::Boolean;

use crate::doom::am_map::automapactive;
use crate::doom::d_event::event_t;
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::g_game::{testcontrols, G_ScreenShot};
use crate::doom::hu_stuff::chat_on;
use crate::doom::i_input::vanilla_keyboard_mapping;
use crate::doom::i_system::I_Quit;
use crate::doom::i_timer::I_GetTime;
use crate::doom::i_video::{usegamma, I_SetPalette};
use crate::doom::m_controls::{
    joybmenu, key_menu_abort, key_menu_activate, key_menu_back, key_menu_confirm, key_menu_decscreen,
    key_menu_detail, key_menu_down, key_menu_endgame, key_menu_forward, key_menu_gamma,
    key_menu_help, key_menu_incscreen, key_menu_left, key_menu_load, key_menu_messages,
    key_menu_qload, key_menu_qsave, key_menu_quit, key_menu_right, key_menu_save,
    key_menu_screenshot, key_menu_up, key_menu_volume,
};
use crate::doom::m_misc::M_StringCopy;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::w_wad::W_CacheLumpName;

/// Tic time after which the next joystick event will be processed (rate-limiting).
static mut RESP_joywait: c_int = 0;
/// Tic time after which the next mouse event will be processed (rate-limiting).
static mut RESP_mousewait: c_int = 0;
/// Accumulated mouse y movement since the last `RESP_lasty` reset.
static mut RESP_mousey: c_int = 0;
/// Mouse y baseline; updated in 30-unit steps to produce discrete menu scrolls.
static mut RESP_lasty: c_int = 0;
/// Accumulated mouse x movement since the last `RESP_lastx` reset.
static mut RESP_mousex: c_int = 0;
/// Mouse x baseline; updated in 30-unit steps to produce discrete menu navigation.
static mut RESP_lastx: c_int = 0;

/// Handle an input event for the menu system.
///
/// Translates joystick, mouse, and keyboard events into menu navigation actions
/// (up/down/left/right/forward/back/activate/abort) and F-key shortcuts
/// (quicksave, quickload, screen-size, gamma, screenshot, etc.).
/// Also handles save-game string editing character-by-character.
///
/// Returns `Boolean::TRUE` if the event was consumed, `Boolean::FALSE` otherwise.
/// Called by `G_Responder` in `g_game.c`.
///
/// The window-close branch's `messageRoutine == M_QuitResponse`
/// function-address compare is carried VERBATIM from the pre-split
/// flat m_menu.rs (line 1539-1543):
/// both sides reference the same `pages::quit_response` function item
/// (the set-site in `pages::quit_doom` and this compare), the identity
/// held by the root shim + the brief-mandated `#[export_name]` pin.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/events.rs` (BEFORE `G_Responder` -- the consume decision
/// changes what the simulation sees) and the oracle reach the upstream
/// name through the root shim.
#[doc(alias = "M_Responder")]
#[export_name = "M_Responder"]
pub extern "C" fn responder(ev: *mut event_t) -> Boolean {
    unsafe {
        let ev = &*ev;

        // testcontrols mode
        if testcontrols != 0 {
            if ev.type_ == EV_QUIT
                || (ev.type_ == EV_KEYDOWN
                    && (ev.data1 == key_menu_activate || ev.data1 == key_menu_quit))
            {
                I_Quit();
                return Boolean::TRUE;
            }
            return Boolean::FALSE;
        }

        // window close button
        if ev.type_ == EV_QUIT {
            if menuactive != 0
                && messageToPrint != 0
                && messageRoutine
                    .map(|f| f as usize)
                    .unwrap_or(0)
                    == super::pages::quit_response as *const () as usize
            {
                super::pages::quit_response(key_menu_confirm);
            } else {
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::pages::quit_doom(0);
            }
            return Boolean::TRUE;
        }

        let mut ch: c_int = 0;
        let mut key: c_int = -1;

        if ev.type_ == EV_JOYSTICK && RESP_joywait < I_GetTime() {
            if ev.data3 < 0 {
                key = key_menu_up;
                RESP_joywait = I_GetTime() + 5;
            } else if ev.data3 > 0 {
                key = key_menu_down;
                RESP_joywait = I_GetTime() + 5;
            }
            if ev.data2 < 0 {
                key = key_menu_left;
                RESP_joywait = I_GetTime() + 2;
            } else if ev.data2 > 0 {
                key = key_menu_right;
                RESP_joywait = I_GetTime() + 2;
            }
            if ev.data1 & 1 != 0 {
                key = key_menu_forward;
                RESP_joywait = I_GetTime() + 5;
            }
            if ev.data1 & 2 != 0 {
                key = key_menu_back;
                RESP_joywait = I_GetTime() + 5;
            }
            if joybmenu >= 0 && (ev.data1 & (1 << joybmenu)) != 0 {
                key = key_menu_activate;
                RESP_joywait = I_GetTime() + 5;
            }
        } else if ev.type_ == EV_MOUSE && RESP_mousewait < I_GetTime() {
            RESP_mousey += ev.data3;
            if RESP_mousey < RESP_lasty - 30 {
                key = key_menu_down;
                RESP_mousewait = I_GetTime() + 5;
                RESP_lasty -= 30;
                RESP_mousey = RESP_lasty;
            } else if RESP_mousey > RESP_lasty + 30 {
                key = key_menu_up;
                RESP_mousewait = I_GetTime() + 5;
                RESP_lasty += 30;
                RESP_mousey = RESP_lasty;
            }

            RESP_mousex += ev.data2;
            if RESP_mousex < RESP_lastx - 30 {
                key = key_menu_left;
                RESP_mousewait = I_GetTime() + 5;
                RESP_lastx -= 30;
                RESP_mousex = RESP_lastx;
            } else if RESP_mousex > RESP_lastx + 30 {
                key = key_menu_right;
                RESP_mousewait = I_GetTime() + 5;
                RESP_lastx += 30;
                RESP_mousex = RESP_lastx;
            }

            if ev.data1 & 1 != 0 {
                key = key_menu_forward;
                RESP_mousewait = I_GetTime() + 15;
            }
            if ev.data1 & 2 != 0 {
                key = key_menu_back;
                RESP_mousewait = I_GetTime() + 15;
            }
        } else if ev.type_ == EV_KEYDOWN {
            key = ev.data1;
            ch = ev.data2;
        }

        if key == -1 {
            return Boolean::FALSE;
        }

        // Save Game string input
        if saveStringEnter != 0 {
            if key == KEY_BACKSPACE {
                if saveCharIndex > 0 {
                    saveCharIndex -= 1;
                    savegamestrings[saveSlot as usize][saveCharIndex as usize] = 0;
                }
            } else if key == KEY_ESCAPE {
                saveStringEnter = 0;
                M_StringCopy(
                    savegamestrings[saveSlot as usize].as_mut_ptr(),
                    std::ptr::addr_of!(saveOldString[0]),
                    super::consts::SAVESTRINGSIZE,
                );
            } else if key == KEY_ENTER {
                saveStringEnter = 0;
                if savegamestrings[saveSlot as usize][0] != 0 {
                    super::saveload::do_save(saveSlot);
                }
            } else {
                if vanilla_keyboard_mapping != 0 {
                    ch = key;
                }
                ch = super::toupper(ch);

                if ch != b' ' as c_int
                    && (ch - super::consts::HU_FONTSTART < 0
                        || ch - super::consts::HU_FONTSTART >= super::consts::HU_FONTSIZE as c_int)
                {
                    return Boolean::TRUE;
                }

                if (32..=127).contains(&ch)
                    && saveCharIndex < (super::consts::SAVESTRINGSIZE as c_int - 1)
                    && string_width(savegamestrings[saveSlot as usize].as_mut_ptr())
                        < (super::consts::SAVESTRINGSIZE as c_int - 2) * 8
                {
                    savegamestrings[saveSlot as usize][saveCharIndex as usize] = ch as c_char;
                    saveCharIndex += 1;
                    savegamestrings[saveSlot as usize][saveCharIndex as usize] = 0;
                }
            }
            return Boolean::TRUE;
        }

        // Messages that need input
        if messageToPrint != 0 {
            if messageNeedsInput != 0
                && key != b' ' as c_int
                && key != KEY_ESCAPE
                && key != key_menu_confirm
                && key != key_menu_abort
            {
                return Boolean::FALSE;
            }

            menuactive = messageLastMenuActive;
            messageToPrint = 0;
            if let Some(r) = messageRoutine {
                r(key);
            }
            menuactive = 0;
            S_StartSound(ptr::null_mut(), Sfx::Swtchx as c_int);
            return Boolean::TRUE;
        }

        // Screenshot
        if (crate::doom::d_main::devparm != 0 && key == key_menu_help)
            || (key != 0 && key == key_menu_screenshot)
        {
            G_ScreenShot();
            return Boolean::TRUE;
        }

        // F-Keys (when menu not active)
        if menuactive == 0 {
            if key == key_menu_decscreen {
                if automapactive != 0 || chat_on != 0 {
                    return Boolean::FALSE;
                }
                super::pages::size_display(0);
                S_StartSound(ptr::null_mut(), Sfx::Stnmov as c_int);
                return Boolean::TRUE;
            } else if key == key_menu_incscreen {
                if automapactive != 0 || chat_on != 0 {
                    return Boolean::FALSE;
                }
                super::pages::size_display(1);
                S_StartSound(ptr::null_mut(), Sfx::Stnmov as c_int);
                return Boolean::TRUE;
            } else if key == key_menu_help {
                start_control_panel();
                if gamemode == d_mode::retail {
                    currentMenu = &raw mut ReadDef2;
                } else {
                    currentMenu = &raw mut ReadDef1;
                }
                itemOn = 0;
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                return Boolean::TRUE;
            } else if key == key_menu_save {
                start_control_panel();
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::saveload::save_game(0);
                return Boolean::TRUE;
            } else if key == key_menu_load {
                start_control_panel();
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::saveload::load_game(0);
                return Boolean::TRUE;
            } else if key == key_menu_volume {
                start_control_panel();
                currentMenu = &raw mut SoundDef;
                itemOn = sfx_vol as i16;
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                return Boolean::TRUE;
            } else if key == key_menu_detail {
                super::pages::change_detail(0);
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                return Boolean::TRUE;
            } else if key == key_menu_qsave {
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::saveload::quick_save();
                return Boolean::TRUE;
            } else if key == key_menu_endgame {
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::pages::end_game(0);
                return Boolean::TRUE;
            } else if key == key_menu_messages {
                super::pages::change_messages(0);
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                return Boolean::TRUE;
            } else if key == key_menu_qload {
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::saveload::quick_load();
                return Boolean::TRUE;
            } else if key == key_menu_quit {
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                super::pages::quit_doom(0);
                return Boolean::TRUE;
            } else if key == key_menu_gamma {
                usegamma += 1;
                if usegamma > 4 {
                    usegamma = 0;
                }
                crate::doom::d_player::M_Menu_SetPlayerMessage(
                    super::tables::gammamsg[usegamma as usize].as_ptr(),
                );
                I_SetPalette(W_CacheLumpName(c"PLAYPAL".as_ptr(), 0) as *mut u8);
                return Boolean::TRUE;
            }
        }

        // Pop-up menu?
        if menuactive == 0 {
            if key == key_menu_activate {
                start_control_panel();
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
                return Boolean::TRUE;
            }
            return Boolean::FALSE;
        }

        // Keys usable within menu
        if key == key_menu_down {
            loop {
                if itemOn + 1 > (*currentMenu).numitems - 1 {
                    itemOn = 0;
                } else {
                    itemOn += 1;
                }
                S_StartSound(ptr::null_mut(), Sfx::Pstop as c_int);
                if (*(*currentMenu).menuitems.offset(itemOn as isize)).status != -1 {
                    break;
                }
            }
            return Boolean::TRUE;
        } else if key == key_menu_up {
            loop {
                if itemOn == 0 {
                    itemOn = (*currentMenu).numitems - 1;
                } else {
                    itemOn -= 1;
                }
                S_StartSound(ptr::null_mut(), Sfx::Pstop as c_int);
                if (*(*currentMenu).menuitems.offset(itemOn as isize)).status != -1 {
                    break;
                }
            }
            return Boolean::TRUE;
        } else if key == key_menu_left {
            let item = &*(*currentMenu).menuitems.offset(itemOn as isize);
            if item.status == 2 {
                if let Some(routine) = item.routine {
                    S_StartSound(ptr::null_mut(), Sfx::Stnmov as c_int);
                    routine(0);
                }
            }
            return Boolean::TRUE;
        } else if key == key_menu_right {
            let item = &*(*currentMenu).menuitems.offset(itemOn as isize);
            if item.status == 2 {
                if let Some(routine) = item.routine {
                    S_StartSound(ptr::null_mut(), Sfx::Stnmov as c_int);
                    routine(1);
                }
            }
            return Boolean::TRUE;
        } else if key == key_menu_forward {
            let item = &*(*currentMenu).menuitems.offset(itemOn as isize);
            if let Some(routine) = item.routine {
                if item.status != 0 {
                    (*currentMenu).lastOn = itemOn;
                    if item.status == 2 {
                        routine(1);
                        S_StartSound(ptr::null_mut(), Sfx::Stnmov as c_int);
                    } else {
                        routine(itemOn as c_int);
                        S_StartSound(ptr::null_mut(), Sfx::Pistol as c_int);
                    }
                }
            }
            return Boolean::TRUE;
        } else if key == key_menu_activate {
            (*currentMenu).lastOn = itemOn;
            super::lifecycle::clear_menus();
            S_StartSound(ptr::null_mut(), Sfx::Swtchx as c_int);
            return Boolean::TRUE;
        } else if key == key_menu_back {
            (*currentMenu).lastOn = itemOn;
            if !(*currentMenu).prevMenu.is_null() {
                currentMenu = (*currentMenu).prevMenu;
                itemOn = (*currentMenu).lastOn;
                S_StartSound(ptr::null_mut(), Sfx::Swtchn as c_int);
            }
            return Boolean::TRUE;
        }

        // Keyboard shortcut
        if ch != 0 || is_null_key(key) {
            let ch_u = ch as c_char;
            for i in (itemOn + 1)..(*currentMenu).numitems {
                if (*(*currentMenu).menuitems.offset(i as isize)).alphaKey == ch_u {
                    itemOn = i;
                    S_StartSound(ptr::null_mut(), Sfx::Pstop as c_int);
                    return Boolean::TRUE;
                }
            }
            for i in 0..=itemOn {
                if (*(*currentMenu).menuitems.offset(i as isize)).alphaKey == ch_u {
                    itemOn = i;
                    S_StartSound(ptr::null_mut(), Sfx::Pstop as c_int);
                    return Boolean::TRUE;
                }
            }
        }

        Boolean::FALSE
    }
}

/// Open the main menu and set `currentMenu` to `MainDef`.
///
/// No-op if the menu is already active. Called from `responder` and from
/// game code that needs to force the menu open (e.g. after a level warp cheat).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/responder.rs` reaches the upstream name through the root
/// shim.
#[doc(alias = "M_StartControlPanel")]
#[export_name = "M_StartControlPanel"]
pub extern "C" fn start_control_panel() {
    unsafe {
        if menuactive != 0 {
            return;
        }
        menuactive = 1;
        currentMenu = &raw mut super::tables::MainDef;
        itemOn = (*currentMenu).lastOn;
    }
}
