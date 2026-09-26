//! Plat state and vocabulary: the `repr(C)` `plat_t` thinker state,
//! the fixed-size `activeplats` table, and the movement/type/return
//! constants -- bit-exact with the data half of
//! `vendor/doomgeneric/p_plats.c` and `p_local.h`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::os::raw::c_int;

use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_lights::sector_t;
use crate::doom::p_tick::thinker_t;

/// Default platform movement speed: 1 fixed-point unit per tic.
///
/// Individual platform types multiply or divide this value:
/// `downWaitUpStay` uses `PLATSPEED * 4`, `blazeDWUS` uses `PLATSPEED * 8`,
/// and `raiseAndChange`/`raiseToNearestAndChange` use `PLATSPEED / 2`.
pub(super) const PLATSPEED: fixed_t = FRACUNIT;

/// Unitless platform wait count; multiplied by `TICRATE` at call sites to give
/// the actual wait duration in tics (3 × 35 = 105 tics ≈ 3 s at 35 Hz).
pub(super) const PLATWAIT: c_int = 3;

/// Maximum number of simultaneously active platform thinkers.
///
/// Matches `MAXPLATS` in `p_local.h`.  If all slots are full,
/// [`P_AddActivePlat`](crate::doom::p_plats::P_AddActivePlat) calls `I_Error`
/// and the engine aborts.
pub(super) const MAXPLATS: usize = 30;

// result_e enum values returned by T_MovePlane.
/// `T_MovePlane` result: the floor moved without incident.
pub(super) const result_ok: c_int = 0;
/// `T_MovePlane` result: the floor crushed something while moving.
pub(super) const result_crushed: c_int = 1;
/// `T_MovePlane` result: the floor reached its destination height.
pub(super) const result_pastdest: c_int = 2;

// plat_e enum values — current platform motion state.
/// Platform is moving upward toward `high`.
pub(super) const up: c_int = 0;
/// Platform is moving downward toward `low`.
pub(super) const down: c_int = 1;
/// Platform has reached a target and is counting down before reversing.
pub(super) const waiting: c_int = 2;
/// Platform has been suspended; its thinker callback is suppressed.
pub(super) const in_stasis: c_int = 3;

// plattype_e enum values — platform behaviour types.
/// Perpetually bounces between the lowest and highest surrounding floor heights.
pub(super) const perpetualRaise: c_int = 0;
/// Lowers to the lowest surrounding floor, waits, then rises back and stops.
pub(super) const downWaitUpStay: c_int = 1;
/// Rises by `amount` units while matching the front sidedef's floor texture,
/// then stops.
pub(super) const raiseAndChange: c_int = 2;
/// Rises to the next higher surrounding floor while matching texture, then
/// stops.
pub(super) const raiseToNearestAndChange: c_int = 3;
/// Like [`downWaitUpStay`] but at twice the speed (`PLATSPEED * 8`).
pub(super) const blazeDWUS: c_int = 4;

/// Per-sector thinker for an active platform mover.
///
/// Allocated via `Z_Malloc` and linked into the thinker list.  The `thinker`
/// field must be at offset 0 (verified by the compile-time assertions in
/// `layout_checks`) so that a `*mut plat_t` can be cast safely to
/// `*mut thinker_t`.
///
/// Layout matches the C `plat_t` struct.  The overall size is 72 bytes on
/// 64-bit targets.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct plat_t
{
    /// Thinker header — must be at offset 0.
    pub thinker: thinker_t,
    /// The sector whose floor this thinker is moving.
    pub sector: *mut sector_t,
    /// Current movement speed in fixed-point units per tic.
    pub speed: fixed_t,
    /// Lower target height (destination when moving down).
    pub low: fixed_t,
    /// Upper target height (destination when moving up).
    pub high: fixed_t,
    /// Total tics to wait at a target before reversing (set from `PLATWAIT`).
    pub wait: c_int,
    /// Remaining tics in the current wait period; decremented each tic.
    pub count: c_int,
    /// Current motion state: one of `up`, `down`, `waiting`, `in_stasis`.
    pub status: c_int,
    /// Saved status used to resume after stasis.
    pub oldstatus: c_int,
    /// Non-zero if the platform damages things it crushes (unused for plats).
    pub crush: c_int,
    /// Linedef tag that activated this platform.
    pub tag: c_int,
    /// Platform behaviour type (one of the `plattype_e` integer constants).
    pub r#type: c_int,
}

#[cfg(target_pointer_width = "64")]
mod layout_checks
{
    use super::*;
    const _: () = assert!(std::mem::size_of::<plat_t>() == 72);
    const _: () = assert!(std::mem::offset_of!(plat_t, thinker) == 0);
    const _: () = assert!(std::mem::offset_of!(plat_t, sector) == 24);
    const _: () = assert!(std::mem::offset_of!(plat_t, speed) == 32);
    const _: () = assert!(std::mem::offset_of!(plat_t, low) == 36);
    const _: () = assert!(std::mem::offset_of!(plat_t, high) == 40);
    const _: () = assert!(std::mem::offset_of!(plat_t, wait) == 44);
    const _: () = assert!(std::mem::offset_of!(plat_t, count) == 48);
    const _: () = assert!(std::mem::offset_of!(plat_t, status) == 52);
    const _: () = assert!(std::mem::offset_of!(plat_t, oldstatus) == 56);
    const _: () = assert!(std::mem::offset_of!(plat_t, crush) == 60);
    const _: () = assert!(std::mem::offset_of!(plat_t, tag) == 64);
    const _: () = assert!(std::mem::offset_of!(plat_t, r#type) == 68);
}

/// Table of all currently active platform thinkers.
///
/// Null entries represent free slots.
/// [`P_AddActivePlat`](crate::doom::p_plats::P_AddActivePlat) fills the first
/// null slot;
/// [`P_RemoveActivePlat`](crate::doom::p_plats::P_RemoveActivePlat) nulls the
/// matching slot.  Unlike ceilings, overflowing this table is a fatal error.
///
/// Exported as `activeplats` for C linkage.
#[no_mangle]
pub static mut activeplats: [*mut plat_t; MAXPLATS] = [std::ptr::null_mut(); MAXPLATS];

#[cfg(test)]
mod tests
{
    use crate::doom::p_plats::plat_t;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const PLAT_T_SIZEOF: usize = 72;
    const PLAT_T_THINKER: usize = 0;
    const PLAT_T_SECTOR: usize = 24;
    const PLAT_T_SPEED: usize = 32;
    const PLAT_T_LOW: usize = 36;
    const PLAT_T_HIGH: usize = 40;
    const PLAT_T_WAIT: usize = 44;
    const PLAT_T_COUNT: usize = 48;
    const PLAT_T_STATUS: usize = 52;
    const PLAT_T_OLDSTATUS: usize = 56;
    const PLAT_T_CRUSH: usize = 60;
    const PLAT_T_TAG: usize = 64;
    const PLAT_T_TYPE: usize = 68;

    /// `plat_t` must keep the exact C layout (72 bytes): the thinker
    /// state is `Z_Malloc`-allocated by `size_of` and `p_saveg`
    /// reads the struct field-wise through the module-root path, so
    /// size and field offsets are load-bearing.
    #[test]
    fn plat_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<plat_t>(), PLAT_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(plat_t, thinker), PLAT_T_THINKER);
        assert_eq!(std::mem::offset_of!(plat_t, sector), PLAT_T_SECTOR);
        assert_eq!(std::mem::offset_of!(plat_t, speed), PLAT_T_SPEED);
        assert_eq!(std::mem::offset_of!(plat_t, low), PLAT_T_LOW);
        assert_eq!(std::mem::offset_of!(plat_t, high), PLAT_T_HIGH);
        assert_eq!(std::mem::offset_of!(plat_t, wait), PLAT_T_WAIT);
        assert_eq!(std::mem::offset_of!(plat_t, count), PLAT_T_COUNT);
        assert_eq!(std::mem::offset_of!(plat_t, status), PLAT_T_STATUS);
        assert_eq!(std::mem::offset_of!(plat_t, oldstatus), PLAT_T_OLDSTATUS);
        assert_eq!(std::mem::offset_of!(plat_t, crush), PLAT_T_CRUSH);
        assert_eq!(std::mem::offset_of!(plat_t, tag), PLAT_T_TAG);
        assert_eq!(std::mem::offset_of!(plat_t, r#type), PLAT_T_TYPE);
    }
}
