//! The menu run-time state: the config/video-facing `#[no_mangle]`
//! statics (config binds by path and by C-string name; `hu_stuff`
//! extern-links `showMessages` by symbol), the modal-message state,
//! the save-name editor state, and the cursor state -- upstream names,
//! reached via `super::state::`.

use std::ffi::{c_char, c_int};
use std::ptr;

use super::consts::SAVESTRINGSIZE;
use super::types::menu_t;

/// Current mouse sensitivity setting (0-9); exported to C for `m_config.c` serialization.
#[no_mangle]
pub static mut mouseSensitivity: c_int = 5;

/// Non-zero when player messages are enabled; exported for `hu_stuff.c` and config.
#[no_mangle]
pub static mut showMessages: c_int = 1;

/// Graphics detail level: 0 = high, 1 = low; exported for `r_main.c` and config.
#[no_mangle]
pub static mut detailLevel: c_int = 0;

/// Number of screen blocks to render (3-12); exported for `r_main.c` and config.
#[no_mangle]
pub static mut screenblocks: c_int = 10;

/// Current display size index (0-8), derived from `screenblocks - 3`; updated by `M_SizeDisplay`.
pub(super) static mut screenSize: c_int = 0;

/// Save slot used for quicksave/quickload (-1 = none, -2 = slot selection pending).
pub(super) static mut quickSaveSlot: c_int = -1;

/// Non-zero when a modal message/confirmation overlay is active.
pub(super) static mut messageToPrint: c_int = 0;

/// Pointer to the C-string displayed in the active modal overlay.
pub(super) static mut messageString: *mut c_char = ptr::null_mut();

/// Screen x pixel of the modal message's left edge.
pub(super) static mut messx: c_int = 0;

/// Screen y pixel of the modal message's top edge.
pub(super) static mut messy: c_int = 0;

/// Value of `menuactive` saved when a modal message was opened, restored on dismissal.
pub(super) static mut messageLastMenuActive: c_int = 0;

/// Non-zero when the modal message requires a y/n or specific key response.
pub(super) static mut messageNeedsInput: c_int = 0;

/// Callback invoked with the key code when the modal message is dismissed.
pub(super) static mut messageRoutine: Option<extern "C" fn(c_int)> = None;

/// Non-zero while the player is typing a save-game description string.
pub(super) static mut saveStringEnter: c_int = 0;

/// Save slot index currently being named.
pub(super) static mut saveSlot: c_int = 0;

/// Cursor position (character count) within the save-game description string.
pub(super) static mut saveCharIndex: c_int = 0;

/// Copy of the original description before editing, restored on Escape.
pub(super) static mut saveOldString: [c_char; SAVESTRINGSIZE] = [0; SAVESTRINGSIZE];

/// Non-zero while a help/read-this screen is being displayed; suppresses status bar.
/// Exported for `d_main.c` which checks it before re-drawing the play area.
#[no_mangle]
pub static mut inhelpscreens: c_int = 0;

/// Non-zero while the menu overlay is open; read by `G_Responder` and `D_Display`.
#[no_mangle]
pub static mut menuactive: c_int = 0;

/// Save-game description strings for all 6 save slots, read from each save file header.
pub(super) static mut savegamestrings: [[c_char; SAVESTRINGSIZE]; 10] =
    [[0; SAVESTRINGSIZE]; 10];

/// Formatted quit confirmation string (game-specific quip + y/n prompt).
pub(super) static mut endstring: [c_char; 160] = [0; 160];

/// Index of the currently highlighted menu item within the active page.
pub(super) static mut itemOn: i16 = 0;

/// Countdown (in tics) until the skull cursor frame toggles.
pub(super) static mut skullAnimCounter: i16 = 0;

/// Index into `skullName`: 0 = `M_SKULL1`, 1 = `M_SKULL2`.
pub(super) static mut whichSkull: i16 = 0;

/// Pointer to the currently active menu page descriptor.
/// Exported for C callers (e.g. `d_main.c`).
#[no_mangle]
pub static mut currentMenu: *mut menu_t = ptr::null_mut();

/// Selected episode index (0-based), set by `M_Episode` and consumed by `M_ChooseSkill`.
pub(super) static mut epi: c_int = 0;
