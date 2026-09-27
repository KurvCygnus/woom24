//! The numeric vocabulary of the special-sector/line dispatcher: the
//! animation table bounds, the adjoining-sector overrun bound, the door /
//! floor / ceiling / platform / stair movement-type codes the cross-line
//! dispatch passes to the `EV_*` action functions, and the radiation-suit
//! powers index -- bit-exact with the constants of
//! `vendor/doomgeneric/p_spec.c`.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;

/// Maximum number of animated texture/flat cycles that can be active at once.
/// Corresponds to `MAXANIMS` in p_spec.c.
pub(super) const MAXANIMS: usize = 32;

/// Maximum number of scrolling-wall linedefs that may be registered per level.
/// Corresponds to `MAXLINEANIMS` in p_spec.c (Vanilla Doom limit is 64).
pub(super) const MAXLINEANIMS: usize = 64;

/// Maximum number of adjoining sectors examined by `P_FindNextHighestFloor`.
/// Exceeding this limit emulates the vanilla stack-overflow behaviour.
/// Corresponds to `MAX_ADJOINING_SECTORS` in p_spec.c.
pub(super) const MAX_ADJOINING_SECTORS: usize = 20;

// vldoor_e — door direction/behaviour constants used by `EV_DoDoor`.
/// Door opens and stays open. (`vld_open`)
pub(super) const vld_normal: c_int = 0;
/// Door closes, waits 30 seconds, then opens again. (`vld_close30ThenOpen`)
pub(super) const vld_close30ThenOpen: c_int = 1;
/// Door closes and stays closed. (`vld_close`)
pub(super) const vld_close: c_int = 2;
/// Door opens normally (raise then wait then lower). (`vld_open` in C is 3 here mapped as `vld_normal=0`)
pub(super) const vld_open: c_int = 3;
/// Blaze-speed raise-then-lower door. (`vld_blazeRaise`)
pub(super) const vld_blazeRaise: c_int = 5;
/// Blaze-speed open door. (`vld_blazeOpen`)
pub(super) const vld_blazeOpen: c_int = 6;
/// Blaze-speed close door. (`vld_blazeClose`)
pub(super) const vld_blazeClose: c_int = 7;

// floor_e — floor movement type constants used by `EV_DoFloor`.
/// Lower floor to next lowest neighbouring floor. (`lowerFloor`)
pub(super) const lowerFloor: c_int = 0;
/// Lower floor to the absolute lowest neighbouring floor. (`lowerFloorToLowest`)
pub(super) const lowerFloorToLowest: c_int = 1;
/// Lower floor at turbo speed. (`turboLower`)
pub(super) const turboLower: c_int = 2;
/// Raise floor to next highest neighbouring floor. (`raiseFloor`)
pub(super) const raiseFloor: c_int = 3;
/// Raise floor to nearest neighbouring floor. (`raiseFloorToNearest`)
pub(super) const raiseFloorToNearest: c_int = 4;
/// Raise floor to the height of the shortest lower texture on a bounding wall. (`raiseToTexture`)
pub(super) const raiseToTexture: c_int = 5;
/// Lower floor and change its texture/special to match the destination sector. (`lowerAndChange`)
pub(super) const lowerAndChange: c_int = 6;
/// Raise floor by exactly 24 map units (fixed-point 16.16). (`raiseFloor24`)
pub(super) const raiseFloor24: c_int = 7;
/// Raise floor by 24 map units and change texture/special. (`raiseFloor24AndChange`)
pub(super) const raiseFloor24AndChange: c_int = 8;
/// Raise floor while crushing anything caught between floor and ceiling. (`raiseFloorCrush`)
pub(super) const raiseFloorCrush: c_int = 9;
/// Raise floor at turbo speed. (`raiseFloorTurbo`)
pub(super) const raiseFloorTurbo: c_int = 10;
/// Raise floor to the height of the outer (ring) sector in a donut effect. (`donutRaise`)
pub(super) const donutRaise: c_int = 11;

// ceiling_e — ceiling movement type constants used by `EV_DoCeiling`.
/// Raise ceiling to the highest neighbouring ceiling. (`raiseToHighest`)
pub(super) const raiseToHighest: c_int = 1;
/// Lower ceiling while crushing. (`lowerAndCrush`)
pub(super) const lowerAndCrush: c_int = 2;
/// Crush ceiling down then raise it repeatedly. (`crushAndRaise`)
pub(super) const crushAndRaise: c_int = 3;
/// Fast crush-and-raise ceiling. (`fastCrushAndRaise`)
pub(super) const fastCrushAndRaise: c_int = 4;
/// Silent crush-and-raise ceiling (no grinding sound). (`silentCrushAndRaise`)
pub(super) const silentCrushAndRaise: c_int = 5;

// plattype_e — platform movement type constants used by `EV_DoPlat`.
/// Platform goes down, waits, then rises back up. (`downWaitUpStay`)
pub(super) const downWaitUpStay: c_int = 1;
/// Platform raises to the nearest floor height and changes texture. (`raiseToNearestAndChange`)
pub(super) const raiseToNearestAndChange: c_int = 3;
/// Blaze-speed down-wait-up-stay platform. (`blazeDWUS`)
pub(super) const blazeDWUS: c_int = 4;
/// Platform oscillates perpetually up and down. (`perpetualRaise`)
pub(super) const perpetualRaise: c_int = 0;

// stair_e — stair-building step size constants used by `EV_BuildStairs`.
/// Build stairs in 8-unit steps. (`build8`)
pub(super) const build8: c_int = 0;
/// Build stairs in 16-unit turbo steps. (`turbo16`)
pub(super) const turbo16: c_int = 1;

// powers — index into `player_t::powers[]`.
/// Index of the Radiation Suit power in the powers array; grants immunity to slime damage.
pub(super) const pw_ironfeet: usize = 3;

#[cfg(test)]
mod tests
{
    use std::ffi::c_int;

    use crate::doom::d_player::CF_GODMODE;
    use crate::doom::sounds::Sfx;
    use crate::doom::z_zone::PU_LEVSPEC;

    use super::*;

    /// Carried verbatim from the pre-split module (`p_spec.rs`
    /// `constants_match_c`): the dispatcher's limits and vocabulary
    /// match the C constants exactly.
    #[test]
    fn constants_match_c()
    {
        assert_eq!(MAXANIMS, 32);
        assert_eq!(MAXLINEANIMS, 64);
        assert_eq!(MAX_ADJOINING_SECTORS, 20);
        assert_eq!(PU_LEVSPEC, 6);
        assert_eq!(Sfx::Swtchn as c_int, 23);
        assert_eq!(CF_GODMODE, 2);
        assert_eq!(pw_ironfeet, 3);
    }
}
