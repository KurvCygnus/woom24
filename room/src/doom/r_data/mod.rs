//! Texture, flat, sprite-lump, and colormap data loading, caching, and
//! lookup -- the Rust port of `vendor/doomgeneric/r_data.c`.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the data vocabulary: the packed on-disk `mappatch_t` /
//!   `maptexture_t` WAD records, the runtime `texture_t` / `texpatch_t`
//!   descriptors, the WAD `patch_t` / `post_t` (`column_t`) graphics
//!   headers, and the `spriteframe_t` / `spritedef_t` precache vocabulary
//! - `globals.rs` -- all 20 `#[no_mangle]` public statics, one data home
//! - `column_cache.rs` -- the frame-side hot half: the private lookup-table
//!   statics, `get_column` (the per-column hot path), the lazy multi-patch
//!   compositor, and the `le_i16` / `le_i32` byte-order helpers
//! - `init.rs` -- the startup half: PNAMES/TEXTURE1/TEXTURE2 parsing, flat
//!   and sprite lump ranges, the COLORMAP pointer, and the texture hash
//!   table builder
//! - `lookup.rs` -- the flat/texture name-resolution entry points the
//!   freeze-zone callers invoke
//! - `dtmc.rs` -- the extracted demo-synchronization surface
//!   (`texture_index_for_name`) with its baseline vectors
//! - `precache.rs` -- `precache_level`, the `p_setup`-invoked cache warmer
//! - `anchor.rs` -- the module link anchor (existed pre-split; carried)
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below. Every freeze-zone importer (`g_game`,
//! `p_setup`, `p_spec`, `p_switch`, `p_floor`, `f_finale`, the renderer,
//! `c_tests/r_data_c.rs`) still names the upstream identifiers through
//! this root.
//!
//! # Doom graphics model
//!
//! Doom wall and sprite graphics are stored as vertical runs of opaque pixels
//! called *posts*.  A *column* is zero or more posts; a *patch* (or sprite) is
//! zero or more columns.  A *texture* is a rectangular surface composed of one
//! or more patches composited together.
//!
//! # Composite texture cache
//!
//! When a texture is first needed, `generate_lookup` pre-computes per-column
//! metadata stored in `texturecolumnlump` and `texturecolumnofs`:
//! - If only one patch covers a column, `texturecolumnlump[tex][col]` points
//!   directly into that patch's WAD lump, and no composite is needed.
//! - If multiple patches overlap the column, `texturecolumnlump[tex][col]` is
//!   set to `-1` and the column must be composited into a heap buffer.
//!   `generate_composite` does that work on first use and caches the result.
//!
//! `R_GetColumn` is the hot path: it returns a pointer to the column data,
//! triggering composite generation as needed.
//!
//! # Globals exported to C
//!
//! Most `#[no_mangle]` statics in this module are declared `extern` in
//! `r_state.h` and consumed by multiple renderer and physics C files.
//! Exceptions: `lastflat` and `numflats` are only declared locally in
//! `p_spec.c`, not in `r_state.h`.
//!
//! # Rust-vs-C differences
//!
//! - `flatmemory` / `texturememory` / `spritememory` are write-only
//!   diagnostics in this port (zero readers in tree) -- kept for symbol
//!   parity.
//! - The `patch_t` / `post_t` / `column_t` graphics headers were `pub`
//!   before the split but had zero external consumers (`v_video` and
//!   `r_things` define their own copies -- deliberate mirrors, never
//!   unify across modules); the graduation tightens them to
//!   `pub(super)`.
//! - Vanilla quirks carried verbatim, none of them emulated tramples
//!   (no `docs/vanilla-workarounds.md` row): lump 0 is treated as a
//!   composite by `get_column` even though it is a valid WAD lump number
//!   (`column_cache.rs`); the vanilla >64 KiB composite-size `I_Error`
//!   limit is kept (`column_cache.rs`); duplicate texture names resolve
//!   first-insert-wins (lower index, `init.rs` hash builder).
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
//! | `R_DrawColumnInCache` (C static) | `column_cache::draw_column_in_cache` | glue | private before and after (doc alias only); post-walk clipping ladder moves bit-exact |
//! | `R_GenerateComposite` | `column_cache::generate_composite` | glue | lazy frame-side composite; zone-heap marshalling; shim + pin |
//! | `R_GenerateLookup` | `column_cache::generate_lookup` | glue | keeps the vanilla >64 KiB composite `I_Error` limit; shim + pin |
//! | `R_GetColumn` | `column_cache::get_column` | glue | per-column hot path; the lump-0-as-composite quirk moves verbatim; shim + pin |
//! | `GenerateTextureHashTable` (C static) | `init::generate_texture_hash_table` | data | startup; the duplicate-name tie-break (lower index wins) is load-bearing and pinned via the dtmc vectors; private before and after |
//! | `R_InitTextures` | `init::init_textures` | glue | startup orchestration + WAD parse ladders (PNAMES/TEXTURE1/TEXTURE2); shim + pin |
//! | `R_InitFlats` | `init::init_flats` | glue | shim + pin |
//! | `R_InitSpriteLumps` | `init::init_sprite_lumps` | glue | shim + pin |
//! | `R_InitColormaps` | `init::init_colormaps` | glue | shim + pin |
//! | `R_InitData` | `init::init_data` | glue | the init order IS the behavior; shim + pin |
//! | `R_FlatNumForName` | `lookup::flat_num_for_name` | dtmc (wrapper kept whole) | the result IS sim content (`F_SKY1` -> `skyflatnum` gates hitscan in `p_map`); the qualifying part is the single `i - firstflat` -- too trivial to extract; wrapper + `I_Error` path protected by the demo goldens end-to-end; shim + pin |
//! | `R_CheckTextureNumForName` | `lookup::check_texture_num_for_name` + **extracted `dtmc::texture_index_for_name`** | dtmc | the hash-chain walk + `'-'` no-texture rule + duplicate tie-break is the genuinely qualifying pure computation (map-load content resolution, p_setup-dtmc class); the `W_LumpNameHash % numtextures` bucket selection stays at the wrapper (static access); shim + pin |
//! | `R_TextureNumForName` | `lookup::texture_num_for_name` | dtmc (wrapper kept whole) | `I_Error` marshalling over the check fn; shim + pin |
//! | `R_PrecacheLevel` | `precache::precache_level` | glue | no-op during demo playback -- pure cache warming, zero frame/state effect on the golden path; the `acp1 == P_MobjThinker` pointer-compare thinker walk moves verbatim; shim + pin |
//! | `R_Data_Link_Anchor` (house) | `anchor::R_Data_Link_Anchor` | house | anchor EXISTS (unlike `r_sky`) -> name + `#[no_mangle]` carried, body re-pointed at the renamed fns through the root shims |
//! | `SHORT` / `LONG` (C macros, i_swap) | `column_cache::{le_i16, le_i32}` | data | private -> `pub(super)` (shared with `init`); doc aliases only; p_setup `le_i16` precedent |
//! | `DEH_String` | `init::deh_string` | glue | identity; private before and after; same house name as `g_game/responder::deh_string` |
//! | 20 `#[no_mangle]` statics (`firstflat`..`spritememory`) | `globals` | data | upstream names + `#[no_mangle]` retained; every root path held by the re-export block below (`c_tests/r_data_c.rs` reads seven of them through it; `p_spec` writes the two translation tables) |
//! | 7 private statics (`textures`..`texturecomposite`) | `column_cache` | data | `pub(super)` beside their readers/writers (`init` fills, `column_cache`/`lookup`/`dtmc` read) |
//! | `mappatch_t`, `maptexture_t`, `texpatch_t`, `texture_t`, `patch_t`, `post_t`/`column_t`, `spriteframe_t`, `spritedef_t` | `types` | data | carried verbatim; visibility tightened to `pub(super)` -- the old `pub` `patch_t`/`post_t`/`column_t` had zero external consumers; the `v_video`/`r_things` copies stay deliberate mirrors |
//!
//! ## Deterministic Aspects
//!
//! The module's dtmc surface is exactly one extraction:
//! `dtmc::texture_index_for_name` -- the `'-'` marker, the hash-chain
//! walk, and the first-insert-wins duplicate tie-break that resolve
//! wall-texture names into side-def content at map load. It is pinned by
//! the pre-move baseline vectors (commit `1fcfd54`: miss / `'-'` / hit /
//! duplicate lower-index-wins / case-insensitivity), re-pointed onto the
//! extraction by this split; the `check_texture_num_for_name` wrapper is
//! additionally driven live through an installed two-bucket table. The
//! flat lookup wrapper stays whole: its only qualifying arithmetic is one
//! subtraction. `R_FlatNumForName`'s result flows into simulation content
//! (`skyflatnum` gates sky-hack hitscan suppression in `p_map`), which is
//! why the wrapper's `I_Error` path is demo-golden-relevant end-to-end.
//! Everything else here shapes the *presented frame* (the column cache is
//! on every wall/plane/sky draw) or runs at startup; the F9 frame goldens
//! (`video_anchor.rs`, scenario `frame_hash`) are the behavioral gate,
//! while `harness_hash`'s state ledger never reads renderer state.
//! `precache_level` is a no-op during demo playback.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

pub mod anchor;
pub mod column_cache;
pub mod dtmc;
pub mod globals;
pub mod init;
pub mod lookup;
pub mod precache;
pub mod types;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::r_data::R_*`). The C symbol each shim
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. There are no C
//* referencers in the default build, so the pins are wasm-surface
//* conservatism. Shims die with the freeze zone.
pub use column_cache::{generate_composite as R_GenerateComposite, generate_lookup as R_GenerateLookup, get_column as R_GetColumn};
pub use init::{
    init_colormaps as R_InitColormaps, init_data as R_InitData, init_flats as R_InitFlats,
    init_sprite_lumps as R_InitSpriteLumps, init_textures as R_InitTextures,
};
pub use lookup::{
    check_texture_num_for_name as R_CheckTextureNumForName, flat_num_for_name as R_FlatNumForName,
    texture_num_for_name as R_TextureNumForName,
};
pub use precache::precache_level as R_PrecacheLevel;

//* path-stability re-export: the 20 `#[no_mangle]` statics keep their
//* module-root paths (data tier, upstream names retained). Load-bearing:
//* `p_spec` writes `flattranslation`/`texturetranslation` under the freeze
//* zone, `r_main`/`r_plane`/`r_things`/`r_segs`/`p_floor`/`f_finale`
//* read the rest, and `c_tests/r_data_c.rs` pins seven of them plus their
//* `c_int` widths through this root.
pub use globals::{
    colormaps, firstflat, firstpatch, firstspritelump, flatmemory, flattranslation, lastflat,
    lastpatch, lastspritelump, numflats, numpatches, numspritelumps, numtextures, spriteoffset,
    spritetopoffset, spritewidth, textureheight, texturememory, texturetranslation, spritememory,
};
