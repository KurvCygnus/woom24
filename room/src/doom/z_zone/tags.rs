//! The `PU_*` purge-level tags that classify every Zone allocation.
//!
//! Tag values are byte-stable contract data: the DOS zone-header bytes
//! vanilla reads past an undersized REJECT lump are modeled in
//! `p_setup::reject` with `PU_LEVEL` and the tag ordering quoted
//! literally (`docs/vanilla-workarounds.md` entry 4), so these values
//! must never change. The upstream tag constants `PU_SOUND` (2),
//! `PU_MUSIC` (3) and `PU_NUM_TAGS` were never ported and have zero
//! Rust consumers; they are deliberately not added here.

use std::ffi::c_int;

/// Pinned allocation; lives for the lifetime of the program unless
/// explicitly freed. Matches `PU_STATIC` in `z_zone.h`.
pub const PU_STATIC: c_int = 1;
/// Tag value marking a block as currently free. Matches `PU_FREE`.
pub const PU_FREE: c_int = 4;
/// Pinned to the current level; mass-freed by `Z_FreeTags(PU_LEVEL, ...)`
/// when the player exits the map. Matches `PU_LEVEL`.
pub const PU_LEVEL: c_int = 5;
/// Pinned to the current level for specials (sector effects, plats,
/// doors). Matches `PU_LEVSPEC`.
pub const PU_LEVSPEC: c_int = 6;
/// First purgeable tag; anything at or above this can be reclaimed by
/// `Z_Malloc`. Matches `PU_PURGELEVEL`.
pub const PU_PURGELEVEL: c_int = 7;
/// Purgeable cache (textures, sprites). Matches `PU_CACHE`.
pub const PU_CACHE: c_int = 8;
