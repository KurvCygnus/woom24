//! The intermission type tier: the two `#[repr(C)]` ABI structs that
//! cross the g_game boundary by pointer, the state/config enums, the
//! animation types, the geometry constants, and the runtime-raster-aware
//! `SP_TIMEY()`.

use std::cell::UnsafeCell;
use std::ffi::c_int;
use std::ptr;

use crate::doom::d_player::MAXPLAYERS;
use crate::doom::v_video::patch_t;

/// Number of Doom episodes (E1-E4, matches C `NUMEPISODES`).
pub(super) const NUMEPISODES: usize = 4;
/// Maps per episode (1-9, matches C `NUMMAPS`).
pub(super) const NUMMAPS: usize = 9;

/// Screen y coordinate of the level-name title patch (matches C `WI_TITLEY`).
pub(super) const WI_TITLEY: c_int = 2;
/// Vertical pixel spacing between player rows in the netgame and deathmatch views (matches C `WI_SPACINGY`).
pub(super) const WI_SPACINGY: c_int = 33;

/// Screen x of the stats column in single-player view (matches C `SP_STATSX`).
pub(super) const SP_STATSX: c_int = 50;
/// Screen y of the stats area in single-player view (matches C `SP_STATSY`).
pub(super) const SP_STATSY: c_int = 50;
/// Screen x of the time display in single-player view (matches C `SP_TIMEX`).
pub(super) const SP_TIMEX: c_int = 16;

/// Screen y of the time display in single-player view (matches C `SP_TIMEY`).
/// F1 M2: `SCREENHEIGHT` is the runtime raster height, so SP_TIMEY is
/// computed at the call sites instead of baked into a const.
pub(super) fn SP_TIMEY() -> c_int {
    let height = unsafe { crate::doom::i_video::SCREENHEIGHT };
    height - 32
}

/// Screen y of the stats header row in netgame view (matches C `NG_STATSY`).
pub(super) const NG_STATSY: c_int = 50;
/// Horizontal pixel spacing between stat columns in netgame view (matches C `NG_SPACINGX`).
pub(super) const NG_SPACINGX: c_int = 64;

/// Screen x of the frag matrix top-left in deathmatch view (matches C `DM_MATRIXX`).
pub(super) const DM_MATRIXX: c_int = 42;
/// Screen y of the frag matrix top in deathmatch view (matches C `DM_MATRIXY`).
pub(super) const DM_MATRIXY: c_int = 68;
/// Horizontal spacing between columns in the deathmatch frag matrix (matches C `DM_SPACINGX`).
pub(super) const DM_SPACINGX: c_int = 40;
/// Screen x of the "Totals" column in deathmatch view (matches C `DM_TOTALSX`).
pub(super) const DM_TOTALSX: c_int = 269;
/// Screen x of the "Killers" label in deathmatch view (matches C `DM_KILLERSX`).
pub(super) const DM_KILLERSX: c_int = 10;
/// Screen y of the "Killers" label in deathmatch view (matches C `DM_KILLERSY`).
pub(super) const DM_KILLERSY: c_int = 100;
/// Screen x of the "Victims" label in deathmatch view (matches C `DM_VICTIMSX`).
pub(super) const DM_VICTIMSX: c_int = 5;
/// Screen y of the "Victims" label in deathmatch view (matches C `DM_VICTIMSY`).
pub(super) const DM_VICTIMSY: c_int = 50;

/// Number of seconds (in `TICRATE` units) the "Show Next Location" map is displayed (matches C `SHOWNEXTLOCDELAY`).
pub(super) const SHOWNEXTLOCDELAY: c_int = 4;

// ---------------------------------------------------------------------------
// Types that must match C layout (g_game.c is still C)
// ---------------------------------------------------------------------------

/// Per-player intermission data passed in from the game loop (C typedef `wbplayerstruct_t`).
///
/// Layout must be ABI-identical to the C struct because `g_game.c` populates it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct wbplayerstruct_t {
    /// Non-zero if this player slot is in use.
    pub in_: c_int,
    /// Number of kills this player scored on the level.
    pub skills: c_int,
    /// Number of items this player collected.
    pub sitems: c_int,
    /// Number of secrets this player found.
    pub ssecret: c_int,
    /// Elapsed level time in tics.
    pub stime: c_int,
    /// Frag counts against each of the 4 possible players.
    pub frags: [c_int; 4],
    /// Unused score field (carried from the C struct for ABI compatibility).
    pub score: c_int,
}

/// Overall intermission input record passed from `G_WorldDone` (C typedef `wbstartstruct_t`).
///
/// Layout must be ABI-identical to the C struct; `#[repr(C)]` ensures this.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct wbstartstruct_t {
    /// Episode index (0-based).
    pub epsd: c_int,
    /// Non-zero if the player found the secret level on this episode.
    pub didsecret: c_int,
    /// Index of the map the player just completed (0-based).
    pub last: c_int,
    /// Index of the map the player is going to next (0-based).
    pub next: c_int,
    /// Total killable monsters on the level (denominator for kill percentage).
    pub maxkills: c_int,
    /// Total collectible items on the level (denominator for item percentage).
    pub maxitems: c_int,
    /// Total secrets on the level (denominator for secret percentage).
    pub maxsecret: c_int,
    /// Maximum frag count (not used in single-player).
    pub maxfrags: c_int,
    /// Par time for the level in tics.
    pub partime: c_int,
    /// Console player number (0-based).
    pub pnum: c_int,
    /// Per-player stats for up to `MAXPLAYERS` players.
    pub plyr: [wbplayerstruct_t; MAXPLAYERS],
}

/// Intermission state machine states (C `stateenum_t` in `wi_stuff.c`).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum stateenum_t {
    /// Transitioning to the next level; brief pause before `G_WorldDone`.
    NoState = -1,
    /// Counting up kill/item/secret/time statistics.
    StatCount,
    /// Showing the episode map with the "you are here" pointer.
    ShowNextLoc,
}

/// Screen coordinate pair used for level-node positions and animation locations (C `point_t`).
#[derive(Clone, Copy)]
pub(super) struct point_t {
    /// Horizontal screen pixel.
    pub x: c_int,
    /// Vertical screen pixel.
    pub y: c_int,
}

/// Animation playback mode for background animations (C `animenum_t`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum animenum_t {
    /// Play every `period` tics regardless of game state.
    ANIM_ALWAYS,
    /// Play randomly with a delay between cycles.
    ANIM_RANDOM,
    /// Play only when the "next" map index matches `data1`.
    ANIM_LEVEL,
}

/// Immutable per-animation configuration (set at compile time, never written at runtime).
///
/// Corresponds to the read-only fields of C `anim_t` in `wi_stuff.c`.
#[derive(Clone, Copy)]
pub(super) struct anim_config_t {
    /// Playback mode: always, random, or level-triggered.
    pub type_: animenum_t,
    /// Tics between frame advances (or between random retriggers for `ANIM_RANDOM`).
    pub period: c_int,
    /// Number of patch frames in this animation (1-3).
    pub nanims: c_int,
    /// Screen position where the animation is drawn.
    pub loc: point_t,
    /// For `ANIM_LEVEL`: the map index that triggers playback.
    /// For `ANIM_RANDOM`: the max additional delay (in tics) before the next play.
    pub data1: c_int,
    /// For `ANIM_RANDOM`: the minimum delay (in tics) before the next play.
    pub data2: c_int,
}

/// Mutable per-animation runtime state (zeroed at startup, written every frame).
///
/// Corresponds to the mutable fields of C `anim_t` in `wi_stuff.c`.
#[derive(Clone, Copy)]
pub(super) struct anim_state_t {
    /// Cached patch pointers for each frame; loaded by `lifecycle::load_data`.
    pub p: [*mut patch_t; 3],
    /// Game tic on which the next frame advance is scheduled.
    pub nexttic: c_int,
    /// Frame index drawn on the previous tic (unused in this port; kept for layout parity).
    pub lastdrawn: c_int,
    /// Current frame index within `p` (-1 = not yet started).
    pub ctr: c_int,
    /// Internal sub-state for `ANIM_RANDOM` sequencing.
    pub state: c_int,
}

/// Zero-initialised animation state used to fill state tables at program start.
pub(super) const ZERO_STATE: anim_state_t = anim_state_t {
    p: [ptr::null_mut(); 3],
    nexttic: 0,
    lastdrawn: 0,
    ctr: 0,
    state: 0,
};

/// Interior-mutable wrapper for a fixed-size animation state array.
/// Single-threaded Doom: safe because all access is from the main game thread.
pub(super) struct AnimStateTable<const N: usize>(pub(super) UnsafeCell<[anim_state_t; N]>);
// SAFETY: Doom is single-threaded; no concurrent access to these tables.
unsafe impl<const N: usize> Sync for AnimStateTable<N> {}
