//! The config-binding statics: the 109 `#[no_mangle] pub static mut`
//! keyboard / mouse / joystick binding defaults, keeping their upstream C
//! names and initializers byte-for-byte (`.cfg` compat surface).

#![allow(non_upper_case_globals, static_mut_refs)]

use std::ffi::c_int;

use crate::doom::doomkeys::{
    KEY_BACKSPACE, KEY_DEL, KEY_DOWNARROW, KEY_END, KEY_ENTER, KEY_EQUALS, KEY_ESCAPE, KEY_F1,
    KEY_F10, KEY_F11, KEY_F12, KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9,
    KEY_HOME, KEY_INS, KEY_LEFTARROW, KEY_MINUS, KEY_PAUSE, KEY_PGDN, KEY_PGUP, KEY_RALT,
    KEY_RIGHTARROW, KEY_RSHIFT, KEY_STRAFE_L, KEY_STRAFE_R, KEY_TAB, KEY_UPARROW, KEY_USE,
};

//
// Keyboard controls
//

/// Turn right (default: right arrow).
#[no_mangle]
pub static mut key_right: c_int = KEY_RIGHTARROW as c_int;
/// Turn left (default: left arrow).
#[no_mangle]
pub static mut key_left: c_int = KEY_LEFTARROW as c_int;
/// Move forward (default: up arrow).
#[no_mangle]
pub static mut key_up: c_int = KEY_UPARROW as c_int;
/// Move backward (default: down arrow).
#[no_mangle]
pub static mut key_down: c_int = KEY_DOWNARROW as c_int;
/// Strafe left (default: comma `KEY_STRAFE_L`).
#[no_mangle]
pub static mut key_strafeleft: c_int = KEY_STRAFE_L as c_int;
/// Strafe right (default: period `KEY_STRAFE_R`).
#[no_mangle]
pub static mut key_straferight: c_int = KEY_STRAFE_R as c_int;
/// Fire current weapon (default: `KEY_FIRE`, the right Ctrl).
#[no_mangle]
pub static mut key_fire: c_int = crate::doom::doomkeys::KEY_FIRE as c_int;
/// Use / open / activate trigger (default: space `KEY_USE`).
#[no_mangle]
pub static mut key_use: c_int = KEY_USE as c_int;
/// Hold to convert turn into strafe (default: right Alt).
#[no_mangle]
pub static mut key_strafe: c_int = KEY_RALT as c_int;
/// Hold to run (default: right Shift).
#[no_mangle]
pub static mut key_speed: c_int = KEY_RSHIFT as c_int;

//
// Heretic keyboard controls
//

/// Heretic: fly up (default: Page Up).
#[no_mangle]
pub static mut key_flyup: c_int = KEY_PGUP as c_int;
/// Heretic: fly down (default: Insert).
#[no_mangle]
pub static mut key_flydown: c_int = KEY_INS as c_int;
/// Heretic: stop flying / centre vertically (default: Home).
#[no_mangle]
pub static mut key_flycenter: c_int = KEY_HOME as c_int;

/// Heretic: look up (default: Page Down).
#[no_mangle]
pub static mut key_lookup: c_int = KEY_PGDN as c_int;
/// Heretic: look down (default: Delete).
#[no_mangle]
pub static mut key_lookdown: c_int = KEY_DEL as c_int;
/// Heretic: centre vertical look (default: End).
#[no_mangle]
pub static mut key_lookcenter: c_int = KEY_END as c_int;

/// Heretic/Strife: cycle inventory left (default: `[`; Strife uses Insert).
#[no_mangle]
pub static mut key_invleft: c_int = b'[' as c_int;
/// Heretic/Strife: cycle inventory right (default: `]`; Strife uses Delete).
#[no_mangle]
pub static mut key_invright: c_int = b']' as c_int;
/// Heretic: use selected inventory artifact (default: Enter).
#[no_mangle]
pub static mut key_useartifact: c_int = KEY_ENTER as c_int;

//
// Hexen key controls
//

/// Hexen/Strife: jump (default: `/`; Strife rebinds to `a`).
#[no_mangle]
pub static mut key_jump: c_int = b'/' as c_int;

/// Hexen: use all artifacts of selected kind (default: Backspace).
#[no_mangle]
pub static mut key_arti_all: c_int = KEY_BACKSPACE as c_int;
/// Hexen: use Quartz Flask / Mystic Urn (default: `\\`).
#[no_mangle]
pub static mut key_arti_health: c_int = b'\\' as c_int;
/// Hexen: use poison bag artifact (default: `0`).
#[no_mangle]
pub static mut key_arti_poisonbag: c_int = b'0' as c_int;
/// Hexen: use disc of repulsion (default: `9`).
#[no_mangle]
pub static mut key_arti_blastradius: c_int = b'9' as c_int;
/// Hexen: use teleport artifact (default: `8`).
#[no_mangle]
pub static mut key_arti_teleport: c_int = b'8' as c_int;
/// Hexen: use banishment device (default: `7`).
#[no_mangle]
pub static mut key_arti_teleportother: c_int = b'7' as c_int;
/// Hexen: use porkalator (default: `6`).
#[no_mangle]
pub static mut key_arti_egg: c_int = b'6' as c_int;
/// Hexen: use icon of the defender (default: `5`).
#[no_mangle]
pub static mut key_arti_invulnerability: c_int = b'5' as c_int;

//
// Strife key controls
//

/// Strife: use a medkit from inventory (default: `h`).
#[no_mangle]
pub static mut key_usehealth: c_int = b'h' as c_int;
/// Strife: query selected NPC / object (default: `q`).
#[no_mangle]
pub static mut key_invquery: c_int = b'q' as c_int;
/// Strife: open mission log (default: `w`).
#[no_mangle]
pub static mut key_mission: c_int = b'w' as c_int;
/// Strife: open the inventory popup (default: `z`).
#[no_mangle]
pub static mut key_invpop: c_int = b'z' as c_int;
/// Strife: open the keys popup (default: `k`).
#[no_mangle]
pub static mut key_invkey: c_int = b'k' as c_int;
/// Strife: cycle to the first inventory item (default: Home).
#[no_mangle]
pub static mut key_invhome: c_int = KEY_HOME as c_int;
/// Strife: cycle to the last inventory item (default: End).
#[no_mangle]
pub static mut key_invend: c_int = KEY_END as c_int;
/// Strife: use selected inventory item (default: Enter).
#[no_mangle]
pub static mut key_invuse: c_int = KEY_ENTER as c_int;
/// Strife: drop selected inventory item (default: Backspace).
#[no_mangle]
pub static mut key_invdrop: c_int = KEY_BACKSPACE as c_int;

//
// Mouse controls
//

/// Mouse button bound to fire (default: button 0).
#[no_mangle]
pub static mut mousebfire: c_int = 0;
/// Mouse button bound to strafe (default: button 1).
#[no_mangle]
pub static mut mousebstrafe: c_int = 1;
/// Mouse button bound to move forward (default: button 2).
#[no_mangle]
pub static mut mousebforward: c_int = 2;

/// Mouse button bound to jump (default: -1 = unbound; Hexen/Strife only).
#[no_mangle]
pub static mut mousebjump: c_int = -1;

/// Mouse button bound to strafe left (default: -1 = unbound).
#[no_mangle]
pub static mut mousebstrafeleft: c_int = -1;
/// Mouse button bound to strafe right (default: -1 = unbound).
#[no_mangle]
pub static mut mousebstraferight: c_int = -1;
/// Mouse button bound to move backward (default: -1 = unbound).
#[no_mangle]
pub static mut mousebbackward: c_int = -1;
/// Mouse button bound to "use" (default: -1 = unbound).
#[no_mangle]
pub static mut mousebuse: c_int = -1;

/// Mouse button bound to previous weapon (default: -1 = unbound).
#[no_mangle]
pub static mut mousebprevweapon: c_int = -1;
/// Mouse button bound to next weapon (default: -1 = unbound).
#[no_mangle]
pub static mut mousebnextweapon: c_int = -1;

/// Re-display last HUD message (default: Enter).
#[no_mangle]
pub static mut key_message_refresh: c_int = KEY_ENTER as c_int;
/// Pause the game (default: Pause).
#[no_mangle]
pub static mut key_pause: c_int = KEY_PAUSE as c_int;
/// Abort current demo playback (default: `q`).
#[no_mangle]
pub static mut key_demo_quit: c_int = b'q' as c_int;
/// Spy on next player in multiplayer (default: F12).
#[no_mangle]
pub static mut key_spy: c_int = KEY_F12 as c_int;

// Multiplayer chat keys:

/// Send chat to all players (default: `t`).
#[no_mangle]
pub static mut key_multi_msg: c_int = b't' as c_int;
/// Per-player private-chat keys (default: 0 / unbound for each).
/// Indexed by player number 0..7; populated by [`M_BindChatControls`].
#[no_mangle]
pub static mut key_multi_msgplayer: [c_int; 8] = [0; 8];

// Weapon selection keys:

/// Select weapon slot 1 (default: `1`).
#[no_mangle]
pub static mut key_weapon1: c_int = b'1' as c_int;
/// Select weapon slot 2 (default: `2`).
#[no_mangle]
pub static mut key_weapon2: c_int = b'2' as c_int;
/// Select weapon slot 3 (default: `3`).
#[no_mangle]
pub static mut key_weapon3: c_int = b'3' as c_int;
/// Select weapon slot 4 (default: `4`).
#[no_mangle]
pub static mut key_weapon4: c_int = b'4' as c_int;
/// Select weapon slot 5 (default: `5`).
#[no_mangle]
pub static mut key_weapon5: c_int = b'5' as c_int;
/// Select weapon slot 6 (default: `6`).
#[no_mangle]
pub static mut key_weapon6: c_int = b'6' as c_int;
/// Select weapon slot 7 (default: `7`).
#[no_mangle]
pub static mut key_weapon7: c_int = b'7' as c_int;
/// Select weapon slot 8 (default: `8`).
#[no_mangle]
pub static mut key_weapon8: c_int = b'8' as c_int;
/// Cycle to previous weapon (default: 0 = unbound).
#[no_mangle]
pub static mut key_prevweapon: c_int = 0;
/// Cycle to next weapon (default: 0 = unbound).
#[no_mangle]
pub static mut key_nextweapon: c_int = 0;

// Map control keys:

/// Automap: pan north (default: up arrow).
#[no_mangle]
pub static mut key_map_north: c_int = KEY_UPARROW as c_int;
/// Automap: pan south (default: down arrow).
#[no_mangle]
pub static mut key_map_south: c_int = KEY_DOWNARROW as c_int;
/// Automap: pan east (default: right arrow).
#[no_mangle]
pub static mut key_map_east: c_int = KEY_RIGHTARROW as c_int;
/// Automap: pan west (default: left arrow).
#[no_mangle]
pub static mut key_map_west: c_int = KEY_LEFTARROW as c_int;
/// Automap: zoom in (default: `=`).
#[no_mangle]
pub static mut key_map_zoomin: c_int = b'=' as c_int;
/// Automap: zoom out (default: `-`).
#[no_mangle]
pub static mut key_map_zoomout: c_int = b'-' as c_int;
/// Toggle automap on/off (default: Tab).
#[no_mangle]
pub static mut key_map_toggle: c_int = KEY_TAB as c_int;
/// Automap: fully zoom out / max zoom (default: `0`).
#[no_mangle]
pub static mut key_map_maxzoom: c_int = b'0' as c_int;
/// Automap: toggle follow-player mode (default: `f`).
#[no_mangle]
pub static mut key_map_follow: c_int = b'f' as c_int;
/// Automap: toggle grid overlay (default: `g`).
#[no_mangle]
pub static mut key_map_grid: c_int = b'g' as c_int;
/// Automap: drop a marker at the cursor (default: `m`).
#[no_mangle]
pub static mut key_map_mark: c_int = b'm' as c_int;
/// Automap: clear all markers (default: `c`).
#[no_mangle]
pub static mut key_map_clearmark: c_int = b'c' as c_int;

// menu keys:

/// Toggle the in-game menu (default: Escape).
#[no_mangle]
pub static mut key_menu_activate: c_int = KEY_ESCAPE as c_int;
/// Menu: move cursor up (default: up arrow).
#[no_mangle]
pub static mut key_menu_up: c_int = KEY_UPARROW as c_int;
/// Menu: move cursor down (default: down arrow).
#[no_mangle]
pub static mut key_menu_down: c_int = KEY_DOWNARROW as c_int;
/// Menu: decrease slider value / left (default: left arrow).
#[no_mangle]
pub static mut key_menu_left: c_int = KEY_LEFTARROW as c_int;
/// Menu: increase slider value / right (default: right arrow).
#[no_mangle]
pub static mut key_menu_right: c_int = KEY_RIGHTARROW as c_int;
/// Menu: go back one screen (default: Backspace).
#[no_mangle]
pub static mut key_menu_back: c_int = KEY_BACKSPACE as c_int;
/// Menu: select / activate item (default: Enter).
#[no_mangle]
pub static mut key_menu_forward: c_int = KEY_ENTER as c_int;
/// Menu: confirm `Yes` (default: `y`).
#[no_mangle]
pub static mut key_menu_confirm: c_int = b'y' as c_int;
/// Menu: confirm `No` / cancel (default: `n`).
#[no_mangle]
pub static mut key_menu_abort: c_int = b'n' as c_int;

/// Function shortcut: Help / read this (default: F1).
#[no_mangle]
pub static mut key_menu_help: c_int = KEY_F1 as c_int;
/// Function shortcut: Save Game menu (default: F2).
#[no_mangle]
pub static mut key_menu_save: c_int = KEY_F2 as c_int;
/// Function shortcut: Load Game menu (default: F3).
#[no_mangle]
pub static mut key_menu_load: c_int = KEY_F3 as c_int;
/// Function shortcut: Volume menu (default: F4).
#[no_mangle]
pub static mut key_menu_volume: c_int = KEY_F4 as c_int;
/// Function shortcut: toggle detail level (default: F5).
#[no_mangle]
pub static mut key_menu_detail: c_int = KEY_F5 as c_int;
/// Function shortcut: quick save (default: F6).
#[no_mangle]
pub static mut key_menu_qsave: c_int = KEY_F6 as c_int;
/// Function shortcut: end current game (default: F7).
#[no_mangle]
pub static mut key_menu_endgame: c_int = KEY_F7 as c_int;
/// Function shortcut: toggle HUD messages (default: F8).
#[no_mangle]
pub static mut key_menu_messages: c_int = KEY_F8 as c_int;
/// Function shortcut: quick load (default: F9).
#[no_mangle]
pub static mut key_menu_qload: c_int = KEY_F9 as c_int;
/// Function shortcut: quit to OS (default: F10).
#[no_mangle]
pub static mut key_menu_quit: c_int = KEY_F10 as c_int;
/// Function shortcut: cycle gamma correction (default: F11).
#[no_mangle]
pub static mut key_menu_gamma: c_int = KEY_F11 as c_int;

/// Increase screen size / shrink HUD borders (default: `=`).
#[no_mangle]
pub static mut key_menu_incscreen: c_int = KEY_EQUALS as c_int;
/// Decrease screen size / grow HUD borders (default: `-`).
#[no_mangle]
pub static mut key_menu_decscreen: c_int = KEY_MINUS as c_int;
/// Take a screenshot (default: 0 = unbound; Vanilla does not bind).
#[no_mangle]
pub static mut key_menu_screenshot: c_int = 0;

//
// Joystick controls
//

/// Joystick button bound to fire (default: button 0).
#[no_mangle]
pub static mut joybfire: c_int = 0;
/// Joystick button bound to strafe modifier (default: button 1).
#[no_mangle]
pub static mut joybstrafe: c_int = 1;
/// Joystick button bound to use (default: button 3).
#[no_mangle]
pub static mut joybuse: c_int = 3;
/// Joystick button bound to run / speed modifier (default: button 2).
#[no_mangle]
pub static mut joybspeed: c_int = 2;

/// Joystick button bound to strafe left (default: -1 = unbound).
#[no_mangle]
pub static mut joybstrafeleft: c_int = -1;
/// Joystick button bound to strafe right (default: -1 = unbound).
#[no_mangle]
pub static mut joybstraferight: c_int = -1;

/// Joystick button bound to jump (default: -1 = unbound; Hexen/Strife only).
#[no_mangle]
pub static mut joybjump: c_int = -1;

/// Joystick button bound to previous weapon (default: -1 = unbound).
#[no_mangle]
pub static mut joybprevweapon: c_int = -1;
/// Joystick button bound to next weapon (default: -1 = unbound).
#[no_mangle]
pub static mut joybnextweapon: c_int = -1;

/// Joystick button bound to activate menu (default: -1 = unbound).
#[no_mangle]
pub static mut joybmenu: c_int = -1;

// Control whether if a mouse button is double clicked, it acts like
// "use" has been pressed

/// If non-zero, a mouse double-click acts as a "use" press (default: 1).
#[no_mangle]
pub static mut dclick_use: c_int = 1;

/// Baseline pin written before the graduation move (F10 wave F2-b): a
/// defaults-parity snapshot of upstream initializers, read pre-bind
/// through `addr_of!` (no mutable-static references).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doom::doomkeys::KEY_RIGHTARROW;

    /// The C initializers must survive the split byte-for-byte:
    /// `key_right = KEY_RIGHTARROW`, `dclick_use = 1`, `joybmenu = -1`,
    /// and the eight per-player chat keys start at `0`.
    #[test]
    fn defaults_parity_snapshot() {
        unsafe {
            assert_eq!(std::ptr::addr_of!(key_right).read(), KEY_RIGHTARROW as c_int);
            assert_eq!(std::ptr::addr_of!(dclick_use).read(), 1);
            assert_eq!(std::ptr::addr_of!(joybmenu).read(), -1);
            assert_eq!(std::ptr::addr_of!(key_multi_msgplayer).read(), [0; 8]);
        }
    }
}
