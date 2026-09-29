//! Rust port of vendor/doomgeneric/i_video.c.
//!
//! Video output shim: allocates the palette-indexed `I_VideoBuffer`,
//! expands it through the gamma-corrected palette into BGRA
//! `DG_ScreenBuffer` on `finish_update`, and forwards calls to the
//! platform layer.
//!
//! The C source supports two compile-time paths: an 8-bit `CMAP256`
//! direct-blit and a 16/32-bit framebuffer path. Only the latter is
//! ported here. The `s_Fb` structure tracks pixel layout chosen at
//! init time via `-gfxmode rgba8888|rgb565`; `cmap_to_framebuffer` /
//! `cmap_to_rgb565` perform the conversion from palette indices to
//! the chosen format. Many of the stub entry points
//! (`begin_read`, `bind_video_variables`, ...) exist purely to
//! satisfy the engine's call sites without doing any work on the
//! generic platform.
//!
//! ## Submodule Responsibility
//!
//! - `buffer.rs` -- the exported video statics, the `s_Fb` layout
//!   descriptor, and the buffer lifecycle (`init_graphics`,
//!   `reinit_video_buffer`, `shutdown_graphics`)
//! - `palette.rs` -- the `Color` entry type, the gamma-corrected `COLORS`
//!   table, `set_palette`, `get_palette_index`, and their tests
//! - `present.rs` -- the `cmap_to_*` converters and the per-frame /
//!   per-tic raster hooks (`finish_update`, `read_screen`, `start_frame`,
//!   `start_tic`, `update_no_blit`)
//! - `stubs.rs` -- the nine no-op ABI hooks and the `I_Video_Link_Anchor`
//!   link anchor
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `cmap_to_fb` | `present::cmap_to_framebuffer` | glue | private in C too; 16-bpp dispatch + 32-bpp packing honouring `fb_scaling` |
//! | `cmap_to_rgb565` | `present::cmap_to_rgb565` | glue | private, name kept (already snake) |
//! | `I_InitGraphics` | `buffer::init_graphics` | glue | `-gfxmode` / `-scaling` parsing, buffer allocation, `I_InitInput`; C symbol pinned via `#[export_name]`; upstream `i_video.c:205` |
//! | `reinit_video_buffer` (F1 M2 Rust-only) | `buffer::reinit_video_buffer` | glue | name kept (already house style); path-called by `video_cfg::apply` |
//! | `I_ShutdownGraphics` | `buffer::shutdown_graphics` | glue | zone free + null the buffer; C symbol pinned; upstream `i_video.c:298` |
//! | `I_StartFrame` | `present::start_frame` | glue | no-op; C symbol pinned (dead-but-exported); upstream `i_video.c:303` |
//! | `I_StartTic` | `present::start_tic` | glue | pumps `I_GetEvent`; C symbol pinned (`d_loop/mod.rs` extern-declares it); upstream `i_video.c:308` |
//! | `I_UpdateNoBlit` | `present::update_no_blit` | glue | no-op; C symbol pinned; upstream `i_video.c:313` |
//! | `I_FinishUpdate` | `present::finish_update` | glue | centring pad clamps at 0 (`.max(0)`) where C would wrap unsigned -- documented deviation, render-side only; C symbol pinned; upstream `i_video.c:321` |
//! | `I_ReadScreen` | `present::read_screen` | glue | wipe/melt screen copy; C symbol pinned (`f_wipe/mod.rs` extern-declares it); upstream `i_video.c:375` |
//! | `I_SetPalette` | `palette::set_palette` | glue | gamma-corrected PLAYPAL load; C symbol pinned; upstream `i_video.c:388` |
//! | `I_GetPaletteIndex` | `palette::get_palette_index` | glue | nearest-RGB probe, `i64` diff accumulator; C symbol pinned (baseline-tested); upstream `i_video.c:424` |
//! | `I_BeginRead` / `I_EndRead` | `stubs::{begin_read, end_read}` | glue | no-op SDL-era hooks; C symbols pinned (dead-but-exported); upstream `i_video.c:460/464` |
//! | `I_SetWindowTitle` | `stubs::set_window_title` | glue | forwards to `DG_SetWindowTitle`; C symbol pinned; upstream `i_video.c:468` |
//! | `I_GraphicsCheckCommandLine` | `stubs::graphics_check_command_line` | glue | no-op; C symbol pinned; upstream `i_video.c:473` |
//! | `I_SetGrabMouseCallback` | `stubs::set_grab_mouse_callback` | glue | no-op; C symbol pinned; upstream `i_video.c:477` |
//! | `I_EnableLoadingDisk` | `stubs::enable_loading_disk` | glue | no-op; C symbol pinned; upstream `i_video.c:481` |
//! | `I_BindVideoVariables` | `stubs::bind_video_variables` | glue | no-op; C symbol pinned (`d_main/bind.rs` calls through the root shim); upstream `i_video.c:485` |
//! | `I_DisplayFPSDots` | `stubs::display_fps_dots` | glue | no-op; C symbol pinned; upstream `i_video.c:489` |
//! | `I_CheckIsScreensaver` | `stubs::check_is_screensaver` | glue | no-op; C symbol pinned; upstream `i_video.c:493` |
//! | `I_Video_Link_Anchor` (Rust-only) | `stubs::I_Video_Link_Anchor` | glue | link anchor kept verbatim (name and body); retires with the freeze zone |
//! | `FB_BitField` / `FB_ScreenInfo` (C types) | `buffer::{FbBitField, FbScreenInfo}` | data | private types renamed to plain English at graduation |
//! | `struct color` (C type) | `palette::Color` | data | private; BGRA pixel entry |
//! | video statics | `buffer` | data | `I_VideoBuffer`, `screenvisible`, `screensaver_mode`, `usegamma`, `mouse_acceleration`, `mouse_threshold`, `fb_scaling`, `usemouse` keep names + `#[no_mangle]` (`I_VideoBuffer` is extern-declared by `f_wipe` / `r_draw`) |
//! | `DG_DrawFrame` / `DG_SetWindowTitle` externs | `present` / `stubs` | -- | reverse platform contract (shell-implemented), carried verbatim beside their callers |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: the framebuffer bytes are
//! present-side state and the raster hooks run between tics, not inside
//! them. The runtime reconfiguration consumers are `video_cfg::apply`
//! and the automap reseat (F1 M2), both render-side; the simulation
//! never reads back anything this module writes. The two documented
//! deviations (`.max(0)` padding clamp, non-16/32-bpp silent fallback)
//! are carried verbatim from the flat file and must not be "cleaned up".

pub mod buffer;
pub mod palette;
pub mod present;
pub mod stubs;

//* path-stability re-export: the raster dimensions stay runtime values
//* owned by `video_cfg`, re-exported here exactly as before the split
//* (F1 M2) so every `use crate::doom::i_video::SCREENWIDTH` import keeps
//* working.
pub use crate::doom::video_cfg::{SCREENHEIGHT, SCREENWIDTH};

//* path-stability re-export: the buffer pointer keeps its module-root
//* path (`am_map`, `video_cfg`; extern-declared by `f_wipe`, `r_draw`).
pub use buffer::I_VideoBuffer;

//* path-stability re-export: the F1 M2 reconfiguration entry keeps its
//* module-root path (`video_cfg.rs` calls it; name never renamed).
pub use buffer::reinit_video_buffer;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use buffer::{
    init_graphics as I_InitGraphics, shutdown_graphics as I_ShutdownGraphics,
};
pub use palette::{get_palette_index as I_GetPaletteIndex, set_palette as I_SetPalette};
pub use present::{
    finish_update as I_FinishUpdate, read_screen as I_ReadScreen, start_frame as I_StartFrame,
    start_tic as I_StartTic, update_no_blit as I_UpdateNoBlit,
};
pub use stubs::{
    begin_read as I_BeginRead, bind_video_variables as I_BindVideoVariables,
    check_is_screensaver as I_CheckIsScreensaver, display_fps_dots as I_DisplayFPSDots,
    enable_loading_disk as I_EnableLoadingDisk, end_read as I_EndRead,
    graphics_check_command_line as I_GraphicsCheckCommandLine,
    set_grab_mouse_callback as I_SetGrabMouseCallback, set_window_title as I_SetWindowTitle,
};

//* path-stability re-export: the link anchor keeps its module-root path
//* (the name was never renamed, so this is wiring, not a shim).
pub use stubs::I_Video_Link_Anchor;

//* path-stability re-export: the video statics keep their module-root
//* paths (`d_main/boot.rs` reads `screensaver_mode`; `d_main/bind.rs`
//* binds the config cvars).
pub use buffer::{
    mouse_acceleration, mouse_threshold, screenvisible, screensaver_mode, usegamma, usemouse,
    fb_scaling,
};
