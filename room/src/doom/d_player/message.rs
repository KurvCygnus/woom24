//! The HUD message setter for the console player, invoked from menu
//! actions. Historically the body of `m_menu_shim.c` (removed upstream;
//! `doomgeneric-sys/build.rs:100` keeps the comment) -- this is the port
//! of that shim, not of anything in `d_player.h`.

use std::ffi::c_char;

use super::{consoleplayer, players};

/// Set the HUD hint message for the console player.
///
/// This function replaces the tiny `m_menu_shim.c` that previously bridged
/// the C `M_Menu` code to the player struct.  It writes `msg` into
/// `players[consoleplayer].message` so the HUD renderer can display it.
///
/// Called from C (`m_menu.c`) when a menu action produces a status message
/// (e.g., "Gamma correction OFF"); the freeze-zone caller `m_menu.rs`
/// reaches it through the upstream-name shim at the module root.
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism: zero extern declarers exist in-tree,
//* the five call sites are Rust-side via the shim). The pre-move
//* signature is `pub unsafe extern "C"` and stays unsafe
//* (signature-parity rule, p_saveg 7631dfa precedent).
///
/// # Safety
/// - `msg` must be a valid pointer to a NUL-terminated C string that remains
///   valid at least until the next game tic clears the message field.
/// - `consoleplayer` must be in the range `[0, MAXPLAYERS)` before this
///   function is called (guaranteed by the engine's startup sequence).
#[doc(alias = "M_Menu_SetPlayerMessage")]
#[export_name = "M_Menu_SetPlayerMessage"]
pub unsafe extern "C" fn set_player_message(msg: *const c_char)
{
    (*std::ptr::addr_of_mut!(players[0]).offset(consoleplayer as isize)).message =
        msg as *mut c_char;
}
