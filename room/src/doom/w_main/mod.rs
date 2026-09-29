//! Rust port of vendor/doomgeneric/w_main.c.
//!
//! Common command-line WAD loader. Scans `argv` for `-file <wad>...` and
//! adds each matching WAD to the lump directory via `W_AddFile`. The C
//! original also handled `-merge`, `-nwtmerge`, `-af`, `-as` and `-aa`
//! under `#ifdef FEATURE_WAD_MERGE`, but doomgeneric `#undef`s that
//! feature in `doomfeatures.h`, so the Rust port only ports the `-file`
//! path. Returns whether any additional WAD was loaded (the "homebrew
//! levels" / modified-game flag).
//!
//! ## Submodule Responsibility
//!
//! - `parse.rs` -- `parse_command_line_wads`, the `-file` scanner
//!
//! The module root is documentation + wiring only: the `mod` declaration and
//! the upstream-name shim below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `W_ParseCommandLine` | `parse::parse_command_line_wads` | glue | walks `myargv` after `-file`, feeds each non-flag argument through `D_TryFindWADByName` + `W_AddFile`; C symbol pinned via `#[export_name]` (the only caller, `d_main/boot.rs`, imports the upstream name through the root shim); the `FEATURE_WAD_MERGE` parameters are not ported (doomgeneric `#undef`s the feature); upstream `vendor/doomgeneric/w_main.c:30` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the function runs once during engine boot, before any
//! tic is simulated. It shapes game *content* (which WADs enter the lump
//! directory), the same load-time category `w_wad` adjudicates to glue, and
//! its Boolean result only sets the `modifiedgame` boot flag.

mod parse;

//* upstream-name shim: freeze-zone callers keep the upstream name. The C
//* symbol it forwards to is re-pinned at the definition with
//* `#[export_name = "W_ParseCommandLine"]`.
pub use parse::parse_command_line_wads as W_ParseCommandLine;
