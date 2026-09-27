//! The 29 enum-discriminant constants of `GameMission_t`, `GameMode_t`,
//! and `GameVersion_t` (`d_mode.h`), kept as plain `c_int` values for
//! direct FFI storage. Names and values are verbatim from the pre-split
//! module; the freeze zone addresses them as `d_mode::<name>` via the
//! root re-export.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;

// ---------------------------------------------------------------------------
// GameMission_t constants
//
// Identify which game (IWAD) is loaded. Corresponds to `GameMission_t` in
// `d_mode.h`. Stored as plain `c_int` for direct FFI compatibility.
// ---------------------------------------------------------------------------

/// No game mission; used as a sentinel / unset value.
///
/// Maps to `none` in `GameMission_t`. Stored as `9` in the C enum.
pub const none: c_int = 9;

/// Doom / Ultimate Doom (IWAD: `doom.wad`, `doom1.wad`, or `doomu.wad`).
///
/// Maps to `doom` (discriminant 0) in `GameMission_t`.
pub const doom: c_int = 0;

/// Doom II: Hell on Earth (IWAD: `doom2.wad`).
///
/// Maps to `doom2` (discriminant 1) in `GameMission_t`.
pub const doom2: c_int = 1;

/// Final Doom: TNT Evilution (IWAD: `tnt.wad`).
///
/// Maps to `pack_tnt` (discriminant 2) in `GameMission_t`.
pub const pack_tnt: c_int = 2;

/// Final Doom: The Plutonia Experiment (IWAD: `plutonia.wad`).
///
/// Maps to `pack_plut` (discriminant 3) in `GameMission_t`.
pub const pack_plut: c_int = 3;

/// Chex Quest (shareware Doom mod; IWAD: `chex.wad`).
///
/// Maps to `pack_chex` (discriminant 4) in `GameMission_t`. Uses episode-based
/// map layout like Doom 1 rather than the `MAPxx` layout of Doom 2.
pub const pack_chex: c_int = 4;

/// Hacx: Twitch 'n Kill (Doom 2 mod; IWAD: `hacx.wad`).
///
/// Maps to `pack_hacx` (discriminant 5) in `GameMission_t`.
pub const pack_hacx: c_int = 5;

/// Heretic: Shadow of the Serpent Riders (IWAD: `heretic.wad`).
///
/// Maps to `heretic` (discriminant 6) in `GameMission_t`.
pub const heretic: c_int = 6;

/// Hexen: Beyond Heretic (IWAD: `hexen.wad`).
///
/// Maps to `hexen` (discriminant 7) in `GameMission_t`.
pub const hexen: c_int = 7;

/// Strife: Quest for the Sigil (IWAD: `strife1.wad`).
///
/// Maps to `strife` (discriminant 8) in `GameMission_t`.
pub const strife: c_int = 8;

// ---------------------------------------------------------------------------
// GameMode_t constants
//
// Identify the release tier of the loaded IWAD. Corresponds to `GameMode_t`
// in `d_mode.h`. Stored as plain `c_int` for direct FFI compatibility.
// ---------------------------------------------------------------------------

/// Shareware release of Doom or Heretic (one episode, freely distributable).
///
/// Maps to `shareware` (discriminant 0) in `GameMode_t`.
pub const shareware: c_int = 0;

/// Registered (three-episode) release of Doom or Heretic.
///
/// Maps to `registered` (discriminant 1) in `GameMode_t`.
pub const registered: c_int = 1;

/// Commercial (MAPxx-based) release: Doom II, Final Doom, Hexen, Strife, etc.
///
/// Maps to `commercial` (discriminant 2) in `GameMode_t`.
pub const commercial: c_int = 2;

/// Retail / Ultimate Doom (four-episode release, `doom.wad`).
///
/// Maps to `retail` (discriminant 3) in `GameMode_t`.
pub const retail: c_int = 3;

/// Unknown or undetected game mode (IWAD not yet loaded or not recognised).
///
/// Maps to `indetermined` (discriminant 4) in `GameMode_t`.
pub const indetermined: c_int = 4;

// ---------------------------------------------------------------------------
// GameVersion_t constants
//
// Identify which executable version is being emulated, primarily for demo
// compatibility. Corresponds to `GameVersion_t` in `d_mode.h`. Stored as
// plain `c_int` for direct FFI compatibility.
// ---------------------------------------------------------------------------

/// Doom v1.2: earliest shareware and registered release.
///
/// Maps to `exe_doom_1_2` (discriminant 0) in `GameVersion_t`.
pub const exe_doom_1_2: c_int = 0;

/// Doom v1.666: first release compatible with all three edition types.
///
/// Maps to `exe_doom_1_666` (discriminant 1) in `GameVersion_t`.
pub const exe_doom_1_666: c_int = 1;

/// Doom v1.7 / v1.7a.
///
/// Maps to `exe_doom_1_7` (discriminant 2) in `GameVersion_t`.
pub const exe_doom_1_7: c_int = 2;

/// Doom v1.8.
///
/// Maps to `exe_doom_1_8` (discriminant 3) in `GameVersion_t`.
pub const exe_doom_1_8: c_int = 3;

/// Doom v1.9: the most widely distributed version; default emulation target.
///
/// Maps to `exe_doom_1_9` (discriminant 4) in `GameVersion_t`.
pub const exe_doom_1_9: c_int = 4;

/// Hacx standalone executable (based on Doom 1.9).
///
/// Maps to `exe_hacx` (discriminant 5) in `GameVersion_t`.
pub const exe_hacx: c_int = 5;

/// Ultimate Doom (retail four-episode) executable.
///
/// Maps to `exe_ultimate` (discriminant 6) in `GameVersion_t`.
pub const exe_ultimate: c_int = 6;

/// Final Doom executable (v1.9 variant used by `tnt.wad` and `plutonia.wad`).
///
/// Maps to `exe_final` (discriminant 7) in `GameVersion_t`.
pub const exe_final: c_int = 7;

/// Alternate Final Doom executable (second `final.exe` binary).
///
/// Maps to `exe_final2` (discriminant 8) in `GameVersion_t`.
pub const exe_final2: c_int = 8;

/// Chex Quest executable (derived from the Final Doom binary).
///
/// Maps to `exe_chex` (discriminant 9) in `GameVersion_t`.
pub const exe_chex: c_int = 9;

/// Heretic v1.3 executable.
///
/// Maps to `exe_heretic_1_3` (discriminant 10) in `GameVersion_t`.
pub const exe_heretic_1_3: c_int = 10;

/// Hexen v1.1 executable.
///
/// Maps to `exe_hexen_1_1` (discriminant 11) in `GameVersion_t`.
pub const exe_hexen_1_1: c_int = 11;

/// Strife v1.2 executable.
///
/// Maps to `exe_strife_1_2` (discriminant 12) in `GameVersion_t`.
pub const exe_strife_1_2: c_int = 12;

/// Strife v1.31 executable.
///
/// Maps to `exe_strife_1_31` (discriminant 13) in `GameVersion_t`.
pub const exe_strife_1_31: c_int = 13;
