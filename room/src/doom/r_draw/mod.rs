//! The span/column drawing functions.
//!
//! Rust port of `vendor/doomgeneric/r_draw.c`. All drawing to the view
//! buffer is accomplished in this module; the other refresh files only know
//! about coordinates, not the architecture of the frame buffer.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the LUT-sizing `MAXWIDTH`/`MAXHEIGHT` constants and all
//!   31 `#[no_mangle]` statics (`viewimage`..`dscount`, `fuzz*`, `dc_*`,
//!   `ds_*`, `translations`, `translationtables`, `ylookup`, `columnofs`)
//!   with their verbatim docs, plus the `fuzzoffset` table tests
//! - `column.rs` -- `draw_column`/`draw_column_low`, `draw_fuzz_column`/
//!   `draw_fuzz_column_low`, `draw_translated_column`/
//!   `draw_translated_column_low`, and their tests
//! - `span.rs` -- `draw_span`/`draw_span_low` and their tests
//! - `backscreen.rs` -- `init_buffer`, `fill_back_screen`, `video_erase`,
//!   `draw_view_border`, the module-local `background_buffer` pair, the
//!   `background_buffer_bytes` test accessor, the private `deh_string`
//!   shim, and the init/border/erase tests
//! - `translate.rs` -- `init_translation_tables` and its test
//!
//! The module root is documentation + wiring only. Consumers keep their
//! upstream identifiers through this root: `r_main`'s fn-pointer statics
//! (`colfunc`/`basecolfunc`/`fuzzcolfunc`/`transcolfunc`/`spanfunc`) are
//! assigned the renderer paths from `R_ExecuteSetViewSize` (fn-pointer
//! values through the shims, not symbols), `r_segs`/`r_plane`/`r_things`
//! write the `dc_*`/`ds_*` per-column/per-span context, `r_bsp` reads
//! `viewwidth`, `d_main/display` and `g_game/savegentry` drive the bezel
//! pass, `hu_lib` extern-declares `R_VideoErase` and the view-window
//! statics, and `c_ffi`'s extern block + re-export block feed
//! `c_tests/r_draw_c.rs`.
//!
//! # Rust-vs-C differences
//!
//! - **FUZZOFF direction-unit table**: vanilla bakes `FUZZOFF = SCREENWIDTH`
//!   (320) into `fuzzoffset`; F1 M2 keeps ±1 direction units scaled by the
//!   runtime stride at the use site (crispy `r_draw.c:409` precedent).
//!   Frame-exact at any raster.
//! - **`background_buffer` live-size re-allocation**: crispy pre-allocates
//!   `MAXWIDTH*(MAXHEIGHT-SBARHEIGHT)` (~16 MiB -- infeasible in the zone);
//!   `fill_back_screen` size-checks and re-allocates instead;
//!   `background_buffer_bytes` is the test accessor.
//! - **`R_DrawSpanLow` C divergence** (FIXME carried at the definition):
//!   C mutates `ds_x1`/`ds_x2` in place (`ds_x1 <<= 1`); the Rust uses a
//!   local `ds_x1_low` and leaves the globals unchanged -- a known
//!   observable divergence from C for post-call readers; no in-tree reader
//!   is affected. Do not "fix" during refactors.
//! - `viewimage`, `translations`, `dccount`, `dscount` are
//!   dead-but-exported (zero consumers in the tree; C parity): kept for
//!   symbol-set byte-identity, retire with the freeze zone.
//! - No module link anchor exists today -- absence preserved.
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
//! comes with freeze-zone retirement). All thirteen were safe
//! `extern "C"` pre-split and stay safe-bodied (parity with today, not
//! with C).
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `R_DrawColumn` | `column::draw_column` | glue | THE wall-column rasterizer; stride hoist (F1 M2) load-bearing; also assigned as fn-pointer value by `r_main::R_ExecuteSetViewSize` through the shim; shim + pin |
//! | `R_DrawColumnLow` | `column::draw_column_low` | glue | blocky 2-column variant; shim + pin |
//! | `R_DrawFuzzColumn` | `column::draw_fuzz_column` | glue | spectre effect; `fuzzpos` advances (frame-golden visible, never sim state); shim + pin |
//! | `R_DrawFuzzColumnLow` | `column::draw_fuzz_column_low` | glue | shim + pin |
//! | `R_DrawTranslatedColumn` | `column::draw_translated_column` | glue | player-color translation; shim + pin |
//! | `R_DrawTranslatedColumnLow` | `column::draw_translated_column_low` | glue | shim + pin |
//! | `R_DrawSpan` | `span::draw_span` | glue | flat-span rasterizer; packed u32 position walk; shim + pin |
//! | `R_DrawSpanLow` | `span::draw_span_low` | glue | carries the documented C `ds_x1`/`ds_x2` divergence (FIXME) verbatim; shim + pin |
//! | `R_InitBuffer` | `backscreen::init_buffer` | glue | `ylookup`/`columnofs` rebuild; extern-declared `c_ffi.rs:239` -- pin mandatory |
//! | `R_InitTranslationTables` | `translate::init_translation_tables` | glue | 3 x 256 ramp remap; extern-declared `c_ffi.rs:242` -- pin mandatory |
//! | `R_FillBackScreen` | `backscreen::fill_back_screen` | glue | bezel pass with live-size buffer re-allocation; extern-declared `c_ffi.rs:245`; `g_game/savegentry.rs:28` calls it on load; shim + pin |
//! | `R_VideoErase` | `backscreen::video_erase` | glue | background-buffer erase blit; extern-declared TWICE (`c_ffi.rs:248` and `hu_lib.rs:156`) -- pin mandatory |
//! | `R_DrawViewBorder` | `backscreen::draw_view_border` | glue | per-frame border restore ladder; extern-declared `c_ffi.rs:251`; shim + pin |
//! | `background_buffer_bytes` (house) | `backscreen::background_buffer_bytes` | data | test accessor, already house-named; no pin (test-only); root re-export keeps `tests/video_raster.rs`'s full path resolving |
//! | private `DEH_String` shim | `backscreen::deh_string` | glue | identity shim used only by `fill_back_screen`; `#[doc(alias = "DEH_String")]` only (private) |
//! | 31 `#[no_mangle]` statics (`viewimage`..`dscount`) | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (`tests/video_raster.rs`, `c_ffi`'s re-export block, `r_main`/`r_segs`/`r_plane`/`r_things`/`r_bsp`, `hu_lib`'s extern statics, `d_main/display`) |
//! | private `background_buffer`/`background_buffer_size` | `backscreen` | data | stay beside their only writers (`fill_back_screen`/`video_erase`/`background_buffer_bytes`); tests reach the buffer via `super::` |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: render pipeline only. Nothing here resolves map-load or
//! simulation content; every function shapes the *presented frame* (the
//! column/span renderers, the bezel pass, the border restore blits) or
//! rebuilds render LUTs, pinned by the F9 frame goldens (`video_anchor.rs`
//! `0x841405eea75ee285`, scenario `frame_hash`). `fuzzpos` advances across
//! pixels and frames -- observable in the frame goldens, never in sim
//! state. The raster invariants are owned by `tests/video_raster.rs`; the
//! harness state ledger never reads renderer state.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod backscreen;
pub mod column;
pub mod span;
pub mod state;
pub mod translate;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_draw::R_*`: r_main's view-size fn-pointer
//* table, r_segs/r_plane/r_things context writes, d_main/display and
//* g_game/savegentry bezel passes, the video_raster sweep). The C symbol
//* each shim forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. Load-bearing pins
//* (extern-by-symbol declarers): `c_ffi.rs:239-251` (`R_InitBuffer`,
//* `R_InitTranslationTables`, `R_FillBackScreen`, `R_VideoErase`,
//* `R_DrawViewBorder`) and `hu_lib.rs:156` (`R_VideoErase`, a second
//* declarer). There are no C referencers in the default build, so the
//* remaining pins are wasm-surface conservatism. Shims die with the
//* freeze zone.
pub use backscreen::{
    background_buffer_bytes, draw_view_border as R_DrawViewBorder,
    fill_back_screen as R_FillBackScreen, init_buffer as R_InitBuffer,
    video_erase as R_VideoErase,
};
pub use column::{
    draw_column as R_DrawColumn, draw_column_low as R_DrawColumnLow,
    draw_fuzz_column as R_DrawFuzzColumn, draw_fuzz_column_low as R_DrawFuzzColumnLow,
    draw_translated_column as R_DrawTranslatedColumn,
    draw_translated_column_low as R_DrawTranslatedColumnLow,
};
pub use span::{draw_span as R_DrawSpan, draw_span_low as R_DrawSpanLow};
pub use translate::init_translation_tables as R_InitTranslationTables;

//* path-stability re-export: the 31 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* `tests/video_raster.rs`'s use block + full-path uses (`dc_translation`,
//* the renderers, `background_buffer_bytes`), `c_ffi.rs:842-848`'s
//* re-export block (feeding `c_tests/r_draw_c.rs`), `hu_lib.rs:160-166`'s
//* extern statics (`viewwindowx`/`viewwindowy`/`viewwidth`/`viewheight`),
//* and the per-column/per-span context writers in r_segs/r_plane/r_things.
pub use state::{
    columnofs, dc_colormap, dc_iscale, dc_source, dc_texturemid, dc_translation, dc_x, dc_yl,
    dc_yh, dccount, ds_colormap, ds_source, ds_x1, ds_x2, ds_xfrac, ds_xstep, ds_y, ds_yfrac,
    ds_ystep, dscount, fuzzoffset, fuzzpos, scaledviewwidth, translationtables, translations,
    viewheight, viewimage, viewwidth, viewwindowx, viewwindowy, ylookup,
};
