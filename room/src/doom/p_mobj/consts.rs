//! `p_mobj`'s numeric vocabulary: movement friction and stopping
//! constants, per-tic momentum and gravity, position sentinels, spawn
//! defaults, the thing-flag and player-state values, and the
//! game-version threshold for the Lost Soul bounce fix -- bit-exact
//! with `vendor/doomgeneric/p_mobj.c` and `p_local.h`.
//!
//! Data tier: upstream names retained (2026-09-27 ruling -- functions
//! only are renamed this round).

#![allow(non_upper_case_globals)]

use std::os::raw::c_int;

use crate::doom::m_fixed::FRACUNIT;

/// Momentum magnitude below which an object's XY velocity is snapped to zero.
pub(super) const STOPSPEED: c_int = 0x1000;
/// Multiplicative XY friction applied each tic when an object is on the floor
/// (0xe800 / 0x10000 ≈ 0.906).
pub(super) const FRICTION: c_int = 0xe800;
/// Maximum XY momentum per tic; momentum is clamped to this before integration.
pub(super) const MAXMOVE: c_int = 30 * FRACUNIT;
/// Gravitational acceleration applied to falling objects each tic (1 fixed unit).
pub(super) const GRAVITY: c_int = FRACUNIT;
/// Vertical speed at which floating monsters adjust their altitude each tic.
pub(super) const FLOATSPEED: c_int = FRACUNIT * 4;
/// Sentinel Z value meaning "place on the floor of the current sector".
pub(super) const ONFLOORZ: c_int = i32::MIN;
/// Sentinel Z value meaning "place on the ceiling of the current sector".
pub(super) const ONCEILINGZ: c_int = i32::MAX;
/// Default eye height above the floor for a live player (41 fixed units).
pub(super) const VIEWHEIGHT: c_int = 41 * FRACUNIT;
/// Maximum reach for melee contact checks (64 map units in fixed-point).
pub(super) const MELEERANGE: c_int = 64 * FRACUNIT;

/// Thing-flag option bit: the thing was placed in ambush mode in the map editor.
pub(super) const MTF_AMBUSH: c_int = 8;

/// Player state: alive and playing.
pub(super) const PST_LIVE: c_int = 0;
/// Player state: needs to respawn (set after death, cleared by
/// [`super::mapthings::spawn_player`]).
pub(super) const PST_REBORN: c_int = 2;

/// Game-version threshold at or above which the Lost Soul floor-bounce bug is
/// corrected (corresponds to Ultimate Doom / `exe_ultimate`).
pub(super) const exe_ultimate: c_int = 6;

#[cfg(test)]
mod tests
{
    use std::os::raw::c_int;

    use crate::doom::m_fixed::FRACUNIT;

    use super::{
        exe_ultimate, FRICTION, GRAVITY, MAXMOVE, MELEERANGE, ONCEILINGZ, ONFLOORZ, STOPSPEED,
        VIEWHEIGHT,
    };

    /// Constant pin (moved with its subject from the pre-split
    /// `p_mobj.rs` suite): the stopping threshold is `0x1000`, both in
    /// the C hex spelling and as plain decimal.
    #[test]
    fn stopspeed_is_0x1000()
    {
        assert_eq!(STOPSPEED, 0x1000);
        assert_eq!(STOPSPEED, 4096);
    }

    /// Constant pin: `FRICTION` is `0xe800`; the `u32` cast matches the
    /// C declaration, which treats the hex literal as unsigned before
    /// storing it in the signed `int` global.
    #[test]
    fn friction_is_0xe800()
    {
        assert_eq!(FRICTION, 0xe800_u32 as i32);
        assert_eq!(FRICTION, 59392_u32 as i32);
    }

    /// Constant pin: friction below 1.0 is what makes friction
    /// decelerate.
    #[test]
    fn friction_is_less_than_fracunit()
    {
        assert!(
            FRICTION < FRACUNIT,
            "FRICTION must be < FRACUNIT for deceleration"
        );
    }

    /// Constant pin: the stop snap engages at 1/16 of a map unit.
    #[test]
    fn stopspeed_is_small_fraction_of_fracunit()
    {
        assert!(STOPSPEED < FRACUNIT);
        assert_eq!(FRACUNIT / STOPSPEED, 16);
    }

    /// Constant pins for the movement budget, the position sentinels,
    /// the melee reach, the eye height, and the Lost Soul bounce
    /// version gate -- all carried verbatim from the pre-split suite.
    #[test]
    fn movement_and_position_constants_are_pinned()
    {
        assert_eq!(MAXMOVE, 30 * FRACUNIT);
        assert_eq!(GRAVITY, FRACUNIT);
        assert_eq!(ONFLOORZ, i32::MIN);
        assert_eq!(ONCEILINGZ, i32::MAX);
        assert_eq!(VIEWHEIGHT, 41 * FRACUNIT);
        assert_eq!(MELEERANGE, 64 * FRACUNIT);
        // exe_doom1_9 < exe_ultimate <= exe_ultimate family threshold.
        assert_eq!(exe_ultimate as c_int, 6);
    }
}
