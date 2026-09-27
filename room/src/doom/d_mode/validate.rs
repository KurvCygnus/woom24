//! Boot-time validators over the game-mode / mission / version vocabulary:
//! membership queries and counters over the private tables, verbatim ports
//! of the `D_Valid*` family in `vendor/doomgeneric/d_mode.c`.

// The C enum vocabulary is lowercase (`doom`, `shareware`, ...); names
// are verbatim upstream data and the match patterns below reference
// them as-is, so the two name lints are silenced file-wide.
#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_int;

use super::consts::{doom, doom2, heretic, pack_chex, pack_hacx, pack_plut, pack_tnt, registered, retail};
use super::tables::{VALID_MODES, VALID_VERSIONS};
use crate::types::Boolean;

/// Return `TRUE` if `(mission, mode)` is a recognised game configuration.
///
/// Scans `VALID_MODES` for a matching entry. Used to validate a
/// game-mode/mission pair received over the network before accepting it.
/// Returns `FALSE` for unrecognised combinations (e.g., `doom2` + `shareware`).
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism: no C callers exist and no extern
//* declarer was found in-tree).
///
/// Corresponds to `D_ValidGameMode` in `d_mode.c`.
#[doc(alias = "D_ValidGameMode")]
#[export_name = "D_ValidGameMode"]
pub extern "C" fn valid_game_mode(mission: c_int, mode: c_int) -> Boolean
{
    for vm in &VALID_MODES
    {
        if vm.mission == mission && vm.mode == mode
        {
            return Boolean::TRUE;
        }
    }
    Boolean::FALSE
}

/// Return `TRUE` if `episode`/`map` is reachable in the given `(mission, mode)`.
///
/// Checks that `episode` and `map` are both at least 1 and do not exceed the
/// bounds recorded in `VALID_MODES`. Two Heretic-specific secret episodes are
/// handled as special cases before the table lookup:
///
/// * Heretic retail, episode 6: only maps 1-3 are valid (the secret episode).
/// * Heretic registered, episode 4: only map 1 is valid.
///
/// Returns `FALSE` for unknown mission/mode combinations.
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism: no C callers exist and no extern
//* declarer was found in-tree).
///
/// Corresponds to `D_ValidEpisodeMap` in `d_mode.c`.
#[doc(alias = "D_ValidEpisodeMap")]
#[export_name = "D_ValidEpisodeMap"]
pub extern "C" fn valid_episode_map(
    mission: c_int,
    mode: c_int,
    episode: c_int,
    map: c_int,
) -> Boolean
{
    // Hacks for Heretic secret episodes
    if mission == heretic
    {
        if mode == retail && episode == 6
        {
            return Boolean::from((1..=3).contains(&map));
        }
        else if mode == registered && episode == 4
        {
            return Boolean::from(map == 1);
        }
    }

    for vm in &VALID_MODES
    {
        if mission == vm.mission && mode == vm.mode
        {
            return Boolean::from(
                episode >= 1 && episode <= vm.episode && map >= 1 && map <= vm.map,
            );
        }
    }

    Boolean::FALSE
}

/// Return the number of valid episodes for the given `(mission, mode)`.
///
/// Increments an episode counter starting at 1, calling
/// [`valid_episode_map`] with map 1 until it returns false, then returns
/// the last valid episode number. Commercial games (Doom 2, Hexen, Strife)
/// have only episode 1. Returns 0 for unknown combinations.
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism: no C callers exist and no extern
//* declarer was found in-tree).
///
/// Corresponds to `D_GetNumEpisodes` in `d_mode.c`.
#[doc(alias = "D_GetNumEpisodes")]
#[export_name = "D_GetNumEpisodes"]
pub extern "C" fn num_episodes(mission: c_int, mode: c_int) -> c_int
{
    let mut episode = 1;
    while valid_episode_map(mission, mode, episode, 1).is_truthy()
    {
        episode += 1;
    }
    episode - 1
}

/// Return `TRUE` if `version` is a valid executable version for `mission`.
///
/// All Doom-family variants (`doom2`, `pack_plut`, `pack_tnt`, `pack_hacx`,
/// `pack_chex`) are normalised to `doom` before the lookup because they share
/// the same set of valid executable versions. Returns `FALSE` for unknown
/// combinations.
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism: no C callers exist and no extern
//* declarer was found in-tree).
///
/// Corresponds to `D_ValidGameVersion` in `d_mode.c`.
#[doc(alias = "D_ValidGameVersion")]
#[export_name = "D_ValidGameVersion"]
pub extern "C" fn valid_game_version(mission: c_int, version: c_int) -> Boolean
{
    let mission = if mission == doom2
        || mission == pack_plut
        || mission == pack_tnt
        || mission == pack_hacx
        || mission == pack_chex
    {
        doom
    }
    else
    {
        mission
    };

    for vv in &VALID_VERSIONS
    {
        if vv.mission == mission && vv.version == version
        {
            return Boolean::TRUE;
        }
    }

    Boolean::FALSE
}

/// Return `TRUE` if `mission` uses `ExMy` episode-map naming rather than `MAPxx`.
///
/// `doom`, `heretic`, and `pack_chex` use the `ExMy` format (e.g., `E1M1`).
/// All other missions use `MAPxx` (e.g., `MAP01`). This distinction drives
/// level-name formatting and warp/cheat parsing throughout the engine.
///
//* The pre-move export symbol is kept with `#[export_name]` below
//* (wasm-surface conservatism: no C callers exist and no extern
//* declarer was found in-tree).
///
/// Corresponds to `D_IsEpisodeMap` in `d_mode.c`.
#[doc(alias = "D_IsEpisodeMap")]
#[export_name = "D_IsEpisodeMap"]
pub extern "C" fn is_episode_map(mission: c_int) -> Boolean
{
    match mission
    {
        doom | heretic | pack_chex => Boolean::TRUE,
        _ => Boolean::FALSE,
    }
}

/// The graduation baseline: the known-vector module written against the
/// pre-move `D_Valid*` bodies, moved unchanged from `d_mode.rs` and
/// re-pointed to the graduated names with cross-root consts imported by
/// module-root path (F10 §1 convention (b)) -- same vectors, same results.
#[cfg(test)]
mod tests
{
    use super::*;
    use crate::doom::d_mode::{
        commercial, exe_doom_1_9, exe_final2, hexen, shareware, strife,
    };
    use crate::types::Boolean;

    /// Verifies that `doom` shareware is a valid game mode combination.
    #[test]
    fn valid_game_mode_doom_shareware()
    {
        assert_eq!(valid_game_mode(doom, shareware), Boolean::TRUE);
    }

    /// Verifies that `doom2` shareware is not a valid game mode combination.
    #[test]
    fn valid_game_mode_doom2_shareware_invalid()
    {
        assert_eq!(valid_game_mode(doom2, shareware), Boolean::FALSE);
    }

    /// Verifies that Doom retail has exactly 4 episodes.
    #[test]
    fn get_num_episodes_doom_retail()
    {
        assert_eq!(num_episodes(doom, retail), 4);
    }

    /// Verifies that `doom2` does not use the episode-map (`ExMy`) naming scheme.
    #[test]
    fn is_episode_map_doom2_false()
    {
        assert_eq!(is_episode_map(doom2), Boolean::FALSE);
    }

    /// Verifies that `doom` uses the episode-map (`ExMy`) naming scheme.
    #[test]
    fn is_episode_map_doom_true()
    {
        assert_eq!(is_episode_map(doom), Boolean::TRUE);
    }

    /// Verifies that `exe_final2` is a valid game version for the `doom` mission.
    #[test]
    fn valid_game_version_doom_final2()
    {
        assert_eq!(valid_game_version(doom, exe_final2), Boolean::TRUE);
    }

    /// Verifies that `doom2` is normalised to `doom` when checking game versions.
    #[test]
    fn valid_game_version_doom2_mapped_to_doom()
    {
        assert_eq!(valid_game_version(doom2, exe_final2), Boolean::TRUE);
    }

    /// Verifies that episode 4 map 9 is valid for Doom retail.
    #[test]
    fn valid_episode_map_doom_retail_ep4_map9()
    {
        assert_eq!(valid_episode_map(doom, retail, 4, 9), Boolean::TRUE);
    }

    /// Verifies that episode 5 is out of bounds for Doom retail (max is 4).
    #[test]
    fn valid_episode_map_doom_retail_ep5_map1_invalid()
    {
        assert_eq!(valid_episode_map(doom, retail, 5, 1), Boolean::FALSE);
    }

    /// Heretic retail secret episode 6 allows maps 1-3 only.
    #[test]
    fn valid_episode_map_heretic_retail_ep6_maps_1_to_3()
    {
        assert_eq!(valid_episode_map(heretic, retail, 6, 1), Boolean::TRUE);
        assert_eq!(valid_episode_map(heretic, retail, 6, 2), Boolean::TRUE);
        assert_eq!(valid_episode_map(heretic, retail, 6, 3), Boolean::TRUE);
        assert_eq!(valid_episode_map(heretic, retail, 6, 4), Boolean::FALSE);
        assert_eq!(valid_episode_map(heretic, retail, 6, 0), Boolean::FALSE);
    }

    /// Heretic registered secret episode 4 allows only map 1.
    #[test]
    fn valid_episode_map_heretic_registered_ep4_map1_only()
    {
        assert_eq!(valid_episode_map(heretic, registered, 4, 1), Boolean::TRUE);
        assert_eq!(valid_episode_map(heretic, registered, 4, 2), Boolean::FALSE);
        assert_eq!(valid_episode_map(heretic, registered, 4, 0), Boolean::FALSE);
    }

    /// Doom 2 only has episode 1; requesting episode 2 should fail.
    #[test]
    fn valid_episode_map_doom2_ep2_invalid()
    {
        assert_eq!(valid_episode_map(doom2, commercial, 2, 1), Boolean::FALSE);
        assert_eq!(valid_episode_map(doom2, commercial, 1, 1), Boolean::TRUE);
        assert_eq!(valid_episode_map(doom2, commercial, 1, 32), Boolean::TRUE);
        assert_eq!(valid_episode_map(doom2, commercial, 1, 33), Boolean::FALSE);
    }

    /// Map 0 is always invalid.
    #[test]
    fn valid_episode_map_map_zero_invalid()
    {
        assert_eq!(valid_episode_map(doom, retail, 1, 0), Boolean::FALSE);
        assert_eq!(valid_episode_map(doom2, commercial, 1, 0), Boolean::FALSE);
    }

    /// D_GetNumEpisodes for doom shareware has 1 episode.
    #[test]
    fn get_num_episodes_doom_shareware()
    {
        assert_eq!(num_episodes(doom, shareware), 1);
    }

    /// D_GetNumEpisodes for doom2 (commercial) returns 1.
    #[test]
    fn get_num_episodes_doom2()
    {
        assert_eq!(num_episodes(doom2, commercial), 1);
    }

    /// D_GetNumEpisodes for doom registered returns 3.
    #[test]
    fn get_num_episodes_doom_registered()
    {
        assert_eq!(num_episodes(doom, registered), 3);
    }

    /// D_IsEpisodeMap is true for pack_chex (chex.wad is episode-based).
    #[test]
    fn is_episode_map_pack_chex_true()
    {
        assert_eq!(is_episode_map(pack_chex), Boolean::TRUE);
    }

    /// pack_tnt / pack_plut are commercial (MAP01–MAP32), not episode-based.
    #[test]
    fn is_episode_map_pack_tnt_false()
    {
        assert_eq!(is_episode_map(pack_tnt), Boolean::FALSE);
        assert_eq!(is_episode_map(pack_plut), Boolean::FALSE);
    }

    /// D_ValidGameMode: all expected valid combinations succeed.
    #[test]
    fn valid_game_mode_all_missions()
    {
        assert_eq!(valid_game_mode(doom, retail), Boolean::TRUE);
        assert_eq!(valid_game_mode(doom, registered), Boolean::TRUE);
        assert_eq!(valid_game_mode(doom2, commercial), Boolean::TRUE);
        assert_eq!(valid_game_mode(heretic, shareware), Boolean::TRUE);
        assert_eq!(valid_game_mode(hexen, commercial), Boolean::TRUE);
        assert_eq!(valid_game_mode(strife, commercial), Boolean::TRUE);
    }

    /// D_ValidGameVersion: doom2 maps to doom for version checks.
    #[test]
    fn valid_game_version_all_doom_variants_map_to_doom()
    {
        for mission in [doom2, pack_plut, pack_tnt, pack_hacx, pack_chex]
        {
            assert_eq!(
                valid_game_version(mission, exe_doom_1_9),
                Boolean::TRUE,
                "mission {mission} should accept exe_doom_1_9"
            );
        }
    }
}
