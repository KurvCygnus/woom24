//! The private validation tables: the (mission, mode) bounds and the
//! (mission, version) pairs the `validate` queries scan. Verbatim data
//! from the pre-split module; entries stay ordered exactly as the C
//! `valid_modes[]` / `valid_versions[]` arrays in `d_mode.c`.

use std::ffi::c_int;

use super::consts::{
    commercial, doom, doom2, exe_chex, exe_doom_1_9, exe_final, exe_final2, exe_hacx,
    exe_heretic_1_3, exe_hexen_1_1, exe_strife_1_2, exe_strife_1_31, exe_ultimate, heretic, hexen,
    pack_chex, pack_hacx, pack_plut, pack_tnt, registered, retail, shareware, strife,
};

/// A single valid (mission, mode) combination with its episode and map bounds.
///
/// Each entry in [`VALID_MODES`] records the maximum episode and maximum map
/// number accessible in that combination. [`super::validate::valid_episode_map`]
/// uses these bounds for range-checking. Corresponds to the anonymous struct
/// inside the `valid_modes[]` array in `d_mode.c`.
pub(super) struct ValidMode {
    /// `GameMission_t` constant for this entry.
    pub(super) mission: c_int,
    /// `GameMode_t` constant for this entry.
    pub(super) mode: c_int,
    /// Maximum valid episode number (inclusive).
    pub(super) episode: c_int,
    /// Maximum valid map number within any episode (inclusive).
    pub(super) map: c_int,
}

/// Table of all valid (mission, mode) combinations and their map bounds.
///
/// Iterated by [`super::validate::valid_game_mode`],
/// [`super::validate::valid_episode_map`], and (through it)
/// [`super::validate::num_episodes`]. Entries are ordered as in the C
/// `valid_modes[]` array in `d_mode.c`. There is no entry for unknown /
/// indetermined combinations; those return false from the validation
/// functions.
pub(super) static VALID_MODES: [ValidMode; 13] = [
    ValidMode {
        mission: pack_chex,
        mode: shareware,
        episode: 1,
        map: 5,
    },
    ValidMode {
        mission: doom,
        mode: shareware,
        episode: 1,
        map: 9,
    },
    ValidMode {
        mission: doom,
        mode: registered,
        episode: 3,
        map: 9,
    },
    ValidMode {
        mission: doom,
        mode: retail,
        episode: 4,
        map: 9,
    },
    ValidMode {
        mission: doom2,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: pack_tnt,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: pack_plut,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: pack_hacx,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: heretic,
        mode: shareware,
        episode: 1,
        map: 9,
    },
    ValidMode {
        mission: heretic,
        mode: registered,
        episode: 3,
        map: 9,
    },
    ValidMode {
        mission: heretic,
        mode: retail,
        episode: 5,
        map: 9,
    },
    ValidMode {
        mission: hexen,
        mode: commercial,
        episode: 1,
        map: 60,
    },
    ValidMode {
        mission: strife,
        mode: commercial,
        episode: 1,
        map: 34,
    },
];

/// A single valid (mission, version) pair for game-version checking.
///
/// Each entry in [`VALID_VERSIONS`] asserts that a given `GameVersion_t` is
/// legal for a given `GameMission_t`. Corresponds to the anonymous struct
/// inside `valid_versions[]` in `d_mode.c`.
pub(super) struct ValidVersion {
    /// `GameMission_t` constant for this entry.
    pub(super) mission: c_int,
    /// `GameVersion_t` constant for this entry.
    pub(super) version: c_int,
}

/// Table of valid (mission, version) pairs.
///
/// Iterated by [`super::validate::valid_game_version`]. Doom-family variants
/// (`doom2`, `pack_plut`, `pack_tnt`, `pack_hacx`, `pack_chex`) are normalised
/// to `doom` before the lookup, so only `doom` entries need to appear here for
/// those games. Corresponds to `valid_versions[]` in `d_mode.c`.
pub(super) static VALID_VERSIONS: [ValidVersion; 10] = [
    ValidVersion {
        mission: doom,
        version: exe_doom_1_9,
    },
    ValidVersion {
        mission: doom,
        version: exe_hacx,
    },
    ValidVersion {
        mission: doom,
        version: exe_ultimate,
    },
    ValidVersion {
        mission: doom,
        version: exe_final,
    },
    ValidVersion {
        mission: doom,
        version: exe_final2,
    },
    ValidVersion {
        mission: doom,
        version: exe_chex,
    },
    ValidVersion {
        mission: heretic,
        version: exe_heretic_1_3,
    },
    ValidVersion {
        mission: hexen,
        version: exe_hexen_1_1,
    },
    ValidVersion {
        mission: strife,
        version: exe_strife_1_2,
    },
    ValidVersion {
        mission: strife,
        version: exe_strife_1_31,
    },
];
