//! Rust port of vendor/doomgeneric/i_endoom.c.
//!
//! Stub for the text-mode ENDOOM screen displayed after the game quits. The
//! original chocolate-doom code initialises the textgraphics library and
//! blits 80x25 character cells from the ENDOOM lump until the user presses a
//! key. Doomgeneric (and this port) does not ship a text-mode backend, so
//! the function returns immediately without rendering anything.
//!
//! ## Submodule Responsibility
//!
//! - `stub.rs` -- the single no-op `show_endoom_screen` entry point
//!
//! The module root is documentation + wiring only: the `mod` declaration and
//! the upstream-name shim below; no content lives here. The module is a
//! `mod.rs` + single-responsibility-file graduation: one no-op function, no
//! statics, no extracted surface.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `I_Endoom` | `stub::show_endoom_screen` | glue | no-op stub; `endoom_data` ignored; C symbol pinned via `#[export_name]` (the only caller, `d_main/boot.rs`, imports the upstream name through the root shim); upstream `vendor/doomgeneric/i_endoom.c:42` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the function is a shutdown-path no-op that touches no
//! simulation state; the ENDOOM lump is present-side cosmetics that never
//! enters a tic's observables.

mod stub;

//* upstream-name shim: freeze-zone callers keep the upstream name. The C
//* symbol it forwards to is re-pinned at the definition with
//* `#[export_name = "I_Endoom"]`.
pub use stub::show_endoom_screen as I_Endoom;
