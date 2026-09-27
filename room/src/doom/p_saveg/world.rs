//! The world archive/unarchive pair: per-sector and per-sidedef deltas of
//! the current map, in index order. Heights are stored as the top 16 bits
//! of the fixed-point value; the load pass also clears each sector's
//! `specialdata`/`soundtarget` for the later passes to rebuild.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;

use super::stream::{read_le16, write_le16};

// `sectors` / `lines` / `sides` / `numsectors` / `numlines` are p_saveg's
// extern-by-symbol declarations of p_setup's statics (mod.rs wiring block);
// the declared types are the p_lights/p_floor mirrors -- a latent layout
// assumption carried verbatim (module root docs).
use super::{lines, numlines, numsectors, sectors, sides};

/// Serializes all map sector and sidedef deltas to `save_stream`.
///
/// For each sector: floor height, ceiling height (both divided by 65536 to
/// strip the fixed-point fractional part and store as 16-bit integers),
/// floor/ceiling picture indices, light level, special, and tag.
///
/// For each linedef: flags, special, and tag. Then for each of the two
/// sidedefs (skipping missing sides where `sidenum[j] == -1`): texture
/// offsets (divided by 65536, stored as 16-bit), and top/bottom/mid texture
/// indices.
///
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `save_stream` must be an open, writable `FILE *` with sufficient capacity.
/// The global `sectors` array (length `numsectors`), `lines` array (length
/// `numlines`), and `sides` array must all be fully initialized. Every
/// `line_t.sidenum[j]` that is not `-1` must be a valid index into `sides`.
#[doc(alias = "P_ArchiveWorld")]
#[export_name = "P_ArchiveWorld"]
pub extern "C" fn archive_world()
{
    unsafe
    {
        let num_sec = numsectors as usize;
        let num_li = numlines as usize;

        // do sectors
        for i in 0..num_sec
        {
            let sec = sectors.add(i);
            write_le16(((*sec).floorheight >> 16) as u16);
            write_le16(((*sec).ceilingheight >> 16) as u16);
            write_le16((*sec).floorpic as u16);
            write_le16((*sec).ceilingpic as u16);
            write_le16((*sec).lightlevel as u16);
            write_le16((*sec).special as u16);
            write_le16((*sec).tag as u16);
        }

        // do lines
        for i in 0..num_li
        {
            let li = lines.add(i);
            write_le16((*li).flags as u16);
            write_le16((*li).special as u16);
            write_le16((*li).tag as u16);
            for j in 0..2
            {
                if (*li).sidenum[j as usize] == -1
                {
                    continue;
                }
                let si = sides.add((*li).sidenum[j as usize] as usize);
                write_le16(((*si).textureoffset >> 16) as u16);
                write_le16(((*si).rowoffset >> 16) as u16);
                write_le16((*si).toptexture as u16);
                write_le16((*si).bottomtexture as u16);
                write_le16((*si).midtexture as u16);
            }
        }
    }
}

/// Deserializes all map sector and sidedef deltas from `save_stream`.
///
/// For each sector: reads heights as signed 16-bit integers and shifts them
/// left 16 bits to restore the 16.16 fixed-point format. Clears
/// `specialdata` and `soundtarget` to null (they will be rebuilt by
/// `P_UnArchiveSpecials` and the sound code respectively).
///
/// For each linedef: reads flags, special, tag, then each present sidedef's
/// texture offsets (shifted left 16 to restore 16.16) and texture indices.
///
/// Called by `g_game.c` (`G_DoLoadGame`).
///
/// # Safety
///
/// `save_stream` must be an open, readable `FILE *` positioned at the byte
/// sequence written by `P_ArchiveWorld`. The `sectors` array (length
/// `numsectors`), `lines` array (length `numlines`), and `sides` array must
/// all be allocated and at least partially initialized (map load must have
/// completed). Every `line_t.sidenum[j]` that is not `-1` must be a valid
/// index into `sides`.
#[doc(alias = "P_UnArchiveWorld")]
#[export_name = "P_UnArchiveWorld"]
pub extern "C" fn unarchive_world()
{
    unsafe
    {
        let num_sec = numsectors as usize;
        let num_li = numlines as usize;

        // do sectors
        for i in 0..num_sec
        {
            let sec = sectors.add(i);
            (*sec).floorheight = (read_le16() as i16 as c_int) << 16;
            (*sec).ceilingheight = (read_le16() as i16 as c_int) << 16;
            (*sec).floorpic = read_le16() as i16;
            (*sec).ceilingpic = read_le16() as i16;
            (*sec).lightlevel = read_le16() as i16;
            (*sec).special = read_le16() as i16;
            (*sec).tag = read_le16() as i16;
            (*sec).specialdata = std::ptr::null_mut();
            (*sec).soundtarget = std::ptr::null_mut();
        }

        // do lines
        for i in 0..num_li
        {
            let li = lines.add(i);
            (*li).flags = read_le16() as i16;
            (*li).special = read_le16() as i16;
            (*li).tag = read_le16() as i16;
            for j in 0..2
            {
                if (*li).sidenum[j as usize] == -1
                {
                    continue;
                }
                let si = sides.add((*li).sidenum[j as usize] as usize);
                (*si).textureoffset = (read_le16() as i16 as c_int) << 16;
                (*si).rowoffset = (read_le16() as i16 as c_int) << 16;
                (*si).toptexture = read_le16() as i16;
                (*si).bottomtexture = read_le16() as i16;
                (*si).midtexture = read_le16() as i16;
            }
        }
    }
}
