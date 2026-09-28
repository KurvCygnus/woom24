//! The known-IWAD table: the `iwad_t` record type, the `IWADS` priority
//! table, and the `MAX_IWAD_DIRS` capacity bound. Verbatim data from the
//! pre-split module; the entry order is load-bearing (it decides which
//! IWAD wins when several candidates exist in the same directory).
//! 
//! The C record type is lowercase (`iwad_t`); the name is verbatim
//! upstream data, so the type-name lint is silenced file-wide.
#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int};

use crate::doom::d_mode::{
    commercial, doom, doom2, heretic, hexen, pack_chex, pack_hacx, pack_plut, pack_tnt, retail,
    shareware, strife,
};

/// Hard limit on the number of IWAD search directories.
///
/// Corresponds to `MAX_IWAD_DIRS` in `d_iwad.c`.
pub(super) const MAX_IWAD_DIRS: usize = 128;

/// Metadata for a single known IWAD file.
///
/// Corresponds to `iwad_t` / `typedef struct { ... } iwad_t` in `d_iwad.h`.
/// Instances live in the static `IWADS` table.
///
/// # Layout invariant
/// This type is `#[repr(C)]` to match the C struct layout exactly, allowing
/// pointers to array elements to be passed back to C callers.
#[repr(C)]
pub struct iwad_t
{
    /// Canonical filename of the IWAD (e.g. `"doom2.wad\0"`).
    pub name: *mut c_char,
    /// Game mission identifier; one of the `d_mode::*` integer constants.
    pub mission: c_int,
    /// Game mode identifier (shareware, retail, commercial, etc.).
    pub mode: c_int,
    /// Human-readable name of the game (e.g. `"Doom II\0"`).
    pub description: *mut c_char,
}

/// SAFETY: All `*mut c_char` fields point into static string literals and are
/// never mutated; the struct is effectively immutable once constructed.
unsafe impl Sync for iwad_t {}

/// Table of all known IWAD files, in priority order.
///
/// The ordering determines which IWAD is selected when multiple candidates are
/// present in the same directory. Commercial releases are listed before
/// shareware releases. Mirrors `iwads[]` in `d_iwad.c`.
pub(super) static IWADS: [iwad_t; 14] = [
    iwad_t
    {
        name: c"doom2.wad".as_ptr().cast_mut(),
        mission: doom2,
        mode: commercial,
        description: c"Doom II".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"plutonia.wad".as_ptr().cast_mut(),
        mission: pack_plut,
        mode: commercial,
        description: c"Final Doom: Plutonia Experiment".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"tnt.wad".as_ptr().cast_mut(),
        mission: pack_tnt,
        mode: commercial,
        description: c"Final Doom: TNT: Evilution".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"doom.wad".as_ptr().cast_mut(),
        mission: doom,
        mode: retail,
        description: c"Doom".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"doom1.wad".as_ptr().cast_mut(),
        mission: doom,
        mode: shareware,
        description: c"Doom Shareware".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"chex.wad".as_ptr().cast_mut(),
        mission: pack_chex,
        mode: shareware,
        description: c"Chex Quest".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"hacx.wad".as_ptr().cast_mut(),
        mission: pack_hacx,
        mode: commercial,
        description: c"Hacx".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"freedm.wad".as_ptr().cast_mut(),
        mission: doom2,
        mode: commercial,
        description: c"FreeDM".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"freedoom2.wad".as_ptr().cast_mut(),
        mission: doom2,
        mode: commercial,
        description: c"Freedoom: Phase 2".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"freedoom1.wad".as_ptr().cast_mut(),
        mission: doom,
        mode: retail,
        description: c"Freedoom: Phase 1".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"heretic.wad".as_ptr().cast_mut(),
        mission: heretic,
        mode: retail,
        description: c"Heretic".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"heretic1.wad".as_ptr().cast_mut(),
        mission: heretic,
        mode: shareware,
        description: c"Heretic Shareware".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"hexen.wad".as_ptr().cast_mut(),
        mission: hexen,
        mode: commercial,
        description: c"Hexen".as_ptr().cast_mut(),
    },
    iwad_t
    {
        name: c"strife1.wad".as_ptr().cast_mut(),
        mission: strife,
        mode: commercial,
        description: c"Strife".as_ptr().cast_mut(),
    },
];
