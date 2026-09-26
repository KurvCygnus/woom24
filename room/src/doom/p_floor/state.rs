//! Floor-mover state and vocabulary: the `repr(C)` `floormove_t`
//! thinker state and the `side_t` partial mirror (the latter shared
//! with `p_saveg`), the movement/type/return-value constants, and the
//! layout-pinning checks -- bit-exact with the data half of
//! `vendor/doomgeneric/p_floor.c` and `p_local.h`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;
use std::os::raw::c_short;

use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_lights::sector_t;
use crate::doom::p_tick::thinker_t;

/// Movement speed for floors: 1 unit per tic in fixed-point (`FRACUNIT`).
pub(super) const FLOORSPEED: fixed_t = FRACUNIT;

// result_e enum values
/// `T_MovePlane` return: plane moved without reaching destination or crushing.
pub(super) const result_ok: c_int = 0;
/// `T_MovePlane` return: plane was blocked by a thing (crushing).
pub(super) const result_crushed: c_int = 1;
/// `T_MovePlane` return: plane reached its destination this tic.
pub(super) const result_pastdest: c_int = 2;

// floor_e enum values
/// Lower floor to the highest surrounding floor height.
pub(super) const floor_lowerFloor: c_int = 0;
/// Lower floor to the lowest surrounding floor height.
pub(super) const floor_lowerFloorToLowest: c_int = 1;
/// Lower floor at 4x speed, rising 8 units above the highest surrounding floor.
pub(super) const floor_turboLower: c_int = 2;
/// Raise floor to the lowest surrounding ceiling minus 8 units (crush variant sets crush flag).
pub(super) const floor_raiseFloor: c_int = 3;
/// Raise floor to the next-highest surrounding floor.
pub(super) const floor_raiseFloorToNearest: c_int = 4;
/// Raise floor by the height of the shortest lower texture on its linedefs.
pub(super) const floor_raiseToTexture: c_int = 5;
/// Lower floor to the lowest surrounding floor and transfer texture/special from neighbor.
pub(super) const floor_lowerAndChange: c_int = 6;
/// Raise floor 24 units.
pub(super) const floor_raiseFloor24: c_int = 7;
/// Raise floor 24 units and copy texture/special from the triggering linedef's front sector.
pub(super) const floor_raiseFloor24AndChange: c_int = 8;
/// Raise floor to lowest surrounding ceiling (with crush enabled).
pub(super) const floor_raiseFloorCrush: c_int = 9;
/// Raise floor at 4x speed to the next-highest surrounding floor.
pub(super) const floor_raiseFloorTurbo: c_int = 10;
/// Raise floor for a donut special (transfers texture/special when done).
pub(super) const floor_donutRaise: c_int = 11;
/// Raise floor 512 units.
pub(super) const floor_raiseFloor512: c_int = 12;

// stair_e enum values
/// Build an 8-unit staircase at quarter speed (`FLOORSPEED / 4`).
pub(super) const stair_build8: c_int = 0;
/// Build a 16-unit staircase at 4x speed (`FLOORSPEED * 4`).
pub(super) const stair_turbo16: c_int = 1;

/// Thinker state for a moving floor.
///
/// Each active `EV_DoFloor` or `EV_BuildStairs` call allocates one of these per
/// affected sector. `T_MoveFloor` is called every tic until the floor reaches
/// `floordestheight`.
// Only .sector is accessed in EV_BuildStairs via pointer arithmetic.
// Probe required for offset. On x86_64, sector is at offset 48.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct floormove_t
{
    /// Embedded thinker header; must be the first field.
    pub thinker: thinker_t,
    /// Floor movement type (`floor_*` constant).
    pub r#type: c_int,
    /// Non-zero if this floor should crush things it encounters.
    pub crush: c_int,
    /// The sector whose floor is being moved.
    pub sector: *mut sector_t,
    /// Direction of movement: `1` = up, `-1` = down.
    pub direction: c_int,
    /// Sector special to apply when the floor finishes (`floor_lowerAndChange` / `floor_donutRaise`).
    pub newspecial: c_int,
    /// Floor flat (texture) to apply when the floor finishes.
    pub texture: c_short,
    _pad: [u8; 2],
    /// Target floor height in fixed-point units.
    pub floordestheight: fixed_t,
    /// Movement speed in fixed-point units per tic.
    pub speed: fixed_t,
}

/// Partial mirror of `side_t` from `p_local.h`, containing the fields required
/// by the floor subsystem.
///
/// Layout is verified by the `side_t_layout_matches_c` test.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct side_t
{
    /// Horizontal texture offset for the sidedef.
    pub textureoffset: c_int,
    /// Vertical texture offset for the sidedef.
    pub rowoffset: c_int,
    /// Top (upper) texture index.
    pub toptexture: c_short,
    /// Bottom (lower) texture index.
    pub bottomtexture: c_short,
    /// Middle texture index.
    pub midtexture: c_short,
    _pad: [u8; 2],
    /// Sector on this side of the linedef.
    pub sector: *mut sector_t,
}

/// Compile-time layout checks for `floormove_t` and `side_t` on 64-bit targets.
#[cfg(target_pointer_width = "64")]
mod layout_checks
{
    use super::*;
    const _: () = assert!(std::mem::size_of::<floormove_t>() == 64);
    const _: () = assert!(std::mem::offset_of!(floormove_t, thinker) == 0);
    const _: () = assert!(std::mem::offset_of!(floormove_t, r#type) == 24);
    const _: () = assert!(std::mem::offset_of!(floormove_t, crush) == 28);
    const _: () = assert!(std::mem::offset_of!(floormove_t, sector) == 32);
    const _: () = assert!(std::mem::offset_of!(floormove_t, direction) == 40);
    const _: () = assert!(std::mem::offset_of!(floormove_t, newspecial) == 44);
    const _: () = assert!(std::mem::offset_of!(floormove_t, texture) == 48);
    const _: () = assert!(std::mem::offset_of!(floormove_t, floordestheight) == 52);
    const _: () = assert!(std::mem::offset_of!(floormove_t, speed) == 56);

    const _: () = assert!(std::mem::size_of::<side_t>() == 24);
    const _: () = assert!(std::mem::offset_of!(side_t, textureoffset) == 0);
    const _: () = assert!(std::mem::offset_of!(side_t, rowoffset) == 4);
    const _: () = assert!(std::mem::offset_of!(side_t, toptexture) == 8);
    const _: () = assert!(std::mem::offset_of!(side_t, bottomtexture) == 10);
    const _: () = assert!(std::mem::offset_of!(side_t, midtexture) == 12);
    const _: () = assert!(std::mem::offset_of!(side_t, sector) == 16);
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_floor::{floormove_t, side_t};
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const FLOORMOVE_T_SIZEOF: usize = 64;
    const FLOORMOVE_T_THINKER: usize = 0;
    const FLOORMOVE_T_SECTOR: usize = 32;
    const FLOORMOVE_T_DIRECTION: usize = 40;
    const FLOORMOVE_T_FLOORDESTHEIGHT: usize = 52;
    const FLOORMOVE_T_SPEED: usize = 56;
    const FLOORMOVE_T_TEXTURE: usize = 48;
    const SIDE_T_SIZEOF: usize = 24;
    const SIDE_T_SECTOR_OFFSET: usize = 16;

    /// `floormove_t` must keep the exact C layout (64 bytes): the
    /// thinker state is `Z_Malloc`-allocated by `size_of` and
    /// `EV_BuildStairs` reaches `.sector` by pointer arithmetic, so
    /// size and field offsets are load-bearing.
    #[test]
    fn floormove_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<floormove_t>(),
            FLOORMOVE_T_SIZEOF,
            "floormove_t size mismatch",
        );
        assert_eq!(
            std::mem::offset_of!(floormove_t, thinker),
            FLOORMOVE_T_THINKER
        );
        assert_eq!(
            std::mem::offset_of!(floormove_t, sector),
            FLOORMOVE_T_SECTOR
        );
        assert_eq!(
            std::mem::offset_of!(floormove_t, direction),
            FLOORMOVE_T_DIRECTION
        );
        assert_eq!(
            std::mem::offset_of!(floormove_t, floordestheight),
            FLOORMOVE_T_FLOORDESTHEIGHT
        );
        assert_eq!(std::mem::offset_of!(floormove_t, speed), FLOORMOVE_T_SPEED);
        assert_eq!(
            std::mem::offset_of!(floormove_t, texture),
            FLOORMOVE_T_TEXTURE
        );
    }

    /// `side_t` must keep the exact C layout (24 bytes): `p_saveg`
    /// reads the struct field-wise through the module-root path, so
    /// the size and the `sector` offset are load-bearing.
    #[test]
    fn side_t_layout_matches_c()
    {
        let _g = LOCK.lock().unwrap();
        assert_eq!(
            std::mem::size_of::<side_t>(),
            SIDE_T_SIZEOF,
            "side_t size mismatch",
        );
        assert_eq!(std::mem::offset_of!(side_t, sector), SIDE_T_SECTOR_OFFSET);
    }
}
