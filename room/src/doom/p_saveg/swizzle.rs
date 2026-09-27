//! Pointer<->index swizzling for the save format: sector and state
//! pointers serialize as zero-based array indices, player pointers as a
//! 1-based index with `0` for NULL. Pure pointer arithmetic over the
//! global tables -- the stream I/O happens at the callers.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use crate::doom::d_player::{players, PlayerT};
use crate::doom::info::{states, State, NUMSTATES};
use crate::doom::p_lights::sector_t;

use super::stream::savegame_error;

// `sectors` is p_saveg's extern-by-symbol declaration of p_setup's static
// (mod.rs wiring block); the declared type is the p_lights mirror, while
// p_setup defines the static with the c_ffi type -- a latent layout
// assumption carried verbatim (see the module root docs).
use super::sectors;

/// Serializes a `sector_t` pointer as a zero-based index into the global
/// `sectors` array.
///
/// Returns `0` for a null pointer. The C equivalent uses pointer subtraction
/// (`sector - sectors`). Callers must ensure `sector` actually points into
/// `sectors` when non-null.
#[doc(alias = "saveg_write_sector_ptr")]
pub(super) unsafe fn write_sector_index(sector: *const sector_t) -> u32
{
    if sector.is_null()
    {
        0
    }
    else
    {
        sector.offset_from(sectors) as u32
    }
}

/// Deserializes a sector index from the save stream into a `*mut sector_t`.
///
/// `index` is an offset into the global `sectors` array. No bounds check is
/// performed; callers must trust that the index was written by a valid
/// `saveg_write_sector_ptr` call.
#[doc(alias = "saveg_read_sector_ptr")]
pub(super) unsafe fn read_sector_index(index: u32) -> *mut sector_t
{
    sectors.add(index as usize)
}

/// Serializes a `State` pointer as a zero-based index into the global `states`
/// array.
///
/// Returns `0` for a null pointer. The C equivalent computes `state - states`.
#[doc(alias = "saveg_write_state_ptr")]
pub(super) unsafe fn write_state_index(state: *const State) -> u32
{
    if state.is_null()
    {
        0
    }
    else
    {
        state.offset_from(std::ptr::addr_of!(states[0])) as u32
    }
}

/// Deserializes a state index into a `*mut State`.
///
/// If `index >= NUMSTATES`, sets `savegame_error = 1` and clamps to
/// `states[0]` rather than accessing out-of-bounds memory. The C version
/// performs no such guard (`&states[saveg_read32()]` would silently
/// out-of-bounds).
#[doc(alias = "saveg_read_state_ptr")]
pub(super) unsafe fn read_state_index(index: u32) -> *mut State
{
    if index as usize >= NUMSTATES
    {
        savegame_error = 1;
        return std::ptr::addr_of_mut!(states[0]);
    }
    std::ptr::addr_of_mut!(states[0]).add(index as usize)
}

/// Serializes a `PlayerT` pointer as a 1-based player index.
///
/// Returns `0` for a null pointer; otherwise returns
/// `(player - players) + 1`. The 1-based encoding reserves `0` to represent
/// NULL. C origin: `str->player - players + 1`.
#[doc(alias = "saveg_write_player_ptr")]
pub(super) unsafe fn write_player_index(player: *const PlayerT) -> u32
{
    if player.is_null()
    {
        0
    }
    else
    {
        (player.offset_from(std::ptr::addr_of!(players[0])) as u32) + 1
    }
}

/// Deserializes a 1-based player index into a `*mut PlayerT`.
///
/// `value == 0` maps to null; `value > 0` indexes `players[value - 1]`.
/// C origin: `&players[pl - 1]`.
#[doc(alias = "saveg_read_player_ptr")]
pub(super) unsafe fn read_player_index(value: u32) -> *mut PlayerT
{
    if value == 0
    {
        std::ptr::null_mut()
    }
    else
    {
        std::ptr::addr_of_mut!(players[0]).add((value - 1) as usize)
    }
}
