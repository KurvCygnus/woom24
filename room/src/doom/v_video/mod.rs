//! Video layer: patch drawing, block copies, dirty-box tracking, buffer
//! selection, and PCX screenshots.
//!
//! Rust port of `vendor/doomgeneric/v_video.c`.
//!
//! # Overview
//!
//! Every HUD/menu/automap/intermission image reaches the framebuffer through
//! this layer. [`crate::doom::v_video::patch::draw_patch`] blits a WAD
//! `patch_t` column-by-column into the current destination buffer; the
//! `blit` primitives copy blocks and track the dirty rectangle that the
//! present step uses; `screenshot` writes PCX files. Buffer selection is
//! explicit: `use_buffer`/`restore_buffer` aim the whole family at an
//! off-screen buffer (the r_draw bezel background, the status-bar refresh)
//! or back at the primary framebuffer.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the WAD patch vocabulary (`patch_t`, `post_t`,
//!   `column_t`, `vpatchclipfunc_t`), the three `#[no_mangle]` statics
//!   (`tinttable`, `xlatab`, `dirtybox`), and the private `dest_screen` /
//!   `patchclip_callback` pair
//! - `patch.rs` -- `set_patch_clip_callback`, `draw_patch`,
//!   `draw_patch_flipped`, `draw_patch_direct`, and the dead-but-exported
//!   `TL`/`XLA`/alt-TL/shadowed patch family
//! - `blit.rs` -- `mark_rect`, `copy_rect`, `draw_block`, the box/line
//!   drawers, `draw_raw_screen`, `init`, `use_buffer`, `restore_buffer`,
//!   and `retarget_after_framebuffer_swap`
//! - `screenshot.rs` -- `write_pcx_file`, `screen_shot`,
//!   `draw_mouse_speed_box`, the `TINTTAB`/`XLATAB` loaders, and the
//!   screenshot-filename unit tests
//! - `anchor.rs` -- the module link anchor (not wired from
//!   `doomgeneric.rs`; pre-move absence preserved)
//!
//! The module root is documentation + wiring only. Consumers keep their
//! upstream identifiers through this root: `am_map`/`f_finale`/`st_lib`/
//! `st_stuff`/`wi_stuff`/`m_menu`/`hu_*` draw patches and copies, `r_draw`
//! builds the bezel through `V_UseBuffer`/`V_DrawPatch`/`V_MarkRect`,
//! `f_wipe` reaches `V_DrawBlock`/`V_MarkRect` through its extern block,
//! `g_game/ticker` takes F-key screenshots, and `video_cfg` retargets
//! `dest_screen` across framebuffer swaps.
//!
//! # Rust-vs-C differences
//!
//! - `draw_patch_direct` is an identity delegation to [`crate::doom::v_video::patch::draw_patch`]
//!   (`V_DrawPatchDirect` == `V_DrawPatch`), matching upstream chocolate
//!   behavior. Kept as a separate symbol; do not merge the call sites.
//! - The variable-length patch `columnofs` array is read via
//!   `read_unaligned` pointer arithmetic (packed-struct idiom) -- kept
//!   verbatim.
//! - `dirtybox` is written here through the `M_AddToBox` protocol and
//!   consumed by the `i_video` present side; the contract stays split
//!   across the two modules exactly as upstream.
//! - The translucent patch family (`draw_tl_patch`, `draw_xla_patch`,
//!   `draw_alt_tl_patch`, `draw_shadowed_patch`), `draw_raw_screen`, and
//!   the `TINTTAB`/`XLATAB` loaders are dead-but-exported (zero callers in
//!   the tree): kept for symbol-set byte-identity, retire with the freeze
//!   zone.
//! - Upstream `WritePNGfile` was never ported -- recorded in the mapping
//!   table below for completeness.
//! - `retarget_after_framebuffer_swap` is the `video_cfg` contract: after a
//!   framebuffer swap only a `dest_screen` that still points at the freed
//!   primary follows to the new one; a `use_buffer`-selected off-screen
//!   buffer survives untouched.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held by
//! the `upstream-name shim` re-exports at this root, and every former
//! `#[no_mangle]` C symbol is re-pinned with
//! `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//! set is byte-identical to the pre-split module. Functions only:
//! statics/consts/tables keep their upstream names (data-tier renaming
//! comes with freeze-zone retirement).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `V_MarkRect` | `blit::mark_rect` | glue | dirty-box growth; extern-declared by `f_wipe.rs:39` -- pin mandatory; upstream `v_video.c:69` |
//! | `V_CopyRect` | `blit::copy_rect` | glue | status-bar background copy (`st_lib`/`st_stuff`); shim + pin |
//! | `V_SetPatchClipCallback` | `patch::set_patch_clip_callback` | glue | automap installs its clipper; shim + pin |
//! | `V_DrawPatch` | `patch::draw_patch` | glue | THE patch blitter (~12 consumers); `read_unaligned` column walk verbatim; shim + pin |
//! | `V_DrawPatchFlipped` | `patch::draw_patch_flipped` | glue | `f_finale` only; shim + pin |
//! | `V_DrawPatchDirect` | `patch::draw_patch_direct` | glue | identity delegation to `draw_patch` (see Rust-vs-C); extern-declared by `hu_lib.rs:153` -- pin mandatory |
//! | `V_DrawTLPatch` | `patch::draw_tl_patch` | glue | dead-but-exported; shim + pin |
//! | `V_DrawXlaPatch` | `patch::draw_xla_patch` | glue | dead-but-exported; shim + pin |
//! | `V_DrawAltTLPatch` | `patch::draw_alt_tl_patch` | glue | dead-but-exported; shim + pin |
//! | `V_DrawShadowedPatch` | `patch::draw_shadowed_patch` | glue | dead-but-exported; shim + pin |
//! | `V_LoadTintTable` | `screenshot::load_tint_table` | glue | dead-but-exported loader (boot never calls it in-tree); shim + pin |
//! | `V_LoadXlaTable` | `screenshot::load_xla_table` | glue | dead-but-exported loader; shim + pin |
//! | `V_DrawBlock` | `blit::draw_block` | glue | wipe melt start frame; extern-declared by `f_wipe.rs:37` -- pin mandatory |
//! | `V_DrawFilledBox` | `blit::draw_filled_box` | glue | only caller `draw_mouse_speed_box`; shim + pin |
//! | `V_DrawHorizLine` | `blit::draw_horiz_line` | glue | shim + pin |
//! | `V_DrawVertLine` | `blit::draw_vert_line` | glue | shim + pin |
//! | `V_DrawBox` | `blit::draw_box` | glue | shim + pin |
//! | `V_DrawRawScreen` | `blit::draw_raw_screen` | glue | dead-but-exported; shim + pin |
//! | `V_Init` | `blit::init` | glue | no-op; boot calls it (`d_main/boot.rs:49`); shim + pin |
//! | `V_UseBuffer` | `blit::use_buffer` | glue | off-screen selection (r_draw bezel, st_stuff refresh); shim + pin |
//! | `V_RestoreBuffer` | `blit::restore_buffer` | glue | shim + pin |
//! | `WritePCXfile` | `screenshot::write_pcx_file` | glue | host-I/O; only caller `screen_shot`; shim + pin |
//! | `V_ScreenShot` | `screenshot::screen_shot` | glue | F-key path (`g_game/ticker.rs:115`); shim + pin |
//! | `V_DrawMouseSpeedBox` | `screenshot::draw_mouse_speed_box` | glue | `testcontrols` HUD (`d_main/display.rs:136`); shim + pin |
//! | `WritePNGfile` | -- | -- | not ported (no mapping row needed; recorded for completeness) |
//! | `V_Video_Link_Anchor` (house) | `anchor::V_Video_Link_Anchor` | house | not called from `doomgeneric.rs`'s anchor list -- absence preserved; the anchor itself is dead in-tree, kept for symbol parity |
//! | `tinttable`, `xlatab`, `dirtybox` | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below |
//! | private `dest_screen`, `patchclip_callback` | `state` | data | `pub(super)` beside their only writers (`blit`/`patch`) |
//! | `patch_t`, `post_t`, `column_t`, `vpatchclipfunc_t` | `state` | data | carried verbatim; root re-exports keep the 12+ freeze-zone `crate::doom::v_video::patch_t` imports resolving |
//! | private `pcx_t`, `screenshot_filename`, `dummy_clip`, 2 mouse-box consts | `screenshot`/`patch` | data | carried verbatim beside their only readers |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: nothing here resolves map-load or simulation content.
//! Every patch blit, block copy, and box draw feeds the *presented frame*
//! (the F9 frame goldens: `video_anchor.rs`, scenario `frame_hash`), and
//! the dirty-box protocol feeds the present path's copy region. The
//! screenshot pair is host filesystem I/O gated behind the F-key --
//! unreachable during golden runs. `retarget_after_framebuffer_swap`
//! belongs to the `video_cfg` reconfiguration path, which re-arms the
//! vanilla resize; the raster sweep pins it.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod anchor;
pub mod blit;
pub mod patch;
pub mod screenshot;
pub mod state;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::v_video::V_*`: am_map/f_finale/st_*/wi_stuff/
//* m_menu/hu_* patch draws, r_draw's bezel pass, g_game's F-key shot,
//* d_main boot/display). The C symbol each shim forwards to is re-pinned
//* at the definition with `#[export_name = "OriginalName"]`, so the
//* wasm/extern symbol name set stays byte-identical to the pre-split
//* module. Load-bearing pins (extern-by-symbol declarers): `f_wipe.rs:37`
//* (`V_DrawBlock`), `f_wipe.rs:39` (`V_MarkRect`), `hu_lib.rs:153`
//* (`V_DrawPatchDirect`). There are no C referencers in the default
//* build, so the remaining pins are wasm-surface conservatism. Shims die
//* with the freeze zone.
pub use anchor::V_Video_Link_Anchor;
pub use blit::{
    copy_rect as V_CopyRect, draw_block as V_DrawBlock, draw_box as V_DrawBox,
    draw_filled_box as V_DrawFilledBox, draw_horiz_line as V_DrawHorizLine,
    draw_raw_screen as V_DrawRawScreen, draw_vert_line as V_DrawVertLine, init as V_Init,
    mark_rect as V_MarkRect, restore_buffer as V_RestoreBuffer, use_buffer as V_UseBuffer,
};
pub use patch::{
    draw_alt_tl_patch as V_DrawAltTLPatch, draw_patch as V_DrawPatch,
    draw_patch_direct as V_DrawPatchDirect, draw_patch_flipped as V_DrawPatchFlipped,
    draw_shadowed_patch as V_DrawShadowedPatch, draw_tl_patch as V_DrawTLPatch,
    draw_xla_patch as V_DrawXlaPatch, set_patch_clip_callback as V_SetPatchClipCallback,
};
pub use screenshot::{
    draw_mouse_speed_box as V_DrawMouseSpeedBox, load_tint_table as V_LoadTintTable,
    load_xla_table as V_LoadXlaTable, screen_shot as V_ScreenShot, write_pcx_file as WritePCXfile,
};

//* path-stability re-export: the three `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained) and the patch
//* type vocabulary stays root-visible (`am_map`, `d_main`, `f_finale`,
//* `hu_lib`, `hu_stuff`, `m_menu`, `r_draw`, `st_lib`, `st_stuff`,
//* `wi_stuff` all import `patch_t`/`column_t` by path).
pub use state::{column_t, dirtybox, patch_t, post_t, tinttable, vpatchclipfunc_t, xlatab};

//* crate-internal shim: `video_cfg` calls the retarget hook by its
//* house-name path (`crate::doom::v_video::retarget_after_framebuffer_swap`);
//* it lives in `blit` and is re-exported here for path stability.
pub(crate) use blit::retarget_after_framebuffer_swap;
