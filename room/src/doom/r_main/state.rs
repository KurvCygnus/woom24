//! Data home for the renderer main state: every `#[no_mangle]` static the
//! renderer subsystems read and write, plus the light-table constants and
//! the `lighttable_t` vocabulary.

use std::ffi::c_int;
use std::ptr;

use crate::doom::d_player::PlayerT;
use crate::doom::m_fixed::{angle_t, fixed_t};
use crate::types::Boolean;
use crate::doom::video_cfg::MAX_SCREENWIDTH;
use crate::doom::tables;

/// Number of distinct light levels used in the `scalelight` / `zlight` tables.
pub(super) const LIGHTLEVELS: usize = 16;

/// Shift used to convert a wall scale value into a light-level index (unused in
/// this module but exported for sibling renderer modules).
#[allow(dead_code)]
pub(super) const LIGHTSEGSHIFT: u32 = 4;

/// Maximum number of scale steps in the `scalelight` table (one entry per
/// screen-pixel column-scale bucket).
pub(super) const MAXLIGHTSCALE: usize = 48;

/// Shift applied to a column scale value before indexing `scalelight`.
pub(super) const LIGHTSCALESHIFT: u32 = 12;

/// Maximum number of Z-distance steps in the `zlight` table.
pub(super) const MAXLIGHTZ: usize = 128;

/// Shift applied to a Z-distance value before indexing `zlight`.
pub(super) const LIGHTZSHIFT: u32 = 20;

/// Number of colormap entries (palette remapping tables); each is 256 bytes.
pub(super) const NUMCOLORMAPS: usize = 32;

/// Divisor applied to the raw scale/distance value when mapping to a light
/// level, controlling how quickly lighting falls off with distance.
pub(super) const DISTMAP: usize = 2;

/// Raw colormap byte type; a palette index remap table entry.
pub(super) type lighttable_t = u8;

/// Additive angle offset applied to the player's map angle before rendering,
/// used by the automap and demo-playback angle overrides. Zero during normal
/// gameplay.
#[no_mangle]
pub static mut viewangleoffset: c_int = 0;

/// Monotonically incrementing counter; bumped once per frame in
/// [`crate::doom::r_main::frame::setup_frame`]. Sectors and linedefs tag
/// themselves with `validcount` when first visited in a frame so they are not
/// processed twice.
#[no_mangle]
pub static mut validcount: c_int = 1;

/// Active fixed colormap pointer, or null when no override is in effect.
///
/// Set to a non-null colormap when the player has an invulnerability or
/// light-amp powerup active (`player.fixedcolormap != 0`). When non-null,
/// all walls, floors, and sprites use this single colormap instead of the
/// distance-based `scalelight`/`zlight` entries.
#[no_mangle]
pub static mut fixedcolormap: *mut lighttable_t = ptr::null_mut();

/// X coordinate of the viewport centre in screen pixels.
///
/// Recomputed by [`crate::doom::r_main::viewsize::execute_set_view_size`]
/// whenever the view size changes.
#[no_mangle]
pub static mut centerx: c_int = 0;

/// Y coordinate of the viewport centre in screen pixels.
///
/// Recomputed by [`crate::doom::r_main::viewsize::execute_set_view_size`]
/// whenever the view size changes.
#[no_mangle]
pub static mut centery: c_int = 0;

/// [`centerx`] expressed as a 16.16 fixed-point value (`centerx << FRACBITS`).
///
/// Used by [`crate::doom::r_main::viewsize::init_texture_mapping`] and
/// projection arithmetic in
/// [`crate::doom::r_main::viewsize::execute_set_view_size`].
#[no_mangle]
pub static mut centerxfrac: fixed_t = 0;

/// [`centery`] expressed as a 16.16 fixed-point value (`centery << FRACBITS`).
///
/// Precomputed by [`crate::doom::r_main::viewsize::execute_set_view_size`];
/// used by wall and sprite renderers in `r_segs` and `r_things` to project
/// top/bottom screen coordinates.
#[no_mangle]
pub static mut centeryfrac: fixed_t = 0;

/// Fixed-point focal length (horizontal projection constant).
///
/// Equal to `centerxfrac`; used by
/// [`crate::doom::r_main::geometry::scale_from_global_angle`] and the
/// texture mapping setup in
/// [`crate::doom::r_main::viewsize::init_texture_mapping`]. Represents the
/// distance from the eye to the projection plane in fixed-point units.
#[no_mangle]
pub static mut projection: fixed_t = 0;

/// Number of frames rendered since [`crate::doom::r_main::frame::init`] was
/// called.
///
/// Incremented once per frame in
/// [`crate::doom::r_main::frame::setup_frame`]. Used for profiling.
#[no_mangle]
pub static mut framecount: c_int = 0;

/// Number of subsectors drawn in the current frame.
///
/// Reset to zero in [`crate::doom::r_main::frame::setup_frame`]; incremented
/// in `r_bsp`. Used for profiling.
#[no_mangle]
pub static mut sscount: c_int = 0;

/// Number of linedefs processed in the current frame (profiling counter).
///
/// Dead-but-exported (zero consumers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut linecount: c_int = 0;

/// Number of BSP traversal iterations in the current frame (profiling
/// counter).
///
/// Dead-but-exported (zero consumers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone.
#[no_mangle]
pub static mut loopcount: c_int = 0;

/// View position X in map fixed-point units. Set each frame by
/// [`crate::doom::r_main::frame::setup_frame`] from the player mobj's `x`
/// field.
#[no_mangle]
pub static mut viewx: fixed_t = 0;

/// View position Y in map fixed-point units. Set each frame by
/// [`crate::doom::r_main::frame::setup_frame`] from the player mobj's `y`
/// field.
#[no_mangle]
pub static mut viewy: fixed_t = 0;

/// View height Z in map fixed-point units (player eye height). Set each
/// frame by [`crate::doom::r_main::frame::setup_frame`] from
/// `player.viewz`.
#[no_mangle]
pub static mut viewz: fixed_t = 0;

/// Current view angle as a Binary Angle Measurement (BAM) `u32`.
///
/// Set each frame in [`crate::doom::r_main::frame::setup_frame`] from the
/// player mobj angle plus [`viewangleoffset`]. Used throughout the BSP
/// traversal and texture mapping pipeline.
#[no_mangle]
pub static mut viewangle: angle_t = 0;

/// `cos(viewangle)` in 16.16 fixed-point. Precomputed each frame in
/// [`crate::doom::r_main::frame::setup_frame`] for fast world-space
/// projection.
#[no_mangle]
pub static mut viewcos: fixed_t = 0;

/// `sin(viewangle)` in 16.16 fixed-point. Precomputed each frame in
/// [`crate::doom::r_main::frame::setup_frame`] for fast world-space
/// projection.
#[no_mangle]
pub static mut viewsin: fixed_t = 0;

/// Pointer to the player struct whose view is currently being rendered.
///
/// Set at the start of each frame by
/// [`crate::doom::r_main::frame::setup_frame`]; read by several renderer
/// subsystems that need player-specific state (e.g. weapon sprites).
#[no_mangle]
pub static mut viewplayer: *mut PlayerT = ptr::null_mut();

/// Detail level shift: `0` = high detail, `1` = low detail (half-width
/// columns doubled horizontally). Controls which column/span draw functions
/// are active and affects several scaling calculations.
#[no_mangle]
pub static mut detailshift: c_int = 0;

/// Half the horizontal field of view as a BAM angle.
///
/// Set by [`crate::doom::r_main::viewsize::init_texture_mapping`] to
/// `xtoviewangle[0]` - the largest view angle that still maps to screen
/// column 0. Used by the BSP clipper in `r_bsp` to cull out-of-frustum segs.
#[no_mangle]
pub static mut clipangle: angle_t = 0;

/// Maps fine-angle index to screen X column.
///
/// `viewangletox[i]` is the screen column (or sentinel `-1` /
/// `viewwidth+1` for out-of-frustum angles) for fine-angle `i`. Indexed
/// by `(viewangle >> ANGLETOFINESHIFT)`. Sized one element larger than
/// strictly necessary (`FINEANGLES/2 + 1`) to match `r_bsp`'s defensive
/// bounds.
// r_bsp.rs originally declared this as [c_int; FINEANGLES/2 + 1] to guard
// against a potential off-by-one in the original C code. Keep the same
// size so the two modules agree.
#[no_mangle]
pub static mut viewangletox: [c_int; tables::FINEANGLES / 2 + 1] = [0; tables::FINEANGLES / 2 + 1];

/// Maps screen X column to the smallest view angle that projects onto that
/// column.
///
/// `xtoviewangle[x]` gives the left-edge angle of the frustum slice at
/// column `x`. Sized `MAX_SCREENWIDTH + 1` (boom `xtoviewangle[MAX_SCREENWIDTH+1]`,
/// `r_main.c:81`) to include the right-edge sentinel for any raster the
/// `video_cfg` validation admits; only `[0..=viewwidth]` is filled and read.
#[no_mangle]
pub static mut xtoviewangle: [angle_t; MAX_SCREENWIDTH as usize + 1] =
    [0; MAX_SCREENWIDTH as usize + 1];

/// Distance-to-light lookup table indexed by `[light_level][scale]`.
///
/// `scalelight[i][j]` points into the master `colormaps` array. `i` is
/// derived from the sector light level, `j` from the projected wall/sprite
/// scale. Recomputed by
/// [`crate::doom::r_main::viewsize::execute_set_view_size`] because it
/// depends on `viewwidth`.
#[no_mangle]
pub static mut scalelight: [[*mut lighttable_t; MAXLIGHTSCALE]; LIGHTLEVELS] =
    [[ptr::null_mut(); MAXLIGHTSCALE]; LIGHTLEVELS];

/// Fixed-scale colormap array used when [`fixedcolormap`] is active.
///
/// All `MAXLIGHTSCALE` entries are set to [`fixedcolormap`] in
/// [`crate::doom::r_main::frame::setup_frame`], so that wall-light lookup
/// code does not need a special case for fixed-colormap mode.
#[no_mangle]
pub static mut scalelightfixed: [*mut lighttable_t; MAXLIGHTSCALE] =
    [ptr::null_mut(); MAXLIGHTSCALE];

/// Z-distance-to-light lookup table indexed by `[light_level][z_bucket]`.
///
/// `zlight[i][j]` points into `colormaps`. Used for flat (floor/ceiling)
/// and sprite lighting. Computed once in
/// [`crate::doom::r_main::viewsize::init_light_tables`] (unlike
/// `scalelight`, it does not depend on `viewwidth`).
#[no_mangle]
pub static mut zlight: [[*mut lighttable_t; MAXLIGHTZ]; LIGHTLEVELS] =
    [[ptr::null_mut(); MAXLIGHTZ]; LIGHTLEVELS];

/// Additional light bonus added to the sector's base light level, produced
/// by muzzle flashes and similar effects. Set each frame from
/// `player.extralight` in [`crate::doom::r_main::frame::setup_frame`].
#[no_mangle]
pub static mut extralight: c_int = 0;

/// Active column-drawing function pointer.
///
/// Points to either the normal or low-detail column renderer; may be
/// temporarily overridden by `r_things` to a fuzz or translated variant.
/// Reset to `basecolfunc` after each sprite.
#[no_mangle]
pub static mut colfunc: Option<unsafe extern "C" fn()> = None;

/// Base (unmodified) column-drawing function pointer.
///
/// Always points to the standard solid-column renderer for the current
/// detail level (`R_DrawColumn` or `R_DrawColumnLow`). Used to restore
/// `colfunc` after drawing special-effect sprites.
#[no_mangle]
pub static mut basecolfunc: Option<unsafe extern "C" fn()> = None;

/// Fuzz (partial-invisibility) column-drawing function pointer.
///
/// Points to `R_DrawFuzzColumn` or `R_DrawFuzzColumnLow` depending on the
/// active detail level.
#[no_mangle]
pub static mut fuzzcolfunc: Option<unsafe extern "C" fn()> = None;

/// Translated (palette-remapped) column-drawing function pointer.
///
/// Points to `R_DrawTranslatedColumn` or `R_DrawTranslatedColumnLow`.
/// Used for colored player sprites in multiplayer.
#[no_mangle]
pub static mut transcolfunc: Option<unsafe extern "C" fn()> = None;

/// Horizontal span (floor/ceiling) drawing function pointer.
///
/// Points to `R_DrawSpan` or `R_DrawSpanLow` depending on the active
/// detail level.
#[no_mangle]
pub static mut spanfunc: Option<unsafe extern "C" fn()> = None;

/// Flag set by [`crate::doom::r_main::viewsize::set_view_size`] when a
/// view-size change is pending.
///
/// [`crate::doom::r_main::viewsize::execute_set_view_size`] checks this at
/// the start of each frame and applies the pending change if set. The
/// deferred approach avoids changing viewport dimensions mid-frame.
#[no_mangle]
pub static mut setsizeneeded: Boolean = Boolean::FALSE;

/// Pending viewport block size (1-11). Set by
/// [`crate::doom::r_main::viewsize::set_view_size`] and consumed by
/// [`crate::doom::r_main::viewsize::execute_set_view_size`]. Value 11
/// selects full-screen rendering.
#[no_mangle]
pub static mut setblocks: c_int = 0;

/// Pending detail level (0 = high, 1 = low). Set by
/// [`crate::doom::r_main::viewsize::set_view_size`] and consumed by
/// [`crate::doom::r_main::viewsize::execute_set_view_size`].
#[no_mangle]
pub static mut setdetail: c_int = 0;
