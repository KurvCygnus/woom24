//! L3 `video_cfg` — runtime video configuration and raster reconfiguration
//! (F1 M2).
//!
//! This module owns the engine's internal raster resolution. The classic
//! `SCREENWIDTH`/`SCREENHEIGHT` constants become runtime values owned here
//! (same `static mut` pattern the port uses for every other C global), and a
//! `VideoConfig` change re-creates the frame buffer through the same
//! zone-allocation path `I_InitGraphics` uses, then schedules the deferred
//! view rebuild (`R_SetViewSize` -> `R_ExecuteSetViewSize`) that vanilla
//! already runs for view-size changes.
//!
//! Reference practice (verified in the local clones):
//! - Boom keeps compile-time `SCREENWIDTH`/`MAX_SCREENWIDTH` with every
//!   per-column lookup array sized to the max (`doomdef.h:97-101`,
//!   `r_main.c:81`); prboom/dsda mature the same shapes into runtime ints.
//!   Here: `MAX_SCREENWIDTH`/`MAX_SCREENHEIGHT` size every static lookup
//!   array, and `validate` rejects anything past the caps — array safety is
//!   provable at compile time.
//! - Crispy rounds the raster width up to a multiple of 4 and caps it at a
//!   compile-time MAXWIDTH "(array size!)" (`i_video.c:1769-1772`); the same
//!   rounding and cap live in [`VideoConfig::validated`].
//! - The doomgeneric present buffer follows the config: the classic integer
//!   `fb_scaling` doubling (`i_video.rs` `I_FinishUpdate`) survives only
//!   while the raster is smaller than the present target (320x200 keeps the
//!   2x doubling to 640x400 — byte-identical to the pre-M2 engine, pinned by
//!   the `video_raster` regression anchor); any raster at or above the target
//!   presents pass-through at the raster's own size.

// ---------------------------------------------------------------------------
// Caps — the compile-time array sizing contract
// ---------------------------------------------------------------------------

/// Hard upper bound for the raster width, in palette-indexed pixels.
///
/// Every per-column lookup array in the renderer (`xtoviewangle`,
/// `columnofs`, `floorclip`, `distscale`, visplane `top`/`bottom`, ...) is
/// statically sized to this value (the Boom `MAX_SCREENWIDTH` shape), so
/// [`VideoConfig::validated`] must reject any larger width — that rejection
/// is what makes the array indexing provably in-bounds.
pub const MAX_SCREENWIDTH: u32 = 4096;

/// Hard upper bound for the raster height, in palette-indexed pixels.
///
/// Per-row arrays (`ylookup`, `yslope`, `spanstart`, the `cached*` row
/// caches) are statically sized to this value; same proof obligation as
/// [`MAX_SCREENWIDTH`].
pub const MAX_SCREENHEIGHT: u32 = 4096;

/// Minimum raster width the engine accepts.
///
/// 320 is the vanilla view coordinate system's native width; smaller rasters
/// would break the vanilla 320x200 semantics this layer must preserve
/// (status-bar layout, patch coordinates, psprite scales).
pub const MIN_SCREENWIDTH: u32 = 320;

/// Minimum raster height the engine accepts (vanilla height).
pub const MIN_SCREENHEIGHT: u32 = 200;

// ---------------------------------------------------------------------------
// AspectMode — M2 ships the enum; only VanillaStretch is wired up
// ---------------------------------------------------------------------------

/// Aspect handling mode. M2 implements the `VanillaStretch` baseline only;
/// `Wide` (extended view frustum, dsda atan-based FOV extension) is the M3
/// milestone and is currently inert — a `Wide` config is accepted and stored,
/// but does not yet change any render geometry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AspectMode
{
    /// The vanilla 4:3 correctness baseline: internal raster rendered at
    /// `width` x `height`, with the 1.2 pixel-aspect correction applied by
    /// the presenter's CSS/window sizing (Chocolate's present-time-only
    /// stretch model). Today's behavior.
    VanillaStretch,
    /// Horizontally extended view frustum (render more world columns).
    /// Reserved for F1 M3; not yet wired to any frustum change.
    Wide,
}

// ---------------------------------------------------------------------------
// VideoConfig
// ---------------------------------------------------------------------------

/// The engine's video configuration: internal raster dimensions plus aspect
/// mode. Lives at the contract surface for the F7 settings registry (M4);
/// the shells consume it through [`apply`] today.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VideoConfig
{
    /// Internal raster width in palette-indexed pixels (wide widths are
    /// rounded up to a multiple of 4 — crispy practice).
    pub width: u32,
    /// Internal raster height in palette-indexed pixels.
    pub height: u32,
    /// Aspect handling mode (only [`AspectMode::VanillaStretch`] is wired).
    pub aspect: AspectMode,
}

impl VideoConfig
{
    /// Build a config without validation. Prefer [`VideoConfig::validated`]
    /// at the contract surface; [`apply`] validates anyway.
    pub const fn new(width: u32, height: u32, aspect: AspectMode) -> Self
    {
        Self
        {
            width,
            height,
            aspect,
        }
    }

    /// Validate a copy of this config, rounding the width up to a multiple
    /// of 4 (crispy `i_video.c:1769-1772`) and rejecting sizes outside
    /// `[MIN_SCREENWIDTH, MAX_SCREENWIDTH] x [MIN_SCREENHEIGHT,
    /// MAX_SCREENHEIGHT]` — the exact domain the renderer's statically
    /// capped lookup arrays can address.
    pub fn validated(&self) -> Result<VideoConfig, String>
    {
        if self.width < MIN_SCREENWIDTH || self.height < MIN_SCREENHEIGHT
        {
            return Err(format!(
                "video config {}x{} below the minimum {}x{}",
                self.width, self.height, MIN_SCREENWIDTH, MIN_SCREENHEIGHT
            ));
        }
        if self.width > MAX_SCREENWIDTH || self.height > MAX_SCREENHEIGHT
        {
            return Err(format!(
                "video config {}x{} above the maximum {}x{} (renderer array caps)",
                self.width, self.height, MAX_SCREENWIDTH, MAX_SCREENHEIGHT
            ));
        }
        // Crispy: make sure the raster width is an integer multiple of 4.
        let width = (self.width + 3) & !3;
        Ok(VideoConfig
        {
            width,
            height: self.height,
            aspect: self.aspect,
        })
    }
}

impl Default for VideoConfig
{
    /// Today's behavior: the vanilla 320x200 raster with the classic 4:3
    /// stretch. The regression anchor pins this config's present output as
    /// byte-identical to the pre-M2 engine.
    fn default() -> Self
    {
        VideoConfig
        {
            width: 320,
            height: 200,
            aspect: AspectMode::VanillaStretch,
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime raster dimensions (the former i_video constants)
// ---------------------------------------------------------------------------

/// Doom's logical raster width in palette-indexed pixels. Formerly the
/// `SCREENWIDTH` constant in `i_video`; now a runtime value owned by this
/// module, written only by [`apply`]. Mirrors the C global of the same name
/// (prboom/dsda keep theirs runtime ints the same way).
#[no_mangle]
pub static mut SCREENWIDTH: i32 = 320;

/// Doom's logical raster height in palette-indexed pixels. Formerly the
/// `SCREENHEIGHT` constant in `i_video`; see [`SCREENWIDTH`].
#[no_mangle]
pub static mut SCREENHEIGHT: i32 = 200;

/// Safe read of the live raster width.
pub fn screen_width() -> i32
{
    unsafe { SCREENWIDTH }
}

/// Safe read of the live raster height.
pub fn screen_height() -> i32
{
    unsafe { SCREENHEIGHT }
}

/// The live config as constructed from the runtime raster dimensions. The
/// aspect mode is not recoverable from the statics (both modes share the
/// same dimension representation), so it always reads
/// [`AspectMode::VanillaStretch`]; the wired config is tracked separately
/// once M4's settings registry lands.
pub fn current() -> VideoConfig
{
    VideoConfig
    {
        width: screen_width() as u32,
        height: screen_height() as u32,
        aspect: AspectMode::VanillaStretch,
    }
}

// ---------------------------------------------------------------------------
// Present-scale policy
// ---------------------------------------------------------------------------

/// The doomgeneric present target: today's `DOOMGENERIC_RESX/RESY` defaults.
/// The integer doubling in `I_FinishUpdate` scales the raster up to this
/// floor and no further; rasters at or above it present pass-through.
const PRESENT_TARGET_WIDTH: u32 = 640;
const PRESENT_TARGET_HEIGHT: u32 = 400;

/// Integer present scale for a raster size: the largest doubling that keeps
/// the present buffer at or above the classic 640x400 target, floored at 1.
///
/// 320x200 -> 2 (today's behavior, byte-identical present); 640x400 and
/// anything larger -> 1 (pass-through; the spec's "degrades to pass-through
/// when the raster already matches the target").
pub fn present_scale(width: u32, height: u32) -> u32
{
    let sx = PRESENT_TARGET_WIDTH / width.max(1);
    let sy = PRESENT_TARGET_HEIGHT / height.max(1);
    sx.min(sy).max(1)
}

// ---------------------------------------------------------------------------
// Reconfiguration
// ---------------------------------------------------------------------------

/// Apply a video configuration at runtime.
///
/// The mid-game resolution-switch path: validates the config, re-creates the
/// palette frame buffer through the zone allocator (freeing the previous
/// buffer — the `I_InitGraphics`-adjacent path the spec names), resizes the
/// doomgeneric present buffer, updates the raster statics, and schedules the
/// deferred view rebuild via `R_SetViewSize` (the vanilla resize mechanism:
/// the next rendered frame's `R_ExecuteSetViewSize` recomputes every view
/// global and lookup table for the new raster).
///
/// Allocation failure follows the established degradation contract:
/// `I_Error` (native) / the log-channel banner (web) via the `i_error!` path
/// inside [`crate::doom::i_video`].
pub fn apply(cfg: VideoConfig) -> Result<(), String>
{
    let cfg = cfg.validated()?;
    if cfg == current()
    {
        return Ok(());
    }

    let w = cfg.width;
    let h = cfg.height;
    let fb = present_scale(w, h) as i32;

    // Snapshot the current primary framebuffer so the V_* layer can follow
    // the swap (or stay on an off-screen buffer it was pointed at).
    let old_fb = unsafe { crate::doom::i_video::I_VideoBuffer };

    // Re-create the doomgeneric present buffer first: the framebuffer
    // descriptor in `i_video` follows its dimensions.
    let (dg_w, dg_h) = (w * fb as u32, h * fb as u32);
    unsafe
    {
        crate::doom::doomgeneric::realloc_screen_buffer(dg_w as usize, dg_h as usize)?;
    }

    // Re-create the palette frame buffer and retarget the present geometry.
    // This crosses into `i_video` because the framebuffer pointer, the FB
    // descriptor, and `fb_scaling` are module state there.
    unsafe
    {
        crate::doom::i_video::reinit_video_buffer(w as i32, h as i32, fb);
    }

    // Publish the new raster dimensions. Everything downstream (renderer
    // lookup tables, V_* strides, patch bounds, wipe buffers) reads these.
    unsafe
    {
        SCREENWIDTH = w as i32;
        SCREENHEIGHT = h as i32;
    }

    // If the V_* layer was pointed at the old (freed) framebuffer, retarget
    // it; a V_UseBuffer'd off-screen buffer survives untouched.
    unsafe
    {
        crate::doom::v_video::retarget_after_framebuffer_swap(old_fb);
    }

    // Schedule the deferred view rebuild — vanilla's own resize path. The
    // next frame's R_ExecuteSetViewSize rebuilds ylookup/columnofs for the
    // new buffer, re-derives xtoviewangle, psprite scales, yslope/distscale
    // and scalelight from the new raster dimensions.
    unsafe
    {
        crate::doom::r_main::R_SetViewSize(
            crate::doom::m_menu::screenblocks,
            crate::doom::m_menu::detailLevel,
        );
    }

    //* The re-created palette framebuffer lost the status bar's copied-in
    //* background (ST_drawWidgets only refreshes widgets over persisted
    //* pixels). Re-arm ST's one-shot full-redraw latch: st_backing_screen
    //* survives the swap (raster-independent ST_WIDTH x ST_HEIGHT), so the
    //* next ST_Drawer re-copies the background exactly like a fresh boot
    //* does. This status-bar staleness was surfaced by the apply diagnostic
    //* path and is ADJACENT to spec-4 C, whose sprite invisibility remains
    //* open (auto-diag instrumented); host-reproduced at 30,436 px.
    crate::doom::st_stuff::force_full_redraw();

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — the pure configuration surface (validation + present-scale math)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests
{
    use super::*;

    // -- Default ------------------------------------------------------------

    #[test]
    fn default_is_the_vanilla_320x200_stretch()
    {
        let cfg = VideoConfig::default();
        assert_eq!(cfg.width, 320);
        assert_eq!(cfg.height, 200);
        assert_eq!(cfg.aspect, AspectMode::VanillaStretch);
    }

    // -- Validation ---------------------------------------------------------

    #[test]
    fn validation_accepts_the_spec_test_sizes()
    {
        // 1366 validates to 1368: widths round up to a multiple of 4
        // (crispy i_video.c:1769-1772).
        for (w, h, want_w) in [(320u32, 200u32, 320u32), (640, 400, 640), (1366, 768, 1368)]
        {
            let got = VideoConfig::new(w, h, AspectMode::VanillaStretch)
                .validated()
                .unwrap_or_else(|e| panic!("{w}x{h} must validate: {e}"));
            assert_eq!((got.width, got.height), (want_w, h));
        }
    }

    #[test]
    fn validation_rejects_below_the_minimum()
    {
        assert!(VideoConfig::new(319, 200, AspectMode::VanillaStretch)
            .validated()
            .is_err());
        assert!(VideoConfig::new(320, 199, AspectMode::VanillaStretch)
            .validated()
            .is_err());
    }

    #[test]
    fn validation_rejects_above_the_array_caps()
    {
        assert!(VideoConfig::new(MAX_SCREENWIDTH + 1, 200, AspectMode::VanillaStretch)
            .validated()
            .is_err());
        assert!(VideoConfig::new(320, MAX_SCREENHEIGHT + 1, AspectMode::VanillaStretch)
            .validated()
            .is_err());
        assert!(VideoConfig::new(4096, 4096, AspectMode::VanillaStretch)
            .validated()
            .is_ok());
    }

    #[test]
    fn validation_rounds_width_up_to_a_multiple_of_four()
    {
        // Crispy i_video.c:1769-1772 — wide widths round up to 4.
        let got = VideoConfig::new(1367, 768, AspectMode::VanillaStretch)
            .validated()
            .unwrap();
        assert_eq!(got.width, 1368);
        assert_eq!(got.height, 768);
    }

    #[test]
    fn wide_mode_is_accepted_but_unwired_in_m2()
    {
        // The M3 variant constructs and validates; nothing may reject it yet.
        let got = VideoConfig::new(896, 200, AspectMode::Wide).validated().unwrap();
        assert_eq!(got.aspect, AspectMode::Wide);
    }

    // -- Present-scale policy -----------------------------------------------

    #[test]
    fn present_scale_keeps_the_classic_double_at_the_default()
    {
        assert_eq!(present_scale(320, 200), 2);
    }

    #[test]
    fn present_scale_degrades_to_passthrough_at_or_above_the_target()
    {
        assert_eq!(present_scale(640, 400), 1);
        assert_eq!(present_scale(1366, 768), 1);
        assert_eq!(present_scale(4096, 4096), 1);
    }

    #[test]
    fn present_scale_picks_the_limiting_axis()
    {
        // 320x200 fits twice horizontally but only once vertically at the
        // 400-row target: the limiting axis is y.
        assert_eq!(present_scale(320, 400), 1);
        assert_eq!(present_scale(640, 200), 1);
    }
}
