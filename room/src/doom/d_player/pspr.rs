//! The player-sprite definition (`pspdef_t`, `p_pspr.h`): the on-screen
//! weapon/hand overlay animation state. The `state_t` here is d_player's
//! own opaque mirror -- NOT the `c_ffi` mirror; do not unify.

use std::os::raw::c_int;

use super::player::state_t;

/// Player-sprite definition: the on-screen weapon/hand overlay animation state.
///
/// Corresponds to `pspdef_t` in `p_pspr.h`.  Each player has
/// [`NUMPSPRITES`](super::player::NUMPSPRITES) slots (typically `ps_weapon`
/// and `ps_flash`), stored in `PlayerT::psprites`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PspdefT
{
    /// Pointer to the current animation state for this sprite overlay.
    pub state: *mut state_t,
    /// Remaining tics in the current animation state; 0 means advance to next.
    pub tics: c_int,
    /// Screen-space X position of the sprite, in fixed-point pixels.
    pub sx: c_int,
    /// Screen-space Y position of the sprite, in fixed-point pixels.
    pub sy: c_int,
}
