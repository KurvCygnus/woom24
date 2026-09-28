//! The `repr(C)` input-event struct with its data-datum table, moved
//! from the pre-graduation `d_event.rs` (F10 wave A3).

#![allow(non_snake_case)]

use std::ffi::c_int;

/// A single input event delivered to the game logic.
///
/// Corresponds to `event_t` in `d_event.h`.  The meaning of `data1`-`data4`
/// depends on `type_`:
///
/// | `type_` (evtype_t) | `data1` | `data2` | `data3` | `data4` |
/// |--------------------|---------|---------|---------|---------|
/// | `ev_keydown` / `ev_keyup` | key code (from `doomkeys`) | ASCII char pressed | - | - |
/// | `ev_mouse` | button bitfield (bit 0=left, 1=right, 2=middle) | X delta (turn) | Y delta (fwd/back) | - |
/// | `ev_joystick` | button bitfield | X axis (turn) | Y axis (fwd/back) | Z axis (strafe) |
/// | `ev_quit` | - | - | - | - |
///
/// The `type_` field is typed as `c_int` rather than an enum to preserve
/// exact ABI compatibility with the C `evtype_t` enum (which is an `int` in
/// C).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct event_t
{
    /// Event type discriminant; one of the `ev_*` constants from `evtype_t`.
    pub type_: c_int,
    /// Primary event datum; interpretation depends on `type_`.
    pub data1: c_int,
    /// Secondary event datum; interpretation depends on `type_`.
    pub data2: c_int,
    /// Tertiary event datum; interpretation depends on `type_`.
    pub data3: c_int,
    /// Quaternary event datum; used only by `ev_joystick` for the strafe axis.
    pub data4: c_int,
}

#[cfg(test)]
mod tests
{
    use super::*;

    /// ABI pin (A3 report §1.5): `event_t` crosses the extern-block
    /// boundary at `i_input.rs:38`
    /// (`extern "C" { fn D_PostEvent(ev: *const event_t); }`), so the
    /// `repr(C)` layout must stay five `c_int` fields -- 20 bytes, no
    /// padding. A field reorder would silently break that ABI.
    #[test]
    fn event_t_layout_is_pinned()
    {
        assert_eq!(std::mem::size_of::<event_t>(), 20);
        assert_eq!(std::mem::size_of::<c_int>(), 4);
    }
}
