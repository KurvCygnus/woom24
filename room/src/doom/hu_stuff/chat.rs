//! The chat surface: the outgoing character ring and the keyboard
//! responder that drives chat entry, macros, and the message-refresh
//! key.

use std::ffi::{c_char, c_int};

use super::state::{
    chat_on, head, chatchars, message_counter, message_on, plr, tail, w_chat,
};
use super::tables::{chat_macros, QUEUESIZE, HU_BROADCAST};
use crate::doom::d_event::event_t;
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::doomkeys::{KEY_ENTER, KEY_ESCAPE, KEY_LALT, KEY_RALT, KEY_RSHIFT};
use crate::doom::hu_lib::{HUlib_keyInIText, HUlib_resetIText};
use crate::doom::m_controls::{key_message_refresh, key_multi_msg, key_multi_msgplayer};
use crate::doom::m_misc::M_StringCopy;

/// Enqueue a chat character into the outgoing circular buffer.
///
/// If the buffer is full (writing would overlap `tail`), posts an
/// `"[Message unsent]"` notice to the player instead of enqueuing.
/// The head index advances modulo `QUEUESIZE` (128; power-of-two bitmask).
/// Called from `responder` and the macro-expansion path.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `responder` reaches the upstream name directly.
#[doc(alias = "HU_queueChatChar")]
#[export_name = "HU_queueChatChar"]
pub extern "C" fn queue_chat_char(c: c_char) {
    unsafe {
        if ((head + 1) & (QUEUESIZE - 1)) == tail {
            (*plr).message = c"[Message unsent]".as_ptr().cast_mut();
        } else {
            chatchars[head] = c;
            head = (head + 1) & (QUEUESIZE - 1);
        }
    }
}

/// Dequeue the next chat character from the outgoing circular buffer.
///
/// Returns the character at `tail` and advances `tail` modulo `QUEUESIZE` (128).
/// Returns `0` (NUL) if the buffer is empty (`head == tail`).
/// Called by the network layer (`d_net.c`) to read queued chat characters;
/// the net layer is unported/optional (extern-surface conservatism).
///
/// The pre-move export symbol is kept with `#[export_name]` below.
#[doc(alias = "HU_dequeueChatChar")]
#[export_name = "HU_dequeueChatChar"]
pub extern "C" fn dequeue_chat_char() -> c_char {
    unsafe {
        if head != tail {
            let c = chatchars[tail];
            tail = (tail + 1) & (QUEUESIZE - 1);
            c
        } else {
            0
        }
    }
}

/// Handle a keyboard event for the HUD system.
///
/// Returns 1 if the event was consumed (and should not be passed to the game),
/// 0 otherwise. Handles the following cases:
/// * Shift, RAlt, LAlt: track modifier state; not consumed.
/// * Non-keydown events: ignored (return 0).
/// * When chat is off: message-refresh key, broadcast-chat key, and
///   player-targeted chat keys (multiplayer only).
/// * When chat is on: Alt+digit sends a chat macro; other printable keys are
///   fed to the chat input widget and queued for transmission; Enter confirms
///   and sends the message; Escape cancels.
///
/// Called from `G_Responder` in `g_game.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game/responder.rs:20,121` imports the upstream name through the
/// root shim.
#[doc(alias = "HU_Responder")]
#[export_name = "HU_Responder"]
pub extern "C" fn responder(ev: *mut event_t) -> c_int {
    unsafe {
        static mut lastmessage: [c_char; 81] = [0; 81];
        static mut altdown: bool = false;
        static mut num_nobrainers: c_int = 0;

        let ev = &mut *ev;

        let mut numplayers = 0;
        for i in 0..MAXPLAYERS {
            if super::playeringame[i] != 0 {
                numplayers += 1;
            }
        }

        if ev.data1 == KEY_RSHIFT as c_int {
            return 0;
        } else if ev.data1 == KEY_RALT as c_int || ev.data1 == KEY_LALT as c_int {
            altdown = ev.type_ == 0; // ev_keydown
            return 0;
        }

        if ev.type_ != 0 {
            // not ev_keydown
            return 0;
        }

        let mut eatkey = false;

        if chat_on == 0 {
            if ev.data1 == key_message_refresh {
                message_on = 1;
                message_counter = super::tables::HU_MSGTIMEOUT;
                eatkey = true;
            } else if super::netgame != 0 && ev.data2 == key_multi_msg {
                eatkey = true;
                chat_on = 1;
                HUlib_resetIText(&raw mut w_chat);
                queue_chat_char(HU_BROADCAST as c_char);
            } else if super::netgame != 0 && numplayers > 2 {
                for i in 0..MAXPLAYERS {
                    if ev.data2 == key_multi_msgplayer[i] {
                        if super::playeringame[i] != 0 && i != super::consoleplayer as usize {
                            eatkey = true;
                            chat_on = 1;
                            HUlib_resetIText(&raw mut w_chat);
                            queue_chat_char((i + 1) as c_char);
                            break;
                        } else if i == super::consoleplayer as usize {
                            num_nobrainers += 1;
                            (*plr).message = match num_nobrainers {
                                1..=2 => c"You mumble to yourself".as_ptr().cast_mut(),
                                3..=5 => c"Who's there?".as_ptr().cast_mut(),
                                6..=8 => c"You scare yourself".as_ptr().cast_mut(),
                                9..=31 => c"You start to rave".as_ptr().cast_mut(),
                                _ => c"You've lost it...".as_ptr().cast_mut(),
                            };
                        }
                    }
                }
            }
        } else {
            // send a macro
            if altdown {
                let c = (ev.data1 - b'0' as c_int) as u8;
                if c > 9 {
                    return 0;
                }
                let macromessage = chat_macros[c as usize];
                queue_chat_char(KEY_ENTER as c_char);
                let mut p = macromessage;
                while *p != 0 {
                    queue_chat_char(*p);
                    p = p.add(1);
                }
                queue_chat_char(KEY_ENTER as c_char);
                chat_on = 0;
                M_StringCopy(std::ptr::addr_of_mut!(lastmessage[0]), macromessage, 81);
                (*plr).message = std::ptr::addr_of_mut!(lastmessage[0]);
                eatkey = true;
            } else {
                let c = ev.data2 as u8;
                eatkey = HUlib_keyInIText(&raw mut w_chat, c) != 0;
                if eatkey {
                    queue_chat_char(c as c_char);
                }
                if c == KEY_ENTER {
                    chat_on = 0;
                    if w_chat.l.len != 0 {
                        M_StringCopy(
                            std::ptr::addr_of_mut!(lastmessage[0]),
                            std::ptr::addr_of_mut!(w_chat.l.l[0]),
                            81,
                        );
                        (*plr).message = std::ptr::addr_of_mut!(lastmessage[0]);
                    }
                } else if c == KEY_ESCAPE {
                    chat_on = 0;
                }
            }
        }

        if eatkey {
            1
        } else {
            0
        }
    }
}
