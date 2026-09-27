//! Extracted demo-synchronization surface of the monster AI: the
//! weighted monster pick of the Icon of Sin cube arrival. The module
//! is the engine's heaviest `P_Random` consumer; its RNG ledger is
//! documented in the parent's Deterministic Aspects section and every
//! draw stays at its call site.

use std::os::raw::c_int;

use crate::doom::info::{
    MT_BABY, MT_BRUISER, MT_FATSO, MT_HEAD, MT_KNIGHT, MT_PAIN, MT_SERGEANT, MT_SHADOWS, MT_TROOP,
    MT_UNDEAD, MT_VILE,
};

use super::consts::mobjtype_t;

/// The pure weighted monster table of `A_SpawnFly`, extracted verbatim
/// from the if-else chain that consumed the drawn byte (`r`, one
/// `P_Random()` draw kept at the call site). Imp band 50/256 through
/// the Baron of Hell top band; the eleven thresholds tile `0..=255`
/// exactly, so the drawn byte determines the spawned monster type
/// one-to-one. This pick is demo-observable on every Icon of Sin
/// arrival: the same byte must always select the same monster.
///
/// No named C counterpart (upstream keeps the chain inline in
/// `A_SpawnFly`, `p_enemy.c:1946-1966`), so no `#[doc(alias)]`.
///
/// ## Technical Details
///
/// The comparison chain is strictly `<` on `c_int` bands
/// (50/90/120/130/160/162/172/192/222/246); the trailing `else` is the
/// 246..=255 band. Never "simplify" to a lookup table with `>=`
/// bounds or reordered arms -- any reordering changes which monster a
/// boundary byte spawns (the 161/162 and 245/246 one-wide Vile/Knight
/// adjacent bands are the sharpest edges).
///
/// ## On Calling
///
/// Pure function over the drawn byte; draws exactly nothing itself.
/// Call with the raw `P_Random()` result only -- pre-masking or
/// re-rolling the byte is a demo desync by construction.
pub(super) fn spawn_fly_pick(r: c_int) -> mobjtype_t
{
    if r < 50 as c_int
    {
        MT_TROOP
    }
    else if r < 90 as c_int
    {
        MT_SERGEANT
    }
    else if r < 120 as c_int
    {
        MT_SHADOWS
    }
    else if r < 130 as c_int
    {
        MT_PAIN
    }
    else if r < 160 as c_int
    {
        MT_HEAD
    }
    else if r < 162 as c_int
    {
        MT_VILE
    }
    else if r < 172 as c_int
    {
        MT_UNDEAD
    }
    else if r < 192 as c_int
    {
        MT_BABY
    }
    else if r < 222 as c_int
    {
        MT_FATSO
    }
    else if r < 246 as c_int
    {
        MT_KNIGHT
    }
    else
    {
        MT_BRUISER
    }
}

#[cfg(test)]
mod tests
{
    // --- F10 wave B3c baseline vectors, retargeted post-move (F10 §2.3) ---

    /// Baseline vectors: each of the eleven threshold boundaries (49/50
    /// through 245/246) plus the 0 and 255 rails -- the drawn byte lands
    /// exactly one monster per band, and the bands tile 0..=255 with no
    /// gap or overlap. Retargeted onto `dtmc::spawn_fly_pick` (pre-move
    /// commit `7d60051` ran the same vectors against the in-file
    /// transcription of `A_SpawnFly`'s chain).
    #[test]
    fn baseline_spawn_fly_pick_thresholds()
    {
        use crate::doom::info::{
            MT_BABY, MT_BRUISER, MT_FATSO, MT_HEAD, MT_KNIGHT, MT_PAIN, MT_SERGEANT, MT_SHADOWS,
            MT_TROOP, MT_UNDEAD, MT_VILE,
        };
        assert_eq!(super::spawn_fly_pick(0), MT_TROOP);
        assert_eq!(super::spawn_fly_pick(49), MT_TROOP);
        assert_eq!(super::spawn_fly_pick(50), MT_SERGEANT);
        assert_eq!(super::spawn_fly_pick(89), MT_SERGEANT);
        assert_eq!(super::spawn_fly_pick(90), MT_SHADOWS);
        assert_eq!(super::spawn_fly_pick(119), MT_SHADOWS);
        assert_eq!(super::spawn_fly_pick(120), MT_PAIN);
        assert_eq!(super::spawn_fly_pick(129), MT_PAIN);
        assert_eq!(super::spawn_fly_pick(130), MT_HEAD);
        assert_eq!(super::spawn_fly_pick(159), MT_HEAD);
        assert_eq!(super::spawn_fly_pick(160), MT_VILE);
        assert_eq!(super::spawn_fly_pick(161), MT_VILE);
        assert_eq!(super::spawn_fly_pick(162), MT_UNDEAD);
        assert_eq!(super::spawn_fly_pick(171), MT_UNDEAD);
        assert_eq!(super::spawn_fly_pick(172), MT_BABY);
        assert_eq!(super::spawn_fly_pick(191), MT_BABY);
        assert_eq!(super::spawn_fly_pick(192), MT_FATSO);
        assert_eq!(super::spawn_fly_pick(221), MT_FATSO);
        assert_eq!(super::spawn_fly_pick(222), MT_KNIGHT);
        assert_eq!(super::spawn_fly_pick(245), MT_KNIGHT);
        assert_eq!(super::spawn_fly_pick(246), MT_BRUISER);
        assert_eq!(super::spawn_fly_pick(255), MT_BRUISER);
    }
}
