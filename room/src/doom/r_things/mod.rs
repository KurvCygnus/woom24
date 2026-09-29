//! Sprite rendering for the Doom software renderer.
//!
//! Rust port of `vendor/doomgeneric/r_things.c`. Handles projection of map
//! objects (things) into screen-space `vissprite_t` records, depth-sorting
//! them back-to-front (painter's algorithm), clipping each sprite against
//! floor/ceiling silhouettes from the BSP drawseg list, and drawing masked
//! columns for both world sprites and player weapon (psprite) overlays.
//!
//! ## Submodule Responsibility
//!
//! - `state.rs` -- the shared constants, `spritedef_t`, all 13
//!   `#[no_mangle]` statics, and the private vissprite pool
//! - `spriteinit.rs` -- `install_sprite_lump` (private), `init_sprite_defs`
//!   (private), `init_sprites` (the map-load entry point)
//! - `project.rs` -- `clear_sprites`, `new_vis_sprite`, `project_sprite`
//!   (reads `r_interp` samples), `add_sprites` (the `r_bsp`-invoked pass)
//! - `draw.rs` -- `draw_masked_column`, `draw_vis_sprite`, `draw_psprite`,
//!   `draw_player_sprites`, `sort_vis_sprites`, `draw_sprite`,
//!   `draw_masked`, and the module-local `patch_t`/`column_t` copies
//! - `anchor.rs` -- the module link anchor (not wired from
//!   `doomgeneric.rs`; pre-move absence preserved)
//!
//! The module root is documentation + wiring only. Consumers keep their
//! upstream identifiers through this root: `r_main` resets the pool and
//! closes the frame with `R_ClearSprites`/`R_DrawMasked`, `r_bsp` collects
//! per-sector sprites via `R_AddSprites`, `r_segs` drives
//! `R_DrawMaskedColumn` for masked mid-textures and reads
//! `spryscale`/`mfloorclip`/`mceilingclip`/`sprtopscreen` mid-loop,
//! `p_setup/level` calls `R_InitSprites` at map load, `r_data` and
//! `f_finale` read `numsprites`/`sprites`, and `c_ffi`'s re-export block
//! feeds `c_tests/r_things_c.rs`.
//!
//! # Rust-vs-C differences
//!
//! - `MAXVISSPRITES = 128` pool with silent overflow into
//!   `overflowsprite`: vanilla behavior replicated as-is (dropped sprites
//!   are observable in frames). Not a bug emulation; no
//!   `docs/vanilla-workarounds.md` row.
//! - The `MF_TRANSLATION` colormap arithmetic
//!   (`colormaps.sub(256).add(...)`) in `draw_vis_sprite` is
//!   exactness-bearing; carried verbatim.
//! - `draw_sprite`'s function-local `static mut CLIPBOT`/`CLIPTOP` are
//!   per-frame scratch shared across invocations -- carried verbatim; do
//!   not convert to stack arrays (size `MAXW`).
//! - `newvissprite` is a dead counter retained for ABI parity with the C
//!   original (zero reads; kept verbatim).
//! - `patch_t`/`column_t` are deliberate module-local copies (packed WAD
//!   layout); never unify with the `v_video`/`c_ffi` copies.
//! - No module link anchor is wired from `doomgeneric.rs`'s anchor list --
//!   absence preserved (`R_Things_Link_Anchor` itself is kept for symbol
//!   parity).
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
//! comes with freeze-zone retirement). Unsafe-signature parity: every
//! function was `pub unsafe extern "C"` (or private `unsafe fn`)
//! pre-split and stays so.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `R_InstallSpriteLump` (C static) | `spriteinit::install_sprite_lump` | glue | load-time validation; the `i_error!` arms are load-bearing (boot, not demo sync); private before and after (doc alias only) |
//! | `R_InitSpriteDefs` (C static) | `spriteinit::init_sprite_defs` | glue | WAD lump scan + rotation-completeness ladder; private before and after (doc alias only) |
//! | `R_InitSprites` | `spriteinit::init_sprites` | glue | map-load entry (`p_setup/level.rs`); negonearray -1 fill; shim + pin |
//! | `R_ClearSprites` | `project::clear_sprites` | glue | per-frame pool reset (`r_main` frame sequence); shim + pin |
//! | `R_NewVisSprite` | `project::new_vis_sprite` | glue | pool claim with silent `overflowsprite` fallback (vanilla cap); shim + pin |
//! | `R_DrawMaskedColumn` | `draw::draw_masked_column` | glue | post-walk clip + `colfunc` dispatch; called from `r_segs` by path (no extern declarer); pin for export parity |
//! | `R_DrawVisSprite` | `draw::draw_vis_sprite` | glue | colfunc override ladder (fuzz/trans/base); MF_TRANSLATION arithmetic exactness-bearing; shim + pin |
//! | `R_ProjectSprite` | `project::project_sprite` | glue | frame-golden; reads `r_interp::sample_mobj` (F1 M1 call point kept exactly); shim + pin |
//! | `R_AddSprites` | `project::add_sprites` | glue | per-sector pass (`r_bsp.rs:588`); validcount skip is the cross-module contract; shim + pin |
//! | `R_DrawPSprite` | `draw::draw_psprite` | glue | HUD-space weapon overlay; reads `r_interp::sample_psp` (F1 M1 call point kept exactly); shim + pin |
//! | `R_DrawPlayerSprites` | `draw::draw_player_sprites` | glue | light band + clip-array defaults; shim + pin |
//! | `R_SortVisSprites` | `draw::sort_vis_sprites` | glue | O(n^2) selection sort must stay exact -- draw order is frame-visible; shim + pin |
//! | `R_DrawSprite` | `draw::draw_sprite` | glue | drawseg silhouette clip; CLIPBOT/CLIPTOP function-local statics stay inline; shim + pin |
//! | `R_DrawMasked` | `draw::draw_masked` | glue | end-of-frame pass: sort, sprite loop, remaining mid-textures, psprites; shim + pin |
//! | `R_Things_Link_Anchor` (house) | `anchor::R_Things_Link_Anchor` | house | not called from `doomgeneric.rs`'s anchor list -- absence preserved; kept for symbol parity |
//! | 13 `#[no_mangle]` statics (`pspritescale`..`sprtopscreen`) | `state` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (`c_ffi`'s re-export block, `r_main`/`r_segs`/`r_bsp`/`r_plane`/`r_data`, `f_finale`, `p_setup/level`) |
//! | private pool `vissprites`/`vissprite_p`/`newvissprite`/`overflowsprite`/`vsprsortedhead` | `state` | data | `pub(super)` shared between `project` and `draw` via `super::` (the pool/scratch contract) |
//! | `spritedef_t`, 11 constants | `state` | data | `pub(super)` beside their readers; `patch_t`/`column_t` stay module-local in `draw.rs` (deliberate copies) |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: sprite selection, clipping, and draw order are visible
//! in every frame golden but never write sim state. `project_sprite` READS
//! sim state (mobjs) and `r_interp` samples, but its outputs land only in
//! vissprites. The sprite-definition build (`install_sprite_lump`/
//! `init_sprite_defs`) is load-time validation whose `i_error!` arms affect
//! boot, not demo sync. Mover-census interpolation is `r_interp`'s
//! contract, not this module's. Pinned by the F9 frame goldens
//! (`video_anchor.rs`, scenario `frame_hash`, the sprite probe/regression
//! binaries); the harness state ledger never reads renderer state.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod anchor;
pub mod draw;
pub mod project;
pub mod spriteinit;
pub mod state;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_things::R_*`: r_main's frame sequence,
//* r_bsp's per-sector pass, r_segs' masked-column driver, p_setup's
//* map-load init). The C symbol each shim forwards to is re-pinned at
//* the definition with `#[export_name = "OriginalName"]`, so the
//* wasm/extern symbol name set stays byte-identical to the pre-split
//* module. There are no extern-by-symbol declarers for r_things
//* functions and no C referencers in the default build, so the pins
//* are wasm-surface conservatism. Shims die with the freeze zone.
pub use anchor::R_Things_Link_Anchor;
pub use draw::{
    draw_masked as R_DrawMasked, draw_masked_column as R_DrawMaskedColumn,
    draw_player_sprites as R_DrawPlayerSprites, draw_psprite as R_DrawPSprite,
    draw_sprite as R_DrawSprite, draw_vis_sprite as R_DrawVisSprite,
    sort_vis_sprites as R_SortVisSprites,
};
pub use project::{
    add_sprites as R_AddSprites, clear_sprites as R_ClearSprites,
    new_vis_sprite as R_NewVisSprite, project_sprite as R_ProjectSprite,
};
pub use spriteinit::init_sprites as R_InitSprites;

//* path-stability re-export: the 13 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* `c_ffi.rs:856-859`'s re-export block (feeding `c_tests/r_things_c.rs`),
//* `r_main` (psprite scales + screenheightarray), `r_segs` (the masked-seg
//* clip relay: spryscale/mfloorclip/mceilingclip/sprtopscreen +
//* negonearray/screenheightarray), `r_bsp`/`r_plane` (pspriteiscale),
//* `r_data`/`f_finale` (sprites/numsprites), `p_setup/level` (sprite names).
pub use state::{
    mceilingclip, mfloorclip, maxframe, negonearray, numsprites, pspriteiscale, pspritescale,
    screenheightarray, spritelights, spritename, sprites, sprtemp, sprtopscreen, spryscale,
};
