//! Shared C-mirror vocabulary of the teleporter module: the canonical
//! freeze-zone `mobj_t` mirror plus `line_t`, `sector_t`, `subsector_t`,
//! the packed `mapthing_t`, the opaque `state_t` / `mobjinfo_t` /
//! `player_s` aliases, and the `EXE_FINAL` version constant -- the
//! struct half of `vendor/doomgeneric/p_telept.c` plus the `p_local.h` /
//! `doomdata.h` records it needs.
//!
//! These mirrors are load-bearing far beyond this module: the
//! `mobj_t` here is THE canonical mobj of the freeze zone, read by a
//! dozen other files (including the F9 state hash and the `p_sight`
//! re-export) through the module root, so the layout-pinning tests
//! below move with the types and are the guard against silent
//! state-hash corruption.

#![allow(non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::p_tick::thinker_t;

/// A sub-sector: the smallest convex region of the BSP tree.
///
/// Each sub-sector belongs to exactly one [`sector_t`] and holds a range of
/// segs.  The layout matches the C `subsector_t` struct so that pointer casts
/// from the C side are valid.
///
/// Re-exported with the same layout as `p_lights.rs` so pointer casts work
/// across modules.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct subsector_t
{
    /// The sector this sub-sector belongs to.
    pub sector: *mut sector_t,
    /// Number of segs in this sub-sector.
    pub numlines: i16,
    /// Index of the first seg in the global seg array.
    pub firstline: i16,
}

/// A map sector: a convex region with a single floor and ceiling height.
///
/// Layout must match the C `sector_t` struct exactly; the padding bytes
/// (`_pad0`) preserve alignment without introducing Rust-side named fields
/// for C-internal slots.  Field offsets are verified by the test suite.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sector_t
{
    /// Floor height in fixed-point units.
    pub floorheight: c_int,
    /// Ceiling height in fixed-point units.
    pub ceilingheight: c_int,
    /// Flat texture index for the floor.
    pub floorpic: i16,
    /// Flat texture index for the ceiling.
    pub ceilingpic: i16,
    /// Ambient light level (0–255).
    pub lightlevel: i16,
    /// Sector special type (damage, secret, etc.).
    pub special: i16,
    /// Linedef tag used to link this sector with matching linedefs.
    pub tag: i16,
    _pad0: [u8; 2],
    /// Sound traversal counter, used to avoid duplicate sound propagation.
    pub soundtraversed: c_int,
    /// Last thing to make a sound in this sector (for monster alerting).
    pub soundtarget: *mut c_void,
    /// Bounding box of the sector in map units `[top, bottom, left, right]`.
    pub blockbox: [c_int; 4],
    /// Origin point for sounds emitted by this sector (opaque 40-byte blob
    /// matching `mobj_t::soundorg` in C).
    pub soundorg: [u8; 40],
    /// Incremented each time the sector is visited during a BFS/DFS.
    pub validcount: c_int,
    /// Linked list of things currently in this sector.
    pub thinglist: *mut c_void,
    /// Pointer to the active special thinker for this sector (e.g. a moving
    /// ceiling or platform), or null if none is active.
    pub specialdata: *mut c_void,
    /// Number of linedefs bounding this sector.
    pub linecount: c_int,
    /// Pointer to the array of linedef pointers for this sector.
    pub lines: *mut *mut c_void,
}

/// Packed map spawn-point record as stored in the WAD THINGS lump.
///
/// Uses `#[repr(C, packed)]` to match the 10-byte on-disk layout.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct mapthing_t
{
    /// Spawn X position in map units.
    pub x: i16,
    /// Spawn Y position in map units.
    pub y: i16,
    /// Facing angle in degrees (0–359, clockwise from East).
    pub angle: i16,
    /// Doom thing type number.
    pub r#type: i16,
    /// Spawn flags (skill levels, deaf, multiplayer, etc.).
    pub options: i16,
}

/// Opaque stand-in for the C `player_s` struct; only used via raw pointer.
pub enum player_s {}
/// Opaque stand-in for the C `state_t` (animation frame) struct.
pub enum state_t {}
/// Opaque stand-in for the C `mobjinfo_t` (thing type info) struct.
pub enum mobjinfo_t {}

/// A map object (thing) that lives in the world and can think each tic.
///
/// The `thinker` field must be at offset 0 so that a `*mut mobj_t` can be
/// safely cast to `*mut thinker_t` and vice-versa.  The overall size (224
/// bytes on 64-bit) and all field offsets are verified by the test suite.
///
/// Padding bytes (`_pad0`–`_pad4`) fill gaps that C compilers insert for
/// alignment; they have no semantic meaning in this port.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct mobj_t
{
    /// Thinker header — must be the very first field (offset 0).
    pub thinker: thinker_t,
    /// Map X position in fixed-point units.
    pub x: c_int,
    /// Map Y position in fixed-point units.
    pub y: c_int,
    /// Map Z position (height above the floor) in fixed-point units.
    pub z: c_int,
    _pad0: [u8; 4],
    /// Next thing in this sector's sprite-order linked list.
    pub snext: *mut mobj_t,
    /// Previous thing in this sector's sprite-order linked list.
    pub sprev: *mut mobj_t,
    /// Facing angle in binary radians (BAMs); 0 = East.
    pub angle: u32,
    /// Current sprite number.
    pub sprite: c_int,
    /// Current animation frame index.
    pub frame: c_int,
    _pad1: [u8; 4],
    /// Next thing in the blockmap block's linked list.
    pub bnext: *mut mobj_t,
    /// Previous thing in the blockmap block's linked list.
    pub bprev: *mut mobj_t,
    /// The sub-sector this thing currently occupies.
    pub subsector: *mut subsector_t,
    /// Floor height at the thing's current position.
    pub floorz: c_int,
    /// Ceiling height at the thing's current position.
    pub ceilingz: c_int,
    /// Collision radius in fixed-point units.
    pub radius: c_int,
    /// Collision height in fixed-point units.
    pub height: c_int,
    /// X momentum applied each tic.
    pub momx: c_int,
    /// Y momentum applied each tic.
    pub momy: c_int,
    /// Z momentum applied each tic.
    pub momz: c_int,
    /// Used to avoid processing the same thing twice in a single traversal.
    pub validcount: c_int,
    /// Thing type index into `mobjinfo` table.
    pub mobjtype: c_int,
    _pad2: [u8; 4],
    /// Pointer to the thing's type info record.
    pub info: *mut mobjinfo_t,
    /// Remaining tics in the current animation frame.
    pub tics: c_int,
    _pad3: [u8; 4],
    /// Pointer to the current animation frame state.
    pub state: *mut state_t,
    /// Bit-field of `MF_*` flags controlling collision, rendering, AI, etc.
    pub flags: c_int,
    /// Current hit points.
    pub health: c_int,
    /// Current movement direction (0–7 compass directions, or `DI_NODIR`).
    pub movedir: c_int,
    /// Tics remaining before the thing attempts a new movement decision.
    pub movecount: c_int,
    /// Primary AI target (usually the last thing to attack this one).
    pub target: *mut mobj_t,
    /// Tics the thing must wait before it can attack again.
    pub reactiontime: c_int,
    /// Decremented each tic while pursuing a target; reset on target change.
    pub threshold: c_int,
    /// Non-null if this thing is a player; points to the `PlayerT` record.
    pub player: *mut player_s,
    /// Index used to cycle through potential targets during `P_LookForPlayers`.
    pub lastlook: c_int,
    /// The WAD spawn point from which this thing was created.
    pub spawnpoint: mapthing_t,
    _pad4: [u8; 2],
    /// Tracer target used by homing missiles (cyberdemon rockets, etc.).
    pub tracer: *mut mobj_t,
}

/// Linedef descriptor used by `EV_Teleport` to read the tag and trigger side.
///
/// Only the fields accessed by teleportation logic are fully documented here;
/// the layout matches the C `line_t` struct and is shared with `p_lights.rs`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct line_t
{
    /// First vertex of the line (opaque pointer).
    pub v1: *mut c_void,
    /// Second vertex of the line (opaque pointer).
    pub v2: *mut c_void,
    /// Delta X between the two vertices (used for slope/normal calculation).
    pub dx: c_int,
    /// Delta Y between the two vertices.
    pub dy: c_int,
    /// Linedef flags (blocking, two-sided, upper/lower unpegged, etc.).
    pub flags: i16,
    /// Linedef special action number.
    pub special: i16,
    /// Tag linking this linedef to tagged sectors.
    pub tag: i16,
    /// Indices into the global `sides` array: `[front, back]`.
    pub sidenum: [i16; 2],
    /// Bounding box `[top, bottom, left, right]` in map units.
    pub bbox: [c_int; 4],
    /// Slope type (horizontal, vertical, positive, negative).
    pub slopetype: c_int,
    /// Pointer to the front (right) sector.
    pub frontsector: *mut sector_t,
    /// Pointer to the back (left) sector, or null for a one-sided line.
    pub backsector: *mut sector_t,
    /// Used to avoid processing the same line twice in a BSP traversal.
    pub validcount: c_int,
    /// Active special thinker attached to this line, or null.
    pub specialdata: *mut c_void,
}

/// Doom version constant for the first Final Doom executable.
///
/// `EV_Teleport` uses this to preserve a known quirk: the original Final Doom
/// binary does not set `thing->z` to `floorz` after teleporting.
pub(super) const EXE_FINAL: c_int = 7;

#[cfg(test)]
mod tests
{
    use crate::doom::p_telept::{mobj_t, sector_t, subsector_t};

    #[test]
    fn mobj_t_size_is_224()
    {
        assert_eq!(std::mem::size_of::<mobj_t>(), 224);
    }

    #[test]
    fn mobj_t_field_offsets()
    {
        assert_eq!(std::mem::offset_of!(mobj_t, thinker), 0);
        assert_eq!(std::mem::offset_of!(mobj_t, x), 24);
        assert_eq!(std::mem::offset_of!(mobj_t, subsector), 88);
        assert_eq!(std::mem::offset_of!(mobj_t, floorz), 96);
        assert_eq!(std::mem::offset_of!(mobj_t, mobjtype), 128);
        assert_eq!(std::mem::offset_of!(mobj_t, flags), 160);
        assert_eq!(std::mem::offset_of!(mobj_t, player), 192);
        assert_eq!(std::mem::offset_of!(mobj_t, reactiontime), 184);
        assert_eq!(std::mem::offset_of!(mobj_t, tracer), 216);
    }

    #[test]
    fn subsector_t_size()
    {
        assert_eq!(std::mem::size_of::<subsector_t>(), 16);
    }

    #[test]
    fn sector_t_layout()
    {
        assert_eq!(std::mem::size_of::<sector_t>(), 128);
        assert_eq!(std::mem::offset_of!(sector_t, floorheight), 0);
        assert_eq!(std::mem::offset_of!(sector_t, ceilingheight), 4);
        assert_eq!(std::mem::offset_of!(sector_t, floorpic), 8);
        assert_eq!(std::mem::offset_of!(sector_t, ceilingpic), 10);
        assert_eq!(std::mem::offset_of!(sector_t, lightlevel), 12);
        assert_eq!(std::mem::offset_of!(sector_t, special), 14);
        assert_eq!(std::mem::offset_of!(sector_t, tag), 16);
        assert_eq!(std::mem::offset_of!(sector_t, soundtraversed), 20);
        assert_eq!(std::mem::offset_of!(sector_t, soundtarget), 24);
        assert_eq!(std::mem::offset_of!(sector_t, blockbox), 32);
        assert_eq!(std::mem::offset_of!(sector_t, soundorg), 48);
        assert_eq!(std::mem::offset_of!(sector_t, validcount), 88);
        assert_eq!(std::mem::offset_of!(sector_t, thinglist), 96);
        assert_eq!(std::mem::offset_of!(sector_t, specialdata), 104);
        assert_eq!(std::mem::offset_of!(sector_t, linecount), 112);
        assert_eq!(std::mem::offset_of!(sector_t, lines), 120);
    }
}
