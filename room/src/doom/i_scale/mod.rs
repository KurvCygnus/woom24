//! Rust port of vendor/doomgeneric/i_scale.c.
//!
//! Screen scale-up code: integer pixel-doubling (1x..5x) and aspect-ratio
//! correcting stretch/squash drivers that present Doom's 320x200 paletted
//! framebuffer at a 4:3 physical aspect.
//!
//! ## Unwired upstream parity (dormancy note)
//!
//! This module is an unwired upstream-parity surface: **nothing in
//! `i_video` references it**. The live present path (`I_FinishUpdate`)
//! upscales via `fb_scaling` directly and `I_InitGraphics` computes the
//! factor manually, so upstream's `screen_mode_t` selection machinery is
//! not routed through this port. The only non-test consumer is `c_ffi`
//! (the `screen_mode_t` type and the fifteen `mode_*` re-exports), feeding
//! `c_tests/i_scale_c.rs`. Do not "wire it in" by accident: connecting it
//! to the present path is an explicit design decision, not a cleanup.
//!
//! ## Submodule Responsibility
//!
//! - `blend.rs` -- the palette blend machinery: `stdout_stream`, the
//!   `stretch_tables` / `half_stretch_table` statics, `find_nearest_color`,
//!   `generate_stretch_table`, the lazy `init_stretch_tables` /
//!   `init_squash_table` callbacks, and `reset_scale_tables`
//! - `drivers.rs` -- the frame statics (`src_buffer` / `dest_buffer` /
//!   `dest_pitch`), `init_scale_buffers`, the `scale_nx` generic, the
//!   pixel-doubling / stretch / squash driver functions, the line-writer
//!   helpers, the `draw_pixel*!` macros, and the fifteen `mode_*` statics
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! Statics keep names throughout: `src_buffer`, `dest_buffer`,
//! `dest_pitch`, `stretch_tables`, `half_stretch_table`, and the fifteen
//! exported `mode_*` `screen_mode_t` statics (re-exported by `c_ffi` and
//! layout/dimension-pinned by `c_tests/i_scale_c.rs`). Private types and
//! macros keep names. Functions:
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `stdout_placeholder` (port helper) | `blend::stdout_stream` | glue | Windows/wasm `stdout` placeholder for the progress flushes; private, Rust-only name |
//! | `I_InitScale` | `drivers::init_scale_buffers` | glue | stashes src/dest/pitch into the frame statics; C symbol pinned via `#[export_name]` (dead-but-exported: zero in-tree extern declarers, kept for wasm symbol-set parity); upstream `vendor/doomgeneric/i_scale.c:61` |
//! | `i_scale_1x`..`i_scale_5x` | `drivers::scale_draw_1x`..`scale_draw_5x` | glue | pixel-doubling drivers (1x has its own byte-copy body; 2x-5x delegate to `scale_nx::<N>`); upstream `i_scale.c:75-240` |
//! | `scale_nx::<N>` (Rust factoring) | `drivers::scale_nx` | glue | name kept; the const-generic refactor already diverged structurally from the C unrolled bodies (port note, `i_scale.c:507-924`) |
//! | `write_line_nx::<N>` | `drivers::write_hexpand_line` | glue | full-width N-pixel horizontal expansion row; upstream `WriteLine2x..5x` |
//! | `write_blended_line_nx::<N>` | `drivers::write_blended_hexpand_line` | glue | blended + expanded row; upstream `WriteBlendedLine2x..4x` |
//! | `write_blended_line_1x` | `drivers::write_blended_line` | glue | 1x blend row (no expansion); upstream `WriteBlendedLine1x` (`i_scale.c:434`) |
//! | `find_nearest_color` | `blend::find_nearest_color` | glue | name kept (already plain); upstream `i_scale.c:295` |
//! | `generate_stretch_table` | `blend::generate_stretch_table` | glue | name kept; upstream `i_scale.c:332` |
//! | `i_init_stretch_tables` | `blend::init_stretch_tables` | glue | `init_mode` callback; upstream `I_InitStretchTables` (`i_scale.c:361`) |
//! | `i_init_squash_table` | `blend::init_squash_table` | glue | `init_mode` callback; upstream `I_InitSquashTable` (`i_scale.c:387`) |
//! | `I_ResetScaleTables` | `blend::reset_scale_tables` | glue | frees + regenerates the populated tables after a palette switch; C symbol pinned via `#[export_name]` (wasm symbol-set parity); upstream `i_scale.c:404` |
//! | `i_stretch_1x`..`i_stretch_5x` | `drivers::stretch_draw_1x`..`stretch_draw_5x` | glue | aspect-correcting stretch drivers (full-screen updates only); upstream `I_Stretch1x..5x` (`i_scale.c:450-942`) |
//! | `write_squashed_line_1x`..`5x` | `drivers::write_squashed_line_1x`..`5x` | glue | names kept (already plain); upstream `WriteSquashedLine1x..5x` |
//! | `i_squash_1x`..`i_squash_5x` | `drivers::squash_draw_1x`..`squash_draw_5x` | glue | aspect-correcting squash drivers; upstream `I_Squash1x..5x` |
//! | `draw_pixel2!`..`draw_pixel5!` | `drivers::` | glue | private `macro_rules!` kept (translations of the C pixel macros) |
//!
//! The pointer-arithmetic parity notes (`i_scale.c` port notes, raw
//! `*mut u8` byte indexing into `src_buffer`/`dest_buffer`) are load-bearing
//! and stay carried on the driver docs. `M_CheckParm` / `Z_Malloc` /
//! `Z_Free` / `c_printf` are consumed by path through their upstream-name
//! shims (`m_argv`, `z_zone`, `crt`) -- unaffected by this graduation.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: every function is render output glue -- the drivers
//! write present-side framebuffer bytes and never read or write simulation
//! state. Given identical source buffers, palettes, and mode selection the
//! output bytes are identical on every host and target; the blend tables
//! are pure integer functions of the palette. The module is dormant
//! (unwired upstream parity, see above), which additionally guarantees it
//! cannot reach a tic's observables in the current wiring.

pub mod blend;
pub mod drivers;

//* path-stability re-export: the fifteen mode descriptors keep their
//* module-root paths (`c_ffi.rs` re-exports them to `c_tests/i_scale_c.rs`).
pub use drivers::{
    mode_scale_1x, mode_scale_2x, mode_scale_3x, mode_scale_4x, mode_scale_5x, mode_squash_1x,
    mode_squash_2x, mode_squash_3x, mode_squash_4x, mode_squash_5x, mode_stretch_1x,
    mode_stretch_2x, mode_stretch_3x, mode_stretch_4x, mode_stretch_5x,
};

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use blend::reset_scale_tables as I_ResetScaleTables;
pub use drivers::init_scale_buffers as I_InitScale;
