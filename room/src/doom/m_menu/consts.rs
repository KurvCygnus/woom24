//! Menu constants: the event/key codes and the per-menu item indices
//! (the C anonymous enums of m_menu.c).

use std::ffi::c_int;

/// Vertical pixel stride between adjacent menu items (matches C `LINEHEIGHT`).
pub(super) const LINEHEIGHT: c_int = 16;
/// Horizontal offset of the skull cursor relative to the menu item's x position (matches C `SKULLXOFF`).
pub(super) const SKULLXOFF: c_int = -32;

/// ASCII code of the first character in the HUD font (matches C `HU_FONTSTART`).
pub(super) const HU_FONTSTART: c_int = b'!' as c_int;
/// ASCII code of the last character in the HUD font (matches C `HU_FONTEND`).
pub(super) const HU_FONTEND: c_int = b'_' as c_int;
/// Number of glyphs in the HUD font (derived from `HU_FONTSTART`/`HU_FONTEND`).
pub(super) const HU_FONTSIZE: usize = (HU_FONTEND - HU_FONTSTART + 1) as usize;

/// Maximum number of characters in a save-game description string (matches C `SAVESTRINGSIZE`).
pub(super) const SAVESTRINGSIZE: usize = 24;
/// Maximum number of simultaneous players; local alias for `d_player::MAXPLAYERS`.
const MAXPLAYERS: usize = 4;

/// Event type: key-down (matches C `ev_keydown`).
pub(super) const EV_KEYDOWN: c_int = 0;
/// Event type: key-up (matches C `ev_keyup`).
pub(super) const EV_KEYUP: c_int = 1;
/// Event type: mouse motion or button (matches C `ev_mouse`).
pub(super) const EV_MOUSE: c_int = 2;
/// Event type: joystick input (matches C `ev_joystick`).
pub(super) const EV_JOYSTICK: c_int = 3;
/// Event type: window close / quit request (matches C `ev_quit`).
pub(super) const EV_QUIT: c_int = 4;

/// Key code for the Escape key (ASCII 27).
pub(super) const KEY_ESCAPE: c_int = 27;
/// Key code for the Enter/Return key (ASCII 13).
pub(super) const KEY_ENTER: c_int = 13;
/// Key code for the Backspace key (0x7f delete).
pub(super) const KEY_BACKSPACE: c_int = 0x7f;
/// Key code for the Pause key (matches C `KEY_PAUSE`).
pub(super) const KEY_PAUSE: c_int = 0xff;
/// Key code for Caps Lock (matches C `KEY_CAPSLOCK`).
pub(super) const KEY_CAPSLOCK: c_int = 0x80 + 0x3a;
/// Key code for Num Lock (matches C `KEY_NUMLOCK`).
pub(super) const KEY_NUMLOCK: c_int = 0x80 + 0x45;
/// Key code for Scroll Lock (matches C `KEY_SCRLCK`).
pub(super) const KEY_SCRLCK: c_int = 0x80 + 0x46;

// ---------------------------------------------------------------------------
// Main menu item indices (C anonymous enums in m_menu.c)
// ---------------------------------------------------------------------------

/// Main menu: "New Game" item index.
pub(super) const newgame: usize = 0;
/// Main menu: "Options" item index.
pub(super) const options: usize = 1;
/// Main menu: "Load Game" item index.
pub(super) const loadgame: usize = 2;
/// Main menu: "Save Game" item index.
pub(super) const savegame: usize = 3;
/// Main menu: "Read This" item index.
pub(super) const readthis: usize = 4;
/// Main menu: "Quit DOOM" item index.
pub(super) const quitdoom: usize = 5;
/// Total items in the main menu.
pub(super) const main_end: usize = 6;

// ---------------------------------------------------------------------------
// Episode menu item indices
// ---------------------------------------------------------------------------

/// Episode menu: Knee-Deep in the Dead (E1).
pub(super) const ep1: usize = 0;
/// Episode menu: The Shores of Hell (E2).
pub(super) const ep2: usize = 1;
/// Episode menu: Inferno (E3).
pub(super) const ep3: usize = 2;
/// Episode menu: Thy Flesh Consumed (E4, Ultimate Doom only).
pub(super) const ep4: usize = 3;
/// Total items in the episode menu (trimmed at runtime for non-Ultimate builds).
pub(super) const ep_end: usize = 4;

// ---------------------------------------------------------------------------
// New-game / skill menu item indices
// ---------------------------------------------------------------------------

/// Skill menu: "I'm Too Young to Die" (easiest).
pub(super) const killthings: usize = 0;
/// Skill menu: "Hey, Not Too Rough".
pub(super) const toorough: usize = 1;
/// Skill menu: "Hurt Me Plenty" (default).
pub(super) const hurtme: usize = 2;
/// Skill menu: "Ultra-Violence".
pub(super) const violence: usize = 3;
/// Skill menu: "Nightmare!" (triggers confirmation prompt).
pub(super) const nightmare: usize = 4;
/// Total items in the skill menu.
pub(super) const newg_end: usize = 5;

// ---------------------------------------------------------------------------
// Options menu item indices
// ---------------------------------------------------------------------------

/// Options menu: "End Game" item index.
pub(super) const endgame: usize = 0;
/// Options menu: "Messages" toggle index.
pub(super) const messages: usize = 1;
/// Options menu: "Graphic Detail" toggle index.
pub(super) const detail: usize = 2;
/// Options menu: "Screen Size" slider index.
pub(super) const scrnsize: usize = 3;
/// Options menu: "Mouse Sensitivity" slider index.
pub(super) const mousesens: usize = 5;
/// Options menu: "Sound Volume" link index.
pub(super) const soundvol: usize = 7;
/// Total items in the options menu.
pub(super) const opt_end: usize = 8;

// ---------------------------------------------------------------------------
// Sound menu item indices
// ---------------------------------------------------------------------------

/// Sound menu: SFX volume slider index.
pub(super) const sfx_vol: usize = 0;
/// Alias for `pages` (its `sfx_vol` callback shares the name).
pub(super) const sfx_vol_idx: usize = sfx_vol;
/// Sound menu: music volume slider index.
pub(super) const music_vol: usize = 2;
/// Alias for `pages` (its `music_vol` callback shares the name).
pub(super) const music_vol_idx: usize = music_vol;
/// Total items in the sound menu.
pub(super) const sound_end: usize = 4;

/// Total save/load slots.
pub(super) const load_end: usize = 6;

/// Total items in the first "Read This" help page.
pub(super) const read1_end: usize = 1;
/// Total items in the second "Read This" help page.
pub(super) const read2_end: usize = 1;


