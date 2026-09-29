//! The status-bar constants: geometry, face-table structure, automap
//! event values, palette indices, and the STSTR_* message strings.

use std::ffi::c_int;

use crate::doom::i_timer::TICRATE;

/// Height of the status bar in screen pixels (matches C `ST_HEIGHT`).
/// Duplicated in the graduated `st_lib` by upstream design -- do not unify.
pub(super) const ST_HEIGHT: c_int = 32;
/// Width of the status bar in screen pixels (matches C `ST_WIDTH`).
pub(super) const ST_WIDTH: c_int = 320;
/// Left edge of the status bar in screen coordinates (matches C `ST_X`).
pub(super) const ST_X: c_int = 0;
/// Top edge of the status bar in screen coordinates (matches C `ST_Y`).
pub(super) const ST_Y: c_int = 200 - ST_HEIGHT;

/// X pixel position of the face widget background (matches C `ST_FX`).
pub(super) const ST_FX: c_int = 143;
/// Y pixel position of the face widget background (matches C `ST_FY`).
pub(super) const ST_FY: c_int = 169;

/// Number of pain-level face rows (0 = healthy, 4 = critical; matches C `ST_NUMPAINFACES`).
pub const ST_NUMPAINFACES: c_int = 5;
/// Straight-ahead frames per pain row (matches C `ST_NUMSTRAIGHTFACES`).
pub(super) const ST_NUMSTRAIGHTFACES: c_int = 3;
/// Turn-direction frames per pain row (matches C `ST_NUMTURNFACES`).
pub(super) const ST_NUMTURNFACES: c_int = 2;
/// Special-expression frames per pain row: ouch, evil-grin, rampage (matches C `ST_NUMSPECIALFACES`).
pub(super) const ST_NUMSPECIALFACES: c_int = 3;

/// Total frames per pain-level row (matches C `ST_FACESTRIDE`).
pub const ST_FACESTRIDE: c_int = ST_NUMSTRAIGHTFACES + ST_NUMTURNFACES + ST_NUMSPECIALFACES;

/// Number of frames appended after all pain rows: god-mode and dead (matches C `ST_NUMEXTRAFACES`).
pub(super) const ST_NUMEXTRAFACES: c_int = 2;
/// Total face patch count loaded from the WAD (matches C `ST_NUMFACES`).
pub(super) const ST_NUMFACES: c_int = ST_FACESTRIDE * ST_NUMPAINFACES + ST_NUMEXTRAFACES;

/// Offset within a pain-row to the first turn face (matches C `ST_TURNOFFSET`).
pub(super) const ST_TURNOFFSET: c_int = ST_NUMSTRAIGHTFACES;
/// Offset within a pain-row to the ouch face (matches C `ST_OUCHOFFSET`).
pub(super) const ST_OUCHOFFSET: c_int = ST_TURNOFFSET + ST_NUMTURNFACES;
/// Offset within a pain-row to the evil-grin face (matches C `ST_EVILGRINOFFSET`).
pub(super) const ST_EVILGRINOFFSET: c_int = ST_OUCHOFFSET + 1;
/// Offset within a pain-row to the rampage face (matches C `ST_RAMPAGEOFFSET`).
pub(super) const ST_RAMPAGEOFFSET: c_int = ST_EVILGRINOFFSET + 1;
/// Index of the god-mode face in the flat `faces` array (matches C `ST_GODFACE`).
pub(super) const ST_GODFACE: c_int = ST_NUMPAINFACES * ST_FACESTRIDE;
/// Index of the dead face in the flat `faces` array (matches C `ST_DEADFACE`).
pub(super) const ST_DEADFACE: c_int = ST_GODFACE + 1;

/// X screen coordinate of the face widget (matches C `ST_FACESX`).
pub(super) const ST_FACESX: c_int = 143;
/// Y screen coordinate of the face widget (matches C `ST_FACESY`).
pub(super) const ST_FACESY: c_int = 168;

/// Duration of the evil-grin expression in tics (matches C `ST_EVILGRINCOUNT`).
pub(super) const ST_EVILGRINCOUNT: c_int = 2 * TICRATE;
/// Duration of a straight-ahead expression in tics (matches C `ST_STRAIGHTFACECOUNT`).
pub(super) const ST_STRAIGHTFACECOUNT: c_int = TICRATE / 2;
/// Duration of turn and ouch expressions in tics (matches C `ST_TURNCOUNT`).
pub(super) const ST_TURNCOUNT: c_int = TICRATE;
/// Duration of the ouch expression in tics (matches C `ST_OUCHCOUNT`).
pub(super) const ST_OUCHCOUNT: c_int = TICRATE;
/// Tics of continuous fire before the rampage face appears (matches C `ST_RAMPAGEDELAY`).
pub(super) const ST_RAMPAGEDELAY: c_int = 2 * TICRATE;

/// Health-point drop threshold that triggers the ouch face (matches C `ST_MUCHPAIN`).
pub(super) const ST_MUCHPAIN: c_int = 20;

/// Digit width of the ready-ammo number widget (matches C `ST_AMMOWIDTH`).
pub(super) const ST_AMMOWIDTH: c_int = 3;
/// X coordinate of the ready-ammo display (matches C `ST_AMMOX`).
pub(super) const ST_AMMOX: c_int = 44;
/// Y coordinate of the ready-ammo display (matches C `ST_AMMOY`).
pub(super) const ST_AMMOY: c_int = 171;

/// Digit width of the health display (matches C `ST_HEALTHWIDTH`).
pub(super) const ST_HEALTHWIDTH: c_int = 3;
/// X coordinate of the health display (matches C `ST_HEALTHX`).
pub(super) const ST_HEALTHX: c_int = 90;
/// Y coordinate of the health display (matches C `ST_HEALTHY`).
pub(super) const ST_HEALTHY: c_int = 171;

/// X coordinate of the weapons-owned grid (matches C `ST_ARMSX`).
pub(super) const ST_ARMSX: c_int = 111;
/// Y coordinate of the weapons-owned grid (matches C `ST_ARMSY`).
pub(super) const ST_ARMSY: c_int = 172;
/// X coordinate of the arms background patch (matches C `ST_ARMSBGX`).
pub(super) const ST_ARMSBGX: c_int = 104;
/// Y coordinate of the arms background patch (matches C `ST_ARMSBGY`).
pub(super) const ST_ARMSBGY: c_int = 168;
/// Horizontal spacing between arms-grid cells in pixels (matches C `ST_ARMSXSPACE`).
pub(super) const ST_ARMSXSPACE: c_int = 12;
/// Vertical spacing between arms-grid cells in pixels (matches C `ST_ARMSYSPACE`).
pub(super) const ST_ARMSYSPACE: c_int = 10;

/// X coordinate of the frag counter in deathmatch (matches C `ST_FRAGSX`).
pub(super) const ST_FRAGSX: c_int = 138;
/// Y coordinate of the frag counter in deathmatch (matches C `ST_FRAGSY`).
pub(super) const ST_FRAGSY: c_int = 171;
/// Digit width of the frag counter (matches C `ST_FRAGSWIDTH`).
pub(super) const ST_FRAGSWIDTH: c_int = 2;

/// Digit width of the armor display (matches C `ST_ARMORWIDTH`).
pub(super) const ST_ARMORWIDTH: c_int = 3;
/// X coordinate of the armor display (matches C `ST_ARMORX`).
pub(super) const ST_ARMORX: c_int = 221;
/// Y coordinate of the armor display (matches C `ST_ARMORY`).
pub(super) const ST_ARMORY: c_int = 171;

/// X coordinate of the blue key slot (matches C `ST_KEY0X`).
pub(super) const ST_KEY0X: c_int = 239;
/// Y coordinate of the blue key slot (matches C `ST_KEY0Y`).
pub(super) const ST_KEY0Y: c_int = 171;
/// X coordinate of the yellow key slot (matches C `ST_KEY1X`).
pub(super) const ST_KEY1X: c_int = 239;
/// Y coordinate of the yellow key slot (matches C `ST_KEY1Y`).
pub(super) const ST_KEY1Y: c_int = 181;
/// X coordinate of the red key slot (matches C `ST_KEY2X`).
pub(super) const ST_KEY2X: c_int = 239;
/// Y coordinate of the red key slot (matches C `ST_KEY2Y`).
pub(super) const ST_KEY2Y: c_int = 191;

/// Digit width of per-ammo-type current-ammo displays (matches C `ST_AMMO0WIDTH`).
pub(super) const ST_AMMO0WIDTH: c_int = 3;
/// X coordinate of bullets current-ammo display (matches C `ST_AMMO0X`).
pub(super) const ST_AMMO0X: c_int = 288;
/// Y coordinate of bullets current-ammo display (matches C `ST_AMMO0Y`).
pub(super) const ST_AMMO0Y: c_int = 173;
/// X coordinate of shells current-ammo display (matches C `ST_AMMO1X`).
pub(super) const ST_AMMO1X: c_int = 288;
/// Y coordinate of shells current-ammo display (matches C `ST_AMMO1Y`).
pub(super) const ST_AMMO1Y: c_int = 179;
/// X coordinate of cells current-ammo display (matches C `ST_AMMO2X`).
pub(super) const ST_AMMO2X: c_int = 288;
/// Y coordinate of cells current-ammo display (matches C `ST_AMMO2Y`).
pub(super) const ST_AMMO2Y: c_int = 191;
/// X coordinate of rockets current-ammo display (matches C `ST_AMMO3X`).
pub(super) const ST_AMMO3X: c_int = 288;
/// Y coordinate of rockets current-ammo display (matches C `ST_AMMO3Y`).
pub(super) const ST_AMMO3Y: c_int = 185;

/// Digit width of per-ammo-type max-ammo displays (matches C `ST_MAXAMMO0WIDTH`).
pub(super) const ST_MAXAMMO0WIDTH: c_int = 3;
/// X coordinate of bullets max-ammo display (matches C `ST_MAXAMMO0X`).
pub(super) const ST_MAXAMMO0X: c_int = 314;
/// Y coordinate of bullets max-ammo display (matches C `ST_MAXAMMO0Y`).
pub(super) const ST_MAXAMMO0Y: c_int = 173;
/// X coordinate of shells max-ammo display (matches C `ST_MAXAMMO1X`).
pub(super) const ST_MAXAMMO1X: c_int = 314;
/// Y coordinate of shells max-ammo display (matches C `ST_MAXAMMO1Y`).
pub(super) const ST_MAXAMMO1Y: c_int = 179;
/// X coordinate of cells max-ammo display (matches C `ST_MAXAMMO2X`).
pub(super) const ST_MAXAMMO2X: c_int = 314;
/// Y coordinate of cells max-ammo display (matches C `ST_MAXAMMO2Y`).
pub(super) const ST_MAXAMMO2Y: c_int = 191;
/// X coordinate of rockets max-ammo display (matches C `ST_MAXAMMO3X`).
pub(super) const ST_MAXAMMO3X: c_int = 314;
/// Y coordinate of rockets max-ammo display (matches C `ST_MAXAMMO3Y`).
pub(super) const ST_MAXAMMO3Y: c_int = 185;

/// Header magic for automap messages sent via the event system (matches C `AM_MSGHEADER`).
pub(super) const AM_MSGHEADER: c_int = (('a' as c_int) << 24) + (('m' as c_int) << 16);
/// Event data value signalling that the automap was opened (matches C `AM_MSGENTERED`).
pub(super) const AM_MSGENTERED: c_int = AM_MSGHEADER | (('e' as c_int) << 8);
/// Event data value signalling that the automap was closed (matches C `AM_MSGEXITED`).
pub(super) const AM_MSGEXITED: c_int = AM_MSGHEADER | (('x' as c_int) << 8);

/// Index of the first red-damage palette in `PLAYPAL` (matches C `STARTREDPALS`).
pub(super) const STARTREDPALS: c_int = 1;
/// Index of the first bonus-pickup palette in `PLAYPAL` (matches C `STARTBONUSPALS`).
pub(super) const STARTBONUSPALS: c_int = 9;
/// Number of red-damage palette entries (matches C `NUMREDPALS`).
pub(super) const NUMREDPALS: c_int = 8;
/// Number of bonus-pickup palette entries (matches C `NUMBONUSPALS`).
pub(super) const NUMBONUSPALS: c_int = 4;
/// Index of the radiation-suit palette in `PLAYPAL` (matches C `RADIATIONPAL`).
pub(super) const RADIATIONPAL: c_int = 13;

// ---------------------------------------------------------------------------
// String literals (from d_englsh.h)
// ---------------------------------------------------------------------------

/// Message shown when god mode is activated (C `STSTR_DQDON` from `d_englsh.h`).
pub(super) const STSTR_DQDON: *mut std::ffi::c_char = c"Degreelessness Mode On".as_ptr().cast_mut();
/// Message shown when god mode is deactivated (C `STSTR_DQDOFF` from `d_englsh.h`).
pub(super) const STSTR_DQDOFF: *mut std::ffi::c_char = c"Degreelessness Mode Off".as_ptr().cast_mut();
/// Message shown when `idfa` (ammo, no keys) cheat fires (C `STSTR_FAADDED`).
pub(super) const STSTR_FAADDED: *mut std::ffi::c_char = c"Ammo (no keys) Added".as_ptr().cast_mut();
/// Message shown when `idkfa` (ammo + keys) cheat fires (C `STSTR_KFAADDED`).
pub(super) const STSTR_KFAADDED: *mut std::ffi::c_char = c"Very Happy Ammo Added".as_ptr().cast_mut();
/// Message shown when a music-change cheat fires (C `STSTR_MUS`).
pub(super) const STSTR_MUS: *mut std::ffi::c_char = c"Music Change".as_ptr().cast_mut();
/// Message shown when an invalid music number is entered (C `STSTR_NOMUS`).
pub(super) const STSTR_NOMUS: *mut std::ffi::c_char = c"IMPOSSIBLE SELECTION".as_ptr().cast_mut();
/// Message shown when no-clip mode is activated (C `STSTR_NCON`).
pub(super) const STSTR_NCON: *mut std::ffi::c_char = c"No Clipping Mode ON".as_ptr().cast_mut();
/// Message shown when no-clip mode is deactivated (C `STSTR_NCOFF`).
pub(super) const STSTR_NCOFF: *mut std::ffi::c_char = c"No Clipping Mode OFF".as_ptr().cast_mut();
/// Prompt shown when the `idbehold` cheat prefix fires (C `STSTR_BEHOLD`).
pub(super) const STSTR_BEHOLD: *mut std::ffi::c_char = c"inVuln, Str, Inviso, Rad, Allmap, or Lite-amp"
    .as_ptr()
    .cast_mut();
/// Message shown when a `idbeholdX` power-up cheat fires (C `STSTR_BEHOLDX`).
pub(super) const STSTR_BEHOLDX: *mut std::ffi::c_char = c"Power-up Toggled".as_ptr().cast_mut();
/// Message shown when the `idchoppers` cheat fires (C `STSTR_CHOPPERS`).
pub(super) const STSTR_CHOPPERS: *mut std::ffi::c_char = c"... doesn't suck - GM".as_ptr().cast_mut();
/// Message shown when the level-change cheat (`idclev`) fires (C `STSTR_CLEV`).
pub(super) const STSTR_CLEV: *mut std::ffi::c_char = c"Changing Level...".as_ptr().cast_mut();
