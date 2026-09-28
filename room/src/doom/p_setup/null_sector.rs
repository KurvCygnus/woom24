//! The null-sector sentinel: the synthetic `sector_t` that substitutes
//! for segs of two-sided linedefs with a missing or out-of-range back
//! sidedef. Vanilla Doom dereferenced the null pointer and read its
//! sector fields from DOS address 0; this module reproduces that read
//! deterministically. The emulation is cataloged as
//! `docs/vanilla-workarounds.md` row 5 ("missed-backside null sector").

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};

use crate::doom::c_ffi::sector_t;
use crate::doom::i_system::I_GetMemoryValue;

/// Return a pointer to a synthetic `sector_t` that represents the sector at
/// address 0, used to handle malformed WAD data (the "glass hack").
///
/// The returned sector is initialized on first call by reading 4 bytes each
/// from address 0 and 4 via `I_GetMemoryValue`, matching vanilla Doom's
/// behavior of reading the initial heap block header at address 0.
/// Subsequent calls return the already-initialized sector without re-reading.
///
/// C callers: `P_LoadSegs` (this file). Also called via C FFI from legacy
/// unported code that encounters two-sided linedefs with an invalid back sidenum.
///
/// The returned pointer aliases the module's function-local static state;
/// callers must not free it or write through it (the sentinel is shared).
#[doc(alias = "GetSectorAtNullAddress")]
#[export_name = "GetSectorAtNullAddress"]
pub extern "C" fn sector_at_null_address() -> *mut sector_t
{
    static mut NULL_SECTOR_IS_INITIALIZED: bool = false;
    static mut NULL_SECTOR: sector_t = unsafe { std::mem::zeroed() };

    unsafe
    {
        if !NULL_SECTOR_IS_INITIALIZED
        {
            NULL_SECTOR = std::mem::zeroed();
            I_GetMemoryValue(
                0,
                &raw mut NULL_SECTOR.floorheight as *mut c_int as *mut c_void,
                4,
            );
            I_GetMemoryValue(
                4,
                &raw mut NULL_SECTOR.ceilingheight as *mut c_int as *mut c_void,
                4,
            );
            NULL_SECTOR_IS_INITIALIZED = true;
        }
        &raw mut NULL_SECTOR
    }
}
