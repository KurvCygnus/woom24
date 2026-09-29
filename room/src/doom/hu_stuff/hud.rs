//! The HUD lifecycle and per-frame/per-tic drivers: font load, widget
//! construction, draw/erase, the message ticker, and the netgame chat
//! pump.

use std::ffi::{c_char, c_int};
use std::ptr;

use super::logical_gamemission;
use super::short_swap;
use super::state::{
    chat_dest, chat_on, headsupactive, hu_font, message_counter, message_dontfuckwithme,
    message_nottobefuckedwith, message_on, plr, w_chat, w_inputbuffer, w_message, w_title,
};
use super::tables::{
    HU_BROADCAST, HU_FONTSTART, HU_FONTSIZE, HU_MSGHEIGHT, HU_MSGTIMEOUT, HU_MSGX, HU_MSGY,
};
use super::tables::{mapnames, mapnames_commercial};
use crate::doom::d_mode;
use crate::doom::doomstat::gameversion;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::hu_lib::{
    HUlib_addCharToTextLine, HUlib_addMessageToSText, HUlib_drawIText, HUlib_drawSText,
    HUlib_drawTextLine, HUlib_eraseIText, HUlib_eraseSText, HUlib_eraseTextLine,
    HUlib_initIText, HUlib_initSText, HUlib_initTextLine, HUlib_keyInIText, HUlib_resetIText,
};
use crate::doom::sounds::Sfx;
use crate::doom::v_video::patch_t;
use crate::doom::z_zone::PU_STATIC;

/// Load the HUD font patches from the WAD and populate [`hu_font`].
///
/// Must be called once at startup before any other `HU_*` function. Iterates
/// over `HU_FONTSIZE` slots, caching the `STCFNxxx` lump for each character
/// from `HU_FONTSTART` to `HU_FONTEND` as `PU_STATIC`.
/// Called from `D_DoomMain` in `d_main.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs:28,504` imports the upstream name through the root
/// shim and the differential oracle declares the symbol directly.
#[doc(alias = "HU_Init")]
#[export_name = "HU_Init"]
pub extern "C" fn load_font() {
    unsafe {
        let mut j = HU_FONTSTART as c_int;
        for i in 0..HU_FONTSIZE {
            let name = format!("STCFN{:03}\0", j);
            hu_font[i] = super::W_CacheLumpName(name.as_ptr() as *const c_char, PU_STATIC)
                as *mut patch_t;
            j += 1;
        }
    }
}

/// Mark the heads-up display as inactive.
///
/// Sets `headsupactive` to false. Called by [`start`] before
/// re-initializing widgets, and may also be called directly when the HUD
/// should be torn down (e.g., during intermission).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `start` reaches the upstream name directly.
#[doc(alias = "HU_Stop")]
#[export_name = "HU_Stop"]
pub extern "C" fn stop() {
    unsafe {
        headsupactive = false;
    }
}

/// Initialize all HUD widgets for the current level.
///
/// Calls [`stop`] if already active, then creates and positions the
/// message widget (`w_message`), title widget (`w_title`), chat input widget
/// (`w_chat`), and per-player input buffer widgets. The map title string is
/// looked up from [`mapnames`] / [`mapnames_commercial`] based on the current
/// game mission and map number.
///
/// Called from `G_DoLoadLevel` in `g_game.c` at the start of every level.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `p_mobj/mapthings.rs:15,130` imports the upstream name through the
/// root shim.
#[doc(alias = "HU_Start")]
#[export_name = "HU_Start"]
pub extern "C" fn start() {
    unsafe {
        if headsupactive {
            stop();
        }

        plr = std::ptr::addr_of_mut!(crate::doom::d_player::players[0])
            .offset(super::consoleplayer as isize);
        message_on = 0;
        message_dontfuckwithme = 0;
        message_nottobefuckedwith = 0;
        chat_on = 0;

        // create the message widget
        HUlib_initSText(
            &raw mut w_message,
            HU_MSGX,
            HU_MSGY,
            HU_MSGHEIGHT,
            std::ptr::addr_of_mut!(hu_font[0]),
            HU_FONTSTART as c_int,
            &raw mut message_on,
        );

        // compute font height for title/input placement
        let font_h = if !hu_font[0].is_null() {
            short_swap((*hu_font[0]).height) as c_int
        } else {
            0
        };
        let title_y = 167 - font_h;

        // create the map title widget
        HUlib_initTextLine(
            &raw mut w_title,
            0,
            title_y,
            std::ptr::addr_of_mut!(hu_font[0]),
            HU_FONTSTART as c_int,
        );

        let mut s: *mut c_char = match logical_gamemission() {
            d_mode::doom => mapnames[((super::gameepisode - 1) * 9 + super::gamemap - 1) as usize],
            d_mode::doom2 => mapnames_commercial[(super::gamemap - 1) as usize],
            d_mode::pack_plut => mapnames_commercial[(super::gamemap - 1 + 32) as usize],
            d_mode::pack_tnt => mapnames_commercial[(super::gamemap - 1 + 64) as usize],
            _ => c"Unknown level".as_ptr().cast_mut(),
        };

        if gameversion == d_mode::exe_chex {
            s = mapnames[(super::gamemap - 1) as usize];
        }

        // dehacked substitution is identity in this build
        while *s != 0 {
            HUlib_addCharToTextLine(&raw mut w_title, *s);
            s = s.add(1);
        }

        // create the chat widget
        let input_y = HU_MSGY + HU_MSGHEIGHT * (font_h + 1);
        HUlib_initIText(
            &raw mut w_chat,
            HU_MSGX,
            input_y,
            std::ptr::addr_of_mut!(hu_font[0]),
            HU_FONTSTART as c_int,
            &raw mut chat_on,
        );

        // create the inputbuffer widgets
        for i in 0..MAXPLAYERS {
            HUlib_initIText(
                &mut w_inputbuffer[i],
                0,
                0,
                ptr::null_mut(),
                0,
                &raw mut super::state::always_off,
            );
        }

        headsupactive = true;
    }
}

/// Render all visible HUD elements for the current frame.
///
/// Draws the scrolling message widget, the chat input widget, and (if the
/// automap is active) the map title line. Called once per frame from the
/// renderer after the view has been rendered.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/display.rs:22,114` imports the upstream name through the
/// root shim and the differential oracle declares the symbol directly.
#[doc(alias = "HU_Drawer")]
#[export_name = "HU_Drawer"]
pub extern "C" fn drawer() {
    unsafe {
        HUlib_drawSText(&raw mut w_message);
        HUlib_drawIText(&raw mut w_chat);
        if super::automapactive != 0 {
            HUlib_drawTextLine(&raw mut w_title, 0);
        }
    }
}

/// Erase all HUD elements from the screen buffer.
///
/// Delegates to the erase functions for `w_message`, `w_chat`, and `w_title`.
/// Called once per frame just before the view is rendered, so that old HUD
/// graphics are removed before the new frame is drawn.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. Vendor `d_main.c:208`
/// calls it (`if (gamestate == GS_LEVEL && gametic) HU_Erase();`) but
/// the ported `d_main/display.rs` never did.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the differential oracle declares the symbol directly.
#[doc(alias = "HU_Erase")]
#[export_name = "HU_Erase"]
pub extern "C" fn erase() {
    HUlib_eraseSText(&raw mut w_message);
    HUlib_eraseIText(&raw mut w_chat);
    HUlib_eraseTextLine(&raw mut w_title);
}

/// Advance the HUD state by one game tic.
///
/// Performs three tasks each tic:
/// 1. Counts down `message_counter`; clears `message_on` and
///    `message_nottobefuckedwith` when it reaches zero.
/// 2. Posts a new player message from `(*plr).message` to `w_message` if
///    `showMessages` is on (or `message_dontfuckwithme` is set) and the
///    message has not already been displayed.
/// 3. In networked games, reads incoming chat characters from each remote
///    player's `cmd.chatchar`, feeds them into the corresponding input buffer
///    widget, and displays the completed message when Enter is received.
///
/// Called from `G_Ticker` in `g_game.c` once per game tic.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/ticker.rs:23,220` imports the upstream name through the
/// root shim.
#[doc(alias = "HU_Ticker")]
#[export_name = "HU_Ticker"]
pub extern "C" fn ticker() {
    unsafe {
        // tick down message counter if message is up
        if message_counter != 0 {
            message_counter -= 1;
            if message_counter == 0 {
                message_on = 0;
                message_nottobefuckedwith = 0;
            }
        }

        if super::showMessages != 0 || message_dontfuckwithme != 0 {
            // display message if necessary
            if (!(*plr).message.is_null() && message_nottobefuckedwith == 0)
                || (!(*plr).message.is_null() && message_dontfuckwithme != 0)
            {
                HUlib_addMessageToSText(&raw mut w_message, ptr::null_mut(), (*plr).message);
                (*plr).message = ptr::null_mut();
                message_on = 1;
                message_counter = HU_MSGTIMEOUT;
                message_nottobefuckedwith = message_dontfuckwithme;
                message_dontfuckwithme = 0;
            }
        }

        // check for incoming chat characters
        if super::netgame != 0 {
            for i in 0..MAXPLAYERS {
                if super::playeringame[i] == 0 {
                    continue;
                }
                if i != super::consoleplayer as usize {
                    let c = (*plr.add(i)).cmd.chatchar;
                    if c != 0 {
                        if c <= HU_BROADCAST as u8 {
                            chat_dest[i] = c as c_char;
                        } else {
                            let rc = HUlib_keyInIText(&mut w_inputbuffer[i], c);
                            if rc != 0 && c == crate::doom::doomkeys::KEY_ENTER {
                                if w_inputbuffer[i].l.len != 0
                                    && (chat_dest[i] == (super::consoleplayer + 1) as c_char
                                        || chat_dest[i] == HU_BROADCAST as c_char)
                                {
                                    HUlib_addMessageToSText(
                                        &raw mut w_message,
                                        super::player_names[i],
                                        w_inputbuffer[i].l.l.as_mut_ptr(),
                                    );
                                    message_nottobefuckedwith = 1;
                                    message_on = 1;
                                    message_counter = HU_MSGTIMEOUT;
                                    if super::gamemode == d_mode::commercial {
                                        super::S_StartSound(ptr::null_mut(), Sfx::Radio as c_int);
                                    } else {
                                        super::S_StartSound(ptr::null_mut(), Sfx::Tink as c_int);
                                    }
                                }
                                HUlib_resetIText(&mut w_inputbuffer[i]);
                            }
                        }
                        (*plr.add(i)).cmd.chatchar = 0;
                    }
                }
            }
        }
    }
}
