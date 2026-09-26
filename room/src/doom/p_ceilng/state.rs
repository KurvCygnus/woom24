//! Ceiling state and vocabulary: the `repr(C)` `ceiling_t` thinker
//! state, the fixed-size `activeceilings` table, and the
//! speed/type/return-value constants -- bit-exact with the data half
//! of `vendor/doomgeneric/p_ceilng.c` and `p_local.h`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::os::raw::c_int;

use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_lights::sector_t;
use crate::doom::p_tick::thinker_t;

/// Default ceiling movement speed: 1 fixed-point unit per tic (`FRACUNIT`).
///
/// Crusher ceilings slow to `CEILSPEED / 8` when they hit something, and fast
/// crushers move at `CEILSPEED * 2`.  Matches `CEILSPEED` in `p_ceilng.c`.
pub(super) const CEILSPEED: fixed_t = FRACUNIT;

/// Maximum number of simultaneously active ceiling movers.
///
/// Matches `MAXCEILINGS` in `p_local.h`.  If all slots are occupied,
/// [`P_AddActiveCeiling`](crate::doom::p_ceilng::P_AddActiveCeiling) silently
/// discards new ceilings.
pub const MAXCEILINGS: usize = 30;

// result_e enum values returned by T_MovePlane.
/// `T_MovePlane` result: the plane moved without incident.
pub(super) const result_ok: c_int = 0;
/// `T_MovePlane` result: the plane crushed something while moving.
pub(super) const result_crushed: c_int = 1;
/// `T_MovePlane` result: the plane reached its destination height.
pub(super) const result_pastdest: c_int = 2;

// ceiling_e enum values — ceiling mover types.
/// Lower ceiling until it reaches the floor height of the sector.
pub(super) const lowerToFloor: c_int = 0;
/// Raise ceiling to the highest surrounding ceiling height.
pub(super) const raiseToHighest: c_int = 1;
/// Lower ceiling to 8 units above the floor, crushing things in the way.
pub(super) const lowerAndCrush: c_int = 2;
/// Crusher that bounces between floor+8 and its original height; normal speed.
pub(super) const crushAndRaise: c_int = 3;
/// Crusher that bounces at twice the normal speed.
pub(super) const fastCrushAndRaise: c_int = 4;
/// Like [`crushAndRaise`] but does not play the movement sound each tic.
pub(super) const silentCrushAndRaise: c_int = 5;

/// Per-sector thinker for an active ceiling mover.
///
/// Allocated via `Z_Malloc` and linked into the thinker list.  The `thinker`
/// field must be at offset 0 (verified by the compile-time assertions in
/// `layout_checks`) so that a `*mut ceiling_t` can be cast safely to
/// `*mut thinker_t`.
///
/// Layout matches the C `ceiling_t` struct.  Padding (`_pad0`, `_pad1`)
/// preserves C alignment on 64-bit targets.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ceiling_t
{
    /// Thinker header — must be at offset 0.
    pub thinker: thinker_t,
    /// Ceiling type (one of the `ceiling_e` integer constants).
    pub r#type: c_int,
    _pad0: [u8; 4],
    /// The sector whose ceiling this thinker is moving.
    pub sector: *mut sector_t,
    /// Target height when the ceiling is moving downward.
    pub bottomheight: fixed_t,
    /// Target height when the ceiling is moving upward.
    pub topheight: fixed_t,
    /// Current movement speed in fixed-point units per tic.
    pub speed: fixed_t,
    /// Non-zero if the ceiling damages things it crushes.
    pub crush: c_int,
    /// Current movement direction: `1` = up, `-1` = down, `0` = in stasis.
    pub direction: c_int,
    /// Linedef tag that activated this ceiling (used for crush-stop matching).
    pub tag: c_int,
    /// Saved direction used to resume a ceiling that was put into stasis.
    pub olddirection: c_int,
    _pad1: [u8; 4],
}

#[cfg(target_pointer_width = "64")]
mod layout_checks
{
    use super::*;
    const _: () = assert!(std::mem::size_of::<ceiling_t>() == 72);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, thinker) == 0);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, r#type) == 24);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, sector) == 32);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, bottomheight) == 40);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, topheight) == 44);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, speed) == 48);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, crush) == 52);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, direction) == 56);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, tag) == 60);
    const _: () = assert!(std::mem::offset_of!(ceiling_t, olddirection) == 64);
}

/// Table of all currently active ceiling thinkers.
///
/// Null entries represent free slots.
/// [`P_AddActiveCeiling`](crate::doom::p_ceilng::P_AddActiveCeiling) fills the
/// first null slot;
/// [`P_RemoveActiveCeiling`](crate::doom::p_ceilng::P_RemoveActiveCeiling)
/// nulls the matching slot.  Exported as `activeceilings` for C linkage.
#[no_mangle]
pub static mut activeceilings: [*mut ceiling_t; MAXCEILINGS] =
    [std::ptr::null_mut(); MAXCEILINGS];

#[cfg(test)]
mod tests
{
    use crate::doom::p_ceilng::ceiling_t;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const CEILING_T_SIZEOF: usize = 72;
    const CEILING_T_THINKER: usize = 0;
    const CEILING_T_TYPE: usize = 24;
    const CEILING_T_SECTOR: usize = 32;
    const CEILING_T_BOTTOMHEIGHT: usize = 40;
    const CEILING_T_TOPHEIGHT: usize = 44;
    const CEILING_T_SPEED: usize = 48;
    const CEILING_T_CRUSH: usize = 52;
    const CEILING_T_DIRECTION: usize = 56;
    const CEILING_T_TAG: usize = 60;
    const CEILING_T_OLDDIRECTION: usize = 64;

    /// `ceiling_t` must keep the exact C layout (72 bytes): the thinker
    /// state is `Z_Malloc`-allocated by `size_of` and `p_saveg` reads
    /// the struct field-wise through the module-root path, so size
    /// and field offsets are load-bearing.
    #[test]
    fn ceiling_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<ceiling_t>(), CEILING_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(ceiling_t, thinker), CEILING_T_THINKER);
        assert_eq!(std::mem::offset_of!(ceiling_t, r#type), CEILING_T_TYPE);
        assert_eq!(std::mem::offset_of!(ceiling_t, sector), CEILING_T_SECTOR);
        assert_eq!(
            std::mem::offset_of!(ceiling_t, bottomheight),
            CEILING_T_BOTTOMHEIGHT
        );
        assert_eq!(
            std::mem::offset_of!(ceiling_t, topheight),
            CEILING_T_TOPHEIGHT
        );
        assert_eq!(std::mem::offset_of!(ceiling_t, speed), CEILING_T_SPEED);
        assert_eq!(std::mem::offset_of!(ceiling_t, crush), CEILING_T_CRUSH);
        assert_eq!(
            std::mem::offset_of!(ceiling_t, direction),
            CEILING_T_DIRECTION
        );
        assert_eq!(std::mem::offset_of!(ceiling_t, tag), CEILING_T_TAG);
        assert_eq!(
            std::mem::offset_of!(ceiling_t, olddirection),
            CEILING_T_OLDDIRECTION
        );
    }
}
