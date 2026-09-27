//! Sector-special thinker record serialization: the read/write pairs for
//! the seven special types (`ceiling_t`, `vldoor_t`, `floormove_t`,
//! `plat_t`, `lightflash_t`, `strobe_t`, `glow_t`). Field order here is
//! the on-disk save format.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;

use crate::doom::p_ceilng::ceiling_t;
use crate::doom::p_doors::vldoor_t;
use crate::doom::p_floor::floormove_t;
use crate::doom::p_lights::{glow_t, lightflash_t, strobe_t};
use crate::doom::p_plats::plat_t;

use super::records::{read_thinker_header, write_thinker_header};
use super::stream::{read_enum32, read_le16, read_le32, write_enum32, write_le16, write_le32};
use super::swizzle::{read_sector_index, write_sector_index};

//
// ceiling_t
//

/// Reads a `ceiling_t` special thinker from the save stream into `*ceil`.
///
/// The embedded `thinker_t` header is read first, followed by type, sector
/// index, height fields, speed, crush flag, direction, tag, and old direction.
/// The sector index is resolved to a pointer via `saveg_read_sector_ptr`.
/// C origin: `saveg_read_ceiling_t`.
#[doc(alias = "saveg_read_ceiling_t")]
pub(super) unsafe fn read_ceiling_record(ceil: *mut ceiling_t)
{
    let s = &mut *ceil;
    read_thinker_header(&mut s.thinker);
    s.r#type = read_enum32() as c_int;
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.bottomheight = read_le32() as c_int;
    s.topheight = read_le32() as c_int;
    s.speed = read_le32() as c_int;
    s.crush = read_le32() as c_int;
    s.direction = read_le32() as c_int;
    s.tag = read_le32() as c_int;
    s.olddirection = read_le32() as c_int;
}

/// Writes a `ceiling_t` special thinker to the save stream from `*ceil`.
///
/// The sector pointer is serialized as an index via `saveg_write_sector_ptr`.
/// C origin: `saveg_write_ceiling_t`.
#[doc(alias = "saveg_write_ceiling_t")]
pub(super) unsafe fn write_ceiling_record(ceil: *const ceiling_t)
{
    let s = &*ceil;
    write_thinker_header(&s.thinker);
    write_enum32(s.r#type as u32);
    write_le32(write_sector_index(s.sector));
    write_le32(s.bottomheight as u32);
    write_le32(s.topheight as u32);
    write_le32(s.speed as u32);
    write_le32(s.crush as u32);
    write_le32(s.direction as u32);
    write_le32(s.tag as u32);
    write_le32(s.olddirection as u32);
}

//
// vldoor_t
//

/// Reads a `vldoor_t` (vertical-lift door) thinker from the save stream.
///
/// Fields: thinker header, door type enum, sector index, top height (16.16
/// fixed-point), speed (16.16), direction, top-wait tic count, and current
/// top-countdown. C origin: `saveg_read_vldoor_t`.
#[doc(alias = "saveg_read_vldoor_t")]
pub(super) unsafe fn read_door_record(door: *mut vldoor_t)
{
    let s = &mut *door;
    read_thinker_header(&mut s.thinker);
    s.r#type = read_enum32() as c_int;
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.topheight = read_le32() as c_int;
    s.speed = read_le32() as c_int;
    s.direction = read_le32() as c_int;
    s.topwait = read_le32() as c_int;
    s.topcountdown = read_le32() as c_int;
}

/// Writes a `vldoor_t` thinker to the save stream.
///
/// C origin: `saveg_write_vldoor_t`.
#[doc(alias = "saveg_write_vldoor_t")]
pub(super) unsafe fn write_door_record(door: *const vldoor_t)
{
    let s = &*door;
    write_thinker_header(&s.thinker);
    write_enum32(s.r#type as u32);
    write_le32(write_sector_index(s.sector));
    write_le32(s.topheight as u32);
    write_le32(s.speed as u32);
    write_le32(s.direction as u32);
    write_le32(s.topwait as u32);
    write_le32(s.topcountdown as u32);
}

//
// floormove_t
//

/// Reads a `floormove_t` (moving floor) thinker from the save stream.
///
/// Fields: thinker header, floor type enum, crush flag, sector index,
/// direction, new special number, texture (16-bit), destination height
/// (16.16), and speed (16.16). C origin: `saveg_read_floormove_t`.
#[doc(alias = "saveg_read_floormove_t")]
pub(super) unsafe fn read_floormove_record(floor: *mut floormove_t)
{
    let s = &mut *floor;
    read_thinker_header(&mut s.thinker);
    s.r#type = read_enum32() as c_int;
    s.crush = read_le32() as c_int;
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.direction = read_le32() as c_int;
    s.newspecial = read_le32() as c_int;
    s.texture = read_le16() as i16;
    s.floordestheight = read_le32() as c_int;
    s.speed = read_le32() as c_int;
}

/// Writes a `floormove_t` thinker to the save stream.
///
/// C origin: `saveg_write_floormove_t`.
#[doc(alias = "saveg_write_floormove_t")]
pub(super) unsafe fn write_floormove_record(floor: *const floormove_t)
{
    let s = &*floor;
    write_thinker_header(&s.thinker);
    write_enum32(s.r#type as u32);
    write_le32(s.crush as u32);
    write_le32(write_sector_index(s.sector));
    write_le32(s.direction as u32);
    write_le32(s.newspecial as u32);
    write_le16(s.texture as u16);
    write_le32(s.floordestheight as u32);
    write_le32(s.speed as u32);
}

//
// plat_t
//

/// Reads a `plat_t` (raising/lowering platform) thinker from the save stream.
///
/// Fields: thinker header, sector index, speed (16.16), low and high heights
/// (16.16), wait time, count, status enum, old-status enum, crush flag, tag,
/// and platform type enum. C origin: `saveg_read_plat_t`.
#[doc(alias = "saveg_read_plat_t")]
pub(super) unsafe fn read_plat_record(plat: *mut plat_t)
{
    let s = &mut *plat;
    read_thinker_header(&mut s.thinker);
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.speed = read_le32() as c_int;
    s.low = read_le32() as c_int;
    s.high = read_le32() as c_int;
    s.wait = read_le32() as c_int;
    s.count = read_le32() as c_int;
    s.status = read_enum32() as c_int;
    s.oldstatus = read_enum32() as c_int;
    s.crush = read_le32() as c_int;
    s.tag = read_le32() as c_int;
    s.r#type = read_enum32() as c_int;
}

/// Writes a `plat_t` thinker to the save stream.
///
/// C origin: `saveg_write_plat_t`.
#[doc(alias = "saveg_write_plat_t")]
pub(super) unsafe fn write_plat_record(plat: *const plat_t)
{
    let s = &*plat;
    write_thinker_header(&s.thinker);
    write_le32(write_sector_index(s.sector));
    write_le32(s.speed as u32);
    write_le32(s.low as u32);
    write_le32(s.high as u32);
    write_le32(s.wait as u32);
    write_le32(s.count as u32);
    write_enum32(s.status as u32);
    write_enum32(s.oldstatus as u32);
    write_le32(s.crush as u32);
    write_le32(s.tag as u32);
    write_enum32(s.r#type as u32);
}

//
// lightflash_t
//

/// Reads a `lightflash_t` (random light flash) thinker from the save stream.
///
/// Fields: thinker header, sector index, count, max light, min light, max
/// time, min time. C origin: `saveg_read_lightflash_t`.
#[doc(alias = "saveg_read_lightflash_t")]
pub(super) unsafe fn read_lightflash_record(flash: *mut lightflash_t)
{
    let s = &mut *flash;
    read_thinker_header(&mut s.thinker);
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.count = read_le32() as c_int;
    s.maxlight = read_le32() as c_int;
    s.minlight = read_le32() as c_int;
    s.maxtime = read_le32() as c_int;
    s.mintime = read_le32() as c_int;
}

/// Writes a `lightflash_t` thinker to the save stream.
///
/// C origin: `saveg_write_lightflash_t`.
#[doc(alias = "saveg_write_lightflash_t")]
pub(super) unsafe fn write_lightflash_record(flash: *const lightflash_t)
{
    let s = &*flash;
    write_thinker_header(&s.thinker);
    write_le32(write_sector_index(s.sector));
    write_le32(s.count as u32);
    write_le32(s.maxlight as u32);
    write_le32(s.minlight as u32);
    write_le32(s.maxtime as u32);
    write_le32(s.mintime as u32);
}

//
// strobe_t
//

/// Reads a `strobe_t` (strobe light) thinker from the save stream.
///
/// Fields: thinker header, sector index, count, min light, max light, dark
/// time (tics), bright time (tics). C origin: `saveg_read_strobe_t`.
#[doc(alias = "saveg_read_strobe_t")]
pub(super) unsafe fn read_strobe_record(strobe: *mut strobe_t)
{
    let s = &mut *strobe;
    read_thinker_header(&mut s.thinker);
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.count = read_le32() as c_int;
    s.minlight = read_le32() as c_int;
    s.maxlight = read_le32() as c_int;
    s.darktime = read_le32() as c_int;
    s.brighttime = read_le32() as c_int;
}

/// Writes a `strobe_t` thinker to the save stream.
///
/// C origin: `saveg_write_strobe_t`.
#[doc(alias = "saveg_write_strobe_t")]
pub(super) unsafe fn write_strobe_record(strobe: *const strobe_t)
{
    let s = &*strobe;
    write_thinker_header(&s.thinker);
    write_le32(write_sector_index(s.sector));
    write_le32(s.count as u32);
    write_le32(s.minlight as u32);
    write_le32(s.maxlight as u32);
    write_le32(s.darktime as u32);
    write_le32(s.brighttime as u32);
}

//
// glow_t
//

/// Reads a `glow_t` (glow light) thinker from the save stream.
///
/// Fields: thinker header, sector index, min light, max light, direction
/// (+1 or -1). C origin: `saveg_read_glow_t`.
#[doc(alias = "saveg_read_glow_t")]
pub(super) unsafe fn read_glow_record(glow: *mut glow_t)
{
    let s = &mut *glow;
    read_thinker_header(&mut s.thinker);
    let sector_idx = read_le32();
    s.sector = read_sector_index(sector_idx);
    s.minlight = read_le32() as c_int;
    s.maxlight = read_le32() as c_int;
    s.direction = read_le32() as c_int;
}

/// Writes a `glow_t` thinker to the save stream.
///
/// C origin: `saveg_write_glow_t`.
#[doc(alias = "saveg_write_glow_t")]
pub(super) unsafe fn write_glow_record(glow: *const glow_t)
{
    let s = &*glow;
    write_thinker_header(&s.thinker);
    write_le32(write_sector_index(s.sector));
    write_le32(s.minlight as u32);
    write_le32(s.maxlight as u32);
    write_le32(s.direction as u32);
}
