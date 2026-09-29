//! The HUD run-time state: the loaded font array, the four
//! `#[no_mangle]` HUD globals, the widget/message statics, and the
//! outgoing chat ring.

use std::ffi::{c_char, c_int};
use std::ptr;

use super::tables::{HU_FONTSIZE, QUEUESIZE};
use crate::doom::d_player::{PlayerT, MAXPLAYERS};
use crate::doom::hu_lib::{hu_itext_t, hu_stext_t, hu_textline_t};
use crate::doom::v_video::patch_t;

/// Loaded HUD font patches, one per glyph from `'!'` to `'_'`.
///
/// Populated by [`super::hud::load_font`] from the `STCFNxxx` WAD lumps. Index
/// `i` corresponds to ASCII code `HU_FONTSTART + i`. Exported for use by
/// `st_stuff.c` and other C modules that reference the font.
#[no_mangle]
pub static mut hu_font: [*mut patch_t; HU_FONTSIZE] = [ptr::null_mut(); HU_FONTSIZE];

/// Most-recently-transmitted chat character from the local player.
///
/// Written by `chat::queue_chat_char` and read by `D_ProcessEvents` / the
/// network layer in `d_net.c`. Corresponds to `chat_char` in `hu_stuff.c`.
/// The net layer is unported/optional; the static is extern-surface
/// conservatism (report §4.2).
#[no_mangle]
pub static mut chat_char: c_char = 0;

/// Non-zero while the chat input widget is active.
///
/// Used as the `on` pointer for `w_chat`; also read by `g_game.c` to decide
/// whether typing should be consumed by the chat system. Corresponds to
/// `chat_on` in `hu_stuff.c`.
#[no_mangle]
pub static mut chat_on: c_int = 0;

/// When non-zero, the next player message is treated as critical and will not
/// be suppressed even if `showMessages` is off.
///
/// Set by the game logic (e.g., picked-up key messages) and cleared after
/// the message is displayed. Corresponds to `message_dontfuckwithme` in
/// `hu_stuff.c`. Also exported so `p_inter.c` can set it from the C side.
#[no_mangle]
pub static mut message_dontfuckwithme: c_int = 0;

/// Pointer to the console player's `PlayerT` struct, set by [`super::hud::start`].
pub(super) static mut plr: *mut PlayerT = ptr::null_mut();
/// Map-title text line widget rendered on the automap overlay.
pub(super) static mut w_title: hu_textline_t = unsafe { std::mem::zeroed() };
/// Chat input widget; visible only when [`chat_on`] is non-zero.
pub(super) static mut w_chat: hu_itext_t = unsafe { std::mem::zeroed() };
/// Permanent zero used as the `on` pointer for [`w_inputbuffer`] widgets so
/// they are never rendered to the screen.
pub(super) static mut always_off: c_int = 0;
/// Per-player chat destination byte: 1-4 target that player, 5 = broadcast.
pub(super) static mut chat_dest: [c_char; MAXPLAYERS] = [0; MAXPLAYERS];
/// Hidden input buffer widgets that accumulate incoming chat keystrokes for
/// each remote player before the message is confirmed with Enter.
pub(super) static mut w_inputbuffer: [hu_itext_t; MAXPLAYERS] = unsafe { std::mem::zeroed() };
/// Non-zero while a player message is being displayed.
/// Used as the `on` pointer for [`w_message`].
pub(super) static mut message_on: c_int = 0;
/// Non-zero when the current message must not be overridden by a non-critical
/// new message. Set when a `message_dontfuckwithme` message is displayed.
pub(super) static mut message_nottobefuckedwith: c_int = 0;
/// Scrolling player-message widget displayed at the top of the screen.
pub(super) static mut w_message: hu_stext_t = unsafe { std::mem::zeroed() };
/// Countdown timer (in tics) until the current message is hidden.
/// Reset to `HU_MSGTIMEOUT` each time a new message is posted.
pub(super) static mut message_counter: c_int = 0;
/// True after a successful [`super::hud::start`]; used by [`super::hud::start`]
/// to call [`super::hud::stop`] before re-initializing all widgets.
pub(super) static mut headsupactive: bool = false;

/// Circular buffer of outgoing chat characters, shared with the network layer.
pub(super) static mut chatchars: [c_char; QUEUESIZE] = [0; QUEUESIZE];
/// Write index into [`chatchars`]; advanced by [`super::chat::queue_chat_char`].
pub(super) static mut head: usize = 0;
/// Read index into [`chatchars`]; advanced by [`super::chat::dequeue_chat_char`].
pub(super) static mut tail: usize = 0;
