//! The four `extern "C"` declaration blocks, carried VERBATIM from the
//! flat `c_ffi.rs` (r_draw.c functions, g_game statics, savegame
//! statics, p_enemy statics). These blocks are the freeze-zone linkage
//! surface: `doomgeneric-sys` compiles zero C sources, so every entry
//! here resolves against the engine's `#[no_mangle]`/`#[export_name]`
//! exports at link time (wasm-lld / host ld) -- they are never edited,
//! renamed, or modernised (f1 report §0.1); c_tests and roughly half
//! the tree consume them through the hub paths.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::{c_int, c_uint, c_void};

// ---------------------------------------------------------------------------
// r_draw.c — remaining C functions
// ---------------------------------------------------------------------------

extern "C" {
    /// Initialise the view-window buffer for the given dimensions
    /// (`R_InitBuffer` in `r_draw.c`).
    pub fn R_InitBuffer(width: c_int, height: c_int);
    /// Build per-player palette-translation tables
    /// (`R_InitTranslationTables` in `r_draw.c`).
    pub fn R_InitTranslationTables();
    /// Fill the area outside the 3D view window with the back screen
    /// (`R_FillBackScreen` in `r_draw.c`).
    pub fn R_FillBackScreen();
    /// Erase a horizontal run of pixels from the back-screen buffer
    /// (`R_VideoErase` in `r_draw.c`).
    pub fn R_VideoErase(ofs: c_uint, count: c_int);
    /// Redraw the view-window border after a console or menu close
    /// (`R_DrawViewBorder` in `r_draw.c`).
    pub fn R_DrawViewBorder();
}

// ---------------------------------------------------------------------------
// g_game.c — movement tables and game-state globals.
// forwardmove / sidemove / angleturn are static initialisers (non-zero).
// ---------------------------------------------------------------------------

extern "C" {
    /// Forward movement speed table: [slow, fast] (fixed_t, unit/tic).
    /// Values: {0x19, 0x32} = {25, 50}.
    pub static mut forwardmove: [c_int; 2];
    /// Lateral (strafe) movement speed table: [slow, fast] (fixed_t, unit/tic).
    /// Values: {0x18, 0x28} = {24, 40}.
    pub static mut sidemove: [c_int; 2];
    /// Turn-speed table: [normal, fast, slow] (BAM units/tic).
    /// Values: {640, 1280, 320}.  Index 2 is used for the first SLOWTURNTICS (6)
    /// tics; after that index 0 (or 1 with run) is used.
    pub static mut angleturn: [c_int; 3];
    /// Next slot in the circular body-queue ring buffer.
    pub static mut bodyqueslot: c_int;
    /// When true, savegame file size is capped at the vanilla limit (0x2c000).
    pub static mut vanilla_savegame_limit: c_int;
    /// When true, demo file size is capped at the vanilla limit.
    pub static mut vanilla_demo_limit: c_int;
    /// When true, all graphics are preloaded at level start.
    pub static mut precache: c_int;
    /// When true (set by -testcontrols), exit after the first tic.
    pub static mut testcontrols: c_int;
    /// Gametic at which the current level started.
    pub static mut levelstarttic: c_int;
    /// Total enemy count on the current level (for intermission).
    pub static mut totalkills: c_int;
    /// Total item count on the current level (for intermission).
    pub static mut totalitems: c_int;
    /// Total secret count on the current level (for intermission).
    pub static mut totalsecret: c_int;
}

// ---------------------------------------------------------------------------
// p_saveg.c — save-game serialization globals
// ---------------------------------------------------------------------------

extern "C" {
    /// Total number of bytes written to the current save-game stream.
    pub static mut savegamelength: c_int;
    /// True when a write error has occurred during save serialization.
    pub static mut savegame_error: c_int; // boolean
}

// ---------------------------------------------------------------------------
// p_enemy.c — enemy AI globals
// ---------------------------------------------------------------------------

extern "C" {
    /// Per-direction X velocity multipliers for 8-directional monster movement.
    /// Values: {FRACUNIT, 47000, 0, -47000, -FRACUNIT, -47000, 0, 47000}.
    pub static mut xspeed: [c_int; 8];
    /// Per-direction Y velocity multipliers for 8-directional monster movement.
    /// Values: {0, 47000, FRACUNIT, 47000, 0, -47000, -FRACUNIT, -47000}.
    pub static mut yspeed: [c_int; 8];
    /// Array of potential brain-teleport destinations (Spider Mastermind).
    pub static mut braintargets: [*mut c_void; 32];
    /// Number of valid braintargets entries (set by P_SpawnBrainTargets).
    pub static mut numbraintargets: c_int;
    /// Index of the next braintarget to use in the round-robin sequence.
    pub static mut braintargeton: c_int;
}
