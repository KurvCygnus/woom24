//! Switch state and vocabulary: the `repr(C)` `button_t` record, the
//! `switchlist` / `numswitches` / `buttonlist` statics, the `SwitchDef`
//! table mirroring `alphSwitchList[]`, and the dispatch/type constants --
//! bit-exact with the data half of `vendor/doomgeneric/p_switch.c` and
//! `p_local.h`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};

use crate::doom::p_lights::line_t;

/// Maximum number of switch pairs that can be registered at once.
/// Matches `MAXSWITCHES` in `doomdef.h`.
pub const MAXSWITCHES: usize = 50;

/// Maximum number of simultaneously active timed buttons.
/// Matches `MAXBUTTONS` in `p_local.h`.
pub const MAXBUTTONS: usize = 16;

/// Duration of a button press in game tics (1 second at 35 tics/s).
/// Matches `BUTTONTIME` in `p_switch.c`.
pub const BUTTONTIME: c_int = 35;

// bwhere_e values — which sidedef texture slot the switch occupies.

/// Switch is on the upper sidedef texture.
pub(super) const top: c_int = 0;

/// Switch is on the middle sidedef texture.
pub(super) const middle: c_int = 1;

/// Switch is on the lower sidedef texture.
pub(super) const bottom: c_int = 2;

// vldoor_e values — door movement type passed to `EV_DoDoor` / `EV_VerticalDoor`.

/// Normal door: opens then closes after a delay.
pub(super) const vld_normal: c_int = 0;

/// Door closes immediately.
pub(super) const vld_close: c_int = 2;

/// Door opens and stays open.
pub(super) const vld_open: c_int = 3;

/// Blazing door: opens and closes at high speed.
pub(super) const vld_blazeRaise: c_int = 5;

/// Blazing door: opens at high speed and stays open.
pub(super) const vld_blazeOpen: c_int = 6;

/// Blazing door: closes at high speed.
pub(super) const vld_blazeClose: c_int = 7;

// floor_e values — floor movement type passed to `EV_DoFloor`.

/// Lower floor to the highest neighboring floor.
pub(super) const floor_lowerFloor: c_int = 0;

/// Lower floor to the lowest neighboring floor.
pub(super) const floor_lowerFloorToLowest: c_int = 1;

/// Lower floor quickly (turbo speed).
pub(super) const floor_turboLower: c_int = 2;

/// Raise floor to the lowest neighboring ceiling.
pub(super) const floor_raiseFloor: c_int = 3;

/// Raise floor to the nearest higher floor.
pub(super) const floor_raiseFloorToNearest: c_int = 4;

/// Raise floor while crushing — stays at the raised height.
pub(super) const floor_raiseFloorCrush: c_int = 9;

/// Raise floor at turbo speed.
pub(super) const floor_raiseFloorTurbo: c_int = 10;

/// Raise floor exactly 512 map units.
pub(super) const floor_raiseFloor512: c_int = 12;

// ceiling_e values — ceiling movement type passed to `EV_DoCeiling`.

/// Lower ceiling to the floor.
pub(super) const ceiling_lowerToFloor: c_int = 0;

/// Crush-and-raise: ceiling lowers, crushes, then rises repeatedly.
pub(super) const ceiling_crushAndRaise: c_int = 3;

// plattype_e values — platform movement type passed to `EV_DoPlat`.

/// Platform lowers, waits, then rises and stays.
pub(super) const plat_downWaitUpStay: c_int = 1;

/// Raise platform and change its texture to match the neighboring floor.
pub(super) const plat_raiseAndChange: c_int = 2;

/// Raise platform to the nearest higher floor and change texture.
pub(super) const plat_raiseToNearestAndChange: c_int = 3;

/// Blazing `downWaitUpStay` platform (high-speed version).
pub(super) const plat_blazeDWUS: c_int = 4;

// stair_e values — stair build type passed to `EV_BuildStairs`.

/// Build stairs with 8-unit step height.
pub(super) const stair_build8: c_int = 0;

/// Build stairs with 16-unit step height at turbo speed.
pub(super) const stair_turbo16: c_int = 1;

/// A timed button entry: records which linedef was pressed, which texture
/// slot it occupies, the original texture to restore, and how many tics
/// remain before the button resets.
///
/// The layout is verified at compile time to match the C `button_t` struct
/// (32 bytes on 64-bit targets).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct button_t
{
    /// Pointer to the linedef whose switch texture was activated.
    pub line: *mut line_t,
    /// Which sidedef texture slot the button lives in (`top`, `middle`, or `bottom`).
    pub where_: c_int,
    /// Texture number to restore when `btimer` expires.
    pub btexture: c_int,
    /// Remaining tics before the button resets; `0` means this slot is free.
    pub btimer: c_int,
    _pad: [u8; 4],
    /// Pointer to the front-sector sound origin used when the button fires.
    pub soundorg: *mut c_void,
}

#[cfg(target_pointer_width = "64")]
mod layout_checks
{
    use super::*;
    const _: () = assert!(std::mem::size_of::<button_t>() == 32);
    const _: () = assert!(std::mem::offset_of!(button_t, line) == 0);
    const _: () = assert!(std::mem::offset_of!(button_t, where_) == 8);
    const _: () = assert!(std::mem::offset_of!(button_t, btexture) == 12);
    const _: () = assert!(std::mem::offset_of!(button_t, btimer) == 16);
    const _: () = assert!(std::mem::offset_of!(button_t, soundorg) == 24);
}

/// A single entry in the built-in switch-texture table.
///
/// Each entry names the "off" texture (`name1`) and "on" texture (`name2`)
/// and the minimum episode number required for the pair to be loaded.
/// Corresponds to `switchlist_t` in `p_switch.c`.
pub(super) struct SwitchDef
{
    /// Null-terminated name of the switch-off texture.
    pub(super) name1: &'static [u8],
    /// Null-terminated name of the switch-on texture.
    pub(super) name2: &'static [u8],
    /// Minimum episode number (1 = shareware, 2 = registered, 3 = commercial).
    pub(super) episode: i16,
}

/// Built-in switch texture pairs, mirroring `alphSwitchList[]` in
/// `p_switch.c`.  The list is terminated by an entry with `episode == 0`.
pub(super) const ALPH_SWITCH_LIST: [SwitchDef; 41] = [
    // Doom shareware episode 1 switches
    SwitchDef {
        name1: b"SW1BRCOM\0",
        name2: b"SW2BRCOM\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1BRN1\0",
        name2: b"SW2BRN1\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1BRN2\0",
        name2: b"SW2BRN2\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1BRNGN\0",
        name2: b"SW2BRNGN\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1BROWN\0",
        name2: b"SW2BROWN\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1COMM\0",
        name2: b"SW2COMM\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1COMP\0",
        name2: b"SW2COMP\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1DIRT\0",
        name2: b"SW2DIRT\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1EXIT\0",
        name2: b"SW2EXIT\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1GRAY\0",
        name2: b"SW2GRAY\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1GRAY1\0",
        name2: b"SW2GRAY1\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1METAL\0",
        name2: b"SW2METAL\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1PIPE\0",
        name2: b"SW2PIPE\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1SLAD\0",
        name2: b"SW2SLAD\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1STARG\0",
        name2: b"SW2STARG\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1STON1\0",
        name2: b"SW2STON1\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1STON2\0",
        name2: b"SW2STON2\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1STONE\0",
        name2: b"SW2STONE\0",
        episode: 1,
    },
    SwitchDef {
        name1: b"SW1STRTN\0",
        name2: b"SW2STRTN\0",
        episode: 1,
    },
    // Doom registered episodes 2&3 switches
    SwitchDef {
        name1: b"SW1BLUE\0",
        name2: b"SW2BLUE\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1CMT\0",
        name2: b"SW2CMT\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1GARG\0",
        name2: b"SW2GARG\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1GSTON\0",
        name2: b"SW2GSTON\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1HOT\0",
        name2: b"SW2HOT\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1LION\0",
        name2: b"SW2LION\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1SATYR\0",
        name2: b"SW2SATYR\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1SKIN\0",
        name2: b"SW2SKIN\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1VINE\0",
        name2: b"SW2VINE\0",
        episode: 2,
    },
    SwitchDef {
        name1: b"SW1WOOD\0",
        name2: b"SW2WOOD\0",
        episode: 2,
    },
    // Doom II switches
    SwitchDef {
        name1: b"SW1PANEL\0",
        name2: b"SW2PANEL\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1ROCK\0",
        name2: b"SW2ROCK\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1MET2\0",
        name2: b"SW2MET2\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1WDMET\0",
        name2: b"SW2WDMET\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1BRIK\0",
        name2: b"SW2BRIK\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1MOD1\0",
        name2: b"SW2MOD1\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1ZIM\0",
        name2: b"SW2ZIM\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1STON6\0",
        name2: b"SW2STON6\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1TEK\0",
        name2: b"SW2TEK\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1MARB\0",
        name2: b"SW2MARB\0",
        episode: 3,
    },
    SwitchDef {
        name1: b"SW1SKULL\0",
        name2: b"SW2SKULL\0",
        episode: 3,
    },
    // terminator
    SwitchDef {
        name1: b"\0",
        name2: b"\0",
        episode: 0,
    },
];

/// Flat array of texture-number pairs filled by `P_InitSwitchList`.
/// Stored as alternating (off-texture, on-texture) pairs; valid length is
/// `numswitches * 2`.  The sentinel `switchlist[numswitches * 2] == -1`
/// marks the end of valid data.
/// Matches `switchlist[]` in `p_switch.c`.
#[no_mangle]
pub static mut switchlist: [c_int; MAXSWITCHES * 2] = [0; MAXSWITCHES * 2];

/// Number of valid switch pairs in `switchlist` after `P_InitSwitchList`
/// runs.  Zero before initialization.
/// Matches `numswitches` in `p_switch.c`.
#[no_mangle]
pub static mut numswitches: c_int = 0;

/// Ring-buffer of active timed buttons.  Slots with `btimer == 0` are free.
/// Decremented each tic by the thinker subsystem; when a slot's timer
/// reaches zero the original texture is restored.
/// Matches `buttonlist[]` in `p_switch.c`.
#[no_mangle]
pub static mut buttonlist: [button_t; MAXBUTTONS] = [button_t {
    line: std::ptr::null_mut(),
    where_: 0,
    btexture: 0,
    btimer: 0,
    _pad: [0; 4],
    soundorg: std::ptr::null_mut(),
}; MAXBUTTONS];

#[cfg(test)]
mod tests
{
    use super::{bottom, middle, top};
    use crate::doom::p_switch::{button_t, buttonlist, numswitches, switchlist, BUTTONTIME, MAXBUTTONS, MAXSWITCHES};
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const BUTTON_T_SIZEOF: usize = 32;
    const BUTTON_T_LINE: usize = 0;
    const BUTTON_T_WHERE: usize = 8;
    const BUTTON_T_BTEXTURE: usize = 12;
    const BUTTON_T_BTIMER: usize = 16;
    const BUTTON_T_SOUNDORG: usize = 24;

    #[test]
    fn button_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<button_t>(), BUTTON_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(button_t, line), BUTTON_T_LINE);
        assert_eq!(std::mem::offset_of!(button_t, where_), BUTTON_T_WHERE);
        assert_eq!(std::mem::offset_of!(button_t, btexture), BUTTON_T_BTEXTURE);
        assert_eq!(std::mem::offset_of!(button_t, btimer), BUTTON_T_BTIMER);
        assert_eq!(std::mem::offset_of!(button_t, soundorg), BUTTON_T_SOUNDORG);
    }

    #[test]
    fn constants_match_c()
    {
        assert_eq!(MAXSWITCHES, 50);
        assert_eq!(MAXBUTTONS, 16);
        assert_eq!(BUTTONTIME, 35);
        assert_eq!(top, 0);
        assert_eq!(middle, 1);
        assert_eq!(bottom, 2);
    }

    #[test]
    fn globals_are_zero_initialized()
    {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert_eq!(std::ptr::addr_of!(numswitches).read(), 0);
            let switches = std::ptr::addr_of!(switchlist);
            for (i, &v) in (*switches).iter().enumerate()
            {
                assert_eq!(v, 0, "switchlist[{i}] should be 0 before P_InitSwitchList");
            }
            let buttons = std::ptr::addr_of!(buttonlist);
            for (i, b) in (*buttons).iter().enumerate()
            {
                assert!(b.line.is_null(), "buttonlist[{i}].line should be null");
                assert_eq!(b.btimer, 0, "buttonlist[{i}].btimer should be 0");
            }
        }
    }

    #[test]
    fn switchlist_length_is_maxswitches_times_2()
    {
        unsafe {
            let switches = std::ptr::addr_of!(switchlist);
            assert_eq!((*switches).len(), MAXSWITCHES * 2);
            assert_eq!((*switches).len(), 100);
        }
    }
}
