//! Rust port of vendor/doomgeneric/doomstat.c.
//!
//! Global game state variables shared by every module in the engine.
//! The header (`doomstat.h`) serves as the single source of truth for all
//! game-wide state: IWAD identity, active game mode, savegame directory,
//! player bookkeeping, demo flags, and more.  In practice only a small
//! subset of those variables lives here; the rest are defined in the C
//! modules that own them and declared `extern` in the header.
//!
//! This module covers the five variables whose definitions appeared in the
//! original `doomstat.c`: `gamemode`, `gamemission`, `gameversion`,
//! `gamedescription`, and `modifiedgame`.  All are `#[no_mangle]` so the
//! remaining C translation units can link against them directly.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the five statics and their `defaults_match_c` test
//!
//! The module root is documentation + wiring only: the `mod` declaration and
//! the path-stability re-export below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! No functions: the module owns storage only (the `info.rs` / `tables.rs`
//! data-wholesale precedent). All five statics keep their upstream names and
//! `#[no_mangle]` exports byte-for-byte (`vendor/doomgeneric/doomstat.c:24-31`
//! -- the same five), so there are no shims -- only the path-stability
//! re-export above. The freeze-zone `extern "C"` declarer `hu_stuff.rs:408`
//! (`static mut gamemode`) keeps linking by symbol unchanged.
//!
//! ## Deterministic Aspects
//!
//! No dtmc *surface* to extract: these statics ARE part of the demo-sync
//! environment -- `gamemode` / `gameversion` branch simulation behavior in
//! `p_*` / `g_game` every tic -- but as data they need no extraction. The
//! module owns storage only; the writes happen in `d_main`'s boot/identify
//! glue (outside this module), and readers consume the values through the
//! same integer-typed statics on every host and target.

mod state;

//* path-stability re-export: the five statics keep their module-root
//* paths for the freeze-zone callers (`f_finale`, `hu_stuff`, `m_menu`,
//* `r_draw`, `r_things`, `st_stuff`, `s_sound`, `wi_stuff`, `d_main/*`,
//* `g_game/*`, `p_enemy/*`, `p_inter/*`, `p_mobj/*`, `p_pspr/*`,
//* `p_setup/*`, `p_telept`, `p_switch`, `p_user/*`).
pub use state::{gamedescription, gamemission, gamemode, gameversion, modifiedgame};
