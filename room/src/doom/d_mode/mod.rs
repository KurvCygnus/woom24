//! Game mode, mission, and version vocabulary for the DOOM engine: which
//! game (IWAD) is loaded, which release tier it belongs to, and which
//! executable version is emulated -- plus the boot-time validators over
//! that vocabulary. Rust port of `vendor/doomgeneric/d_mode.c` / `d_mode.h`.
//!
//! The three central concepts are modelled as `pub const c_int` groups
//! rather than Rust enums, preserving bit-for-bit compatibility with the C
//! enums and letting the values live directly in the `c_int` doomstat
//! globals (`gamemode` / `gamemission` / `gameversion`) and cross the FFI
//! boundary without conversion.
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the 29 `pub const c_int` discriminants of the three C
//!   enums (`GameMission_t`, `GameMode_t`, `GameVersion_t`);
//!   re-exported at this root because 17 freeze-zone files use
//!   `d_mode::<const>` root paths (~163 references)
//! - `tables.rs` -- the private `ValidMode` / `VALID_MODES` and
//!   `ValidVersion` / `VALID_VERSIONS` lookup tables the validators scan
//! - `validate.rs` -- `valid_game_mode` / `valid_episode_map` /
//!   `num_episodes` / `valid_game_version` / `is_episode_map`, plus their
//!   known-vector test module (the graduation baseline)
//! - `names.rs` -- `game_mission_string`, the WAD-load diagnostic naming
//!   helper, plus its tests
//!
//! The module root is documentation + wiring only: the `mod` declarations,
//! the consts re-export, and the upstream-name shims below; no content
//! lives here.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! plain-English internal names with `#[doc(alias = "OriginalName")]`,
//! upstream-name shims at this root, and every former `#[no_mangle]`
//! symbol re-pinned with `#[export_name = "OriginalName"]`, so the
//! wasm/extern symbol name set is byte-identical to the pre-split module.
//! The pins are wasm-surface conservatism: `doomgeneric-sys/build.rs`
//! compiles zero engine C and no in-tree extern declarer of any of the
//! six was found, so no pin has a live link consumer; `doomgeneric.rs`'s
//! anchor list has no d_mode entry (pre-move or now) -- nothing to add
//! there. Functions only: the 29 consts and the two private tables keep
//! their names. All six functions keep their pre-move SAFE `pub extern
//! "C"` kind.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `D_ValidGameMode` | `validate::valid_game_mode` | glue | shim + pin; boot/menu-time membership query over `VALID_MODES`; zero in-tree callers (the C menu/warp callers were ported with the logic inlined); upstream `vendor/doomgeneric/d_mode.c:50` |
//! | `D_ValidEpisodeMap` | `validate::valid_episode_map` | glue | shim + pin; Heretic secret-episode special cases (retail ep6 maps 1-3, registered ep4 map 1) carried verbatim; zero in-tree callers; upstream `d_mode.c:65` |
//! | `D_GetNumEpisodes` | `validate::num_episodes` | glue | shim + pin; plain-English name (the C `Get` ceremony is dropped, cf. `saveg_read8` -> `read_byte`); iterates `valid_episode_map` at map 1; menu-structure query, zero in-tree callers; upstream `d_mode.c:103` |
//! | `D_ValidGameVersion` | `validate::valid_game_version` | glue | shim + pin; boot-time `-gameversion` acceptance check; the doom-family normalisation (`doom2`/`pack_*` -> `doom`) carried verbatim; zero in-tree callers; upstream `d_mode.c:135` |
//! | `D_IsEpisodeMap` | `validate::is_episode_map` | glue | shim + pin; `ExMy`-vs-`MAPxx` level-naming format query; zero in-tree callers; upstream `d_mode.c:161` |
//! | `D_GameMissionString` | `names::game_mission_string` | glue | shim + pin; sole live consumer is the graduated `w_wad/iwad.rs` (served by the shim); returns `*mut c_char` into static literals -- writing through it is UB exactly as in C, never "fix" to `&'static CStr` during a graduation; upstream `d_mode.c:182` |
//! | (data) 29 enum consts | `consts.rs` | data | names kept; root re-exported. The zero-external-use rows (`heretic`, `hexen`, `strife`, `exe_doom_1_2`, `exe_heretic_1_3`, `exe_hexen_1_1`, `exe_strife_1_2`, `exe_strife_1_31`) are kept regardless -- ID24/MBF21 work consumes the Heretic-family rows |
//! | (data) `ValidMode` / `VALID_MODES`, `ValidVersion` / `VALID_VERSIONS` | `tables.rs` | data | private tables; scope widened file-private -> `pub(super)` by the split (upstream file-statics; m_argv `DIR_SEPARATOR` precedent) |
//! | (data) `skill_t` | -- (never lived here) | data | its `d_mode.h` row is ported elsewhere; noted pre-move (`d_mode.rs:29-30`) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave C1, per function): all six
//! functions run at boot/menu time -- membership, counting, and naming
//! queries over static tables -- none executes per-tic, none consumes the
//! PRNG, and none touches the demo stream, so every verdict is `glue`.
//! The module's real link to determinism is its DATA: the constants type
//! the `#[no_mangle]` doomstat globals `gamemode` / `gamemission` /
//! `gameversion` (doomstat.rs:31/:40/:49), which ARE demo-observable
//! session inputs (`gameversion` gates version-specific compat behavior
//! per tic elsewhere; `gamemode` gates episode/map logic everywhere) --
//! d_mode supplies the vocabulary, not the state, and exactness concerns
//! do not reach the queries themselves. Accordingly d_mode has no `dtmc`
//! submodule and no extraction candidates exist. The 23-test known-vector
//! module is the F10 §2.3 baseline (written against the pre-move bodies);
//! it moved with `validate`/`names` unchanged -- same vectors, same
//! results.

pub mod consts;
pub mod names;
pub mod tables;
pub mod validate;

//* path-stability re-export: all 29 enum constants keep their module-root
//* paths for the freeze-zone callers (`d_main.rs`, `doomstat.rs`,
//* `f_finale.rs`, `m_menu.rs`, `st_stuff.rs`, `wi_stuff.rs`, `hu_stuff.rs`,
//* `g_game`, ... ~163 references).
pub use consts::{
    commercial, doom, doom2, exe_chex, exe_doom_1_2, exe_doom_1_666, exe_doom_1_7, exe_doom_1_8,
    exe_doom_1_9, exe_final, exe_final2, exe_hacx, exe_heretic_1_3, exe_hexen_1_1,
    exe_strife_1_2, exe_strife_1_31, exe_ultimate, heretic, hexen, indetermined, none, pack_chex,
    pack_hacx, pack_plut, pack_tnt, registered, retail, shareware, strife,
};

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use names::game_mission_string as D_GameMissionString;
pub use validate::{
    is_episode_map as D_IsEpisodeMap, num_episodes as D_GetNumEpisodes,
    valid_episode_map as D_ValidEpisodeMap, valid_game_mode as D_ValidGameMode,
    valid_game_version as D_ValidGameVersion,
};
