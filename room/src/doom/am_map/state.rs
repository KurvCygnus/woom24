//! The automap run-time state: the ~50 statics (upstream names) shared
//! by every subfile, including the two `#[no_mangle]` exports
//! (`automapactive` -- extern-declared by hu_lib/hu_stuff and read by
//! Rust path across the tree -- and `cheat_amap`).

use std::ffi::c_int;
use std::ptr;

use super::consts::{mpoint_t, AM_NUMMARKPOINTS, INITSCALEMTOF};
use super::responder::make_cheat_seq;
use crate::doom::d_player::PlayerT;
use crate::doom::m_cheat::cheatseq_t;
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::v_video::patch_t;

/// Current cheat level: 0 = none, 1 = show all walls, 2 = show all + things.
///
/// Cycles through 0-2 when the `iddt` cheat sequence is entered.
/// C origin: `cheating` in am_map.c.
pub(super) static mut cheating: c_int = 0;

/// Non-zero when the background grid is enabled.
///
/// Toggled by the `key_map_grid` key.  C origin: `grid` in am_map.c.
pub(super) static mut grid: c_int = 0;

/// Non-zero when the automap has just been initialised for a new level and
/// `level_init` has not yet run.
///
/// C origin: `leveljuststarted` in am_map.c.
pub(super) static mut leveljuststarted: c_int = 1;

/// Non-zero while the automap is active and being drawn.
///
/// Exported so that other modules (e.g. `f_finale`) can read/clear it;
/// extern-declared by `hu_lib`/`hu_stuff` (link by symbol).
/// C origin: `automapactive` in am_map.c.
#[no_mangle]
pub static mut automapactive: c_int = 0;

/// Width of the automap framebuffer window in pixels.
///
/// The automap always fills the full screen width; refreshed from the
/// runtime raster width on every [`super::lifecycle::init_variables`]
/// (crispy moved the same pair out of static initializers when
/// `SCREENWIDTH` went runtime).
/// C origin: `finit_width` in am_map.c.
pub(super) static mut finit_width: c_int = 320;

/// Height of the automap framebuffer window in pixels.
///
/// `SCREENHEIGHT - 32` to leave room for the status bar; refreshed on every
/// [`super::lifecycle::init_variables`].  C origin: `finit_height` in am_map.c.
pub(super) static mut finit_height: c_int = 168;

/// Left edge of the automap window in screen pixels.
///
/// C origin: `f_x` in am_map.c.
pub(super) static mut f_x: c_int = 0;

/// Top edge of the automap window in screen pixels.
///
/// C origin: `f_y` in am_map.c.
pub(super) static mut f_y: c_int = 0;

/// Width of the automap window in screen pixels.
///
/// C origin: `f_w` in am_map.c.
pub(super) static mut f_w: c_int = 0;

/// Height of the automap window in screen pixels.
///
/// C origin: `f_h` in am_map.c.
pub(super) static mut f_h: c_int = 0;

/// Current light level used to offset wall colours (currently unused at runtime).
///
/// C origin: `lightlev` in am_map.c.
pub(super) static mut lightlev: c_int = 0;

/// Pointer to the automap's framebuffer region (same as `I_VideoBuffer`).
///
/// Set in [`super::lifecycle::init_variables`].  C origin: `fb` in am_map.c.
pub(super) static mut fb: *mut u8 = ptr::null_mut();

/// Tick counter incremented each game tick while the automap is active.
///
/// Used to pace the (currently disabled) light-level animation.
/// C origin: `amclock` in am_map.c.
pub(super) static mut amclock: c_int = 0;

/// Per-tick map-coordinate pan increment.
///
/// Set to a non-zero value while a pan key is held; reset on key-up.
/// C origin: `m_paninc` in am_map.c.
pub(super) static mut m_paninc: mpoint_t = mpoint_t { x: 0, y: 0 };

/// Fixed-point multiplier applied to `scale_mtof` each tick while zooming.
///
/// Normally `FRACUNIT` (no zoom); set to [`super::consts::M_ZOOMIN`] or
/// [`super::consts::M_ZOOMOUT`] while a zoom key is held.
/// C origin: `mtof_zoommul` in am_map.c.
pub(super) static mut mtof_zoommul: fixed_t = FRACUNIT;

/// Fixed-point multiplier applied to `scale_ftom` each tick while zooming.
///
/// Reciprocal of [`mtof_zoommul`].  C origin: `ftom_zoommul` in am_map.c.
pub(super) static mut ftom_zoommul: fixed_t = FRACUNIT;

/// Left edge of the map viewport in map coordinates.
///
/// C origin: `m_x` in am_map.c.
pub(super) static mut m_x: fixed_t = 0;

/// Bottom edge of the map viewport in map coordinates.
///
/// C origin: `m_y` in am_map.c.
pub(super) static mut m_y: fixed_t = 0;

/// Right edge of the map viewport in map coordinates (`m_x + m_w`).
///
/// C origin: `m_x2` in am_map.c.
pub(super) static mut m_x2: fixed_t = 0;

/// Top edge of the map viewport in map coordinates (`m_y + m_h`).
///
/// C origin: `m_y2` in am_map.c.
pub(super) static mut m_y2: fixed_t = 0;

/// Width of the map viewport in map coordinates.
///
/// C origin: `m_w` in am_map.c.
pub(super) static mut m_w: fixed_t = 0;

/// Height of the map viewport in map coordinates.
///
/// C origin: `m_h` in am_map.c.
pub(super) static mut m_h: fixed_t = 0;

/// Minimum x coordinate over all level vertices.
///
/// C origin: `min_x` in am_map.c.
pub(super) static mut min_x: fixed_t = 0;

/// Minimum y coordinate over all level vertices.
///
/// C origin: `min_y` in am_map.c.
pub(super) static mut min_y: fixed_t = 0;

/// Maximum x coordinate over all level vertices.
///
/// C origin: `max_x` in am_map.c.
pub(super) static mut max_x: fixed_t = 0;

/// Maximum y coordinate over all level vertices.
///
/// C origin: `max_y` in am_map.c.
pub(super) static mut max_y: fixed_t = 0;

/// Maximum viewport width in map coordinates (bounding-box width of the level).
///
/// C origin: `max_w` in am_map.c.
pub(super) static mut max_w: fixed_t = 0;

/// Maximum viewport height in map coordinates (bounding-box height of the level).
///
/// C origin: `max_h` in am_map.c.
pub(super) static mut max_h: fixed_t = 0;

/// Minimum viewport width: 2 * `PLAYERRADIUS` (prevents over-zoom).
///
/// C origin: `min_w` in am_map.c.
pub(super) static mut min_w: fixed_t = 0;

/// Minimum viewport height: 2 * `PLAYERRADIUS` (prevents over-zoom).
///
/// C origin: `min_h` in am_map.c.
pub(super) static mut min_h: fixed_t = 0;

/// Minimum allowed `scale_mtof` (most zoomed out to fit the whole level).
///
/// C origin: `min_scale_mtof` in am_map.c.
pub(super) static mut min_scale_mtof: fixed_t = 0;

/// Maximum allowed `scale_mtof` (most zoomed in: 2 * `PLAYERRADIUS` fills the
/// screen).
///
/// C origin: `max_scale_mtof` in am_map.c.
pub(super) static mut max_scale_mtof: fixed_t = 0;

/// Saved map viewport origin x, used to restore after `min_out_window_scale`.
///
/// C origin: `old_m_w`, `old_m_h`, `old_m_x`, `old_m_y` in am_map.c.
pub(super) static mut old_m_w: fixed_t = 0;
/// Saved map viewport height.
pub(super) static mut old_m_h: fixed_t = 0;
/// Saved map viewport left edge.
pub(super) static mut old_m_x: fixed_t = 0;
/// Saved map viewport bottom edge.
pub(super) static mut old_m_y: fixed_t = 0;

/// Last recorded player position, used by [`super::view::do_follow_player`]
/// to detect movement.
///
/// Initialised to `c_int::MAX` to force an update on the first tick.
/// C origin: `f_oldloc` in am_map.c.
pub(super) static mut f_oldloc: mpoint_t = mpoint_t { x: 0, y: 0 };

/// Map-to-frame scale factor (fixed-point pixels per map unit).
///
/// Initialised to [`INITSCALEMTOF`]; adjusted by zoom operations.
/// C origin: `scale_mtof` in am_map.c.
pub(super) static mut scale_mtof: fixed_t = INITSCALEMTOF;

/// Frame-to-map scale factor; reciprocal of [`scale_mtof`] in fixed-point.
///
/// C origin: `scale_ftom` in am_map.c.
pub(super) static mut scale_ftom: fixed_t = 0;

/// Pointer to the player structure being tracked by the automap.
///
/// Set to the console player (or the first active player in a network game)
/// in [`super::lifecycle::init_variables`].  C origin: `plr` in am_map.c.
pub(super) static mut plr: *mut PlayerT = ptr::null_mut();

/// Cached patch pointers for the ten mark-number glyphs (`AMMNUM0`-`AMMNUM9`).
///
/// Loaded from the WAD by [`super::lifecycle::load_pics`] and released by
/// [`super::lifecycle::unload_pics`].
/// C origin: `marknums[]` in am_map.c.
pub(super) static mut marknums: [*mut patch_t; AM_NUMMARKPOINTS] =
    [ptr::null_mut(); AM_NUMMARKPOINTS];

/// Map-coordinate positions of the player-placed mark points.
///
/// An `x` value of `-1` indicates an unused slot.
/// C origin: `markpoints[]` in am_map.c.
pub(super) static mut markpoints: [mpoint_t; AM_NUMMARKPOINTS] =
    [mpoint_t { x: -1, y: -1 }; AM_NUMMARKPOINTS];

/// Index of the next mark slot to fill, wrapping modulo `AM_NUMMARKPOINTS`.
///
/// C origin: `markpointnum` in am_map.c.
pub(super) static mut markpointnum: c_int = 0;

/// Non-zero when the automap camera should track the player position.
///
/// Set to 0 when the player pans the map manually; restored on `key_map_follow`.
/// C origin: `followplayer` in am_map.c.
pub(super) static mut followplayer: c_int = 1;

/// Cheat sequence for the automap reveal (`iddt`).
///
/// Exported so that `responder` can pass a pointer to it to
/// `cht_CheckCheat`.  C origin: `cheat_amap` in am_map.c.
#[no_mangle]
pub static mut cheat_amap: cheatseq_t = cheatseq_t {
    sequence: make_cheat_seq(b"iddt"),
    sequence_len: 4,
    parameter_chars: 0,
    chars_read: 0,
    param_chars_read: 0,
    parameter_buf: [0; 5],
};

/// Non-zero when the automap is fully inactive (after `AM_Stop`).
///
/// Prevents `stop` from running its shutdown logic more than once per
/// open/close cycle.  C origin: `stopped` in am_map.c.
pub(super) static mut stopped: c_int = 1;
