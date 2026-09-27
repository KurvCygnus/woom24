//! The sector-special thinker archive/unarchive pair: the seven special
//! types, tagged per record and terminated by `tc_endspecials`. The
//! archive dispatch compares `acp1` against the ROOT-SHIMMED `T_*`
//! function items (compares #3-#9) plus the `activeceilings` pointer scan
//! (compare #10); the reload pass reinstates functions through
//! `records::actionf_of_*`, which transmute the SAME shimmed items -- all
//! carried verbatim, never a wrapper, or the identity contract breaks
//! silently. `T_FireFlicker` is deliberately NOT in the compare set:
//! vanilla silently drops fireflicker thinkers on save.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};

use crate::i_error;
use crate::doom::p_ceilng::{ceiling_t, P_AddActiveCeiling, T_MoveCeiling, MAXCEILINGS};
use crate::doom::p_doors::{vldoor_t, T_VerticalDoor};
use crate::doom::p_floor::{floormove_t, T_MoveFloor};
use crate::doom::p_lights::{glow_t, lightflash_t, strobe_t, T_Glow, T_LightFlash, T_StrobeFlash};
use crate::doom::p_plats::{plat_t, P_AddActivePlat, T_PlatRaise};
use crate::doom::p_tick::{thinkercap, P_AddThinker};
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

use super::records::{
    actionf_of_glow, actionf_of_light_flash, actionf_of_move_ceiling, actionf_of_move_floor,
    actionf_of_plat_raise, actionf_of_strobe_flash, actionf_of_vertical_door,
};
use super::records_specials::{
    read_ceiling_record, read_door_record, read_floormove_record, read_glow_record,
    read_lightflash_record, read_plat_record, read_strobe_record, write_ceiling_record,
    write_door_record, write_floormove_record, write_glow_record, write_lightflash_record,
    write_plat_record, write_strobe_record,
};
use super::stream::{read_byte, read_padding, write_byte, write_padding};

/// Specials-class tag for a `ceiling_t` record. C origin: `tc_ceiling`.
const tc_ceiling: u8 = 0;
/// Specials-class tag for a `vldoor_t` record. C origin: `tc_door`.
const tc_door: u8 = 1;
/// Specials-class tag for a `floormove_t` record. C origin: `tc_floor`.
const tc_floor: u8 = 2;
/// Specials-class tag for a `plat_t` record. C origin: `tc_plat`.
const tc_plat: u8 = 3;
/// Specials-class tag for a `lightflash_t` record. C origin: `tc_flash`.
const tc_flash: u8 = 4;
/// Specials-class tag for a `strobe_t` record. C origin: `tc_strobe`.
const tc_strobe: u8 = 5;
/// Specials-class tag for a `glow_t` record. C origin: `tc_glow`.
const tc_glow: u8 = 6;
/// Specials-class tag marking the end of the specials list.
/// C origin: `tc_endspecials`.
const tc_endspecials: u8 = 7;

/// Serializes all sector-special thinkers from the active thinker chain.
///
/// Walks the thinker chain and identifies specials by their function pointer:
/// - Thinkers with `acv == NULL` that appear in the `activeceilings` list are
///   written as `tc_ceiling` records (these are ceilings paused mid-crush).
/// - `T_MoveCeiling` → `tc_ceiling`
/// - `T_VerticalDoor` → `tc_door`
/// - `T_MoveFloor` → `tc_floor`
/// - `T_PlatRaise` → `tc_plat`
/// - `T_LightFlash` → `tc_flash`
/// - `T_StrobeFlash` → `tc_strobe`
/// - `T_Glow` → `tc_glow`
///
/// Each matching thinker is preceded by its class tag byte and 4-byte
/// alignment padding. Unrecognized thinkers are silently skipped. The list is
/// terminated by `tc_endspecials`.
///
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `save_stream` must be an open, writable `FILE *` with sufficient capacity.
/// The thinker chain rooted at `thinkercap` must be fully initialized and form
/// a valid doubly-linked circular list. The `activeceilings` array must be
/// initialized. Every thinker that matches a known special function pointer
/// must point to a fully initialized special struct (`ceiling_t`, `vldoor_t`,
/// etc.).
#[doc(alias = "P_ArchiveSpecials")]
#[export_name = "P_ArchiveSpecials"]
pub extern "C" fn archive_specials()
{
    unsafe
    {
        let cap = &raw mut thinkercap;
        let mut th = (*cap).next;

        while th != cap
        {
            let func = (*th).function;

            // Check for ceiling (acv == NULL means in activeceilings list)
            if func.acv.is_none()
            {
                // Check if it's in activeceilings
                let activeceilings_ptr = std::ptr::addr_of!(crate::doom::p_ceilng::activeceilings[0]);
                let mut found = false;
                for i in 0..MAXCEILINGS
                {
                    if *activeceilings_ptr.add(i) == th as *mut ceiling_t
                    {
                        found = true;
                        break;
                    }
                }
                if found
                {
                    write_byte(tc_ceiling);
                    write_padding();
                    write_ceiling_record(th as *const ceiling_t);
                    th = (*th).next;
                    continue;
                }
            }

            if func.acp1.map(|f| f as usize) == Some(T_MoveCeiling as *const () as usize)
            {
                write_byte(tc_ceiling);
                write_padding();
                write_ceiling_record(th as *const ceiling_t);
                th = (*th).next;
                continue;
            }

            if func.acp1.map(|f| f as usize) == Some(T_VerticalDoor as *const () as usize)
            {
                write_byte(tc_door);
                write_padding();
                write_door_record(th as *const vldoor_t);
                th = (*th).next;
                continue;
            }

            if func.acp1.map(|f| f as usize) == Some(T_MoveFloor as *const () as usize)
            {
                write_byte(tc_floor);
                write_padding();
                write_floormove_record(th as *const floormove_t);
                th = (*th).next;
                continue;
            }

            if func.acp1.map(|f| f as usize) == Some(T_PlatRaise as *const () as usize)
            {
                write_byte(tc_plat);
                write_padding();
                write_plat_record(th as *const plat_t);
                th = (*th).next;
                continue;
            }

            if func.acp1.map(|f| f as usize) == Some(T_LightFlash as *const () as usize)
            {
                write_byte(tc_flash);
                write_padding();
                write_lightflash_record(th as *const lightflash_t);
                th = (*th).next;
                continue;
            }

            if func.acp1.map(|f| f as usize) == Some(T_StrobeFlash as *const () as usize)
            {
                write_byte(tc_strobe);
                write_padding();
                write_strobe_record(th as *const strobe_t);
                th = (*th).next;
                continue;
            }

            if func.acp1.map(|f| f as usize) == Some(T_Glow as *const () as usize)
            {
                write_byte(tc_glow);
                write_padding();
                write_glow_record(th as *const glow_t);
                th = (*th).next;
                continue;
            }

            th = (*th).next;
        }

        write_byte(tc_endspecials);
    }
}

/// Deserializes all sector-special thinkers from `save_stream`.
///
/// Reads thinker-class bytes in a loop until `tc_endspecials`:
/// - `tc_ceiling`: allocates `ceiling_t` (`PU_LEVSPEC`), deserializes,
///   links `sector->specialdata`, restores `T_MoveCeiling` if the function
///   slot was non-null, registers with `P_AddThinker` and
///   `P_AddActiveCeiling`.
/// - `tc_door`: allocates `vldoor_t` (`PU_LEVSPEC`), deserializes, links
///   `sector->specialdata`, unconditionally assigns `T_VerticalDoor`,
///   registers with `P_AddThinker`.
/// - `tc_floor`: allocates `floormove_t` (`PU_LEVSPEC`), deserializes,
///   links `sector->specialdata`, assigns `T_MoveFloor`, registers.
/// - `tc_plat`: allocates `plat_t` (`PU_LEVSPEC`), deserializes, links
///   `sector->specialdata`, restores `T_PlatRaise` if function non-null,
///   registers with `P_AddThinker` and `P_AddActivePlat`.
/// - `tc_flash`: allocates `lightflash_t` (`PU_LEVSPEC`), deserializes,
///   assigns `T_LightFlash`, registers.
/// - `tc_strobe`: allocates `strobe_t` (`PU_LEVSPEC`), deserializes,
///   assigns `T_StrobeFlash`, registers.
/// - `tc_glow`: allocates `glow_t` (`PU_LEVSPEC`), deserializes, assigns
///   `T_Glow`, registers.
/// - Any other byte: calls `i_error!` (fatal).
///
/// Note: the C version allocates ceilings with `PU_LEVEL`, not `PU_LEVSPEC`.
/// This port uses `PU_LEVSPEC` consistently for all specials to match the
/// `Z_Malloc` tag used for the other special types.
///
/// Called by `g_game.c` (`G_DoLoadGame`).
///
/// # Safety
///
/// `save_stream` must be an open, readable `FILE *` positioned at the byte
/// sequence written by `P_ArchiveSpecials`. The zone allocator must be
/// operational. The `sectors` array must be fully initialized so that
/// `sector->specialdata` can be set. Must be called after
/// `P_UnArchiveThinkers` so the thinker chain is in a consistent state before
/// new specials are registered with `P_AddThinker`.
// FIXME: p_saveg.c allocates ceiling_t with PU_LEVEL, not PU_LEVSPEC; this
// port uses PU_LEVSPEC here (consistent with other specials). The difference
// affects when the zone allocator may purge the block.
#[doc(alias = "P_UnArchiveSpecials")]
#[export_name = "P_UnArchiveSpecials"]
pub extern "C" fn unarchive_specials()
{
    unsafe
    {
        loop
        {
            let tclass = read_byte();
            match tclass
            {
                x if x == tc_endspecials => return,
                x if x == tc_ceiling =>
                {
                    read_padding();
                    let ceiling = Z_Malloc(
                        std::mem::size_of::<ceiling_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut ceiling_t;
                    read_ceiling_record(ceiling);
                    (*ceiling).sector.as_mut().unwrap().specialdata = ceiling as *mut c_void;

                    if ceiling.as_ref().unwrap().thinker.function.acp1.is_some()
                    {
                        ceiling.as_mut().unwrap().thinker.function = actionf_of_move_ceiling();
                    }

                    P_AddThinker(&mut (*ceiling).thinker);
                    P_AddActiveCeiling(ceiling);
                }
                x if x == tc_door =>
                {
                    read_padding();
                    let door = Z_Malloc(
                        std::mem::size_of::<vldoor_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut vldoor_t;
                    read_door_record(door);
                    (*door).sector.as_mut().unwrap().specialdata = door as *mut c_void;
                    door.as_mut().unwrap().thinker.function = actionf_of_vertical_door();
                    P_AddThinker(&mut (*door).thinker);
                }
                x if x == tc_floor =>
                {
                    read_padding();
                    let floor = Z_Malloc(
                        std::mem::size_of::<floormove_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut floormove_t;
                    read_floormove_record(floor);
                    (*floor).sector.as_mut().unwrap().specialdata = floor as *mut c_void;
                    floor.as_mut().unwrap().thinker.function = actionf_of_move_floor();
                    P_AddThinker(&mut (*floor).thinker);
                }
                x if x == tc_plat =>
                {
                    read_padding();
                    let plat = Z_Malloc(
                        std::mem::size_of::<plat_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut plat_t;
                    read_plat_record(plat);
                    (*plat).sector.as_mut().unwrap().specialdata = plat as *mut c_void;

                    if plat.as_ref().unwrap().thinker.function.acp1.is_some()
                    {
                        plat.as_mut().unwrap().thinker.function = actionf_of_plat_raise();
                    }

                    P_AddThinker(&mut (*plat).thinker);
                    P_AddActivePlat(plat);
                }
                x if x == tc_flash =>
                {
                    read_padding();
                    let flash = Z_Malloc(
                        std::mem::size_of::<lightflash_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut lightflash_t;
                    read_lightflash_record(flash);
                    flash.as_mut().unwrap().thinker.function = actionf_of_light_flash();
                    P_AddThinker(&mut (*flash).thinker);
                }
                x if x == tc_strobe =>
                {
                    read_padding();
                    let strobe = Z_Malloc(
                        std::mem::size_of::<strobe_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut strobe_t;
                    read_strobe_record(strobe);
                    strobe.as_mut().unwrap().thinker.function = actionf_of_strobe_flash();
                    P_AddThinker(&mut (*strobe).thinker);
                }
                x if x == tc_glow =>
                {
                    read_padding();
                    let glow = Z_Malloc(
                        std::mem::size_of::<glow_t>() as c_int,
                        PU_LEVSPEC,
                        std::ptr::null_mut(),
                    ) as *mut glow_t;
                    read_glow_record(glow);
                    glow.as_mut().unwrap().thinker.function = actionf_of_glow();
                    P_AddThinker(&mut (*glow).thinker);
                }
                _ =>
                {
                    i_error!(
                        "P_UnarchiveSpecials:Unknown tclass {} in savegame",
                        tclass as c_int
                    );
                }
            }
        }
    }
}
