//! The numeric vocabulary of the monster AI: melee/missile ranges,
//! floating-motion budgets, the eight-direction chase lattice, the
//! game-mode/version/skill gate values, the boss-death floor/door
//! special types, and the Mancubus / Lost Soul attack constants --
//! all data tier, upstream names kept verbatim.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::os::raw::c_int;

use crate::doom::m_fixed::FRACUNIT;
use crate::doom::tables::ANG90;

/// C `size_t` equivalent; used for buffer sizes matching the C ABI.
pub(super) type size_t = usize;

/// Binary-angle measurement type (`u32`); a full circle is `2^32` units.
pub(super) type angle_t = u32;

/// State index type mirroring C `statenum_t`; indexes the `states` table in `info.rs`.
pub(super) type statenum_t = c_int;

/// C `short` integer type alias.
pub(super) type c_short = i16;

/// Map-object type identifier; mirrors C `mobjtype_t` (index into `mobjinfo` table).
pub(super) type mobjtype_t = c_int;

/// Alias for `MobjInfo`; mirrors C `mobjinfo_t` typedef.
pub(super) type mobjinfo_t = crate::doom::info::MobjInfo;

/// Type alias used for cross-module pointer casts where both sides are
/// `#[repr(C)]`-identical `mobj_t` definitions.
pub(super) type CffiMobj = crate::doom::c_ffi::mobj_t;

/// Maximum melee attack range in fixed-point units (64 map units); from `p_local.h`.
pub(super) const MELEERANGE: c_int = 64 * FRACUNIT;

/// Maximum missile attack range in fixed-point units (2048 map units); from `p_local.h`.
pub(super) const MISSILERANGE: c_int = 32 * 64 * FRACUNIT;

/// Vertical speed at which floating monsters adjust altitude each tic (4 units/tic).
pub(super) const FLOATSPEED: c_int = FRACUNIT * 4;

/// Maximum radius of any map object (32 units), used as a blockmap search margin.
pub(super) const MAXRADIUS: c_int = 32 * FRACUNIT;

/// Eight-direction movement type used by the monster pathfinding code; mirrors C `dirtype_t`.
pub(super) type dirtype_t = c_int;

/// East cardinal direction index (positive X axis).
pub(super) const DI_EAST: c_int = 0;

/// Northeast diagonal direction index.
pub(super) const DI_NORTHEAST: c_int = 1;

/// North cardinal direction index (positive Y axis).
pub(super) const DI_NORTH: c_int = 2;

/// Northwest diagonal direction index.
pub(super) const DI_NORTHWEST: c_int = 3;

/// West cardinal direction index (negative X axis).
pub(super) const DI_WEST: c_int = 4;

/// Southwest diagonal direction index.
pub(super) const DI_SOUTHWEST: c_int = 5;

/// South cardinal direction index (negative Y axis).
pub(super) const DI_SOUTH: c_int = 6;

/// Southeast diagonal direction index.
pub(super) const DI_SOUTHEAST: c_int = 7;

/// Sentinel value meaning the monster has no current movement direction.
pub(super) const DI_NODIR: c_int = 8;

/// Total number of direction values including `DI_NODIR`.
#[allow(dead_code)]
pub(super) const NUMDIRS: c_int = 9;

/// Minimum game version that introduced Ultimate Doom episode logic; mirrors `exe_ultimate` from `doomfeatures.h`.
pub(super) const exe_ultimate: c_int = 6;

/// Game mode value for Doom II (commercial); mirrors `commercial` from `doomdef.h`.
pub(super) const commercial: c_int = 2;

/// Skill level index for Nightmare difficulty; mirrors `sk_nightmare` from `doomdef.h`.
pub(super) const sk_nightmare: c_int = 4;

/// Skill level index for Easy (Hey Not Too Rough) difficulty; mirrors `sk_easy` from `doomdef.h`.
pub(super) const sk_easy: c_int = 1;

/// `EV_DoFloor` floor type: lower floor to the lowest adjacent floor; mirrors `lowerFloorToLowest`.
pub(super) const lowerFloorToLowest: c_int = 5;

/// `EV_DoFloor` floor type: raise floor to nearest texture height; mirrors `raiseToTexture`.
pub(super) const raiseToTexture: c_int = 8;

/// `EV_DoDoor` door type: blaze-open (fast open); mirrors `vld_blazeOpen`.
pub(super) const vld_blazeOpen: c_int = 5;

/// `EV_DoDoor` door type: normal open; mirrors `vld_open`.
pub(super) const vld_open: c_int = 0;

/// `mobjtype_t` index for the player map object; mirrors `MT_PLAYER` from `info.h`.
#[allow(dead_code)]
pub(super) const MT_PLAYER: mobjtype_t = 0;

/// Angular spread (ANG90/8) used between Mancubus fire balls; mirrors `FATSPREAD` from `p_enemy.c`.
pub(super) const FATSPREAD: c_int = ANG90 as c_int / 8;

/// Lost Soul charge speed in fixed-point units per tic (20 map units/tic); mirrors `SKULLSPEED`.
pub(super) const SKULLSPEED: c_int = 20 * FRACUNIT;
