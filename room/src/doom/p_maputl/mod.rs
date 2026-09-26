//! Map geometry utilities, blockmap iterators, and intercept routines:
//! the low-level primitives behind collision detection, line-of-sight,
//! and hitscan/projectile tracing -- bit-exact with
//! `vendor/doomgeneric/p_maputl.c`.

//! ## Submodule Responsibility
//!
//! - `geometry.rs` -- the six `#[no_mangle]` extern geometry entries
//!   (`P_AproxDistance`, `P_PointOnLineSide`, `P_PointOnDivlineSide`,
//!   `P_MakeDivline`, `P_InterceptVector`, `P_BoxOnLineSide`): raw
//!   pointer marshalling over the pure cores in `dtmc`
//! - `opening.rs` -- the opening quartet statics (`opentop`,
//!   `openbottom`, `openrange`, `lowfloor`) and `P_LineOpening`
//! - `position.rs` -- `P_UnsetThingPosition` / `P_SetThingPosition`
//!   (sector thing-list + blockmap surgery)
//! - `blockmap.rs` -- `P_BlockLinesIterator` / `P_BlockThingsIterator`
//! - `intercepts.rs` -- the intercept pipeline and its state:
//!   `MAXINTERCEPTS*` / `PT_*` consts, the `intercepts` / `intercept_p`
//!   / `trace` / `earlyout` statics, the vanilla overrun emulators
//!   (`InterceptsMemoryOverrun` / `InterceptsOverrun` -- the catalog
//!   entry-2 core), the two `PIT_Add*Intercepts` callbacks,
//!   `P_TraverseIntercepts`, and `P_PathTraverse`
//!
//! `dtmc.rs` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_maputl::*` across the freeze zone -- the p_map and
//! p_enemy import blocks, the FULLY-QUALIFIED `crate::doom::p_maputl::trace`
//! reads in p_map's `PTR_ShootTraverse`, and the c_tests root paths
//! `p_maputl_c.rs` / `lookup_tables.rs`); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_AproxDistance` | `geometry::P_AproxDistance` wrapping `dtmc::aprox_distance` | dtmc (extracted wholesale) | pure integer approximation (`|dx|+|dy|-min/2`) feeding AI move distance / sound ranges; no marshalling in the core; upstream `p_maputl.c:43` |
//! | `P_PointOnLineSide` | `geometry::P_PointOnLineSide` wrapping `dtmc::point_on_line_side` | dtmc (core extracted) | pure side predicate consuming trampled `tmbbox` values (catalog G1 consumer); the raw-pointer extern stays as a wrapper; upstream `p_maputl.c:60` |
//! | `P_PointOnDivlineSide` | `geometry::P_PointOnDivlineSide` wrapping `dtmc::point_on_divline_side` | dtmc (core extracted) | sign-bit fast path + 8-bit pre-shifted `FixedMul` pair is exactness-sensitive (long-trace intercepts); upstream `p_maputl.c:155` |
//! | `P_MakeDivline` | `geometry::P_MakeDivline` wrapping `dtmc::make_divline` | glue (pure marshalling core lifted) | pure field marshalling, no arithmetic; the two-pointer extern keeps the C shape; upstream `p_maputl.c:205` |
//! | `P_InterceptVector` | `geometry::P_InterceptVector` wrapping `dtmc::intercept_vector` | dtmc (core extracted) | same formula family as `p_sight::dtmc::intercept_vector2`; `den == 0 -> 0` pin; 8-bit pre-shift; upstream `p_maputl.c:225` |
//! | `P_BoxOnLineSide` | `geometry::P_BoxOnLineSide` wrapping `dtmc::box_on_line_side` | dtmc (whole-body pure; lifted) | four-arm `slopetype` predicate over the box; result `-1` decides `PIT_CheckLine`'s special-record reject -- demo-visible; upstream `p_maputl.c:104` |
//! | `P_LineOpening` | `opening::P_LineOpening` | dtmc (whole-body; nothing extracts) | writes the four opening globals in vanilla comparison order; the `sidenum[1] == -1 -> openrange 0` early-out is sim behavior; upstream `p_maputl.c:295` |
//! | `P_UnsetThingPosition` / `P_SetThingPosition` | `position` | dtmc (whole-body; nothing extracts) | blockmap/sector list surgery -- pointer-chase order is the only behavior, nothing pure to lift; upstream `p_maputl.c:342` / `:390` |
//! | `P_BlockLinesIterator` / `P_BlockThingsIterator` | `blockmap` | dtmc (whole-body; nothing extracts) | `validcount` skip semantics + callback early-out ordering; function-pointer marshalling stays; upstream `p_maputl.c:466` / `:507` |
//! | `InterceptsMemoryOverrun` | `intercepts::InterceptsMemoryOverrun` | dtmc (whole-body; the layout table IS the surface) | byte-faithful vanilla `.bss` order as `skip!` / `write_i32!` / `write_i16_arr!` steps; each slot maps a live global (`lowfloor` ... `bmapheight`); reorder = desync on overrun demos; pinned by the moved baseline layout vectors; upstream `p_maputl.c:782` |
//! | `InterceptsOverrun` | `intercepts::InterceptsOverrun` | dtmc (whole-body) | trigger arithmetic `location = (n - 128 - 1) * 12` + the `violations` census record; the entry-2 emulation itself; upstream `p_maputl.c:827` |
//! | `PIT_AddLineIntercepts` | `intercepts::PIT_AddLineIntercepts` | dtmc (whole-body; nothing extracts) | long/short trace branch choice (`FRACUNIT*16` threshold) + `earlyout` gate + UNGUARDED store/advance order (chocolate parity, catalog G2 audit row); upstream `p_maputl.c:558` |
//! | `PIT_AddThingIntercepts` | `intercepts::PIT_AddThingIntercepts` | dtmc (whole-body; nothing extracts) | `tracepositive` diagonal selection is RNG-free but order-sensitive sim behavior; same unguarded store; upstream `p_maputl.c:614` |
//! | `P_TraverseIntercepts` | `intercepts::P_TraverseIntercepts` | dtmc (whole-body; nothing extracts) | selection-sort nearest-first + `c_int::MAX` visited mark; tie order = scan order is demo-visible; the mark's collision with a genuine `frac == c_int::MAX` intercept is vanilla behavior -- never harden; upstream `p_maputl.c:681` |
//! | `P_PathTraverse` | `intercepts::P_PathTraverse` | dtmc (whole-body; nothing extracts) | grid-nudge by `FRACUNIT`, x/y step selection, the 64-cell cap (upstream comment "prevent a round off error", `p_maputl.c:957-959`), callback wiring -- statement order is the surface; upstream `p_maputl.c:860` |
//! | `opentop` / `openbottom` / `openrange` / `lowfloor` | `opening` | data | all `#[no_mangle]` kept; consumed by p_map / p_enemy through the module-root re-export |
//! | `intercepts` / `intercept_p` / `trace` / `earlyout` | `intercepts` | data | `#[no_mangle]` kept except `earlyout` (private, as before); `trace` has a fully-qualified-path consumer (`p_map.rs` `PTR_ShootTraverse` / `goto_hitline`) -- the root re-export is mandatory, not optional |
//! | `MAXINTERCEPTS_ORIGINAL` / `MAXINTERCEPTS` / `PT_ADDLINES` / `PT_ADDTHINGS` / `PT_EARLYOUT` | `intercepts` | data | consts; `MAXINTERCEPTS = 128 + 61 = 189` is the overrun-emulation spare capacity the c_tests pin through the root path |
//! | (no link anchor) | -- | -- | p_maputl never had a link anchor (the `doomgeneric.rs` anchor list has none) -- the absence is preserved |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! the fifteen exported functions and the seven exported statics
//! keep their C symbols, which keeps the wasm export surface
//! byte-identical. The six `dtmc` cores are new Rust-side names for
//! bodies that were inline in the extern entries (no C name existed),
//! so no aliasing beyond `#[doc(alias)]` applies. No C translation
//! unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes `p_maputl.c`), so the
//! retention is pure conservatism. The emulation's cross-module
//! writers stay FULLY QUALIFIED by design (`crate::doom::p_pspr::bulletslope`,
//! `crate::doom::p_setup::*`) so trample provenance stays obvious --
//! do not rewrite them into `use` imports.
//!
//! ## Deterministic Aspects
//!
//! The module draws nothing from `P_Random` / `M_Random` -- traverse
//! determinism is pure-integer plus statics ordering (wave RNG ledger;
//! movement / aim / shoot outcomes land in the F9 state hash via
//! behavior). The exactness-critical pieces:
//!
//! - `intercepts.rs` is the catalog entry-2 emulation: the
//!   `InterceptsMemoryOverrun` layout table walks the vanilla `.bss`
//!   neighbor order slot by slot, and the `PIT_Add*Intercepts` stores
//!   past `MAXINTERCEPTS_ORIGINAL` (128) are deliberately UNGUARDED
//!   into the 189-entry spare capacity (chocolate parity -- the G2
//!   audit row). A "helpful" bounds guard would be a silent behavior
//!   change on pathological traces. The baseline layout / trigger
//!   vectors in `intercepts.rs` were written and run GREEN against the
//!   pre-move bodies BEFORE the split -- same vectors, same results
//!   (F10 §2.3); every trigger also records a
//!   `VanillaViolation::InterceptsOverrun` census hit.
//! - `P_TraverseIntercepts` marks visited entries with `c_int::MAX`;
//!   the potential collision with a genuine `frac == c_int::MAX` is
//!   vanilla behavior, and the selection-sort tie order (scan order)
//!   is demo-visible.
//! - `dtmc.rs` cores carry the pinned integer arithmetic: the
//!   `den == 0 -> 0` intercept pin, the sign-bit fast path of
//!   `point_on_divline_side`, and the 8-bit pre-shifts -- any
//!   reassociation changes intercept fractions and desyncs demos. The
//!   existing c_tests known-vector suites (`p_maputl_c.rs`,
//!   `lookup_tables.rs`) are the §2.3 baselines for these cores: they
//!   exercise the extern entries end-to-end and keep resolving through
//!   the module-root re-exports.
//! - `validcount` stamp protocol (shared with p_sight / p_map):
//!   `P_PathTraverse` bumps `crate::doom::r_main::validcount` and
//!   `P_BlockLinesIterator` consumes it to dedupe lines spanning
//!   multiple blockmap cells, while `P_CheckSight` (p_sight) bumps and
//!   the p_map position checks bump the same stamp -- a cross-module
//!   determinism contract described once in the sibling modules' docs.

pub mod blockmap;
pub mod dtmc;
pub mod geometry;
pub mod intercepts;
pub mod opening;
pub mod position;

//* path-stability re-export: the blockmap iterators keep their
//* module-root paths (p_map collision / attack traversals).
pub use blockmap::{P_BlockLinesIterator, P_BlockThingsIterator};

//* path-stability re-export: the geometry entries keep their
//* module-root paths (p_map, p_enemy, p_mobj, c_tests).
pub use geometry::{
    P_AproxDistance, P_BoxOnLineSide, P_InterceptVector, P_MakeDivline, P_PointOnDivlineSide,
    P_PointOnLineSide,
};

//* path-stability re-export: the intercept pipeline, its statics, and
//* its consts keep their module-root paths. `trace` is critical: p_map
//* reads it via the FULLY-QUALIFIED path `crate::doom::p_maputl::trace`,
//* which only resolves through this root re-export.
pub use intercepts::{
    intercept_p, intercepts, trace, MAXINTERCEPTS, MAXINTERCEPTS_ORIGINAL, PIT_AddLineIntercepts,
    PIT_AddThingIntercepts, P_PathTraverse, P_TraverseIntercepts, PT_ADDLINES, PT_ADDTHINGS,
    PT_EARLYOUT,
};

//* path-stability re-export: the opening quartet and P_LineOpening
//* keep their module-root paths (p_map, p_enemy).
pub use opening::{lowfloor, openbottom, openrange, opentop, P_LineOpening};

//* path-stability re-export: the thing-position pair keeps its
//* module-root paths (p_map, p_enemy, p_mobj).
pub use position::{P_SetThingPosition, P_UnsetThingPosition};
