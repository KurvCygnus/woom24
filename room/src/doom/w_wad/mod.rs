//! The WAD lump directory: parsing WAD headers and directories into
//! the global `lumpinfo` table, resolving lump names to indices, and
//! serving the engine's cached lump bytes.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- directory state: the `lumpinfo_t` entry layout
//!   shared across the FFI boundary and the `lumpinfo` / `numlumps`
//!   / `lumphash` statics, the in-memory directory of every lump
//!   loaded from all WADs
//! - `file.rs` -- file ingestion: `add_file` opens a WAD or
//!   single-lump file, validates the header magic, and appends its
//!   lumps to the global directory (with the `ExtendLumpInfo` growth
//!   helper and the on-disk `wadinfo_t` / `filelump_t` layouts it
//!   reads)
//! - `lookup.rs` -- name resolution: the case-insensitive 8-char name
//!   hash, the hash-table probe with backwards-linear fallback, and
//!   hash-table construction
//! - `cache.rs` -- lump access: the count/size accessors, disk reads
//!   bracketed by `I_BeginRead`/`I_EndRead`, and the zone-managed
//!   cache acquire/release surface, by lump number or by name
//! - `iwad.rs` -- launch-time validation: refuse an IWAD whose unique
//!   marker lump belongs to a different game
//! - `ffi.rs` -- the libc declarations (string and memory
//!   primitives) shared by `file.rs` and `lookup.rs`
//! - `anchor.rs` -- the `W_Wad_Link_Anchor` link anchor: one
//!   never-called C function referencing every public function so
//!   the linker keeps their symbols alive
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the wiring imports that keep the siblings'
//! `super::` paths valid, the upstream-name shims, and the anchor
//! re-export below; no content lives here. The `state.rs` statics
//! are the shared state the other submodules reach through the
//! module-root re-exports.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `W_AddFile` | `file::add_file` | glue | opens the file, validates the `IWAD`/`PWAD` magic, appends the directory; drops any stale hash table; upstream `vendor/doomgeneric/w_wad.c:139` |
//! | `ExtendLumpInfo` (file-static) | `file::ExtendLumpInfo` | glue | private directory-growth helper with zone-user fixup and chain re-link; upstream file-static name kept (private, no shim needed); upstream `vendor/doomgeneric/w_wad.c:87` |
//! | `W_LumpNameHash` | `lookup::lump_name_hash` | data | case-insensitive djb2 over the 8-char name; upstream `vendor/doomgeneric/w_wad.c:70` |
//! | `W_CheckNumForName` | `lookup::check_num_for_name` | data | hash-table probe, else backwards linear scan so later WADs override earlier ones; C symbol pinned via `#[export_name]` (legacy `extern "C"` consumer in `d_net.rs`); upstream `vendor/doomgeneric/w_wad.c:256` |
//! | `W_GetNumForName` | `lookup::get_num_for_name` | data | strict variant; `I_Error` on miss; upstream `vendor/doomgeneric/w_wad.c:306` |
//! | `W_GenerateHashTable` | `lookup::generate_hash_table` | data | builds the per-bucket chains through `lumpinfo_t::next`; upstream `vendor/doomgeneric/w_wad.c:539` |
//! | `W_NumLumps` | `cache::num_lumps` | data | directory count accessor; upstream `vendor/doomgeneric/w_wad.c:244` |
//! | `W_LumpLength` | `cache::lump_length` | data | directory size accessor, range-checked via `I_Error`; upstream `vendor/doomgeneric/w_wad.c:325` |
//! | `W_ReadLump` | `cache::read_lump` | glue | disk read bracketed by `I_BeginRead`/`I_EndRead`; upstream `vendor/doomgeneric/w_wad.c:342` |
//! | `W_CacheLumpNum` | `cache::cache_lump_num` | glue | mmap shortcut / zone retag / fill-on-miss; upstream `vendor/doomgeneric/w_wad.c:382` |
//! | `W_CacheLumpName` | `cache::cache_lump_name` | glue | name-resolving wrapper; C symbol pinned via `#[export_name]` (legacy `extern "C"` consumers in `hu_stuff.rs`, `r_draw.rs`); upstream `vendor/doomgeneric/w_wad.c:429` |
//! | `W_ReleaseLumpNum` | `cache::release_lump_num` | glue | demotes the cached block to `PU_CACHE`; upstream `vendor/doomgeneric/w_wad.c:444` |
//! | `W_ReleaseLumpName` | `cache::release_lump_name` | glue | name-resolving wrapper; upstream `vendor/doomgeneric/w_wad.c:465` |
//! | `W_CheckCorrectIWAD` | `iwad::check_correct_iwad` | glue | launch-time mission/IWAD mismatch guard; upstream `vendor/doomgeneric/w_wad.c:586` |
//! | `W_Profile` | -- (never ported) | -- | upstream's own debug lump-cache profiler, called `UNUSED` in upstream `p_setup.c:771` and absent from this port; no Rust body and no Rust callers -- nothing to split |
//! | `W_Wad_Link_Anchor` | `anchor::W_Wad_Link_Anchor` | glue | link-only anchor, not part of the upstream API; kept in `anchor.rs`, re-exported at the root; it pins the renamed Rust functions' symbols, not the shims |
//!
//! All renamed functions keep their `pub extern "C"` ABI kind but drop
//! `#[no_mangle]` with the rename: `#[no_mangle]` binds the exported C
//! symbol to the fn name, so keeping it would export spurious
//! snake_case symbols, and the `pub use` shims above already restore
//! the upstream Rust paths. Two exceptions pin their upstream C
//! symbols with `#[export_name]` because freeze-zone legacy
//! `extern "C"` blocks link against them and must compile unchanged:
//! `check_num_for_name` (declared in `d_net.rs`) and
//! `cache_lump_name` (declared in `hu_stuff.rs`, `r_draw.rs`). The
//! `lumpinfo` / `numlumps` statics (in `state.rs`) and the
//! `W_Wad_Link_Anchor` anchor (in `anchor.rs`) keep their
//! `#[no_mangle]` C symbols unchanged (they are not renamed).
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 pilot): WAD I/O shapes game
//! *content* -- which lumps exist and which bytes they hold -- at load
//! time, not the per-tic simulation observables the demo
//! synchronization surface pins. Given identical WAD inputs, the
//! lookups and cached reads here return identical bytes on every host
//! and target, and the 35 tics/s simulation consumes that content
//! through integer-exact code elsewhere; nothing in this module
//! participates in a tic's arithmetic. Accordingly `w_wad` has no
//! `dtmc` submodule: every function adjudicates to `glue` (file I/O,
//! zone-allocator marshalling, engine-lifecycle guards) or `data`
//! (pure directory and name computation) in the mapping table above.

use ffi::{calloc, free, strlen, strncmp, strncpy, toupper};
use state::lumphash;

pub mod anchor;
pub mod cache;
mod ffi;
pub mod file;
pub mod iwad;
pub mod lookup;
pub mod state;

//* path-stability wiring: the `use` bindings above keep the
//* subfiles' `use super::{...}` imports (and `cache.rs`'s test-crate
//* `lumphash` import) valid after the state/FFI moves; `lumphash`
//* stays private to the module subtree.

//* path-stability re-export: the anchor keeps its module-root path
//* (the name was never renamed, so this is wiring, not a shim).
pub use anchor::W_Wad_Link_Anchor;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::cache_lump_name as W_CacheLumpName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::cache_lump_num as W_CacheLumpNum;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::lump_length as W_LumpLength;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::num_lumps as W_NumLumps;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::read_lump as W_ReadLump;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::release_lump_name as W_ReleaseLumpName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use cache::release_lump_num as W_ReleaseLumpNum;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use file::add_file as W_AddFile;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use iwad::check_correct_iwad as W_CheckCorrectIWAD;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::check_num_for_name as W_CheckNumForName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::generate_hash_table as W_GenerateHashTable;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::get_num_for_name as W_GetNumForName;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::lump_name_hash as W_LumpNameHash;

//* path-stability re-export: the directory state keeps its
//* module-root paths (`d_main.rs` consumers and the in-module test
//* imports).
pub use state::{lumpinfo, lumpinfo_t, numlumps};
