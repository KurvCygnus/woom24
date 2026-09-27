//! The thinker-chain archive/unarchive pair: mobj thinkers only, tagged
//! per record and terminated by `tc_end`. The identity contract here is
//! LOAD-BEARING: both passes compare `acp1` against the extern-by-symbol
//! `P_MobjThinker` declaration (whose symbol is pinned by
//! `p_mobj/lifecycle.rs`), and the reload pass reinstates the same
//! extern item as the thinker function -- carried verbatim, never a
//! wrapper, or every compare breaks silently.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};

use crate::i_error;
use crate::doom::info::{mobjinfo, MobjInfo};
use crate::doom::p_lights::sector_t;
use crate::doom::p_tick::{actionf_t, thinker_t, thinkercap, P_AddThinker, P_InitThinkers};
use crate::doom::z_zone::{Z_Free, Z_Malloc, PU_LEVEL};

use super::records::{read_mobj_record, write_mobj_record};
use super::stream::{read_byte, read_padding, write_byte, write_padding};

// `P_MobjThinker` / `P_SetThingPosition` / `P_RemoveMobj` are p_saveg's
// extern-by-symbol declarations (mod.rs wiring block). The `P_MobjThinker`
// symbol is pinned by `p_mobj/lifecycle.rs` (`#[export_name]` on
// `mobj_thinker`) -- that pin is what makes BOTH the compares below and the
// reload relink resolve to the ONE function item the spawn path stores.
// Carried verbatim; never re-point to Rust paths while the freeze zone
// exists.
use super::{P_MobjThinker, P_RemoveMobj, P_SetThingPosition};

/// Thinker-class tag: marks the end of the archived thinker list.
/// C origin: `tc_end` in `thinkerclass_t`.
const tc_end: u8 = 0;

/// Thinker-class tag: marks a `mobj_t` record in the archived thinker list.
/// C origin: `tc_mobj` in `thinkerclass_t`.
const tc_mobj: u8 = 1;

/// Serializes all `mobj_t` thinkers from the active thinker chain.
///
/// Walks `thinkercap.next ... thinkercap`; for each thinker whose function
/// equals `P_MobjThinker`, writes a `tc_mobj` byte, 4-byte alignment padding,
/// then the full `mobj_t` record. Non-mobj thinkers are silently skipped
/// (the C version also skips them without error). Terminates the list with a
/// `tc_end` byte.
///
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `save_stream` must be an open, writable `FILE *` with sufficient capacity.
/// The thinker chain rooted at `thinkercap` must be fully initialized and form
/// a valid doubly-linked circular list. Every thinker in the chain whose
/// function equals `P_MobjThinker` must point to a valid `mobj_t`.
#[doc(alias = "P_ArchiveThinkers")]
#[export_name = "P_ArchiveThinkers"]
pub extern "C" fn archive_thinkers()
{
    unsafe
    {
        let cap = &raw mut thinkercap;
        let mut th = (*cap).next;

        while th != cap
        {
            // Check if function is P_MobjThinker by comparing pointers
            let func = (*th).function.acp1;
            if let Some(fn_ptr) = func
            {
                if fn_ptr as usize == P_MobjThinker as *const () as usize
                {
                    write_byte(tc_mobj);
                    write_padding();
                    write_mobj_record(th as *const c_void);
                    th = (*th).next;
                    continue;
                }
            }
            th = (*th).next;
        }

        // terminating marker
        write_byte(tc_end);
    }
}

/// Clears all existing thinkers then deserializes the thinker chain from
/// `save_stream`.
///
/// First pass: walks the existing thinker chain; mobj thinkers are removed
/// via `P_RemoveMobj`, all others are freed via `Z_Free`. Then
/// `P_InitThinkers` resets the chain to empty.
///
/// Second pass: reads thinker-class bytes from the stream in a loop.
/// - `tc_end`: returns immediately.
/// - `tc_mobj`: allocates a `mobj_t` with `Z_Malloc(PU_LEVEL)`, reads its
///   fields, nulls out `target` and `tracer`, calls `P_SetThingPosition`,
///   rebuilds `info` from `mobjinfo[type]`, recomputes `floorz`/`ceilingz`
///   from the subsector's sector, assigns `P_MobjThinker` as the function,
///   and registers it with `P_AddThinker`.
/// - Any other byte: calls `i_error!` (fatal).
///
/// Called by `g_game.c` (`G_DoLoadGame`).
///
/// # Safety
///
/// `save_stream` must be an open, readable `FILE *` positioned at the byte
/// sequence written by `P_ArchiveThinkers`. The zone allocator must be
/// operational (`Z_Malloc`/`Z_Free` must be safe to call). The map geometry
/// (`sectors`, block-map, etc.) must be loaded so that `P_SetThingPosition`
/// can place each restored mobj. Must be called before `P_UnArchiveSpecials`
/// and before any code dereferences `players[i].mo`.
#[doc(alias = "P_UnArchiveThinkers")]
#[export_name = "P_UnArchiveThinkers"]
pub extern "C" fn unarchive_thinkers()
{
    unsafe
    {
        let cap = &raw mut thinkercap;
        let mut currentthinker = (*cap).next;

        // remove all current thinkers
        while currentthinker != cap
        {
            let next = (*currentthinker).next;

            let func = (*currentthinker).function.acp1;
            if let Some(fn_ptr) = func
            {
                if fn_ptr as usize == P_MobjThinker as *const () as usize
                {
                    P_RemoveMobj(currentthinker as *mut c_void);
                }
                else
                {
                    Z_Free(currentthinker as *mut c_void);
                }
            }
            else
            {
                Z_Free(currentthinker as *mut c_void);
            }

            currentthinker = next;
        }

        P_InitThinkers();

        // read saved thinkers
        loop
        {
            let tclass = read_byte();
            match tclass
            {
                x if x == tc_end => return,
                x if x == tc_mobj =>
                {
                    read_padding();
                    let mobj = Z_Malloc(
                        std::mem::size_of::<crate::doom::c_ffi::mobj_t>() as c_int,
                        PU_LEVEL,
                        std::ptr::null_mut(),
                    );
                    read_mobj_record(mobj);

                    let mo = mobj as *mut crate::doom::c_ffi::mobj_t;

                    // target/tracer set to NULL (will be rebuilt)
                    (*mo).target = std::ptr::null_mut();
                    (*mo).tracer = std::ptr::null_mut();

                    P_SetThingPosition(mobj);

                    // rebuild info from mobjinfo
                    (*mo).info = &mut mobjinfo[(*mo).type_ as usize] as *mut MobjInfo
                        as *mut crate::doom::c_ffi::mobjinfo_t;

                    // rebuild floorz/ceilingz from subsector
                    let subsec = (*mo).subsector as *mut crate::doom::c_ffi::subsector_t;
                    if !subsec.is_null()
                    {
                        let sector = (*subsec).sector as *mut sector_t;
                        if !sector.is_null()
                        {
                            (*mo).floorz = (*sector).floorheight;
                            (*mo).ceilingz = (*sector).ceilingheight;
                        }
                    }

                    // set thinker function
                    let thinker_ptr = std::ptr::addr_of_mut!((*mo).thinker_prev) as *mut thinker_t;
                    (*thinker_ptr).function = actionf_t {
                        acp1: Some(P_MobjThinker),
                    };

                    P_AddThinker(&mut (*thinker_ptr));
                }
                _ =>
                {
                    i_error!("Unknown tclass {} in savegame", tclass as c_int);
                }
            }
        }
    }
}
