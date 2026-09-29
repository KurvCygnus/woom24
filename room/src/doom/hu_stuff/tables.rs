//! The HUD data tables: the four string tables, the HU_* public
//! constants, and the chat-ring capacity.

use std::ffi::{c_char, c_int};

use crate::doom::i_timer::TICRATE;

/// ASCII code of the first character in the HUD font patch array (`'!'`).
/// Mirrors `HU_FONTSTART` from `hu_stuff.h`.
pub const HU_FONTSTART: u8 = b'!';

/// ASCII code of the last character in the HUD font patch array (`'_'`).
/// Mirrors `HU_FONTEND` from `hu_stuff.h`.
pub const HU_FONTEND: u8 = b'_';

/// Number of glyphs in the HUD font; equals `HU_FONTEND - HU_FONTSTART + 1`.
/// Determines the size of the `hu_font` array. Mirrors `HU_FONTSIZE`.
pub const HU_FONTSIZE: usize = (HU_FONTEND - HU_FONTSTART + 1) as usize;

/// Chat destination constant meaning "send to all players".
/// Values 1-4 target individual players; 5 is the broadcast value.
/// Mirrors `HU_BROADCAST` from `hu_stuff.h`.
pub const HU_BROADCAST: c_int = 5;

/// Screen X position of the message widget (top-left of screen).
/// Mirrors `HU_MSGX` from `hu_stuff.h`.
pub const HU_MSGX: c_int = 0;

/// Screen Y position of the message widget (top of screen).
/// Mirrors `HU_MSGY` from `hu_stuff.h`.
pub const HU_MSGY: c_int = 0;

/// Width of the message widget in characters (unused in this port; kept for
/// parity with the C constant `HU_MSGWIDTH`).
pub const HU_MSGWIDTH: c_int = 64;

/// Height of the message widget in lines.
/// Mirrors `HU_MSGHEIGHT` / `HU_TITLEHEIGHT` from `hu_stuff.h`.
pub const HU_MSGHEIGHT: c_int = 1;

/// Number of game tics a message remains visible before it disappears
/// automatically (4 seconds at the default 35-tic-per-second rate).
/// Mirrors `HU_MSGTIMEOUT` from `hu_stuff.h`.
pub const HU_MSGTIMEOUT: c_int = 4 * TICRATE;

/// Capacity of the outgoing chat-character circular buffer.
/// Must be a power of two so the modular index arithmetic uses bitwise AND.
pub(super) const QUEUESIZE: usize = 128;

/// Default chat macro strings bound to Alt+0 through Alt+9.
///
/// Corresponds to `chat_macros[]` in `hu_stuff.c`. Read by `responder`
/// when Alt is held and a digit key is pressed during chat mode. Also exported
/// to the C side so that `m_config.c` can bind user-configurable strings.
#[no_mangle]
pub static mut chat_macros: [*mut c_char; 10] = [
    c"No".as_ptr().cast_mut(),
    c"I'm ready to kick butt!".as_ptr().cast_mut(),
    c"I'm OK.".as_ptr().cast_mut(),
    c"I'm not looking too good!".as_ptr().cast_mut(),
    c"Help!".as_ptr().cast_mut(),
    c"You suck!".as_ptr().cast_mut(),
    c"Next time, scumbag...".as_ptr().cast_mut(),
    c"Come here!".as_ptr().cast_mut(),
    c"I'll take care of it.".as_ptr().cast_mut(),
    c"Yes".as_ptr().cast_mut(),
];

/// Display names for each of the four multiplayer player colors.
///
/// Corresponds to `player_names[]` in `hu_stuff.c`. Prepended to incoming
/// chat messages so the recipient can identify the sender. Exported so the
/// C side can substitute Dehacked strings (not implemented in this port).
#[no_mangle]
pub static mut player_names: [*mut c_char; 4] = [
    c"Green: ".as_ptr().cast_mut(),
    c"Indigo: ".as_ptr().cast_mut(),
    c"Brown: ".as_ptr().cast_mut(),
    c"Red: ".as_ptr().cast_mut(),
];

/// Display names for Doom 1 / Ultimate Doom maps (E1M1-E4M9 plus stubs).
///
/// Indexed as `mapnames[(gameepisode - 1) * 9 + gamemap - 1]`. The last nine
/// entries are `"NEWLEVEL"` placeholders for episode 4 maps that have no
/// canonical name in the original WAD. Corresponds to `mapnames[]` in
/// `hu_stuff.c` and is also exported for access from the C side.
#[no_mangle]
pub static mut mapnames: [*mut c_char; 45] = [
    c"E1M1: Hangar".as_ptr().cast_mut(),
    c"E1M2: Nuclear Plant".as_ptr().cast_mut(),
    c"E1M3: Toxin Refinery".as_ptr().cast_mut(),
    c"E1M4: Command Control".as_ptr().cast_mut(),
    c"E1M5: Phobos Lab".as_ptr().cast_mut(),
    c"E1M6: Central Processing".as_ptr().cast_mut(),
    c"E1M7: Computer Station".as_ptr().cast_mut(),
    c"E1M8: Phobos Anomaly".as_ptr().cast_mut(),
    c"E1M9: Military Base".as_ptr().cast_mut(),
    c"E2M1: Deimos Anomaly".as_ptr().cast_mut(),
    c"E2M2: Containment Area".as_ptr().cast_mut(),
    c"E2M3: Refinery".as_ptr().cast_mut(),
    c"E2M4: Deimos Lab".as_ptr().cast_mut(),
    c"E2M5: Command Center".as_ptr().cast_mut(),
    c"E2M6: Halls of the Damned".as_ptr().cast_mut(),
    c"E2M7: Spawning Vats".as_ptr().cast_mut(),
    c"E2M8: Tower of Babel".as_ptr().cast_mut(),
    c"E2M9: Fortress of Mystery".as_ptr().cast_mut(),
    c"E3M1: Hell Keep".as_ptr().cast_mut(),
    c"E3M2: Slough of Despair".as_ptr().cast_mut(),
    c"E3M3: Pandemonium".as_ptr().cast_mut(),
    c"E3M4: House of Pain".as_ptr().cast_mut(),
    c"E3M5: Unholy Cathedral".as_ptr().cast_mut(),
    c"E3M6: Mt. Erebus".as_ptr().cast_mut(),
    c"E3M7: Limbo".as_ptr().cast_mut(),
    c"E3M8: Dis".as_ptr().cast_mut(),
    c"E3M9: Warrens".as_ptr().cast_mut(),
    c"E4M1: Hell Beneath".as_ptr().cast_mut(),
    c"E4M2: Perfect Hatred".as_ptr().cast_mut(),
    c"E4M3: Sever The Wicked".as_ptr().cast_mut(),
    c"E4M4: Unruly Evil".as_ptr().cast_mut(),
    c"E4M5: They Will Repent".as_ptr().cast_mut(),
    c"E4M6: Against Thee Wickedly".as_ptr().cast_mut(),
    c"E4M7: And Hell Followed".as_ptr().cast_mut(),
    c"E4M8: Unto The Cruel".as_ptr().cast_mut(),
    c"E4M9: Fear".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
    c"NEWLEVEL".as_ptr().cast_mut(),
];

/// Display names for Doom II, Plutonia, and TNT maps (96 entries total).
///
/// Layout: indices 0-31 are Doom II maps, 32-63 are Plutonia, 64-95 are TNT.
/// Indexed as `mapnames_commercial[gamemap - 1]` (Doom II) or with a +32/+64
/// offset for the expansion packs. Corresponds to `mapnames_commercial[]` in
/// `hu_stuff.c`.
#[no_mangle]
pub static mut mapnames_commercial: [*mut c_char; 96] = [
    // DOOM 2
    c"level 1: entryway".as_ptr().cast_mut(),
    c"level 2: underhalls".as_ptr().cast_mut(),
    c"level 3: the gantlet".as_ptr().cast_mut(),
    c"level 4: the focus".as_ptr().cast_mut(),
    c"level 5: the waste tunnels".as_ptr().cast_mut(),
    c"level 6: the crusher".as_ptr().cast_mut(),
    c"level 7: dead simple".as_ptr().cast_mut(),
    c"level 8: tricks and traps".as_ptr().cast_mut(),
    c"level 9: the pit".as_ptr().cast_mut(),
    c"level 10: refueling base".as_ptr().cast_mut(),
    c"level 11: 'o' of destruction!".as_ptr().cast_mut(),
    c"level 12: the factory".as_ptr().cast_mut(),
    c"level 13: downtown".as_ptr().cast_mut(),
    c"level 14: the inmost dens".as_ptr().cast_mut(),
    c"level 15: industrial zone".as_ptr().cast_mut(),
    c"level 16: suburbs".as_ptr().cast_mut(),
    c"level 17: tenements".as_ptr().cast_mut(),
    c"level 18: the courtyard".as_ptr().cast_mut(),
    c"level 19: the citadel".as_ptr().cast_mut(),
    c"level 20: gotcha!".as_ptr().cast_mut(),
    c"level 21: nirvana".as_ptr().cast_mut(),
    c"level 22: the catacombs".as_ptr().cast_mut(),
    c"level 23: barrels o' fun".as_ptr().cast_mut(),
    c"level 24: the chasm".as_ptr().cast_mut(),
    c"level 25: bloodfalls".as_ptr().cast_mut(),
    c"level 26: the abandoned mines".as_ptr().cast_mut(),
    c"level 27: monster condo".as_ptr().cast_mut(),
    c"level 28: the spirit world".as_ptr().cast_mut(),
    c"level 29: the living end".as_ptr().cast_mut(),
    c"level 30: icon of sin".as_ptr().cast_mut(),
    c"level 31: wolfenstein".as_ptr().cast_mut(),
    c"level 32: grosse".as_ptr().cast_mut(),
    // Plutonia
    c"level 1: congo".as_ptr().cast_mut(),
    c"level 2: well of souls".as_ptr().cast_mut(),
    c"level 3: aztec".as_ptr().cast_mut(),
    c"level 4: caged".as_ptr().cast_mut(),
    c"level 5: ghost town".as_ptr().cast_mut(),
    c"level 6: baron's lair".as_ptr().cast_mut(),
    c"level 7: caughtyard".as_ptr().cast_mut(),
    c"level 8: realm".as_ptr().cast_mut(),
    c"level 9: abattoire".as_ptr().cast_mut(),
    c"level 10: onslaught".as_ptr().cast_mut(),
    c"level 11: hunted".as_ptr().cast_mut(),
    c"level 12: speed".as_ptr().cast_mut(),
    c"level 13: the crypt".as_ptr().cast_mut(),
    c"level 14: genesis".as_ptr().cast_mut(),
    c"level 15: the twilight".as_ptr().cast_mut(),
    c"level 16: the omen".as_ptr().cast_mut(),
    c"level 17: compound".as_ptr().cast_mut(),
    c"level 18: neurosphere".as_ptr().cast_mut(),
    c"level 19: nme".as_ptr().cast_mut(),
    c"level 20: the death domain".as_ptr().cast_mut(),
    c"level 21: slayer".as_ptr().cast_mut(),
    c"level 22: impossible mission".as_ptr().cast_mut(),
    c"level 23: tombstone".as_ptr().cast_mut(),
    c"level 24: the final frontier".as_ptr().cast_mut(),
    c"level 25: the temple of darkness".as_ptr().cast_mut(),
    c"level 26: bunker".as_ptr().cast_mut(),
    c"level 27: anti-christ".as_ptr().cast_mut(),
    c"level 28: the sewers".as_ptr().cast_mut(),
    c"level 29: odyssey of noises".as_ptr().cast_mut(),
    c"level 30: the gateway of hell".as_ptr().cast_mut(),
    c"level 31: cyberden".as_ptr().cast_mut(),
    c"level 32: go 2 it".as_ptr().cast_mut(),
    // TNT
    c"level 1: system control".as_ptr().cast_mut(),
    c"level 2: human bbq".as_ptr().cast_mut(),
    c"level 3: power control".as_ptr().cast_mut(),
    c"level 4: wormhole".as_ptr().cast_mut(),
    c"level 5: hanger".as_ptr().cast_mut(),
    c"level 6: open season".as_ptr().cast_mut(),
    c"level 7: prison".as_ptr().cast_mut(),
    c"level 8: metal".as_ptr().cast_mut(),
    c"level 9: stronghold".as_ptr().cast_mut(),
    c"level 10: redemption".as_ptr().cast_mut(),
    c"level 11: storage facility".as_ptr().cast_mut(),
    c"level 12: crater".as_ptr().cast_mut(),
    c"level 13: nukage processing".as_ptr().cast_mut(),
    c"level 14: steel works".as_ptr().cast_mut(),
    c"level 15: dead zone".as_ptr().cast_mut(),
    c"level 16: deepest reaches".as_ptr().cast_mut(),
    c"level 17: processing area".as_ptr().cast_mut(),
    c"level 18: mill".as_ptr().cast_mut(),
    c"level 19: shipping/respawning".as_ptr().cast_mut(),
    c"level 20: central processing".as_ptr().cast_mut(),
    c"level 21: administration center".as_ptr().cast_mut(),
    c"level 22: habitat".as_ptr().cast_mut(),
    c"level 23: lunar mining project".as_ptr().cast_mut(),
    c"level 24: quarry".as_ptr().cast_mut(),
    c"level 25: baron's den".as_ptr().cast_mut(),
    c"level 26: ballistyx".as_ptr().cast_mut(),
    c"level 27: mount pain".as_ptr().cast_mut(),
    c"level 28: heck".as_ptr().cast_mut(),
    c"level 29: river styx".as_ptr().cast_mut(),
    c"level 30: last call".as_ptr().cast_mut(),
    c"level 31: pharaoh".as_ptr().cast_mut(),
    c"level 32: caribbean".as_ptr().cast_mut(),
];
