//! Rust port of vendor/doomgeneric/i_cdmus.c.
//!
//! Stubs for the Hexen-style audio-CD interface. The original chocolate-doom
//! implementation wraps SDL_cdrom to play tracks from a physical CD-ROM,
//! but doomgeneric compiles all of that out (the SDL paths sit behind
//! `#ifdef ORIGCODE`, which is never defined). The Rust port mirrors that:
//! every function is a no-op that returns success and resets the `cd_Error`
//! flag. The symbols remain exported because Hexen and shared Heretic code
//! still reference them.
//!
//! ## Submodule Responsibility
//!
//! - `stubs.rs` -- the `cd_Error` static and all nine no-op stub functions
//!
//! The module root is documentation + wiring only: the `mod` declaration and
//! the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location              | Surface | Notes |
//! |--------------|---------------------------|---------|-------|
//! | `I_CDMusInit` | `stubs::cd_mus_init` | glue | clears `cd_Error`, returns 0; C symbol pinned via `#[export_name]` (dead-but-exported); upstream `vendor/doomgeneric/i_cdmus.c:38` |
//! | `I_CDMusPrintStartup` | `stubs::cd_mus_print_startup` | glue | no-op; upstream `i_cdmus.c:92` |
//! | `I_CDMusPlay` | `stubs::cd_mus_play` | glue | no-op stub returning 0; upstream `i_cdmus.c:107` |
//! | `I_CDMusStop` | `stubs::cd_mus_stop` | glue | no-op stub returning 0; upstream `i_cdmus.c:130` |
//! | `I_CDMusResume` | `stubs::cd_mus_resume` | glue | no-op stub returning 0; upstream `i_cdmus.c:145` |
//! | `I_CDMusSetVolume` | `stubs::cd_mus_set_volume` | glue | clears `cd_Error`, returns 0 (never implemented even outside `ORIGCODE`); upstream `i_cdmus.c:160` |
//! | `I_CDMusFirstTrack` | `stubs::cd_mus_first_track` | glue | no-op stub returning 0; upstream `i_cdmus.c:169` |
//! | `I_CDMusLastTrack` | `stubs::cd_mus_last_track` | glue | no-op stub returning 0; upstream `i_cdmus.c:202` |
//! | `I_CDMusTrackLength` | `stubs::cd_mus_track_length` | glue | no-op stub returning 0; upstream `i_cdmus.c:219` |
//! | `cd_Error` (static) | `stubs::cd_Error` | data | static keeps its upstream name and `#[no_mangle]` export unchanged |
//!
//! Every function in the table is dead-but-exported (zero callers in the
//! tree): kept for symbol-set byte-identity, retires with the freeze zone.
//! Each rename re-pins its pre-move C symbol with `#[export_name]` so the
//! wasm symbol namespace stays identical.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: every function is a platform no-op stub that never reads
//! or writes simulation state, and the only datum (`cd_Error`) is a
//! never-set error flag owned by the (absent) CD backend. Audio-CD playback
//! is boot/platform configuration, not the per-tic demo synchronization
//! surface.

mod stubs;

//* path-stability re-export: the error flag keeps its module-root path.
pub use stubs::cd_Error;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use stubs::{
    cd_mus_first_track as I_CDMusFirstTrack, cd_mus_init as I_CDMusInit,
    cd_mus_last_track as I_CDMusLastTrack, cd_mus_play as I_CDMusPlay,
    cd_mus_print_startup as I_CDMusPrintStartup, cd_mus_resume as I_CDMusResume,
    cd_mus_set_volume as I_CDMusSetVolume, cd_mus_stop as I_CDMusStop,
    cd_mus_track_length as I_CDMusTrackLength,
};
