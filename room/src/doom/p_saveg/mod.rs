//! Rust port of vendor/doomgeneric/p_saveg.c.
//!
//! Implements save-game serialization and deserialization for the entire Doom
//! game state. All active map objects (mobjs), player state, world geometry
//! deltas, thinker chains, and sector specials (ceilings, doors, floors,
//! platforms, lights) are written to or read from a `FILE *` stream managed
//! by `g_game.c`.
//!
//! The on-disk format is a flat byte stream. Multi-byte integers are always
//! little-endian regardless of host byte order. Sector and state pointers are
//! serialized as array indices; player pointers use a 1-based index (0 means
//! NULL). Thinker function pointers stored in saves are raw addresses that
//! must be re-established on load - this port guards against transmuting zero
//! to a function pointer (which is UB) by mapping `0` to `None`.
//!
//! ## Submodule Responsibility
//!
//! - `dtmc.rs` -- extracted demo-synchronization surface (the alignment
//!   formula, the 3-byte big-endian `leveltime` codec, the header version
//!   buffer) and its baseline vectors
//! - `stream.rs` -- the three `#[no_mangle]` stream globals (one data home)
//!   and the byte/word I/O helpers
//! - `swizzle.rs` -- the six pointer<->index codecs (sector/state/player)
//! - `records.rs` -- the per-struct record pairs (mapthing, thinker header,
//!   mobj, ticcmd, pspdef, player), the `make_actionf_p1!` macro and its
//!   seven `actionf_of_*` builders (thinker-identity helpers), and the
//!   unix-gated fmemopen zero-fn-pointer tests
//! - `records_specials.rs` -- the seven sector-special record pairs
//! - `paths.rs` -- the save filename formatting helper, its two lazily
//!   allocated buffers, and the two C-entry path helpers
//! - `header.rs` -- save-game header write/read and the EOF marker byte
//! - `players.rs` -- `archive_players` / `unarchive_players`
//! - `world.rs` -- `archive_world` / `unarchive_world`
//! - `thinkers.rs` -- `archive_thinkers` / `unarchive_thinkers` + the
//!   `tc_end`/`tc_mobj` tags
//! - `specials.rs` -- `archive_specials` / `unarchive_specials` + the
//!   `tc_ceiling`..`tc_endspecials` tags
//! - `anchor.rs` -- `saveg_link_anchor`, the dead-but-kept link anchor
//!
//! # Rust-vs-C differences
//!
//! - `saveg_write8` (`stream::write_byte`) increments `savegamelength`; the
//!   C version does not. Benign (`g_game` checks `ftell`, not the counter),
//!   recorded here as the standing deviation.
//! - `saveg_read16`/`saveg_write16` operate on `u16`/`u16` rather than C
//!   `short`/`short`, avoiding sign-extension ambiguity.
//! - State indices are bounds-checked on read; out-of-range values set
//!   `savegame_error` and clamp to `states[0]`.
//! - `P_SaveGameFile` always rebuilds the filename from the current `slot`
//!   argument (matching C behavior); the allocation is reused across calls.
//! - `TEMP_SAVE_FILENAME` and `SAVE_FILENAME` are module-level statics rather
//!   than function-local statics.
//! - `P_UnArchiveSpecials` allocates ceilings with `PU_LEVSPEC` where the C
//!   uses `PU_LEVEL` (FIXME carried verbatim in `specials.rs`; known port
//!   deviation, never normalize during a refactor).
//!
//! Cross-module contracts documented once here:
//! - **Thinker-function identity (THE load-bearing surface)**: the compare
//!   tables in `specials::archive_specials` and the two `P_MobjThinker`
//!   compares/relinks in `thinkers` resolve through the graduated modules'
//!   root shims, each of which is a plain `pub use` of ONE function item --
//!   any wrapper shim would silently break every compare (no compile
//!   error). The extern block below is carried VERBATIM: `P_MobjThinker` /
//!   `P_RemoveMobj` link BY SYMBOL to the `#[export_name]` pins in
//!   `p_mobj/lifecycle.rs`, and the reload paths reinstate exactly those
//!   items. `T_FireFlicker` is NOT in the compare set -- vanilla silently
//!   drops fireflicker thinkers on save; preserve the absence, do not
//!   "fix".
//! - **Extern-by-symbol wiring**: the two `extern "C"` blocks below are
//!   carried verbatim (w_wad precedent holds externs at the root). They
//!   declare `sectors`/`lines`/`sides`/`numsectors`/`numlines` with the
//!   p_lights/p_floor mirror types while `p_setup::globals` defines those
//!   statics with c_ffi types -- link is by symbol, so the mirror-vs-c_ffi
//!   type skew is a latent layout assumption; noted, do not "unify". The
//!   scalar block links g_game's session statics and d_main's
//!   `savegamedir`.
//! - **No C translation unit references any p_saveg symbol**
//!   (`doomgeneric-sys/build.rs` excludes `p_saveg.c`): every
//!   `#[export_name]` pin is wasm-surface conservatism, and the module
//!   link anchor is dead-but-exported (zero callers; `doomgeneric.rs`'s
//!   anchor list has no p_saveg entry) -- kept for symbol-set
//!   byte-identity, retires with the freeze zone (p_spec precedent).
//! - **Round-trip oracle**: the in-suite real save/load round-trip is the
//!   F9 scenario flow-5 `save_load_roundtrip` (frozen state digests,
//!   not on the divergence allowlist); the demo goldens and flow-5 are
//!   the graduation gate for this module. The in-module tests are
//!   byte-level, not record-level.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the `upstream-name shim` re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module. Functions only:
//! statics/consts/tables keep their upstream names (data-tier renaming
//! comes with freeze-zone retirement).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_TempSaveGameFile` | `paths::temp_save_game_file` | glue | shim + pin; alloc-once `TEMP_SAVE_FILENAME` cache; upstream `p_saveg.c` `P_TempSaveGameFile` |
//! | `P_SaveGameFile` | `paths::save_game_file` | glue | shim + pin; buffer-reuse semantics documented at the definition; upstream `p_saveg.c` `P_SaveGameFile` |
//! | `P_WriteSaveGameHeader` | `header::write_save_game_header` | dtmc (whole-body) | the header field ORDER is the format; version buffer via `dtmc::version_bytes`, `leveltime` via `dtmc::leveltime_pack3`; shim + pin |
//! | `P_ReadSaveGameHeader` | `header::read_save_game_header` | dtmc (whole-body) | returns 0/1; the version compare stays at the call site over `dtmc::version_bytes`; restore via `dtmc::leveltime_unpack3`; shim + pin |
//! | `P_ReadSaveGameEOF` | `header::read_save_game_eof` | glue | shim + pin |
//! | `P_WriteSaveGameEOF` | `header::write_save_game_eof` | glue | shim + pin |
//! | `P_ArchivePlayers` | `players::archive_players` | dtmc (whole-body) | shim + pin |
//! | `P_UnArchivePlayers` | `players::unarchive_players` | dtmc (whole-body) | shim + pin; mo/message/attacker nulling order is the load contract |
//! | `P_ArchiveWorld` | `world::archive_world` | dtmc (whole-body) | shim + pin |
//! | `P_UnArchiveWorld` | `world::unarchive_world` | dtmc (whole-body) | shim + pin; specialdata/soundtarget clears |
//! | `P_ArchiveThinkers` | `thinkers::archive_thinkers` | dtmc (whole-body) | shim + pin; identity compare #1 (extern `P_MobjThinker`) |
//! | `P_UnArchiveThinkers` | `thinkers::unarchive_thinkers` | dtmc (whole-body) | shim + pin; compare #2 + the `acp1: Some(P_MobjThinker)` relink; removal-pass dispatch order is the behavior |
//! | `P_ArchiveSpecials` | `specials::archive_specials` | dtmc (whole-body) | shim + pin; compares #3-#9 against the shimmed `T_*` items + compare #10 (the `activeceilings` pointer scan) |
//! | `P_UnArchiveSpecials` | `specials::unarchive_specials` | dtmc (whole-body) | shim + pin; function reinstalls through `records::actionf_of_*`; PU_LEVSPEC FIXME carried verbatim |
//! | `P_Saveg_Link_Anchor` | `anchor::saveg_link_anchor` | glue | dead-but-exported (zero callers; `doomgeneric.rs` anchor list has no p_saveg entry) -- kept, retires with the freeze zone (p_spec precedent) |
//! | `saveg_read8` | `stream::read_byte` | glue | doc alias; error-flag semantics documented |
//! | `saveg_write8` | `stream::write_byte` | glue | doc alias; increments `savegamelength` (C does not) -- standing deviation |
//! | `saveg_read16` | `stream::read_le16` | glue | doc alias |
//! | `saveg_write16` | `stream::write_le16` | glue | doc alias |
//! | `saveg_read32` | `stream::read_le32` | glue | doc alias |
//! | `saveg_write32` | `stream::write_le32` | glue | doc alias |
//! | `saveg_read_pad` | `stream::read_padding` | glue | doc alias; the alignment formula is `dtmc::pad_len` |
//! | `saveg_write_pad` | `stream::write_padding` | glue | doc alias; same |
//! | `saveg_read_enum` (C macro) | `stream::read_enum32` | glue | doc alias |
//! | `saveg_write_enum` (C macro) | `stream::write_enum32` | glue | doc alias |
//! | `saveg_write_sector_ptr` | `swizzle::write_sector_index` | glue | doc alias |
//! | `saveg_read_sector_ptr` | `swizzle::read_sector_index` | glue | doc alias |
//! | `saveg_write_state_ptr` | `swizzle::write_state_index` | glue | doc alias |
//! | `saveg_read_state_ptr` | `swizzle::read_state_index` | glue | doc alias; bounds guard + clamp (C had none) |
//! | `saveg_write_player_ptr` | `swizzle::write_player_index` | glue | doc alias; 1-based, 0 = NULL |
//! | `saveg_read_player_ptr` | `swizzle::read_player_index` | glue | doc alias |
//! | `saveg_read_mapthing_t` | `records::read_mapthing` | glue | doc alias |
//! | `saveg_write_mapthing_t` | `records::write_mapthing` | glue | doc alias |
//! | `saveg_read_thinker_t` | `records::read_thinker_header` | glue | doc alias; the 0 -> None fn-pointer UB guard |
//! | `saveg_write_thinker_t` | `records::write_thinker_header` | glue | doc alias |
//! | `saveg_read_mobj_t` | `records::read_mobj_record` | glue | doc alias; `c_ffi::mobj_t` FFI record layout |
//! | `saveg_write_mobj_t` | `records::write_mobj_record` | glue | doc alias |
//! | `saveg_read_ticcmd_t` | `records::read_ticcmd` | glue | doc alias; 6 canonical fields only |
//! | `saveg_write_ticcmd_t` | `records::write_ticcmd` | glue | doc alias |
//! | `saveg_read_pspdef_t` | `records::read_pspdef` | glue | doc alias; 0 -> NULL state (unlike mobj) |
//! | `saveg_write_pspdef_t` | `records::write_pspdef` | glue | doc alias |
//! | `saveg_read_player_t` | `records::read_player_record` | glue | doc alias |
//! | `saveg_write_player_t` | `records::write_player_record` | glue | doc alias |
//! | `saveg_read_ceiling_t` | `records_specials::read_ceiling_record` | glue | doc alias |
//! | `saveg_write_ceiling_t` | `records_specials::write_ceiling_record` | glue | doc alias |
//! | `saveg_read_vldoor_t` | `records_specials::read_door_record` | glue | doc alias |
//! | `saveg_write_vldoor_t` | `records_specials::write_door_record` | glue | doc alias |
//! | `saveg_read_floormove_t` | `records_specials::read_floormove_record` | glue | doc alias |
//! | `saveg_write_floormove_t` | `records_specials::write_floormove_record` | glue | doc alias |
//! | `saveg_read_plat_t` | `records_specials::read_plat_record` | glue | doc alias |
//! | `saveg_write_plat_t` | `records_specials::write_plat_record` | glue | doc alias |
//! | `saveg_read_lightflash_t` | `records_specials::read_lightflash_record` | glue | doc alias |
//! | `saveg_write_lightflash_t` | `records_specials::write_lightflash_record` | glue | doc alias |
//! | `saveg_read_strobe_t` | `records_specials::read_strobe_record` | glue | doc alias |
//! | `saveg_write_strobe_t` | `records_specials::write_strobe_record` | glue | doc alias |
//! | `saveg_read_glow_t` | `records_specials::read_glow_record` | glue | doc alias |
//! | `saveg_write_glow_t` | `records_specials::write_glow_record` | glue | doc alias |
//! | -- (Rust-only decomposition of C `M_snprintf` in `P_SaveGameFile`) | `paths::fill_save_filename` | glue | already plain English; no alias |
//! | `make_actionf_p1!` macro | `records` (kept; generates `pub(super)` fns) | glue | instances renamed below; each transmutes the ROOT-SHIMMED `T_*` item -- must keep importing through the shims |
//! | (macro instances; no C names) | `records::{actionf_of_move_ceiling, actionf_of_vertical_door, actionf_of_move_floor, actionf_of_plat_raise, actionf_of_light_flash, actionf_of_strobe_flash, actionf_of_glow}` | glue | consumed by `specials::unarchive_specials` |
//! | -- (new extraction; no named C counterpart) | `dtmc::pad_len` | dtmc (extracted) | the 4-byte stream-alignment formula; the `padding_calculation` vectors carried unchanged (pre-move commit `b7f07b7`) |
//! | -- (new extraction; no named C counterpart) | `dtmc::leveltime_pack3` / `dtmc::leveltime_unpack3` | dtmc (extracted) | the 3-byte big-endian `leveltime` codec (demo-sync input: Nightmare gate); pre-move vectors in `b7f07b7` |
//! | -- (new extraction; no named C counterpart) | `dtmc::version_bytes` | dtmc (extracted) | the NUL-padded `"version N\0"` buffer shared by write+read; the compare stays at the call site; pre-move vectors in `b7f07b7` |
//! | 3 `#[no_mangle]` statics (`save_stream`, `savegamelength`, `savegame_error`) | `stream` | data | upstream names + `#[no_mangle]` retained; root re-export block holds the freeze-zone paths (g_game imports; c_ffi + c_tests reach `savegamelength`/`savegame_error` by symbol) |
//! | file-local statics `TEMP_SAVE_FILENAME` / `SAVE_FILENAME`, `SAVEGAMENAME` | `paths` | data | retained |
//! | `SAVESTRINGSIZE` | `header` | data | retained |
//! | `tc_end` / `tc_mobj` | `thinkers` | data | retained |
//! | `tc_ceiling`..`tc_endspecials` | `specials` | data | retained |
//! | extern decl blocks (old `:88-115`, `:1441-1452`) | this root | wiring | carried VERBATIM (see the extern-by-symbol contract note above) |
//!
//! ## Deterministic Aspects
//!
//! Savegame bytes are the save-FILE surface, not the demo stream; but the
//! LOAD path reconstructs demo-observable simulation state: `leveltime`
//! feeds the Nightmare respawn gates, thinker-function identity decides
//! per-tic dispatch after load, and `playeringame`/skill/map steer the
//! session. Verdicts:
//!
//! - **The Archive/UnArchive families and the header pair are dtmc
//!   WHOLE-BODY with NO extraction**: `write/read_save_game_header`,
//!   `archive_players`/`unarchive_players`, `archive_world`/
//!   `unarchive_world`, `archive_thinkers`/`unarchive_thinkers`, and
//!   `archive_specials`/`unarchive_specials`. Their demo-relevant content
//!   is the field ORDER of every record and the thinker-identity
//!   reinstallation (compares + relinks), and both are inseparable from
//!   the marshalling they live in -- so nothing extracts from them.
//! - **`dtmc::pad_len`** decides the 4-byte alignment of every padded
//!   record (players, mobjs, specials); a one-byte drift desyncs the whole
//!   save file. The `padding_calculation` vectors carried from the
//!   pre-move test pin it.
//! - **`dtmc::leveltime_pack3`/`leveltime_unpack3`** are the exact bits
//!   `leveltime` round-trips through; the low-24-bit fold is documented
//!   upstream shape. `leveltime` is a demo-sync input (Nightmare gate),
//!   so the restored value is post-load sim state.
//! - **`dtmc::version_bytes`** keeps write and read sharing ONE padded
//!   buffer so the version compare cannot skew between directions; the
//!   compare itself stays at the call site.
//! - The whole-body oracle is F9 flow-5 `save_load_roundtrip` (frozen
//!   state/load digest pair at gametic 184, not on the divergence
//!   allowlist) plus the demo goldens; the in-module tests are byte-level
//!   pins on the extracted codecs and the zero-fn-pointer UB guards.

pub mod anchor;
pub mod dtmc;
pub mod header;
pub mod paths;
pub mod players;
pub mod records;
pub mod records_specials;
pub mod specials;
pub mod stream;
pub mod swizzle;
pub mod thinkers;
pub mod world;

use std::ffi::{c_char, c_int, c_void};

use crate::doom::d_player::MAXPLAYERS;
use crate::doom::p_floor::side_t;
use crate::doom::p_lights::{line_t, sector_t};

// ---------------------------------------------------------------------------
// External C declarations -- carried VERBATIM from pre-split p_saveg.rs
// (w_wad precedent holds externs at the module root). These link BY SYMBOL:
// P_MobjThinker / P_RemoveMobj to the #[export_name] pins in
// p_mobj/lifecycle.rs; the map tables to p_setup's #[no_mangle] statics
// (declared here with the p_lights/p_floor mirror types -- a latent layout
// assumption, see the contract note above); the scalars to g_game; and
// savegamedir to d_main. Never re-point to Rust paths while the freeze
// zone exists.
// ---------------------------------------------------------------------------

extern "C" {
    /// C-side mobj thinker; used for type dispatch when reading/writing the
    /// thinker chain.
    fn P_MobjThinker(mobj: *mut c_void);
    /// Places a map object into the sector and blockmap spatial structures.
    fn P_SetThingPosition(thing: *mut c_void);
    /// Removes a mobj from the world without calling its death logic; used
    /// when clearing the thinker list before loading.
    fn P_RemoveMobj(th: *mut c_void);
    /// Returns the vanilla Doom version code (e.g. 109) used in the save
    /// header version string.
    fn G_VanillaVersionCode() -> c_int;

    /// Global array of all map sectors; indexed by sector number.
    static mut sectors: *mut sector_t;
    /// Global array of all map linedefs; indexed by linedef number.
    static mut lines: *mut line_t;
    /// Global array of all map sidedefs; indexed by sidedef number.
    static mut sides: *mut side_t;
    /// Count of entries in `sectors`.
    static mut numsectors: c_int;
    /// Count of entries in `lines`.
    static mut numlines: c_int;

    /// Path to the save-game directory (NUL-terminated C string); assigned
    /// from the command-line or platform default by `g_game.c`.
    static mut savegamedir: *mut c_char;
}

extern "C" {
    /// Current skill level (0-4); serialized into the save header.
    static mut gameskill: c_int;
    /// Current episode number (1-based); serialized into the save header.
    static mut gameepisode: c_int;
    /// Current map number within the episode (1-based); serialized into the
    /// save header.
    static mut gamemap: c_int;
    /// Per-player in-game flags; non-zero means the corresponding player slot
    /// is active. Array of `MAXPLAYERS` elements.
    static mut playeringame: [c_int; MAXPLAYERS];
}

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::p_saveg::P_*`). The C symbol each shim
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. There are no C
//* referencers (build.rs excludes p_saveg.c), so the pins are
//* wasm-surface conservatism. Every shim is a plain `pub use` of ONE
//* function item -- never a wrapper (the thinker-identity compares
//* would silently break). Shims die with the freeze zone.
pub use anchor::saveg_link_anchor as P_Saveg_Link_Anchor;
pub use header::{
    read_save_game_eof as P_ReadSaveGameEOF, read_save_game_header as P_ReadSaveGameHeader,
    write_save_game_eof as P_WriteSaveGameEOF, write_save_game_header as P_WriteSaveGameHeader,
};
pub use paths::{save_game_file as P_SaveGameFile, temp_save_game_file as P_TempSaveGameFile};
pub use players::{archive_players as P_ArchivePlayers, unarchive_players as P_UnArchivePlayers};
pub use specials::{
    archive_specials as P_ArchiveSpecials, unarchive_specials as P_UnArchiveSpecials,
};
pub use stream::{save_stream, savegamelength, savegame_error};
pub use thinkers::{
    archive_thinkers as P_ArchiveThinkers, unarchive_thinkers as P_UnArchiveThinkers,
};
pub use world::{archive_world as P_ArchiveWorld, unarchive_world as P_UnArchiveWorld};
