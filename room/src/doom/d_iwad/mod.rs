//! IWAD discovery and selection: the table of known IWADs, the search
//! directory list, per-directory existence probing, filename-based
//! mission identification, and the save/suggest naming helpers. An IWAD
//! (Internal WAD) is the primary game data file (e.g. `doom.wad`,
//! `doom2.wad`). Rust port of `vendor/doomgeneric/d_iwad.c` / `d_iwad.h`.
//!
//! ## Submodule Responsibility
//!
//! - `table.rs` -- `iwad_t`, the `IWADS` priority table (order is
//!   load-bearing: it decides which IWAD wins when several candidates
//!   exist), and `MAX_IWAD_DIRS`
//! - `dirs.rs` -- the lazily built `iwad_dirs` list, its build guard,
//!   `add_iwad_dir`, and `build_iwad_dir_list`
//! - `search.rs` -- `dir_is_file`, `check_directory_has_iwad`,
//!   `search_directory_for_iwad`, `identify_iwad_by_name`, and the
//!   `DIR_SEPARATOR` / `DIR_SEPARATOR_S` constants (housed beside their
//!   heaviest users)
//! - `find.rs` -- `find_wad_by_name`, `try_find_wad_by_name`, `find_iwad`
//! - `suggest.rs` -- `find_all_iwads`, `save_game_iwad_name`,
//!   `suggest_iwad_name`, `suggest_game_name`, `check_correct_iwad`
//!
//! The module root is documentation + wiring only: the `mod` declarations,
//! the `extern "C"` block below, the `iwad_t` re-export, and the
//! upstream-name shims; no content lives here.
//!
//! # Extern-by-symbol wiring (carried VERBATIM)
//!
//! The `extern "C"` block below is carried verbatim from the pre-split
//! `d_iwad.rs` (p_saveg precedent holds externs at the module root) and
//! links BY SYMBOL: `M_FileExists` to `m_misc.rs:115`'s `#[no_mangle]`
//! definition (freeze zone); `strrchr` / `strcmp` / `strlen` / `free` /
//! `malloc` to the system libc natively and to the web-shell VFS shims on
//! wasm (`shells/web/src/wasm_vfs.rs`); `myargc` / `myargv` to
//! `m_argv`'s `#[no_mangle]` statics; `M_CheckParmWithArgs` to the
//! `#[export_name]` pin in `m_argv/lookup.rs` -- whose documentation
//! names this module as the pin's declarer, so the declarer record
//! survives here and the pin cannot retire while this block exists.
//! Never re-point to Rust paths while the freeze zone exists.
//!
//! The VFS interplay is allocation-ownership-sensitive: the engine frees
//! returned paths through the wasm `free` shim (`wasm_vfs.rs`), so
//! returned-string ownership must never be "modernised" to Rust strings
//! during a graduation.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! plain-English internal names with `#[doc(alias = "OriginalName")]`,
//! upstream-name shims at this root, and every former `#[no_mangle]`
//! symbol re-pinned with `#[export_name = "OriginalName"]`, so the
//! wasm/extern symbol name set is byte-identical to the pre-split
//! module. The pins are wasm-surface conservatism: `doomgeneric-sys/
//! build.rs` compiles zero engine C; the pins serve the in-tree link
//! surface only. All seven `pub unsafe extern "C"` functions keep their
//! pre-move signature kind; the one safe function
//! (`D_CheckCorrectIWAD`) stays safe (signature-parity rule, p_saveg
//! 7631dfa precedent). Private helpers stay private to the module
//! (upstream file-statics; doc alias only, no shim -- `LoadResponseFile`
//! precedent). `doomgeneric.rs`'s anchor list has no d_iwad entry
//! (pre-move or now) -- nothing to add there.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `AddIWADDir` (file-static) | `dirs::add_iwad_dir` | glue | private in C and Rust: doc alias only, no shim; silently drops `dir` at `MAX_IWAD_DIRS`; upstream `vendor/doomgeneric/d_iwad.c:64` |
//! | `DirIsFile` (file-static) | `search::dir_is_file` | glue | private: doc alias only; trailing-component case-insensitive match, separator required; upstream `d_iwad.c:391` |
//! | `CheckDirectoryHasIWAD` (file-static) | `search::check_directory_has_iwad` | glue | private: doc alias only; `dir == "."` elides the prefix; caller frees the returned string; upstream `d_iwad.c:408` |
//! | `SearchDirectoryForIWAD` (file-static) | `search::search_directory_for_iwad` | glue | private: doc alias only; first `IWADS` entry passing `mask` + existing; upstream `d_iwad.c:447` |
//! | `IdentifyIWADByName` (file-static) | `search::identify_iwad_by_name` | glue | private: doc alias only; writes `gamemission` through the out-param on match; upstream `d_iwad.c:475` |
//! | `BuildIWADDirList` (file-static) | `dirs::build_iwad_dir_list` | glue | private: doc alias only; adds only `"."` -- see the archaeology note below; upstream `d_iwad.c:567` |
//! | `D_FindWADByName` | `find::find_wad_by_name` | glue | shim + pin; no external Rust caller (internal helper + web-shell comments); returns the input pointer as-is when the file exists; upstream `d_iwad.c:628` |
//! | `D_TryFindWADByName` | `find::try_find_wad_by_name` | glue | shim + pin; freeze-zone caller `w_main.rs` (`-file` loading) via the shim; never returns null; upstream `d_iwad.c:679` |
//! | `D_FindIWAD` | `find::find_iwad` | glue | shim + pin; freeze-zone caller `d_main.rs:1690` (boot; mask 1 = `IWAD_MASK_DOOM`); the `-iwad` miss aborts via `i_error!`; upstream `d_iwad.c:702` |
//! | `D_FindAllIWADs` | `suggest::find_all_iwads` | glue | shim + pin; dead-but-exported (zero callers) -- kept for symbol-set byte-identity, retires with the freeze zone (p_spec precedent); upstream `d_iwad.c:755` |
//! | `D_SaveGameIWADName` | `suggest::save_game_iwad_name` | glue | shim + pin; freeze-zone caller `d_main.rs:1764`; names the savegame subdirectory; upstream `d_iwad.c:794` |
//! | `D_SuggestIWADName` | `suggest::suggest_iwad_name` | glue | shim + pin; dead-but-exported (zero callers), same retirement rule; upstream `d_iwad.c:818` |
//! | `D_SuggestGameName` | `suggest::suggest_game_name` | glue | shim + pin; graduated consumer `w_wad/iwad.rs` served by the shim; upstream `d_iwad.c:833` |
//! | `D_CheckCorrectIWAD` | `suggest::check_correct_iwad` | glue | shim + pin; keep SAFE `pub extern "C"` (pre-move signature parity); no-op -- see the archaeology note below; declared `d_iwad.h:49` |
//! | (data) `iwad_t`, `IWADS`, `MAX_IWAD_DIRS` | `table.rs` | data | names kept; `iwad_t` re-exported at this root; `IWADS` / `MAX_IWAD_DIRS` file-private -> `pub(super)` by the split (upstream file-statics) |
//! | (data) `iwad_dirs_built`, `iwad_dirs`, `num_iwad_dirs` | `dirs.rs` | data | file-private -> `pub(super)` by the split; mutable statics written only from the single-threaded boot path |
//! | (data) `DIR_SEPARATOR`, `DIR_SEPARATOR_S` | `search.rs` | data | housed beside their heaviest users; `find` reads `DIR_SEPARATOR_S` via `pub(super)` |
//! | extern decl block (pre-move `d_iwad.rs:42-66`) | this root | wiring | carried VERBATIM -- see the extern-by-symbol note above |
//!
//! # Archaeology notes
//!
//! - **ORIGCODE simplification (pre-move `d_iwad.rs:8-16`)**:
//!   `DOOMWADDIR` / `DOOMWADPATH` environment-variable paths and the
//!   Windows-registry / Unix-standard-path scans are permanently omitted
//!   from this port (upstream `d_iwad.c:192-566`); `build_iwad_dir_list`
//!   adds only `"."`. Double-nesting archaeology, NOT a vanilla-behaviour
//!   emulation row (`docs/vanilla-workarounds.md` has no d_iwad entries).
//! - **`D_CheckCorrectIWAD` declared-never-defined upstream**: declared
//!   `d_iwad.h:49`, defined only as an empty stub in the doomgeneric
//!   fork (no in-tree definition or caller in the vendored C); the Rust
//!   no-op mirrors the stub faithfully.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave C1, whole module): IWAD
//! discovery runs once inside `D_DoomMain` before the first tic
//! (`d_main.rs:1690`) and shapes the boot profile, which the two-entry
//! contract fixes for the process lifetime (the IWAD/PWAD set never
//! changes mid-game). Nothing here runs per-tic or consumes the PRNG;
//! no function's observable behavior belongs to the demo synchronization
//! surface. `identify_iwad_by_name` / `search_directory_for_iwad` write
//! `gamemission` (a demo-sync INPUT via doomstat), but the *queries*
//! themselves are boot-time file-system scans, not exact-arithmetic
//! surfaces -- every function verdicts to `glue`, and the tables are
//! `data`. Accordingly d_iwad has no `dtmc` submodule and no extraction
//! candidates exist. The baseline for the two pure string helpers is the
//! six-vector module in `search.rs` (written against the pre-move
//! bodies, pre-move commit `9baf5ff`; re-pointed to the graduated names
//! -- same vectors, same results).

pub mod dirs;
pub mod find;
pub mod search;
pub mod suggest;
pub mod table;

use std::ffi::{c_char, c_int, c_void};

// ---------------------------------------------------------------------------
// External C declarations -- carried VERBATIM from pre-split d_iwad.rs
// (p_saveg precedent holds externs at the module root). These link BY
// SYMBOL: M_FileExists to m_misc's #[no_mangle] definition, the str*
// / alloc family to system libc (web: the wasm_vfs shims), myargc /
// myargv to m_argv's #[no_mangle] statics, and M_CheckParmWithArgs to
// m_argv/lookup.rs's #[export_name] pin (this module is the recorded
// declarer). Never re-point to Rust paths while the freeze zone exists.
// ---------------------------------------------------------------------------

extern "C"
{
    /// Returns non-zero if `filename` names an existing regular file.
    fn M_FileExists(filename: *mut c_char) -> c_int;

    /// Locates the last occurrence of character `c` in string `s`.
    fn strrchr(s: *const c_char, c: c_int) -> *mut c_char;
    /// Case-sensitive string comparison.
    fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
    /// Returns the length of a null-terminated C string (not including the
    /// terminator).
    fn strlen(s: *const c_char) -> usize;
    /// Frees a heap-allocated block.
    fn free(ptr: *mut c_void);
    /// Allocates `size` bytes of uninitialised heap memory.
    fn malloc(size: usize) -> *mut c_void;

    /// Number of command-line arguments (equivalent to C `argc`).
    static mut myargc: c_int;
    /// Command-line argument vector (equivalent to C `argv`).
    static mut myargv: *mut *mut c_char;
    /// Returns the index of the first occurrence of `check` in `myargv`,
    /// provided that at least `num_args` further arguments follow it, or 0
    /// if not found.
    fn M_CheckParmWithArgs(check: *const c_char, num_args: c_int) -> c_int;
}

//* path-stability re-export: the `iwad_t` record keeps its module-root
//* path (`D_FindAllIWADs` exposes it through its return type).
pub use table::iwad_t;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use find::{find_iwad as D_FindIWAD, find_wad_by_name as D_FindWADByName, try_find_wad_by_name as D_TryFindWADByName};
pub use suggest::{
    check_correct_iwad as D_CheckCorrectIWAD, find_all_iwads as D_FindAllIWADs,
    save_game_iwad_name as D_SaveGameIWADName, suggest_game_name as D_SuggestGameName,
    suggest_iwad_name as D_SuggestIWADName,
};
