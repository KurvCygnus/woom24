//! Door state and vocabulary: the `repr(C)` `vldoor_t` thinker state,
//! the door-type / movement / result / keycard constants, and the
//! locked-door DeHacked message helpers -- bit-exact with the data
//! half of `vendor/doomgeneric/p_doors.c` and `p_local.h`.

// Same belt-and-suspenders as p_spec/anims.rs: `vldoor_t` and the other
// repr(C) type names here carry the allow regardless of toolchain probing.
#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_char;
use std::os::raw::c_int;

use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_lights::sector_t;
use crate::doom::p_tick::thinker_t;

/// Movement speed for normal vertical doors (in fixed-point units per tic).
pub(super) const VDOORSPEED: fixed_t = FRACUNIT * 2;

/// Number of tics a normal door waits at the top before closing (`VDOORWAIT` in C).
pub(super) const VDOORWAIT: c_int = 150;

// vldoor_e enum values
/// Normal raise-and-wait door: opens, waits `VDOORWAIT` tics, then closes.
pub(super) const vld_normal: c_int = 0;
/// Door that closes immediately, waits 30 seconds, then opens permanently.
pub(super) const vld_close30ThenOpen: c_int = 1;
/// Door that closes and stays closed (no re-open).
pub(super) const vld_close: c_int = 2;
/// Door that opens and stays open (no auto-close).
pub(super) const vld_open: c_int = 3;
/// Door that starts waiting and raises after 5 minutes.
pub(super) const vld_raiseIn5Mins: c_int = 4;
/// Fast normal door (blaze speed: `VDOORSPEED * 4`), waits then closes.
pub(super) const vld_blazeRaise: c_int = 5;
/// Fast door that opens and stays open.
pub(super) const vld_blazeOpen: c_int = 6;
/// Fast door that closes and stays closed.
pub(super) const vld_blazeClose: c_int = 7;

// result_e enum values
/// `T_MovePlane` return: plane moved without reaching destination or crushing.
pub(super) const result_ok: c_int = 0;
/// `T_MovePlane` return: plane was blocked by a thing (crushing).
pub(super) const result_crushed: c_int = 1;
/// `T_MovePlane` return: plane reached its destination this tic.
pub(super) const result_pastdest: c_int = 2;

// card indices
/// Index into the player card array for the blue keycard.
pub(super) const it_bluecard: usize = 0;
/// Index into the player card array for the yellow keycard.
pub(super) const it_yellowcard: usize = 1;
/// Index into the player card array for the red keycard.
pub(super) const it_redcard: usize = 2;
/// Index into the player card array for the blue skull key.
pub(super) const it_blueskull: usize = 3;
/// Index into the player card array for the yellow skull key.
pub(super) const it_yellowskull: usize = 4;
/// Index into the player card array for the red skull key.
pub(super) const it_redskull: usize = 5;

/// Locked-object message: blue key required.
const PD_BLUEO: *mut c_char = c"You need a blue key to activate this object".
    as_ptr().
    cast_mut();
/// Locked-object message: red key required.
const PD_REDO: *mut c_char = c"You need a red key to activate this object".
    as_ptr().
    cast_mut();
/// Locked-object message: yellow key required.
const PD_YELLOWO: *mut c_char = c"You need a yellow key to activate this object".
    as_ptr().
    cast_mut();
/// Locked-door message: blue key required.
const PD_BLUEK: *mut c_char = c"You need a blue key to open this door".as_ptr().cast_mut();
/// Locked-door message: red key required.
const PD_REDK: *mut c_char = c"You need a red key to open this door".as_ptr().cast_mut();
/// Locked-door message: yellow key required.
const PD_YELLOWK: *mut c_char = c"You need a yellow key to open this door".
    as_ptr().
    cast_mut();

/// Passes `s` through unchanged.
///
/// When DEHacked support is compiled in this function would look up a patched
/// string replacement. Here it is a no-op identity shim because
/// `FEATURE_DEHACKED` is not defined.
#[inline(always)]
pub(super) unsafe fn DEH_String(s: *mut c_char) -> *mut c_char { s }

#[inline(always)]
pub(super) unsafe fn locked_object_message(special: c_int) -> *mut c_char
{
    match special
    {
        99 | 133 => DEH_String(PD_BLUEO),
        134 | 135 => DEH_String(PD_REDO),
        136 | 137 => DEH_String(PD_YELLOWO),
        _ => unreachable!("unexpected locked object special: {special}"),
    }
}

#[inline(always)]
pub(super) unsafe fn locked_door_message(special: c_int) -> *mut c_char
{
    match special
    {
        26 | 32 => DEH_String(PD_BLUEK),
        27 | 34 => DEH_String(PD_YELLOWK),
        28 | 33 => DEH_String(PD_REDK),
        _ => unreachable!("unexpected locked door special: {special}"),
    }
}

/// Thinker state for an active vertical door.
///
/// The door moves its sector's ceiling between `floorheight + 0` (closed) and
/// `topheight` (open). `direction` encodes the current movement:
/// - `1` = moving up (opening)
/// - `-1` = moving down (closing)
/// - `0` = waiting at the top
/// - `2` = initial wait (used by `vld_raiseIn5Mins`)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct vldoor_t
{
    /// Embedded thinker header; must be the first field.
    pub thinker: thinker_t,
    /// Door type (`vld_*` constant); controls behaviour at open/close limits.
    pub r#type: c_int,
    _pad0: [u8; 4],
    /// The sector this door controls (its ceiling is the moving plane).
    pub sector: *mut sector_t,
    /// Target height of the open position (lowest surrounding ceiling - 4 units).
    pub topheight: fixed_t,
    /// Movement speed in fixed-point units per tic.
    pub speed: fixed_t,
    /// Current movement direction: `1` up, `-1` down, `0` waiting, `2` initial wait.
    pub direction: c_int,
    /// Tics to wait at the top before starting to close (set to `VDOORWAIT`).
    pub topwait: c_int,
    /// Countdown tics remaining in the current wait phase.
    pub topcountdown: c_int,
}

/// Compile-time layout checks for `vldoor_t` against the C struct on 64-bit.
#[cfg(target_pointer_width = "64")]
mod layout_checks
{
    use super::*;
    const _: () = assert!(size_of::<vldoor_t>() == 64);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, thinker) == 0);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, r#type) == 24);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, sector) == 32);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, topheight) == 40);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, speed) == 44);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, direction) == 48);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, topwait) == 52);
    const _: () = assert!(std::mem::offset_of!(vldoor_t, topcountdown) == 56);
}

#[cfg(test)]
mod tests
{
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const VLDOOR_T_SIZEOF: usize = 64;
    const VLDOOR_T_THINKER: usize = 0;
    const VLDOOR_T_TYPE: usize = 24;
    const VLDOOR_T_SECTOR: usize = 32;
    const VLDOOR_T_TOPHEIGHT: usize = 40;
    const VLDOOR_T_SPEED: usize = 44;
    const VLDOOR_T_DIRECTION: usize = 48;
    const VLDOOR_T_TOPWAIT: usize = 52;
    const VLDOOR_T_TOPCOUNTDOWN: usize = 56;

    #[test]
    fn vldoor_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            size_of::<vldoor_t>(),
            VLDOOR_T_SIZEOF,
            "vldoor_t size mismatch: Rust={}, expected={}",
            size_of::<vldoor_t>(),
            VLDOOR_T_SIZEOF,
        );
        assert_eq!(std::mem::offset_of!(vldoor_t, thinker), VLDOOR_T_THINKER);
        assert_eq!(std::mem::offset_of!(vldoor_t, r#type), VLDOOR_T_TYPE);
        assert_eq!(std::mem::offset_of!(vldoor_t, sector), VLDOOR_T_SECTOR);
        assert_eq!(
            std::mem::offset_of!(vldoor_t, topheight),
            VLDOOR_T_TOPHEIGHT
        );
        assert_eq!(std::mem::offset_of!(vldoor_t, speed), VLDOOR_T_SPEED);
        assert_eq!(
            std::mem::offset_of!(vldoor_t, direction),
            VLDOOR_T_DIRECTION
        );
        assert_eq!(std::mem::offset_of!(vldoor_t, topwait), VLDOOR_T_TOPWAIT);
        assert_eq!(
            std::mem::offset_of!(vldoor_t, topcountdown),
            VLDOOR_T_TOPCOUNTDOWN
        );
    }

    #[test]
    fn locked_object_specials_map_to_dehacked_messages()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            assert_eq!(locked_object_message(99), DEH_String(PD_BLUEO));
            assert_eq!(locked_object_message(133), DEH_String(PD_BLUEO));
            assert_eq!(locked_object_message(134), DEH_String(PD_REDO));
            assert_eq!(locked_object_message(135), DEH_String(PD_REDO));
            assert_eq!(locked_object_message(136), DEH_String(PD_YELLOWO));
            assert_eq!(locked_object_message(137), DEH_String(PD_YELLOWO));
        }
    }

    #[test]
    fn vertical_door_specials_map_to_dehacked_messages()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            assert_eq!(locked_door_message(26), DEH_String(PD_BLUEK));
            assert_eq!(locked_door_message(32), DEH_String(PD_BLUEK));
            assert_eq!(locked_door_message(27), DEH_String(PD_YELLOWK));
            assert_eq!(locked_door_message(34), DEH_String(PD_YELLOWK));
            assert_eq!(locked_door_message(28), DEH_String(PD_REDK));
            assert_eq!(locked_door_message(33), DEH_String(PD_REDK));
        }
    }
}
