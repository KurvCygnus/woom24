//! Rust port of `vendor/doomgeneric/d_main.c` -- boot, the doomgeneric
//! frame entries (the wasm-linked surface), the display pipeline, the
//! attract-loop sequencer and game identification.
//!
//! Graduated from the freeze zone in F10 wave C4 (investigation report:
//! `.superpowers/sdd/2026-09-26-f10-rolling/c4-dmain-report.md`). The
//! boot-init order and the wipe-loop fraction-refresh gate are
//! hash/golden-bearing and moved VERBATIM; the one sanctioned body
//! change is the `dtmc::attract_sequence_count` extraction (F10 §2).
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the upstream-named data tier: string constants
//!   (`D_DEVSTR`, `HUSTR_KEY*`), the vanilla type aliases
//!   (`gamestate_t` / `gameaction_t` / `skill_t` / `byte`), the `GS_*` /
//!   `ga_*` / `sk_*` discriminants, the `banners` /
//!   `COPYRIGHT_BANNERS` / `IWAD_CHECK_NAMES` tables, the
//!   `-gameversion` and `-pack` lookup tables (`GAME_VERSIONS` /
//!   `PACKS`) with their `SyncPtr`/`sp`/`GameVersionDesc`/`PackDesc`
//!   plumbing, and the table/default tests
//! - `state.rs` -- ALL 22 `#[no_mangle] pub static mut` engine-visible
//!   globals (one auditable home for the extern-by-symbol surface) plus
//!   the test-only `title` buffer and the defaults/size tests
//! - `boot.rs` -- `doom_main`, `doom_loop`, `add_file`,
//!   `print_dehacked_banners`, `endoom`
//! - `identify.rs` -- `identify_version`, `logical_gamemission`,
//!   `init_game_version`, `set_game_description`, `get_game_name`,
//!   `set_mission_for_pack_name`, `print_game_version`
//! - `display.rs` -- `display` (including the wipe loop and the
//!   `D_DISP_*` frame-diff statics) and `grab_mouse_callback`
//! - `entries.rs` -- `tick_entry`, `frame_entry`, `MAX_TICS_PER_FRAME`
//!   (the wasm-symbol-critical frame entries)
//! - `events.rs` -- `process_events`
//! - `sequencing.rs` -- `page_ticker`, `page_drawer`, `advance_demo`,
//!   `do_advance_demo`, `start_title`
//! - `bind.rs` -- `bind_variables`
//! - `compat.rs` -- `deh_string` (identity seam), `ends_with_ci`,
//!   `eq_ci`, `ne_ci_n`, `to_lossy_string`
//! - `dtmc.rs` -- the extracted demo-synchronization pure core
//!   (`attract_sequence_count`) and its baseline vectors
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the upstream-name shims, and the path-stability
//! re-exports below. Three invisible consumer classes drove the wiring:
//!
//! 1. the freeze-zone path consumers (`use crate::doom::d_main::{...}`,
//!    report §2.3): `g_game/demo.rs` (`D_AdvanceDemo` + 3 statics),
//!    `g_game/actions.rs` (4 statics), `g_game/ticker.rs`
//!    (`D_PageTicker`), `f_finale.rs` (`wipegamestate`), `m_menu.rs`
//!    (`devparm`), `p_enemy/chase.rs` / `p_mobj/mapthings.rs` (2
//!    statics), `c_ffi.rs:745-748` (14 statics feeding
//!    `c_tests/d_main_c.rs`) and the F9 harnesses
//!    (`tests/frame_split_common/mod.rs:302`/`:320`/`:327` drives
//!    `doomgeneric_frame` + `MAX_TICS_PER_FRAME`;
//!    `tests/scenario_harness/mod.rs:1258`/`:1335` drives
//!    `doomgeneric_Tick` + `doomgeneric_frame`;
//!    `tests/sprite_apply_regression.rs` / `sprite_interp_probe.rs`
//!    drive `D_Display`; `shells/web/src/lib.rs:80`/`:93`/`:226`/`:234`
//!    and `shells/native/src/main.rs:251`);
//! 2. the extern-by-symbol declarer blocks (report §2.2, never
//!    converted to Rust paths while the freeze zone exists):
//!    `d_net/mod.rs:165` (`D_DoAdvanceDemo`) + `:168`
//!    (`D_ProcessEvents`), `d_net/loop_table.rs:98`/`:110`
//!    (call + fn-pointer), `d_net/mod.rs:196-235` (8 startup statics
//!    forwarded to `net_glue.rs`), `d_net/loop_table.rs:90-96`
//!    (`advancedemo`), `d_loop/mod.rs:327` (`D_ProcessEvents`) and
//!    `p_saveg/mod.rs:262` (`savegamedir`);
//! 3. the engine boot link: `doomgeneric.rs:189` extern-declares
//!    `D_DoomMain` and calls it at `:285` (the wasm shell's
//!    `init_pipeline.rs:137`, the native shell `main.rs:239` and
//!    `tests/scenario_harness/mod.rs:1493` all converge there);
//!    `doomgeneric-sys/src/lib.rs:86` extern-declares
//!    `doomgeneric_Tick` (the demo-golden tic driver).
//!
//! Cross-module contracts documented once here:
//! - **No C translation unit links these symbols directly**:
//!   `doomgeneric-sys/build.rs` compiles only `test_helpers.c` (every
//!   engine `.c` is commented out, incl. `d_main.c`); the vendored C
//!   `vendor/doomgeneric/doomgeneric.c:25` names `D_DoomMain` but is
//!   not compiled -- the live surface is the Rust extern blocks, the
//!   wasm export set, and the `doomgeneric-sys` extern resolutions.
//! - **Anchor absence**: `doomgeneric.rs`'s anchor list has NO d_main
//!   entry (pre-move or now) -- the absence is preserved; no anchors
//!   were added.
//! - The mutual module cycles with the 30+ sibling modules d_main
//!   imports resolve through module-root re-exports on both sides
//!   (p_map convention); subfiles import shared vocabulary and
//!   cross-subfile callers via `super::` alias imports so moved bodies
//!   kept their upstream call names.

//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! functions only -- plain-English internal names with
//! `#[doc(alias = "OriginalName")]`, upstream-name shims at this root,
//! and `#[export_name]` re-pins exactly where the pre-move surface had
//! `#[no_mangle]`, so the exported symbol set is byte-identical. The
//! load-bearing pins are `D_DoomMain` (boot link from
//! `doomgeneric.rs`), `doomgeneric_Tick`/`doomgeneric_frame` (shell +
//! harness frame entries), `D_ProcessEvents`/`D_DoAdvanceDemo`
//! (extern-declared by `d_loop`/`d_net`, fn-pointer-wired in
//! `DOOM_LOOP_INTERFACE`); the other pins exist to keep the wasm export
//! surface unchanged. Statics and consts keep their upstream names
//! (data tier; `#[no_mangle]` retained on all 22). Signatures keep
//! `unsafe` where the pre-move functions were unsafe.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `D_ProcessEvents` | `events::process_events` | glue | event-queue transport; the deterministic part lives downstream in `G_Responder`; shim + pin `D_ProcessEvents` -- LOAD-BEARING (extern-declared `d_loop/mod.rs:327` and `d_net/mod.rs:168`; fn-pointer `Some(D_ProcessEvents)` at `d_net/loop_table.rs:110`) |
//! | `D_Display` | `display::display` | glue | render-loop glue; reads sim state, writes only render statics (`D_DISP_*`, `wipegamestate`); the wipe loop's `I_GetTime`/`I_Sleep`/`refresh_fraction` ordering and `board_active` gate are golden-adjacent and moved verbatim; shim + pin |
//! | `D_BindVariables` | `bind::bind_variables` | glue | cvar/control/chat-macro binding, one-shot boot glue; shim + pin |
//! | `doomgeneric_Tick` | `entries::tick_entry` | glue | legacy always-tic entry behind the DG_CREATED latch; shim + pin `doomgeneric_Tick` -- LOAD-BEARING (extern `doomgeneric-sys/src/lib.rs:86`; wasm shell `shells/web/src/lib.rs:80`; F9 demo harnesses) |
//! | `doomgeneric_frame` | `entries::frame_entry` | glue | F1 M1 frame/pump split -- woom24 addition, no upstream C counterpart; shim + pin `doomgeneric_frame` -- LOAD-BEARING (`shells/web/src/lib.rs:93`, `shells/native/src/main.rs:251`, F9 frame harnesses) |
//! | `D_DoomLoop` | `boot::doom_loop` | glue | arms the loop latches then runs one boot tic and RETURNS on this port -- the stale "never returns" upstream doc is kept verbatim, not "fixed"; shim + pin |
//! | `D_PageTicker` | `sequencing::page_ticker` | dtmc (whole-body) | decrements `pagetic` and fires `D_AdvanceDemo` at exactly tic 0 of the page budget -- WHEN pages rotate decides the state at every demo-golden checkpoint; path-consumed `g_game/ticker.rs:19`; shim + pin |
//! | `D_PageDrawer` | `sequencing::page_drawer` | glue | present-side page blit; shim + pin |
//! | `D_AdvanceDemo` | `sequencing::advance_demo` | dtmc (whole-body) | one-statement `advancedemo` latch polled by `d_net`'s `run_tic` every tic; path-consumed `g_game/demo.rs:20`; shim + pin |
//! | `D_DoAdvanceDemo` | `sequencing::do_advance_demo` | dtmc (whole-body; pure core extracted) | writer of the attract-loop state machine (`playerstate`/`advancedemo`/`usergame`/`paused`/`gameaction`/`gamestate`/`pagename`/`pagetic`/`demosequence` + `G_DeferedPlayDemo`/`S_StartMusic`); shim + pin `D_DoAdvanceDemo` -- LOAD-BEARING (extern-declared `d_net/mod.rs:165`, called `d_net/loop_table.rs:98`) |
//! | `-- (new extraction; no named C counterpart)` | `dtmc::attract_sequence_count` | dtmc (extracted pure) | the `max_seq` page count of `D_DoAdvanceDemo`: 7 for `exe_ultimate`/`exe_final`, else 6; baseline vectors written/run against the inline body pre-move (commit `d1d1493`), re-pointed post-move |
//! | `D_StartTitle` | `sequencing::start_title` | dtmc (whole-body) | `demosequence = -1` + advance -- pins the attract cycle's phase-0 start, hence the golden window's opening state; shim + pin |
//! | `D_IdentifyVersion` | `identify::identify_version` | glue | IWAD mission/mode identification; upstream *input* to the demo surface; shim + pin |
//! | `D_SetGameDescription` | `identify::set_game_description` | glue | description-string selection; shim + pin |
//! | `PrintGameVersion` | `identify::print_game_version` | glue | startup message; shim + pin |
//! | `D_DoomMain` | `boot::doom_main` | glue | one-shot boot orchestration; the init ORDER (Z_Init -> parm scan -> M_LoadDefaults -> D_FindIWAD -> W_AddFile -> identify -> subsystems -> `-playdemo`/`-timedemo` early returns) is hash-bearing and moved verbatim; shim + pin `D_DoomMain` -- LOAD-BEARING (extern-declared `doomgeneric.rs:189`, called `doomgeneric.rs:285`; the vendored C `vendor/doomgeneric/doomgeneric.c:25` names the same symbol but is not compiled) |
//! | (private) `D_GrabMouseCallback` | `display::grab_mouse_callback` | glue | address-taken via `doom_loop`'s inner `grab_cb` -> `I_SetGrabMouseCallback`; never linker-resolved -- no pin; alias only |
//! | (private) `D_Endoom` | `boot::endoom` | glue | `I_AtExit`-registered in `doom_main`; no pin; alias only |
//! | (private) `DEH_String` | `compat::deh_string` | glue | identity seam (`FEATURE_DEHACKED` undefined); alias `DEH_String` |
//! | (private) `c_str_ends_with` / `c_str_eq` / `c_str_ne_n` / `c_str_to_str` | `compat::ends_with_ci` / `compat::eq_ci` / `compat::ne_ci_n` / `compat::to_lossy_string` | glue | case-insensitive C-string helpers; alias only (private pre-move) |
//! | (private) `GetGameName` | `identify::get_game_name` | glue | DEH-banner name formatting into the zone heap; alias `GetGameName` |
//! | (private) `SetMissionForPackName` | `identify::set_mission_for_pack_name` | glue | `-pack` override with `i_error` edge; alias `SetMissionForPackName` |
//! | (private) `logical_gamemission` | `identify::logical_gamemission` | glue | `doomstat.h` macro mirror; KNOWN DUPLICATE of `g_game`'s mirror -- kept, cross-module dedupe is post-freeze-zone work |
//! | (private) `D_AddFile` | `boot::add_file` | glue | WAD attach + stdout line; alias `D_AddFile` |
//! | (private) `PrintDehackedBanners` | `boot::print_dehacked_banners` | glue | DEH copyright print; alias `PrintDehackedBanners` |
//! | (private) `InitGameVersion` | `identify::init_game_version` | glue | `-gameversion` parse + auto-detect + retail/Final clamps; alias `InitGameVersion` |
//! | (data) 22 `#[no_mangle]` statics (`savegamedir` .. `wipegamestate`) | `state` | data | names + `#[no_mangle]` kept verbatim (statics ruling); the extern-by-symbol surface lives here (declarers in the table's Notes column of the wiring list above); root re-exports hold every consumer path |
//! | (data) `wadfile` / `mapdir` | `state` | data | dead-but-exported (zero callers in the tree): kept for symbol-set byte-identity, retires with the freeze zone |
//! | (data) `title` | `state` (private) | data | kept for the buffer-size test only; retires with the freeze zone |
//! | (data) `D_DISP_VIEWACTIVE` / `D_DISP_MENUACTIVE` / `D_DISP_INHELPSCREENS` / `D_DISP_FULLSCREEN` / `D_DISP_OLD_GAMESTATE` / `D_DISP_BORDERDRAWCOUNT` | `display` (private) | data | `display`'s frame-diff statics; written only by `display` |
//! | (data) `D_DEVSTR` / `HUSTR_KEY*` / type aliases / `GS_*` / `ga_*` / `sk_*` / `banners` / `SyncPtr` + `sp` / `GAME_VERSIONS` + `GameVersionDesc` / `PACKS` + `PackDesc` / `IWAD_CHECK_NAMES` / `COPYRIGHT_BANNERS` | `consts` (`pub(super)`) | data | upstream names kept; static-initializer arrays stay Allman-expanded |
//! | (data) `MAX_TICS_PER_FRAME` | `entries` | data | woom24 shell policy const (name kept); root re-export keeps `room::doom::d_main::MAX_TICS_PER_FRAME` resolving (`tests/frame_split_common/mod.rs:327`) |
//! | `doomgeneric.rs` anchor list | -- | wiring | has no d_main entry (pre-move or now); per the wave ruling no anchors were added -- the `#[export_name]` pins plus the untouched `#[no_mangle]` statics keep the symbol set byte-identical |
//!
//! ## Deterministic Aspects
//!
//! `d_main` is mostly one-shot boot/display glue around a small,
//! sharp demo-synchronization surface: the attract-loop sequencer
//! (4 whole-body verdicts + 1 extraction). Load-bearing pieces:
//!
//! - **The F9 goldens drive the entries.** Every harness boots through
//!   `doom_main` (via `doomgeneric_Create`), then pumps
//!   `doomgeneric_Tick`/`doomgeneric_frame` and hashes the buffers
//!   `display` presents. The boot-init ORDER is hash-bearing: Z_Init ->
//!   parm scan -> `M_LoadDefaults` -> `D_FindIWAD` -> `W_AddFile` ->
//!   identify -> subsystem init -> the `-playdemo`/`-timedemo`
//!   early-return arms (which call `doom_loop` and `return`). Never
//!   reorder.
//! - **The attract sequencer** (dtmc): `D_PageTicker`'s page-budget
//!   countdown, `D_AdvanceDemo`'s per-tic latch edge,
//!   `D_DoAdvanceDemo`'s state-machine arms, and `D_StartTitle`'s
//!   phase-0 pin decide the `gamestate`/`pagename`/music phase at every
//!   `demo_playthrough` checkpoint (the golden window boots into
//!   `D_StartTitle`). The pure page count is
//!   `dtmc::attract_sequence_count` (baseline-vector pinned pre-move,
//!   commit `d1d1493`); the BFG `TITLEPIC`->`INTERPIC` fallback arm
//!   stays at the call site reading the live `pagename`/`bfgedition`.
//! - **The wipe-loop fraction-refresh gate** (`display`, F1 M1): the
//!   wipe loop re-samples `r_interp::refresh_fraction()` only under
//!   `r_interp::board_active()` -- the legacy `doomgeneric_Tick` path
//!   never arms the board and must not pay the extra `I_GetTimeMS`
//!   polls (c7bda6c baseline, review round 1). The loop's
//!   `I_GetTime`/`I_Sleep`/refresh ordering moved VERBATIM: reshaping
//!   it moves the video-anchor/raster frame hashes.
//! - **Exactly-once latches**: the frame entries no-op until
//!   `doomgeneric::dg_created()` (the browser BUG A pre-creation latch;
//!   workaround catalog rows cite these guards); `doom_loop` runs one
//!   boot `doomgeneric_Tick` and RETURNS on this port -- the upstream
//!   never-returns loop is deliberately NOT restored.
//! - **argv lifetime contract**: `doom_main` reads `myargv` everywhere;
//!   the three documented leak contracts (frame_split_common,
//!   scenario_harness, web init_pipeline) must keep pointing at
//!   process-lifetime storage (post-boot readers include `i_error`'s
//!   `-nogui` scan).
//! - **Extern-by-symbol fidelity**: the declarer blocks listed in the
//!   wiring notes above link the pinned C symbols and the
//!   `#[no_mangle]` statics by name. Never convert those extern blocks
//!   to Rust-path imports while the freeze zone exists.


pub mod bind;
pub mod boot;
pub mod compat;
pub mod consts;
pub mod display;
pub mod dtmc;
pub mod entries;
pub mod events;
pub mod identify;
pub mod sequencing;
pub mod state;

//* upstream-name shims: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::d_main::<UpstreamName>`). The C symbol each
//* shim forwards to is re-pinned at the definition with
//* `#[export_name = "..."]`, so the wasm/extern symbol name set stays
//* byte-identical to the pre-split module. Shims die with the freeze
//* zone. (Only the pre-move `pub` surface is shimmed at the root; the
//* pre-move-private helpers keep `#[doc(alias)]` at their definitions
//* and are reached cross-subfile via alias imports.)
pub use bind::bind_variables as D_BindVariables;
pub use boot::{doom_loop as D_DoomLoop, doom_main as D_DoomMain};
pub use display::display as D_Display;
pub use entries::{frame_entry as doomgeneric_frame, tick_entry as doomgeneric_Tick};
pub use events::process_events as D_ProcessEvents;
pub use identify::{
    identify_version as D_IdentifyVersion, print_game_version as PrintGameVersion,
    set_game_description as D_SetGameDescription,
};
pub use sequencing::{
    advance_demo as D_AdvanceDemo, do_advance_demo as D_DoAdvanceDemo, page_drawer as D_PageDrawer,
    page_ticker as D_PageTicker, start_title as D_StartTitle,
};

//* path-stability re-export: ALL 22 engine-visible statics keep their
//* module-root paths (`crate::doom::d_main::<name>`) -- the freeze-zone
//* path consumers (report §2.3), the `c_ffi.rs:745-748` re-export block
//* feeding `c_tests/d_main_c.rs`, and every F9 harness import resolve
//* through this block.
pub use state::{
    advancedemo, autostart, bfgedition, devparm, demosequence, fastparm, iwadfile,
    main_loop_started, mapdir, nomonsters, pagename, pagetic, respawnparm, savegamedir,
    show_endoom, startepisode, startloadgame, startmap, startskill, storedemo, wadfile,
    wipegamestate,
};

//* path-stability re-export: the frame/pump policy const keeps its
//* module-root path (`tests/frame_split_common/mod.rs:327` reads and
//* restores it).
pub use entries::MAX_TICS_PER_FRAME;
