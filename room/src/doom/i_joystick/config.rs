//! The joystick configuration surface: the config-bound statics and the
//! `default.cfg` binding entry point.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int, c_void};

/// Number of virtual joystick buttons exposed to the config system. Must
/// match the `NUM_VIRTUAL_BUTTONS` define in `i_joystick.h`.
const NUM_VIRTUAL_BUTTONS: usize = 10;

/// Master enable flag bound to the `use_joystick` config variable.
static mut usejoystick: c_int = 0;
/// SDL joystick index to open, bound to `joystick_index`. `-1` means none
/// selected.
static mut joystick_index: c_int = -1;
/// Axis used for horizontal (turn) movement, bound to `joystick_x_axis`.
static mut joystick_x_axis: c_int = 0;
/// Inversion flag for the horizontal axis, bound to `joystick_x_invert`.
static mut joystick_x_invert: c_int = 0;
/// Axis used for vertical (forward/back) movement, bound to
/// `joystick_y_axis`.
static mut joystick_y_axis: c_int = 1;
/// Inversion flag for the vertical axis, bound to `joystick_y_invert`.
static mut joystick_y_invert: c_int = 0;
/// Axis used for strafing, bound to `joystick_strafe_axis`. `-1` disables
/// strafing.
static mut joystick_strafe_axis: c_int = -1;
/// Inversion flag for the strafe axis, bound to `joystick_strafe_invert`.
static mut joystick_strafe_invert: c_int = 0;
/// Virtual-to-physical button mapping table, bound element-wise to
/// `joystick_physical_button0` .. `joystick_physical_button9`. Default is
/// the identity mapping.
static mut joystick_physical_buttons: [c_int; NUM_VIRTUAL_BUTTONS] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

//* Path call, not extern: `m_config` graduated alongside this module's
//* wave and its `bind_variable` is called by path (the extern declaration
//* this file carried before the m_config graduation converted in the
//* m_config lockstep commit; the upstream C symbol stays pinned at the
//* renamed definition).
use crate::doom::m_config::bind_variable;
use crate::c_write;

/// Register all joystick configuration variables with `bind_variable` so
/// they are persisted to / loaded from `default.cfg`.
///
/// Although the runtime hooks are no-ops, the config bindings are still
/// installed: this preserves the on-disk config schema and lets the user
/// edit values that a future backend could honour. Variable names and
/// order match the chocolate-doom original byte-for-byte so the resulting
/// `default.cfg` is round-trip compatible.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/bind.rs` imports the upstream name through the root shim.
#[doc(alias = "I_BindJoystickVariables")]
#[export_name = "I_BindJoystickVariables"]
pub extern "C" fn bind_joystick_variables() {
    unsafe {
        bind_variable(
            c"use_joystick".as_ptr().cast_mut(),
            &raw mut usejoystick as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_index".as_ptr().cast_mut(),
            &raw mut joystick_index as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_x_axis".as_ptr().cast_mut(),
            &raw mut joystick_x_axis as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_y_axis".as_ptr().cast_mut(),
            &raw mut joystick_y_axis as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_strafe_axis".as_ptr().cast_mut(),
            &raw mut joystick_strafe_axis as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_x_invert".as_ptr().cast_mut(),
            &raw mut joystick_x_invert as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_y_invert".as_ptr().cast_mut(),
            &raw mut joystick_y_invert as *mut c_int as *mut c_void,
        );
        bind_variable(
            c"joystick_strafe_invert".as_ptr().cast_mut(),
            &raw mut joystick_strafe_invert as *mut c_int as *mut c_void,
        );

        for i in 0..NUM_VIRTUAL_BUTTONS {
            let mut name: [c_char; 32] = [0; 32];
            c_write!(name, "joystick_physical_button{}", i);
            bind_variable(
                name.as_ptr() as *mut c_char,
                &mut joystick_physical_buttons[i] as *mut c_int as *mut c_void,
            );
        }
    }
}
