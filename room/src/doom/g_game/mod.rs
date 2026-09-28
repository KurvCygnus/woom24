//! Rust port of `vendor/doomgeneric/g_game.c` -- the gameplay controller:
//! the central dispatcher between input, demo recording/playback, map
//! loading, player command building, and game-tick advancement. It owns
//! the deferred-action state machine ([`gameaction`]) that `G_Ticker`
//! drains every tic, the `gamestate` enumeration (`GS_LEVEL`,
//! `GS_INTERMISSION`, `GS_FINALE`, `GS_DEMOSCREEN`), and the per-player
//! slot tables consumed throughout the engine.
//!
//! Graduated from the freeze zone in F10 wave C3 (report:
//! `.superpowers/sdd/2026-09-26-f10-rolling/c3-ggame-report.md`); the
//! bodies moved verbatim into the subfiles below -- no "obvious
//! cleanups", this file's tic paths are exactly where the F9 goldens
//! bite (`tests/demo_playthrough.rs` checkpoints 35..=5000, with the
//! dense 2000-3500 window historically attributed to this module).
//!
//! ## Submodule Responsibility
//!
//! - `consts.rs` -- the upstream-named private constants (`GS_*`, `ga_*`,
//!   `PST_*`, `wp_*`, `pw_strength`, `BT_*`, `BTS_*`, `NUMKEYS`,
//!   `MAX_MOUSE_BUTTONS`, `MAX_JOY_BUTTONS`, `SAVEGAMESIZE`, `DEMOMARKER`,
//!   `VERSIONSIZE`, `am_clip`, `MT_TFOG`, `DEH_INITIAL_*`) and the vanilla
//!   type aliases (`boolean` / `byte` / `skill_t` / `fixed_t` / `c_long`)
//! - `state.rs` -- ALL 57 `#[no_mangle] pub static mut` engine-visible
//!   globals (one auditable home for the extern-by-symbol surface) plus
//!   the cross-file `SAVEGAMESLOT` / `SAVEDESCRIPTION` latches
//! - `ticcmd.rs` -- `build_ticcmd` (`G_BuildTiccmd`), the input-latch
//!   statics, the weapon-cycling table and its helpers
//! - `responder.rs` -- `responder` (`G_Responder`) and the `deh_string`
//!   identity seam shared by `actions` / `ticker` / `savegentry`
//! - `ticker.rs` -- `ticker` (`G_Ticker`) and the turbo-message buffer
//! - `actions.rs` -- the GA_* machine: `do_load_level`,
//!   `defered_init_new`, `do_new_game`, `init_new`,
//!   `set_fast_monsters` (+ the `set_fast_monsters_bool` wrapper),
//!   `do_completed`, `world_done`, `do_world_done`, `exit_level`,
//!   `secret_exit_level`, `screen_shot`
//! - `demo.rs` -- demo record/playback/benchmark:
//!   `record_demo`, `begin_recording`, `read_demo_ticcmd`,
//!   `write_demo_ticcmd`, `defered_play_demo`, `do_play_demo`,
//!   `time_demo`, `check_demo_status`, `vanilla_version_code`, the
//!   `increase_demo_buffer` / `demo_version_description` helpers and the
//!   verbatim `I_Quit` extern declaration
//! - `savegentry.rs` -- `load_game`, `do_load_game`, `save_game`,
//!   `do_save_game`
//! - `spawn.rs` -- `init_player`, `player_finish_level`,
//!   `player_reborn`, `check_spot`, `deathmatch_spawn_player`,
//!   `do_reborn`
//! - `dtmc.rs` -- the extracted demo-synchronization pure cores
//!   (below), plus their baseline-vector test suite
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the upstream-name shims, and the path-stability
//! re-exports below. Two invisible consumer classes drove the wiring:
//! the 27 path consumers (`use crate::doom::g_game::{...}`, report SS2.1)
//! -- including `shells/web/src/lib.rs:127`/`:322`, which reads
//! `room::doom::g_game::gamestate` BY FULL PATH -- and the extern-by-symbol
//! declarer blocks (report SS2.2): `d_player/mod.rs:104-113`
//! (`players`, `consoleplayer`), `hu_stuff.rs:391-411` (`playeringame`,
//! `consoleplayer`), `d_net.rs:272` + `d_net.rs:186-199` (`playeringame`;
//! extern fns `G_Ticker`, `G_BuildTiccmd`), `p_saveg/mod.rs:274`
//! (`playeringame`), and `c_ffi.rs:507-539` (12 statics feeding
//! `c_tests/g_game_c.rs`). `d_net.rs:509` also binds
//! `BuildTiccmd: Some(G_BuildTiccmd)` -- the only address-taken g_game
//! symbol; the `#[export_name]` pin keeps that wire alive.
//!
//! Cross-module contracts documented once here:
//! - **No C-side linkage**: `doomgeneric-sys/build.rs:70` excludes
//!   `g_game.c` from the C build (ported to Rust), so no C translation
//!   unit links any of these symbols; the live surface is Rust-side
//!   extern blocks plus the wasm export set.
//! - **Anchor absence**: `doomgeneric.rs`'s anchor list has NO g_game
//!   entry (pre-move or now) -- the absence is preserved; no anchors
//!   were added.
//! - The mutual module cycles with the 25+ sibling modules g_game
//!   imports resolve through module-root re-exports on both sides
//!   (p_map convention); subfiles import the shared vocabulary via
//!   `super::`.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! functions only -- plain-English internal names with
//! `#[doc(alias = "OriginalName")]`, upstream-name shims at this root,
//! and `#[export_name]` re-pins exactly where the pre-move surface had
//! `#[no_mangle]`, so the exported symbol set is byte-identical. The
//! load-bearing pins are `G_Ticker` and `G_BuildTiccmd` (extern-linked
//! and fn-pointer-wired from d_net/d_loop); the other 32 pins exist to
//! keep the wasm export surface unchanged. Statics and consts keep
//! their upstream names (data tier; `#[no_mangle]` retained on all 57).
//! Signatures keep `unsafe` where the pre-move functions were unsafe.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `G_CmdChecksum` | `dtmc::ticcmd_checksum` | dtmc (extracted pure) | wrapping word sum over `sizeof(ticcmd_t)/4 - 1` words; shim + pin; upstream `g_game.c` |
//! | `G_BuildTiccmd` | `ticcmd::build_ticcmd` | dtmc (whole-body) | the bytes it produces ARE the recorded stream; the `lowres_turn` rounding (extracted to `dtmc::lowres_turn_round`) and the `consistancy` byte are on-stream; shim + pin -- LOAD-BEARING (extern-declared by `d_net`/`d_loop`, fn-pointer `Some(G_BuildTiccmd)` at `d_net.rs:509`) |
//! | `-- (new extraction; no named C counterpart)` | `dtmc::lowres_turn_round` | dtmc (extracted) | pure 256-BAM rounding step `desired = a.wrapping_add(carry); rounded = (desired as i32 + 128) & 0xff00; carry = desired - rounded`; baseline vectors written/run against the inline body pre-move (commit `1c92fdd`), re-pointed post-move |
//! | `G_Responder` | `responder::responder` | dtmc (whole-body) | input latches feed `build_ticcmd`; demo-window menu-popup and spy-mode arms are demo-session sequencing; shim + pin |
//! | `G_Ticker` | `ticker::ticker` | dtmc (whole-body) | drains `gameaction`; routes netcmds; demo read/write hooks; consistency ring -- reads `rndindex` (NOT `prndindex`), `g_game.c:950-957` analog; shim + pin -- LOAD-BEARING (`d_net` extern, called from `run_tic` every tic) |
//! | `G_DoLoadLevel` | `actions::do_load_level` | dtmc (whole-body) | resets the input latches (cross-file writes into `ticcmd`'s `pub(super)` statics); shim + pin |
//! | `G_DoNewGame` | `actions::do_new_game` | dtmc (whole-body) | clears net/demo state then `init_new`; shim + pin |
//! | `G_DeferedInitNew` | `actions::defered_init_new` | dtmc (whole-body) | latches `d_skill`/`d_episode`/`d_map`; every menu new-game start; shim + pin |
//! | `G_InitNew` | `actions::init_new` | dtmc (whole-body) | `M_ClearRandom` position relative to the `do_load_level` tail is hash-load-bearing (`save_load_roundtrip`, `shells/web/src/harness.rs:56`); shim + pin |
//! | `G_SetFastMonsters` | `actions::set_fast_monsters` | dtmc (whole-body) | patches the `states`/`mobjinfo` tables globally; shim + pin |
//! | `-- (private wrapper)` | `actions::set_fast_monsters_bool` | glue | bool-to-`c_int` marshalling for the two `init_new` call sites; alias `set_fast_monsters` |
//! | `G_DoCompleted` | `actions::do_completed` | dtmc (whole-body; pure cores extracted) | seeds the hashed intermission fields; the ep-4 `cpars[gamemap]` overrun arm (#7) stays at the call site reading the live `pars`/`cpars` statics; the double `gamemap == 8/9` checks after the Chex arm are replicated verbatim, not deduped; shim + pin |
//! | `G_WorldDone` | `actions::world_done` | dtmc (whole-body) | finale kick-off per cluster map; shim + pin |
//! | `G_DoWorldDone` | `actions::do_world_done` | dtmc (whole-body) | `gamemap = wminfo.next + 1` then `do_load_level`; shim + pin |
//! | `G_ExitLevel` | `actions::exit_level` | dtmc (whole-body) | sim-to-`gameaction` edge fired from p_switch/p_spec/p_enemy mid-demo; shim + pin |
//! | `G_SecretExitLevel` | `actions::secret_exit_level` | dtmc (whole-body) | same edge, with the map31-presence gate; shim + pin |
//! | `G_ScreenShot` | `actions::screen_shot` | glue | render-side user action; nothing on the stream; shim + pin |
//! | `G_ReadDemoTiccmd` | `demo::read_demo_ticcmd` | dtmc (whole-body; pure core extracted) | 4/5-byte layout; cursor math pinned by `dtmc::read_demo_ticcmd_bytes` vectors; shim + pin |
//! | `G_WriteDemoTiccmd` | `demo::write_demo_ticcmd` | dtmc (whole-body) | write + rewind + read-back round-trip; the `demoend - 16` guard arms; shim + pin |
//! | `G_RecordDemo` | `demo::record_demo` | dtmc (whole-body) | `-maxdemo` parse + buffer alloc; shim + pin |
//! | `G_VanillaVersionCode` | `demo::vanilla_version_code` | dtmc (wrapper over pure core) | thin wrapper over `dtmc::vanilla_version_code_for`; shim + pin |
//! | `-- (private table)` | `dtmc::vanilla_version_code_for` | dtmc (extracted pure) | version-to-demo-code table; `exe_doom_1_2` arm is the `I_Error` divergence |
//! | `G_BeginRecording` | `demo::begin_recording` | dtmc (whole-body) | 13-byte demo header; `-longtics` flips `lowres_turn`; shim + pin |
//! | `G_DeferedPlayDemo` | `demo::defered_play_demo` | dtmc (whole-body) | latches `defdemoname`; shim + pin |
//! | `G_DoPlayDemo` | `demo::do_play_demo` | dtmc (whole-body) | header parse, version warning (printf, not I_Error), `precache` dance; shim + pin |
//! | `G_TimeDemo` | `demo::time_demo` | dtmc (whole-body) | `singletics` benchmark entry; shim + pin |
//! | `G_CheckDemoStatus` | `demo::check_demo_status` | dtmc (whole-body) | timedemo fps / playback release / recording flush trichotomy; shim + pin |
//! | `-- (private)` | `demo::increase_demo_buffer` | glue | allocation rebase; reachable only with `vanilla_demo_limit == 0`; alias `IncreaseDemoBuffer` |
//! | `-- (private)` | `demo::demo_version_description` | glue | printf formatting into `DEMOVERSIONBUF` |
//! | `G_LoadGame` | `savegentry::load_game` | glue | defers `ga_loadgame`; shim + pin |
//! | `G_DoLoadGame` | `savegentry::do_load_game` | dtmc (whole-body) | re-enters `init_new` (RNG reset) and unarchives the hashed state; `leveltime` save/restore order is load-bearing; shim + pin |
//! | `G_SaveGame` | `savegentry::save_game` | glue | latches slot + description, sets `sendsave`; shim + pin |
//! | `G_DoSaveGame` | `savegentry::do_save_game` | glue | write-only side-channel; the fopen(temp) -> write -> fclose -> remove/rename sequence is pinned by the web VFS tests; shim + pin |
//! | `G_InitPlayer` | `spawn::init_player` | glue | parity wrapper over `player_reborn`; uncalled in-tree -- kept + pinned (symbol-surface rule) |
//! | `G_PlayerFinishLevel` | `spawn::player_finish_level` | dtmc (whole-body) | strips exactly the hashed fields; shim + pin |
//! | `G_PlayerReborn` | `spawn::player_reborn` | dtmc (whole-body) | seeds exactly the hashed `players[]` fields; shim + pin |
//! | `G_CheckSpot` | `spawn::check_spot` | dtmc (whole-body) | teleport-fog emulation site (#6); bounded `bodyque` corpse flush; shim + pin |
//! | `-- (private core)` | `dtmc::teleport_fog_offset` | dtmc (extracted pure) | the chocolate case table; `None` is the caller's `I_Error` arm; vectors in `dtmc` |
//! | `G_DeathMatchSpawnPlayer` | `spawn::deathmatch_spawn_player` | dtmc (whole-body) | `P_Random() % selections` draw at the loop head -- draw count and order are the demo surface; shim + pin |
//! | `G_DoReborn` | `spawn::do_reborn` | dtmc (whole-body) | single-player defers `ga_loadlevel`; netgame spawn ladder; shim + pin |
//! | (private) `DEH_String` (stub) | `responder::deh_string` | glue | identity seam; shared by `actions` / `ticker` / `savegentry`; alias `DEH_String` |
//! | (private) `logical_gamemission` (macro mirror) | `ticcmd::logical_gamemission` | glue | TNT/Plutonia collapse onto `doom2` |
//! | (private) `mousebutton` / `joybutton` | `ticcmd::mouse_button` / `ticcmd::joy_button` | glue | `-1`-sentinel negative-index model (docs/vanilla-workarounds.md "not a bug emulation"); alias `mousebutton` / `joybutton` |
//! | (private) `WeaponSelectable` / `G_NextWeapon` | `ticcmd::weapon_selectable` / `ticcmd::next_weapon_slot` | dtmc | decide the `BT_CHANGE` payload bytes; alias only (private pre-move) |
//! | (private) `SetJoyButtons` / `SetMouseButtons` | `ticcmd::set_joy_buttons` / `ticcmd::set_mouse_buttons` | glue | input marshalling; alias only (private pre-move) |
//! | (private) `g_vanilla_version_code_for` | `dtmc::vanilla_version_code_for` | dtmc (extracted pure) | see `G_VanillaVersionCode` row |
//! | (private) `g_check_spot_fog_offset` | `dtmc::teleport_fog_offset` | dtmc (extracted pure) | see `G_CheckSpot` row |
//! | (private) `commercial_partime` / `gammalvl0_prefix_i32` | `dtmc::commercial_partime` / `dtmc::gammalvl0_prefix_i32` | dtmc (extracted pure) | the map33 GAMMALVL0 model (#9); vectors in `dtmc` |
//! | (private) `read_demo_ticcmd_inner` | `dtmc::read_demo_ticcmd_bytes` | dtmc (extracted pure) | parameterised cursor math; feeds the dtmc test suite |
//! | (data) 57 `#[no_mangle]` statics (`oldgamestate` ... `bodyque`) | `state` | data | names + `#[no_mangle]` kept verbatim (statics ruling); the extern-by-symbol surface lives here; root re-exports below hold every consumer path |
//! | (data) `SAVEGAMESLOT` / `SAVEDESCRIPTION` (private) | `state` (`pub(super)`) | data | written by `save_game`, read by `build_ticcmd` / `ticker` |
//! | (data) `WEAPON_ORDER_TABLE` + `WeaponOrder` | `ticcmd` | data | upstream names kept |
//! | (data) input latches (`GAMEKEYDOWN` ... `NEXT_WEAPON`) | `ticcmd` (`pub(super)`) | data | multi-writer set: `build_ticcmd`, `responder`, `do_load_level`, `write_demo_ticcmd` |
//! | (data) `TURNHELD`, `DCLICK*`, `LOWRES_TURN_CARRY` | `ticcmd` (private) | data | single-function latches |
//! | (data) `TURBOMESSAGE` / `DEMOVERSIONBUF` | `ticker` / `demo` (private) | data | single-function scratch buffers |
//! | (data) `GS_*` / `ga_*` / `PST_*` / `wp_*` / `BT_*` / `BTS_*` / sizing consts / type aliases | `consts` (`pub(super)`) | data | upstream names kept |
//! | `doomgeneric.rs` anchor list | -- | wiring | has no g_game entry (pre-move or now); per the wave ruling no anchors were added -- the `#[export_name]` pins plus the untouched `#[no_mangle]` statics keep the symbol set byte-identical |
//!
//! ## Deterministic Aspects
//!
//! `g_game` is the WRITER of the demo stream, so most of the module is
//! demo-synchronization surface (31 whole-body verdicts, 19 glue, 7+1
//! extracted pure cores; per-function reasoning in the table above).
//! Load-bearing pieces:
//!
//! - **The F9 goldens drive this file.** `tests/demo_playthrough.rs`
//!   checkpoints 35..=5000 with the dense 2000-3500 window attributed to
//!   these tic paths; `harness_hash`'s 72-byte ledger digests `gamestate`
//!   (word 1) and `players[0]` (words 4-13), all written here. Statement
//!   reordering inside `ticker`, `build_ticcmd`, `read/write_demo_ticcmd`,
//!   `do_play_demo` or `init_new` moves hashes.
//! - **RNG ledger**: `init_new`'s `M_ClearRandom` position is
//!   hash-load-bearing (`save_load_roundtrip`); `deathmatch_spawn_player`
//!   draws from `P_Random` once per attempt (draw count and order are the
//!   surface -- no helper may absorb or add a draw). The consistency ring
//!   seeds from `rndindex` when a player has no mobj (`rndindex`, NOT
//!   `prndindex` -- brief correction, matches `g_game.c:950-957`).
//! - **Demo byte layout**: 4 bytes per tic vanilla (angleturn byte read
//!   UNSIGNED, shifted left 8), 5 bytes longtics (little-endian pair,
//!   bytes unsigned); `write_demo_ticcmd` rewinds and re-reads the
//!   record it just wrote. `lowres_turn` quantization accumulates the
//!   residual via `dtmc::lowres_turn_round` (baseline-vector pinned).
//! - **Load-bearing formatting quirks**: the nested
//!   `M_snprintf_clamp(..., c_snprintf1(...))` turbo-message construction
//!   in `ticker` looks wrong and is parity-correct -- never "fix" it.
//!   The double `gamemap == 8/9` checks in `do_completed` (after the
//!   Chex arm) are replicated, not deduped.
//! - **File-I/O order**: `do_save_game`'s fopen(temp) -> write -> fclose
//!   -> remove/rename sequence is pinned by `shells/web/src/wasm_vfs.rs`;
//!   `do_load_game` preserves `leveltime` across the `init_new` call.
//! - **Extern-by-symbol fidelity**: the six declarer blocks / 15 symbols
//!   (SS2.2) link the pinned C symbols and the `#[no_mangle]` statics by
//!   name; `d_net`'s `Some(G_BuildTiccmd)` resolves through the pin.
//!   Never convert those extern blocks to Rust-path imports while the
//!   freeze zone exists (d_player/mod.rs documents the declarer record).

pub mod actions;
pub mod consts;
pub mod demo;
pub mod dtmc;
pub mod responder;
pub mod savegentry;
pub mod spawn;
pub mod state;
pub mod ticcmd;
pub mod ticker;

//* upstream-name shims: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::g_game::G_*`). The C symbol each shim
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name set
//* stays byte-identical to the pre-split module. Shims die with the
//* freeze zone.
pub use actions::{
    defered_init_new as G_DeferedInitNew, do_completed as G_DoCompleted,
    do_load_level as G_DoLoadLevel, do_new_game as G_DoNewGame, do_world_done as G_DoWorldDone,
    exit_level as G_ExitLevel, init_new as G_InitNew, screen_shot as G_ScreenShot,
    secret_exit_level as G_SecretExitLevel, set_fast_monsters as G_SetFastMonsters,
    world_done as G_WorldDone,
};
pub use demo::{
    begin_recording as G_BeginRecording, check_demo_status as G_CheckDemoStatus,
    defered_play_demo as G_DeferedPlayDemo, do_play_demo as G_DoPlayDemo, record_demo as G_RecordDemo,
    read_demo_ticcmd as G_ReadDemoTiccmd, time_demo as G_TimeDemo, vanilla_version_code as G_VanillaVersionCode,
    write_demo_ticcmd as G_WriteDemoTiccmd,
};
pub use dtmc::ticcmd_checksum as G_CmdChecksum;
pub use responder::responder as G_Responder;
pub use savegentry::{
    do_load_game as G_DoLoadGame, do_save_game as G_DoSaveGame, load_game as G_LoadGame,
    save_game as G_SaveGame,
};
pub use spawn::{
    check_spot as G_CheckSpot, deathmatch_spawn_player as G_DeathMatchSpawnPlayer,
    do_reborn as G_DoReborn, init_player as G_InitPlayer,
    player_finish_level as G_PlayerFinishLevel, player_reborn as G_PlayerReborn,
};
pub use ticcmd::build_ticcmd as G_BuildTiccmd;
pub use ticker::ticker as G_Ticker;

//* path-stability re-export: ALL 57 engine-visible statics keep their
//* module-root paths (`crate::doom::g_game::<name>`) -- the 27 path
//* consumers (report SS2.1), the full-path web-shell reads
//* (`shells/web/src/lib.rs:127`/`:322`) and every F9 harness import
//* resolve through this block.
pub use state::{
    bodyque, bodyqueslot, consoleplayer, consistancy, cpars, d_episode, d_map, d_skill,
    deathmatch, defdemoname, demobuffer, demoname, demoend, demo_p, demoplayback, demorecording,
    displayplayer, gameaction, gameepisode, gamemap, gameskill, gamestate, levelstarttic,
    longtics, lowres_turn, netdemo, netgame, nodrawers, oldgamestate, paused, playeringame,
    players, precache, respawnmonsters, savename, secretexit, singledemo, starttime, testcontrols,
    testcontrols_mousespeed, timelimit, timingdemo, totalkills, totalsecret, totalitems,
    turbodetected, usergame, vanilla_demo_limit, vanilla_savegame_limit, viewactive, wminfo,
    angleturn, forwardmove, pars, sendpause, sendsave, sidemove,
};
