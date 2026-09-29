//! Game completion finales: the inter-level text screens, the
//! episode-end art screens (including the episode 3 bunny scroll), and
//! the Doom II cast-of-characters roll.
//!
//! Rust port of `vendor/doomgeneric/f_finale.c`. Three distinct finale
//! stages are driven by the `FinaleStage` enum: scrolling text printed
//! character-by-character over a tiling background flat (`Text`), a
//! full-screen art image with optional bunny scroll (`ArtScreen`), and
//! the cast roll (`Cast`). Episode/map matching runs against the
//! `TEXTSCREENS` table to select the text string and background flat.
//!
//! Notable Rust-vs-C differences:
//! - Episode/map dispatch uses a `for` loop over `TEXTSCREENS` instead
//!   of a series of `if`/`else if` blocks.
//! - The C `goto stopattack` in `F_CastTicker` is refactored into the
//!   helper `cast::stop_attack`.
//! - `FinaleStage` replaces the bare `finalestage` integer, improving
//!   exhaustiveness checking in `match` expressions.
//! - Finale text strings and cast names that the C source takes from
//!   `d_englsh.h` are inlined as `const` C-string literals.
//!
//! ## Submodule Responsibility
//!
//! - `text_tables.rs` -- the finale/cast text tables (`E1TEXT`..`P6TEXT`,
//!   `CC_*`, `TEXTSCREENS`, `CASTORDER`) and the text pacing constants
//! - `lifecycle.rs` -- `start_finale`, `responder`, `ticker`, `drawer`:
//!   the stage machine and its dispatch
//! - `textstage.rs` -- `text_write`: the character-reveal text stage
//! - `cast.rs` -- the cast roll: `start_cast`, `cast_ticker` (with the
//!   load-bearing `caststate` state-address compares), `stop_attack`,
//!   `cast_responder`, `cast_print`, `cast_drawer`, and the local sprite
//!   mirror types
//! - `artscreen.rs` -- `draw_patch_col`, `bunny_scroll`,
//!   `art_screen_drawer`, and the `LAST_STAGE` frame latch
//! - `anchor.rs` -- the dead-but-exported linker anchor
//!
//! The module root holds the shared state every subfile reaches via
//! `super::`: the nine exported finale statics, the `FINALE_STAGE`/
//! `FINALE_COUNT` stage machine, the `FinaleStage` enum, the shared
//! vocabulary (`logical_gamemission`, `DEH_String`), and the C library
//! extern block carried verbatim from the pre-split file. The upstream
//! names are retained so the declarer web keeps resolving (`hu_stuff`
//! links `hu_font` by symbol into this module's reads; `g_game` owns
//! `wipegamestate`, written here to force wipes).
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the upstream-name shim re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module (the differential
//! oracle `c2rust-intermediate/src/d_main.rs` declares `F_Drawer` by
//! symbol; the conductor `d_main/display.rs:16,104` reaches `F_Drawer`
//! through the root shim). The private `goto_stopattack` helper was
//! never exported -- doc alias only, no pin. Functions only: the
//! statics keep their upstream names AND their `#[no_mangle]`
//! attributes.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `F_StartFinale` | `lifecycle::start_finale` | glue | resets game state, selects text/flat from `TEXTSCREENS` (the dead padding-sentinel check and the Chex level-5 inline hack move verbatim); shim + pin |
//! | `F_Responder` | `lifecycle::responder` | glue | Cast-stage forwarder; shim + pin |
//! | `F_Ticker` | `lifecycle::ticker` | glue | skip check, `FINALE_COUNT`, text→art transition writes `wipegamestate = -1` (d_main-owned data; this module's own surface); shim + pin |
//! | `F_TextWrite` | `textstage::text_write` | glue | flat tile + character reveal; shim + pin |
//! | `F_StartCast` | `cast::start_cast` | glue | cast roll reset; writes `wipegamestate = -1`; shim + pin |
//! | `F_CastTicker` | `cast::cast_ticker` | glue | the three `caststate == &mut info::states[..]` address compares move VERBATIM -- they drive the cast attack-stop hack and `info::states` is data (never graduates); shim + pin |
//! | (goto label) `goto_stopattack` | `cast::stop_attack` | glue | Rust refactor of the C `goto stopattack`; never exported, doc alias only |
//! | `F_CastResponder` | `cast::cast_responder` | glue | death-animation trigger; shim + pin |
//! | `F_CastPrint` | `cast::cast_print` | glue | centred cast name; shim + pin |
//! | `F_CastDrawer` | `cast::cast_drawer` | glue | BOSSBACK + sprite frame; shim + pin |
//! | `F_DrawPatchCol` | `artscreen::draw_patch_col` | glue | raw patch-column blit for the bunny scroll; shim + pin |
//! | `F_BunnyScroll` | `artscreen::bunny_scroll` | glue | PFUB scroll + END* overlay; shim + pin |
//! | `F_ArtScreenDrawer` | `artscreen::art_screen_drawer` | glue | per-episode art screen; shim + pin |
//! | `F_Drawer` | `lifecycle::drawer` | glue | stage dispatch; shim + pin (oracle-declared symbol) |
//! | (Rust addition) `F_Finale_Link_Anchor` | `anchor::finale_link_anchor` | glue | dead-but-exported (zero callers in the tree; the `doomgeneric.rs` anchor list has no f_finale entry): kept for symbol-set byte-identity, retires with the freeze zone; shim + pin |
//! | statics `finaletext`/`finaleflat`/`castnum`/`casttics`/`castdeath`/`castframes`/`castonmelee`/`castattacking`/`caststate` | module root | data | upstream names + `#[no_mangle]` retained |
//! | statics `FINALE_STAGE`/`FINALE_COUNT` | module root | data | private stage machine, `pub(super)` for the subfiles |
//! | static `LAST_STAGE` | `artscreen` | data | private; single-consumer (bunny-scroll frame latch) |
//! | types `TextScreen`/`CastInfo` | `text_tables` | data | upstream shapes kept beside their tables |
//! | types `spriteframe_t`/`spritedef_t` | `cast` | data | local sprite mirror types, single consumer `cast_drawer` |
//! | enum `FinaleStage` | module root | data | type of the `FINALE_STAGE` static |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the module draws NO `M_Random` (the only
//! `rndindex` consumer in its call tree is the graduated `f_wipe` melt
//! that the `wipegamestate = -1` writes trigger). Finale selection is a
//! pure function of (gamemission, gameepisode, gamemap, gameversion)
//! via `TEXTSCREENS`. The cast state machine's demo-relevant part is
//! the three `caststate == &mut info::states[..]` address compares --
//! carried verbatim into `cast.rs` with both sides referencing the
//! `info::states` items directly, so the attack-stop hack keeps
//! observing the same state identities. Everything else is
//! frame-golden drawing (the finale sits inside the scenario frames)
//! and tick sequencing pinned by the F9 demo goldens; `wipegamestate`
//! writes are the d_main-owned wipe trigger and stay on this module's
//! own surface.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint};
use std::ptr;

use crate::doom::d_mode;
use crate::doom::doomstat::gamemission;

pub mod anchor;
pub mod artscreen;
pub mod cast;
pub mod lifecycle;
pub mod text_tables;
pub mod textstage;

extern "C" {
    /// C standard library: convert a character to upper-case.
    fn toupper(c: c_int) -> c_int;
    /// C standard library: return the length of a NUL-terminated string.
    fn strlen(s: *const c_char) -> usize;
}

/// Return the effective game mission, collapsing Chex Quest and HacX into
/// their base missions.
///
/// Chex Quest maps to `doom`; HacX maps to `doom2`; all others are returned
/// unchanged.  C origin: `logical_gamemission()` in f_finale.c.
pub(super) unsafe fn logical_gamemission() -> c_int {
    if gamemission == d_mode::pack_chex {
        d_mode::doom
    } else if gamemission == d_mode::pack_hacx {
        d_mode::doom2
    } else {
        gamemission
    }
}

/// DEH_String is identity in this build.
#[inline(always)]
pub(super) unsafe fn DEH_String(s: *mut c_char) -> *mut c_char {
    s
}

// ---------------------------------------------------------------------------
// Exported globals (upstream names + `#[no_mangle]` retained)
// ---------------------------------------------------------------------------

/// Pointer to the NUL-terminated text string being displayed in the Text stage.
///
/// Set by [`lifecycle::start_finale`] to the matching `TEXTSCREENS` entry;
/// null when no text-screen applies.  Read by C code in `f_finale.c` and
/// `g_game.c`.  C origin: `finaletext` in f_finale.c.
#[no_mangle]
pub static mut finaletext: *mut c_char = ptr::null_mut();

/// Name of the WAD flat lump used as the tiling background in the Text stage.
///
/// Set by [`lifecycle::start_finale`].  Null when no text-screen applies.
/// C origin: `finaleflat` in f_finale.c.
#[no_mangle]
pub static mut finaleflat: *mut c_char = ptr::null_mut();

/// Index into `CASTORDER` for the enemy currently shown in the cast roll.
///
/// Incremented by [`cast::cast_ticker`] when the current enemy finishes its
/// death animation.  C origin: `castnum` in f_finale.c.
#[no_mangle]
pub static mut castnum: c_int = 0;

/// Remaining ticks before the cast animation advances to the next state.
///
/// Decremented each game tick by [`cast::cast_ticker`].
/// C origin: `casttics` in f_finale.c.
#[no_mangle]
pub static mut casttics: c_int = 0;

/// Non-zero while the current cast enemy is playing its death animation.
///
/// Set by [`cast::cast_responder`] when the player presses a key; cleared
/// when the next enemy begins.  C origin: `castdeath` in f_finale.c.
#[no_mangle]
pub static mut castdeath: c_int = 0;

/// Number of animation frames the current cast enemy has displayed so far.
///
/// Used to decide when to trigger an attack sequence (at frame 12) and when
/// to stop one (at frame 24).  C origin: `castframes` in f_finale.c.
#[no_mangle]
pub static mut castframes: c_int = 0;

/// Alternates between 0 and 1 to select melee vs. ranged attack during the
/// cast roll.
///
/// C origin: `castonmelee` in f_finale.c.
#[no_mangle]
pub static mut castonmelee: c_int = 0;

/// Non-zero while a cast enemy is in an attack animation.
///
/// C origin: `castattacking` in f_finale.c.
#[no_mangle]
pub static mut castattacking: c_int = 0;

/// Pointer to the current animation state for the cast-roll entity.
///
/// Exported with `#[no_mangle]` so that any remaining C code that reads
/// `caststate` can still resolve the symbol.  The `cast.rs` address
/// compares pin this static against `info::states` items.
/// C origin: `caststate` in f_finale.c.
#[no_mangle]
pub static mut caststate: *mut crate::doom::info::State = ptr::null_mut();

// ---------------------------------------------------------------------------
// Shared stage machine
// ---------------------------------------------------------------------------

/// The three stages a finale sequence passes through in order.
///
/// Replaces the bare `int finalestage` from the C source with an enum for
/// exhaustive `match` coverage.
#[derive(Clone, Copy)]
pub(super) enum FinaleStage {
    /// Scrolling text printed over a tiling background flat.
    Text,
    /// Full-screen art image (or bunny scroll for episode 3).
    ArtScreen,
    /// Cast-of-characters roll (Doom II only).
    Cast,
}

/// Current stage of the finale sequence.
///
/// Drives the dispatch in [`lifecycle::ticker`] and [`lifecycle::drawer`].
/// C origin: `int finalestage` in f_finale.c.
pub(super) static mut FINALE_STAGE: FinaleStage = FinaleStage::Text;

/// Tick counter incremented each game tick while the finale is active.
///
/// Controls text reveal speed, art-screen timing, and bunny-scroll position.
/// C origin: `int finalecount` in f_finale.c.
pub(super) static mut FINALE_COUNT: c_uint = 0;

//* upstream-name shim: the fourteen exported entry points keep their
//* freeze-zone caller paths (`crate::doom::f_finale::F_*`; the conductor
//* d_main/display.rs:16,104 and the c2rust differential oracle) and
//* their C symbols stay re-pinned at the definitions with
//* `#[export_name = "OriginalName"]`. Shims die with the freeze zone.
pub use anchor::finale_link_anchor as F_Finale_Link_Anchor;
pub use artscreen::{art_screen_drawer as F_ArtScreenDrawer, bunny_scroll as F_BunnyScroll, draw_patch_col as F_DrawPatchCol};
pub use cast::{
    cast_drawer as F_CastDrawer, cast_print as F_CastPrint, cast_responder as F_CastResponder,
    cast_ticker as F_CastTicker, start_cast as F_StartCast,
};
pub use lifecycle::{drawer as F_Drawer, responder as F_Responder, start_finale as F_StartFinale, ticker as F_Ticker};
pub use textstage::text_write as F_TextWrite;

#[cfg(test)]
mod tests {
    use super::*;

    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn finaletext_initially_null() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert!(finaletext.is_null());
        }
    }

    #[test]
    fn finaleflat_initially_null() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert!(finaleflat.is_null());
        }
    }

    #[test]
    fn cast_globals_default_zero() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert_eq!(castnum, 0);
            assert_eq!(casttics, 0);
            assert_eq!(castdeath, 0);
            assert_eq!(castframes, 0);
            assert_eq!(castonmelee, 0);
            assert_eq!(castattacking, 0);
        }
    }

    #[test]
    fn cast_globals_are_c_int_width() {
        use std::ffi::c_int;
        const _: () = assert!(std::mem::size_of::<c_int>() == 4);
        unsafe {
            let _: c_int = castnum;
            let _: c_int = casttics;
            let _: c_int = castdeath;
            let _: c_int = castframes;
            let _: c_int = castonmelee;
            let _: c_int = castattacking;
        }
    }

    #[test]
    fn finalestage_default_text() {
        unsafe { assert!(matches!(FINALE_STAGE, FinaleStage::Text)) };
    }
}
