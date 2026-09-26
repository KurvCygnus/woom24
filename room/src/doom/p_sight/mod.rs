//! Line-of-sight checks between map objects: the REJECT-table fast path
//! plus BSP-tree traversal and per-segment portal testing of
//! `P_CheckSight` -- bit-exact with `vendor/doomgeneric/p_sight.c`.

//! ## Submodule Responsibility
//!
//! - `types.rs` -- the seven local `#[repr(C)]` mirrors (`divline_t`,
//!   `vertex_t`, `subsector_t`, `seg_t`, `node_t`, `sector_t`,
//!   `line_t`) with their size/layout tests, and the re-export of the
//!   authoritative `p_telept::mobj_t`
//! - `state.rs` -- the seven `#[no_mangle]` sight statics
//!   (`sightzstart`, `topslope`, `bottomslope`, `strace`, `t2x`, `t2y`,
//!   `sightcounts`)
//! - `bsp.rs` -- `P_CrossSubsector`, `P_CrossBSPNode`, and the
//!   `node_as_divline` cast spelling (the catalog entry-5 consumption
//!   sites live here)
//! - `checks.rs` -- `P_CheckSight`: the REJECT fast path plus the
//!   traversal entry
//! - `anchor.rs` -- the link anchor keeping the `#[no_mangle]` set
//!   alive
//!
//! `dtmc.rs` holds the module's extracted demo-synchronization surface
//! and is covered by Deterministic Aspects. The module root is
//! documentation + wiring only: the `mod` declarations and the
//! re-exports below keep every existing consumer path valid
//! (`crate::doom::p_sight::*` across the freeze zone -- the
//! `P_CheckSight` consumers p_map and p_enemy, the `topslope` /
//! `bottomslope` dual writer p_map, the `P_Sight_Link_Anchor` caller
//! `doomgeneric.rs`, and the local mirror types); no content lives
//! here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `P_DivlineSide` | `dtmc::divline_side` | dtmc (extracted) | pure side-of-divline classifier whose exact integer result steers BSP/seg crossing; the `dy == 0` arm's `x == node.y` quirk (NOT `y == node.y`) is load-bearing demo behavior -- do not correct; upstream `p_sight.c:47` |
//! | `P_InterceptVector2` | `dtmc::intercept_vector2` | dtmc (extracted) | sight-local fixed-point intercept fraction; `den == 0 -> 0` and the 8-bit pre-shift are exactness-sensitive; the publicly exported sibling is `p_maputl::P_InterceptVector`; upstream `p_sight.c:101` |
//! | `P_CrossSubsector` | `bsp::P_CrossSubsector` | dtmc (whole-body; nothing extracts) | mutates `topslope` / `bottomslope` / `line.validcount` mid-walk in vanilla statement order; the entry-5 consumption sites (`line.backsector` null check, the `seg.backsector` read the p_setup load-time substitution covers) land here; the `i_error!` guard on malformed BSP stays at the moved position; upstream `p_sight.c:128` |
//! | `P_CrossBSPNode` | `bsp::P_CrossBSPNode` | dtmc (whole-body; nothing extracts) | recursion order + the `side == 2 -> 0` coercion is the demo-visible traversal order; global-coupled, not extractable; upstream `p_sight.c:258` |
//! | `node_as_divline` | `bsp::node_as_divline` | glue | layout spelling of the C `(divline_t*)node` cast (both structs share the `x`/`y`/`dx`/`dy` header); no upstream C name |
//! | `P_CheckSight` | `checks::P_CheckSight` | dtmc (whole-body; nothing extracts) | REJECT indexing is pure but stays at the entry -- the `sightzstart` / `strace` statics marshalling around it is the surface; returns `c_int` not `bool` (Doom's 32-bit `boolean`; a 1-byte `bool` would leave the upper 24 bits undefined and desync demos); the `offset_from` sector-index trick depends on the LOCAL `sector_t` size -- do not unify the mirrors with `c_ffi`; upstream `p_sight.c:300` |
//! | `P_Sight_Link_Anchor` | `anchor::P_Sight_Link_Anchor` | glue | link scaffolding, never runs in sim; called from `doomgeneric.rs` |
//! | `sightzstart` / `topslope` / `bottomslope` / `strace` / `t2x` / `t2y` / `sightcounts` | `state` | data | all `#[no_mangle]` kept (wasm export surface byte-identical); `topslope` / `bottomslope` are a genuine cross-module dual-writer protocol -- `P_CheckSight` initializes them, `P_CrossSubsector` narrows them, and p_map's `PTR_AimTraverse` / `P_AimLineAttack` write the same globals between LOS checks (the F9 goldens pin the interaction); `sightcounts` is diagnostics-only |
//! | `divline_t` / `vertex_t` / `subsector_t` / `seg_t` / `node_t` / `sector_t` / `line_t` (7 mirrors) | `types` | data | LOCAL `#[repr(C)]` mirrors, NOT the `c_ffi` copies, carried verbatim -- `P_CheckSight` computes sector indices via `offset_from` on the local `sector_t` size, so layout is load-bearing; unification with `c_ffi` is a later, separate decision; the layout/size tests moved with them; `node_t`/`divline_t` header-alias idiom documented there |
//! | `NF_SUBSECTOR` | `bsp` (private const) | data | `NF_SUBSECTOR` from `p_local.h` |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin --
//! the two exported functions and the seven exported statics keep
//! their C symbols, which keeps the wasm export surface
//! byte-identical. The two extracted `dtmc` helpers are new Rust-side
//! names for previously file-private functions (no C name existed), so
//! no aliasing beyond `#[doc(alias)]` applies. No C translation unit
//! references any of these symbols (`doomgeneric-sys/build.rs`
//! excludes `p_sight.c`), so the retention is pure conservatism.
//!
//! ## Deterministic Aspects
//!
//! The module draws nothing from `P_Random` / `M_Random` -- sight
//! determinism is pure-integer plus statics ordering (part of the wave
//! RNG ledger; the demo-hash consumers are p_enemy's LOS-gated AI and
//! p_map's radius-attack gate, which land in the F9 state hash via
//! behavior). The exactness-critical pieces:
//!
//! - `dtmc::divline_side` -- the `dy == 0` arm tests `x == node->y`,
//!   NOT `y == node->y`. This is a long-standing quirk in the Doom
//!   source that demos rely on for deterministic playback; it is
//!   pinned (including the wrong-coordinate equality returning 2) by
//!   the baseline vectors in `dtmc.rs`, which were written and run
//!   GREEN against the pre-extraction body BEFORE the move and
//!   retargeted afterwards -- same vectors, same results (F10 §2.3).
//! - `dtmc::intercept_vector2` -- the `den == 0 -> 0` parallel pin and
//!   the 8-bit pre-shift before `FixedMul` are exactness-sensitive;
//!   the slope narrowing in `P_CrossSubsector` feeds `FixedDiv` with
//!   the result, so any reassociation changes LOS outcomes and hence
//!   the demo hash.
//! - `topslope` / `bottomslope` dual-writer protocol (see the mapping
//!   table): one initialization site here, one narrowing site here,
//!   and the p_map aim-traversal writer in between -- the write ORDER
//!   across tics is the demo surface; both modules re-export the
//!   statics at their roots and neither copies them.
//! - `validcount` stamp protocol (shared with p_maputl / p_map):
//!   `P_CheckSight` bumps `r_main::validcount` before traversal and
//!   the seg walk in `P_CrossSubsector` consumes it to dedupe
//!   linedefs, while `P_PathTraverse` (p_maputl) and
//!   `P_CheckPosition` / `P_TeleportMove` (p_map) bump and consume the
//!   same stamp -- a cross-module determinism contract described once
//!   here and in the sibling modules' docs.
//! - `P_CheckSight`'s `c_int` (not `bool`) return: the upper-24-bits
//!   contract is pinned by the mapping table; changing it would
//!   randomly flip sight checks and desync demos and RNG.

pub mod anchor;
pub mod bsp;
pub mod checks;
pub mod dtmc;
pub mod state;
pub mod types;

//* path-stability re-export: the LOS entry keeps its module-root path
//* (p_map radius gate, p_enemy AI).
pub use checks::P_CheckSight;

//* path-stability re-export: the sight corridor statics keep their
//* module-root paths (p_map aim traversal is the second writer of
//* topslope/bottomslope).
pub use state::{bottomslope, sightcounts, sightzstart, strace, t2x, t2y, topslope};

//* path-stability re-export: the local mirror types keep their
//* module-root paths (currently p_sight-internal; re-exported for
//* path stability with the pre-graduation flat file).
pub use types::{divline_t, line_t, mobj_t, node_t, sector_t, seg_t, subsector_t, vertex_t};

//* path-stability re-export: the link anchor keeps its module-root
//* path (doomgeneric.rs).
pub use anchor::P_Sight_Link_Anchor;
