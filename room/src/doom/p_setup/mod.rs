//! Level/map loading and initialization: reads all BSP and geometry lumps
//! from the WAD into runtime data structures (vertexes, lines, sectors,
//! subsectors, nodes, segs, blockmap, and reject table). `P_SetupLevel` is
//! the single entry point called by the game loop when entering a new map;
//! `P_Init` is called once at startup to initialize switch lists, animated
//! flats, and the sprite name table -- bit-exact with
//! `vendor/doomgeneric/p_setup.c`.
//!
//! ## Submodule Responsibility
//!
//! - `structs.rs` -- data vocabulary: the packed on-disk `map*` structs
//!   (including the `pub` `repr(C, packed)` `mapthing_t`), `PU_PURGELEVEL`,
//!   the `ST_*` slope classes, `MAPBLOCKSHIFT`, `MAXRADIUS`, and
//!   `MAX_DEATHMATCH_STARTS`
//! - `globals.rs` -- all 25 `#[no_mangle]` map-table statics, one data home
//! - `loaders.rs` -- the `SHORT` macro substitute `le_i16` and the nine
//!   lump loaders (`load_vertexes` through `load_blockmap`)
//! - `grouplines.rs` -- `group_lines` (per-sector line tables, cumulative
//!   buffer assignment, bounding boxes) and the `totallines` static
//! - `reject.rs` -- `pad_reject_array` + `load_reject` (workaround row 4)
//! - `null_sector.rs` -- `sector_at_null_address` and its function-local
//!   sentinel statics (workaround row 5)
//! - `level.rs` -- `setup_level` (the load orchestrator; call order is the
//!   behavior) and `init_map_system`
//!
//! `dtmc` holds the module's extracted demo-synchronization surface and is
//! covered by Deterministic Aspects. The module root is documentation +
//! wiring only: the `mod` declarations and the re-exports below. Every
//! freeze-zone importer (`g_game`, `d_main`, the renderer, the graduated
//! `p_*` consumers) still names the upstream identifiers through this root.
//!
//! # Rust-vs-C differences
//!
//! - All WAD data is little-endian; the `le_i16` helper (upstream macro
//!   `SHORT` from `i_swap.h`) performs an explicit `i16::from_le`
//!   conversion instead of relying on the C macro.
//! - `P_LoadThings` increments the `mt` pointer only after the spawn decision
//!   so that a `break` on a non-commercial monster does not advance past it.
//!   The historical port bug (`continue` instead of `break`) is pinned by
//!   regression tests in `loaders` -- a port-bug regression, correctly
//!   outside `docs/vanilla-workarounds.md` (it is not a vanilla emulation).
//! - `P_SetupLevel` uses `format!` for lump-name construction instead of
//!   `DEH_snprintf`, so DeHackEd lump-name patches are not applied (FIXME
//!   carried in `level.rs`; known deviation, never normalize during a
//!   refactor).
//! - Global map tables (`vertexes`, `lines`, etc.) are `#[no_mangle]`
//!   `static mut` values with C linkage, matching the C extern declarations.
//! - Three distinct `mapthing_t` types exist in-tree (this module's `pub`
//!   one, `c_ffi`'s, and `p_telept`'s); they are deliberate ABI mirrors --
//!   never unify them. The `P_LoadThings` cast to `p_telept::mapthing_t` is
//!   a deliberate bridge.
//!
//! Cross-module contracts documented once here:
//! - **p_saveg symbol coupling**: `p_saveg`'s `extern "C"` block declares
//!   `sectors`/`lines`/`sides`/`numsectors`/`numlines` BY SYMBOL. Statics
//!   keep their upstream names + `#[no_mangle]` through this module (data
//!   tier), so those externs are untouched. The declared types are
//!   p_lights/p_floor mirror types while these statics hold `c_ffi` types;
//!   the layout match is a latent assumption -- noted, do not "unify".
//! - **Web shell path stability**: `shells/web/src/lib.rs` (the wasm
//!   debug-overlay/presenter path) takes
//!   `addr_of!(p_setup::sectors)` / `addr_of!(p_setup::numsectors)` across
//!   the workspace boundary -- the statics' module-root paths are
//!   load-bearing, held by the path-stability re-export block below.
//! - **No C translation unit references any symbol here**
//!   (`doomgeneric-sys/build.rs` excludes `p_setup.c`): every
//!   `#[export_name]` pin is wasm-surface conservatism, and no link anchor
//!   exists today -- preserve the absence (`doomgeneric.rs`'s anchor list
//!   has no p_setup entry).
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
//! | `P_SetupLevel` | `level::setup_level` | dtmc (whole-body) | the load orchestrator; lump order, `leveltime = 0`, and the thing-queue resets are the behavior; shim + pin; upstream `p_setup.c` `P_SetupLevel` |
//! | `P_Init` | `level::init_map_system` | glue | once-at-startup wiring of switch/anim/sprite tables; shim + pin |
//! | `P_LoadVertexes` | `loaders::load_vertexes` | dtmc (whole-body) | index-to-fixed-point conversion loop; shim + pin |
//! | `P_LoadSegs` | `loaders::load_segs` | dtmc (whole-body) | seg resolution; the missed-backside hook calls `null_sector::sector_at_null_address` (row 5) and records `MissedBackSideOverrun`; shim + pin |
//! | `P_LoadSubsectors` | `loaders::load_subsectors` | dtmc (whole-body) | shim + pin |
//! | `P_LoadSectors` | `loaders::load_sectors` | dtmc (whole-body) | flat-name resolution + fixed-point heights; shim + pin |
//! | `P_LoadNodes` | `loaders::load_nodes` | dtmc (whole-body) | BSP node conversion; shim + pin |
//! | `P_LoadThings` | `loaders::load_things` | dtmc (whole-body) | the break-on-non-commercial quirk (RNG-sequence semantics; break stays at the call site, the type set is `dtmc::is_noncommercial_thing`); shim + pin |
//! | `P_LoadLineDefs` | `loaders::load_linedefs` | dtmc (whole-body) | the slopetype ladder is `dtmc::slopetype_of`; shim + pin |
//! | `P_LoadSideDefs` | `loaders::load_sidedefs` | dtmc (whole-body) | texture-name resolution; shim + pin |
//! | `P_LoadBlockMap` | `loaders::load_blockmap` | dtmc (whole-body) | header extraction + `blocklinks` zero-fill; shim + pin |
//! | `P_GroupLines` | `grouplines::group_lines` | dtmc (whole-body) | cumulative line-table assignment (pinned regression); the blockmap clamps are `dtmc::clamp_block`/`clamp_block_low`; shim + pin |
//! | `PadRejectArray` (C static fn, private in Rust) | `reject::pad_reject_array` | glue | row-4 workaround core; the header formula is `dtmc::reject_pad_words`; doc alias only (private before and after) |
//! | `P_LoadReject` (C static fn, private in Rust) | `reject::load_reject` | glue | row-4 record site (`RejectPadOverrun` before padding); doc alias only (private before and after) |
//! | `GetSectorAtNullAddress` | `null_sector::sector_at_null_address` | glue | row-5 workaround: the sentinel and its lazy `I_GetMemoryValue` init stay whole; shim + pin |
//! | `SHORT` (C macro `SHORT`, i_swap) | `loaders::le_i16` | glue | private; doc alias only |
//! | -- (new extraction; no named C counterpart) | `dtmc::slopetype_of` | dtmc (extracted) | the `P_LoadLineDefs` dx/dy sign ladder; baseline vectors landed pre-move (commit `ede5d6d`) |
//! | -- (new extraction; no named C counterpart) | `dtmc::reject_pad_words` | dtmc (extracted) | the row-4 `rejectpad[4]` initializer formula; the fill loop and `-reject_pad_with_ff` tail stay in `reject`; pre-move vectors in `ede5d6d` |
//! | -- (new extraction; no named C counterpart) | `dtmc::is_noncommercial_thing` | dtmc (extracted) | the 10-type `P_LoadThings` filter; the break stays at the call site; baselined by the `count_spawnable_things` mirror tests, re-pointed onto it |
//! | -- (new extraction; no named C counterpart) | `dtmc::clamp_block` / `dtmc::clamp_block_low` | dtmc (extracted) | the four `P_GroupLines` blockmap clamp ladders -- TWO functions because the vanilla ladders are asymmetric (TOP/RIGHT clamp high only, BOTTOM/LEFT clamp low only; never merge into one range clamp); the +/- `MAXRADIUS` margin is folded into the `org` argument bit-exactly; pre-move vectors in `ede5d6d` |
//! | 25 `#[no_mangle]` statics (`numvertexes`..`playerstarts`) | `globals` | data | upstream names + `#[no_mangle]` retained; every root path held by the `path-stability re-export` block below; p_saveg's extern-by-symbol block and the web shell's `addr_of!` paths resolve through it |
//! | private `totallines` | `grouplines` | data | single-writer home (written by `group_lines`); `reject` reads it via `super::grouplines::totallines` |
//! | packed `map*` structs, `PU_PURGELEVEL`, `ST_*`, `MAPBLOCKSHIFT`, `MAXRADIUS`, `MAX_DEATHMATCH_STARTS` | `structs` | data | carried verbatim; `mapthing_t` stays `pub` (`repr(C, packed)`); one of three in-tree `mapthing_t` types -- do not unify |
//!
//! ## Deterministic Aspects
//!
//! The loaders build the entire simulation geometry, so the whole loader
//! family is adjudicated `dtmc (whole-body)`: loop order, index arithmetic,
//! and shift widths ARE the map the demo then plays on. Nothing inside a
//! loader is separable except the four pure computations extracted into
//! `dtmc`:
//!
//! - **`slopetype_of`** decides `line_t.slopetype` for every linedef; each
//!   collision, use-line, and bullet trace reads it. The ladder order
//!   (dx-first) and the `FixedDiv(dy, dx)` argument order are pinned by
//!   vectors, including the `(0, 0)` -> `ST_VERTICAL` degenerate.
//! - **`reject_pad_words`** is the workaround row-4 formula: the modeled
//!   zone block header that vanilla read past an undersized REJECT lump.
//!   Sight-blocking on affected WADs and demos depends on these exact
//!   bytes; the fill order and the `-reject_pad_with_ff` tail remain at
//!   the `reject` call site.
//! - **`is_noncommercial_thing`** pins the ten-type shareware monster set.
//!   The break (NOT continue) at the call site is the load-bearing half --
//!   breaking one thing early shifts every subsequent `P_Random` draw of
//!   the spawn pass -- and is pinned by the `loaders` mirror tests.
//! - **`clamp_block` / `clamp_block_low`** bound `P_RadiusAttack`'s blockmap
//!   sweep via `sector.blockbox`. The vanilla ladders are asymmetric (high
//!   clamp without low clamp and vice versa); they are kept as two
//!   functions and never merged into a range clamp, because merging would
//!   change `blockbox` on degenerate bounding boxes.
//! - **`setup_level`'s orchestration order** (BLOCKMAP, VERTEXES, SECTORS,
//!   SIDEDEFS, LINEDEFS, SSECTORS, NODES, SEGS, then GroupLines/REJECT,
//!   then THINGS) is dtmc as a whole: it fixes zone reuse addresses, the
//!   `totallines` value REJECT pads with, and the spawn RNG start state.
//!   `leveltime = 0` here is the Nightmare-gate reset the save/load path
//!   round-trips through.

pub mod dtmc;
pub mod globals;
pub mod grouplines;
pub mod level;
pub mod loaders;
pub mod null_sector;
pub mod reject;
pub mod structs;

//* upstream-name shim: every renamed function keeps its freeze-zone
//* caller path (`crate::doom::p_setup::P_*`). The C symbol each shim
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`, so the wasm/extern symbol name
//* set stays byte-identical to the pre-split module. There are no C
//* referencers (build.rs excludes p_setup.c), so the pins are
//* wasm-surface conservatism. Shims die with the freeze zone.
pub use level::{init_map_system as P_Init, setup_level as P_SetupLevel};
pub use loaders::{
    load_blockmap as P_LoadBlockMap, load_linedefs as P_LoadLineDefs,
    load_nodes as P_LoadNodes, load_sectors as P_LoadSectors, load_segs as P_LoadSegs,
    load_sidedefs as P_LoadSideDefs, load_subsectors as P_LoadSubsectors,
    load_things as P_LoadThings, load_vertexes as P_LoadVertexes,
};
pub use grouplines::group_lines as P_GroupLines;
pub use null_sector::sector_at_null_address as GetSectorAtNullAddress;

//* path-stability re-export: the map-table statics keep their module-root
//* paths (data tier, upstream names + #[no_mangle] retained). Load-bearing
//* across the workspace boundary: shells/web takes addr_of! on
//* `p_setup::sectors` / `p_setup::numsectors`, and p_saveg's extern block
//* links five of these by symbol.
pub use globals::{
    blocklinks, blockmap, blockmaplump, bmapheight, bmaporgx, bmaporgy, bmapwidth, deathmatch_p,
    deathmatchstarts, lines, nodes, numlines, numnodes, numsectors, numsegs, numsides,
    numsubsectors, numvertexes, playerstarts, rejectmatrix, sectors, segs, sides, subsectors,
    vertexes,
};

//* path-stability re-export: the data vocabulary keeps its module-root
//* paths (`mapthing_t` is imported by g_game and p_mobj;
//* MAX_DEATHMATCH_STARTS by c-adjacent tests).
pub use structs::{mapthing_t, MAX_DEATHMATCH_STARTS};
