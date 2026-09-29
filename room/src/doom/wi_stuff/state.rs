//! The intermission run-time state: the ~45 statics (upstream names)
//! shared by every subfile, reached via `super::state::`.

use std::ffi::c_int;
use std::ptr;

use super::types::{stateenum_t, wbplayerstruct_t, wbstartstruct_t};
use crate::doom::d_player::MAXPLAYERS;
use crate::doom::v_video::patch_t;

/// Non-zero when the player pressed fire/use to skip the current count-up animation.
pub(super) static mut acceleratestage: c_int = 0;
/// Console player index (0-based); set from `wbs.pnum` by `lifecycle::init_variables`.
pub(super) static mut me: c_int = 0;
/// Current intermission state machine state.
pub(super) static mut state: stateenum_t = stateenum_t::NoState;
/// Pointer to the level-start record filled by `G_WorldDone`; valid for the duration of the intermission.
pub(super) static mut wbs: *mut wbstartstruct_t = ptr::null_mut();
/// Pointer to `wbs.plyr[0]`; used to index per-player stats by offset.
pub(super) static mut plrs: *mut wbplayerstruct_t = ptr::null_mut();
/// Generic countdown used in `NoState` and `ShowNextLoc` states.
pub(super) static mut cnt: c_int = 0;
/// Background animation beat counter; incremented every tic by `lifecycle::ticker`.
pub(super) static mut bcnt: c_int = 0;
/// Non-zero on the first draw call after `WI_Start`, triggers a full background blit.
pub(super) static mut firstrefresh: c_int = 0;

/// Running kill-percentage display values (count up toward actual percentage).
pub(super) static mut cnt_kills: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];
/// Running item-percentage display values.
pub(super) static mut cnt_items: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];
/// Running secret-percentage display values.
pub(super) static mut cnt_secret: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];
/// Running displayed level time in seconds (counts up from 0).
pub(super) static mut cnt_time: c_int = 0;
/// Running displayed par time in seconds (counts up from 0).
pub(super) static mut cnt_par: c_int = 0;
/// Tic countdown used as a pause between successive stat reveals.
pub(super) static mut cnt_pause: c_int = 0;

/// Number of Doom II maps to load level-name patches for (32 for the retail release).
pub(super) static mut NUMCMAPS: c_int = 0;

/// "You Are Here" arrow patches (2 frames, `WIURH0`/`WIURH1`); third slot unused.
pub(super) static mut yah: [*mut patch_t; 3] = [ptr::null_mut(); 3];
/// Completed-level splat patches (`WISPLAT`); second slot unused.
pub(super) static mut splat: [*mut patch_t; 2] = [ptr::null_mut(); 2];
/// Percent sign patch (`WIPCNT`).
pub(super) static mut percent: *mut patch_t = ptr::null_mut();
/// Colon separator patch (`WICOLON`) for time display.
pub(super) static mut colon: *mut patch_t = ptr::null_mut();
/// Digit patches 0-9 (`WINUM0`-`WINUM9`).
pub(super) static mut num: [*mut patch_t; 10] = [ptr::null_mut(); 10];
/// Minus sign patch (`WIMINUS`) for negative frag counts.
pub(super) static mut wiminus: *mut patch_t = ptr::null_mut();
/// "Finished" label patch (`WIF`).
pub(super) static mut finished: *mut patch_t = ptr::null_mut();
/// "Entering" label patch (`WIENTER`).
pub(super) static mut entering: *mut patch_t = ptr::null_mut();
/// Single-player secret label patch (`WISCRT2`).
pub(super) static mut sp_secret: *mut patch_t = ptr::null_mut();
/// Kills column header patch (`WIOSTK`).
pub(super) static mut kills: *mut patch_t = ptr::null_mut();
/// Secrets column header patch (`WIOSTS`).
pub(super) static mut secret: *mut patch_t = ptr::null_mut();
/// Items column header patch (`WIOSTI` or `WIOBJ` in co-op).
pub(super) static mut items: *mut patch_t = ptr::null_mut();
/// Frags column header patch (`WIFRGS`).
pub(super) static mut frags: *mut patch_t = ptr::null_mut();
/// Time label patch (`WITIME`).
pub(super) static mut timepatch: *mut patch_t = ptr::null_mut();
/// Par-time label patch (`WIPAR`).
pub(super) static mut par: *mut patch_t = ptr::null_mut();
/// "Sucks" patch displayed when the level time exceeds the representable maximum (`WISUCKS`).
pub(super) static mut sucks: *mut patch_t = ptr::null_mut();
/// "Killers" row label patch (`WIKILRS`) used in deathmatch view.
pub(super) static mut killers: *mut patch_t = ptr::null_mut();
/// "Victims" column label patch (`WIVCTMS`) used in deathmatch view.
pub(super) static mut victims: *mut patch_t = ptr::null_mut();
/// "Total" column header patch (`WIMSTT`) used in deathmatch view.
pub(super) static mut total: *mut patch_t = ptr::null_mut();
/// "You are here" star patch (`STFST01`) marking the local player in netgame view.
pub(super) static mut star: *mut patch_t = ptr::null_mut();
/// Dead-face patch (`STFDEAD0`) marking the local player when dead.
pub(super) static mut bstar: *mut patch_t = ptr::null_mut();
/// Player face patches for each slot (`STPB0`-`STPB3`).
pub(super) static mut p: [*mut patch_t; MAXPLAYERS] = [ptr::null_mut(); MAXPLAYERS];
/// Alternative player face patches for each slot (`WIBP1`-`WIBP4`).
pub(super) static mut bp: [*mut patch_t; MAXPLAYERS] = [ptr::null_mut(); MAXPLAYERS];
/// Heap-allocated array of level-name patches (`WILV##` or `CWILV##`); length is `NUMCMAPS` or `NUMMAPS`.
pub(super) static mut lnames: *mut *mut patch_t = ptr::null_mut();
/// Background map graphic for the current episode (`WIMAP#` or `INTERPIC`).
pub(super) static mut background: *mut patch_t = ptr::null_mut();

/// Deathmatch count-up sub-state index (odd = pause, even = ticking).
pub(super) static mut dm_state: c_int = 0;
/// Running frag-count display for each `[killer][victim]` pair in deathmatch view.
pub(super) static mut dm_frags: [[c_int; MAXPLAYERS]; MAXPLAYERS] = [[0; MAXPLAYERS]; MAXPLAYERS];
/// Running per-player frag totals for deathmatch view.
pub(super) static mut dm_totals: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];

/// Running frag-count display per player in cooperative netgame view.
pub(super) static mut cnt_frags: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];
/// Non-zero when at least one player has a non-zero frag count (gates frags column display).
pub(super) static mut dofrags: c_int = 0;
/// Netgame count-up sub-state index.
pub(super) static mut ng_state: c_int = 0;

/// Single-player count-up sub-state index.
pub(super) static mut sp_state: c_int = 0;
/// Non-zero when the "you are here" pointer should be drawn in `ShowNextLoc` state.
pub(super) static mut snl_pointeron: bool = false;
