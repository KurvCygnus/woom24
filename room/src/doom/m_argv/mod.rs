//! Command-line argument handling: parameter lookup, `@responsefile`
//! expansion, and executable-basename extraction around the shared
//! `myargc` / `myargv` pair. Rust port of `vendor/doomgeneric/m_argv.c`.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the process-wide `myargc` / `myargv` statics (the
//!   single mutable argv pair shared with the host) and the two
//!   private constants (`MAXARGVS`, `DIR_SEPARATOR`)
//! - `lookup.rs` -- the pure command-line scan:
//!   `check_parm_with_args` plus its `check_parm` / `parm_exists`
//!   wrappers, and the baseline-vector test module
//! - `response.rs` -- `@responsefile` expansion: tokenising,
//!   quoted-string handling, and the in-place `myargv` splice
//! - `exename.rs` -- executable basename extraction from `myargv[0]`
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the statics re-export, and the upstream-name shims
//! below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! plain-English internal names with `#[doc(alias = "OriginalName")]`,
//! upstream-name shims at this root, `#[no_mangle]` dropped with the
//! rename -- EXCEPT the two functions freeze-zone legacy `extern "C"`
//! blocks link by symbol (re-pinned with `#[export_name]`) and the two
//! statics, which keep their names and `#[no_mangle]` outright
//! (functions-only ruling). All functions keep their pre-move SAFE
//! `pub extern "C"` kind.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `M_CheckParmWithArgs` | `lookup::check_parm_with_args` | glue | shim + `#[export_name = "M_CheckParmWithArgs"]` pin (declared extern by the `d_iwad` module root, carried verbatim at `d_iwad/mod.rs`); case-insensitive scan with the `i < myargc - num_args` window; pinned by the baseline tests; upstream `vendor/doomgeneric/m_argv.c:43` |
//! | `M_ParmExists` | `lookup::parm_exists` | glue | shim; sole live consumer paths: `i_system/error.rs` (`I_Error`'s `-nogui` scan -- keep allocation-free) and `statdump/capture.rs`; upstream `m_argv.c:63` |
//! | `M_CheckParm` | `lookup::check_parm` | glue | shim + `#[export_name = "M_CheckParm"]` pin -- a FIFTH pin beyond the report's four-pin list: `d_net/mod.rs:155` declares it extern and `D_ConnectNetGame` (every boot) links it by symbol; the report's declarer sweep missed it, surfaced by the `struct_sizes` bin link, and the AGENTS.md declarer rule forced the pin (disclosed deviation); upstream `m_argv.c:68` |
//! | `LoadResponseFile` (file-static) | `response::load_response_file` | glue | private; upstream kept it file-static too, so doc alias only (no shim); whitespace/quoted-string tokenising with `I_Error` on missing file / unclosed quotes; upstream `m_argv.c:75` |
//! | `M_FindResponseFile` | `response::find_response_file` | glue | shim + `#[export_name = "M_FindResponseFile"]` pin (declared extern by `doomgeneric.rs`, called from `doomgeneric_Create` before `D_DoomMain`); upstream `m_argv.c:235` |
//! | `M_GetExecutableName` | `exename::get_executable_name` | glue | shim; `strrchr` over `DIR_SEPARATOR`; upstream `m_argv.c:250` |
//! | `myargc` (static) | `state::myargc` | data | name + `#[no_mangle]` kept (statics ruling): written by `doomgeneric_Create` through extern blocks (`doomgeneric.rs`, `d_iwad.rs`), read by the web shell's argv-anchor contract (`shells/web/src/init_pipeline.rs` -- the argv array must survive for the process lifetime, never modernise into accessors); upstream `m_argv.c:29` |
//! | `myargv` (static) | `state::myargv` | data | same contract as `myargc`; upstream `m_argv.c:30` |
//! | `MAXARGVS` / `DIR_SEPARATOR` | `state.rs` | data | private consts kept (housed beside the statics they serve) |
//!
//! Upstream archaeology note: upstream gates `LoadResponseFile` /
//! `M_FindResponseFile` behind `#if ORIGCODE` (dead there, live in
//! this port) -- double-nesting archaeology, not a vanilla-behaviour
//! emulation row.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave B5, whole module):
//! everything here is boot-time. The argv pair is fixed at process
//! start (web: `anchor_argv`; native: `main`) and consumed during
//! `D_DoomMain` before the first tic; the parm scan and response-file
//! expansion shape the CLI profile, which per the two-entry contract
//! is boot-fixed (the PWAD/asset set never changes mid-game), so
//! nothing here runs per-tic or feeds the demo sequence.
//! `load_response_file` mutates the `myargv` / `myargc` statics --
//! marshalling glue, not simulation state. Accordingly `m_argv` has
//! no `dtmc` submodule: every function adjudicates to `glue`
//! (boot-time CLI parsing and argv rewriting) or `data` (the argv
//! statics and constants) in the mapping table above.

pub mod exename;
pub mod lookup;
pub mod response;
pub mod state;

//* path-stability re-export: the argv statics keep their module-root
//* paths (`d_main.rs`, `g_game`, and the freeze-zone extern
//* declarers' provenance); the C symbols are unaffected (statics keep
//* `#[no_mangle]` at the definition).
pub use state::{myargc, myargv};

//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::check_parm as M_CheckParm;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::check_parm_with_args as M_CheckParmWithArgs;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use lookup::parm_exists as M_ParmExists;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use response::find_response_file as M_FindResponseFile;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use exename::get_executable_name as M_GetExecutableName;
