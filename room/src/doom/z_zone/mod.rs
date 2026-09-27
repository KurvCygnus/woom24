//! Zone memory allocation: the doubly-linked-list allocator whose
//! blocks sit inside a buffer returned by `I_ZoneBase`, with the
//! `PU_*` purge-level tags that classify them. Rust port of
//! `vendor/doomgeneric/z_zone.c`.
//!
//! ## Submodule Responsibility
//!
//! - `tags.rs` -- the `PU_*` purge-level tag constants (names and
//!   values kept; byte-stable contract data quoted by the REJECT-pad
//!   workaround model in `p_setup::reject`)
//! - `zone.rs` -- allocator core: the `memblock_t` / `memzone_t`
//!   layouts, the `mainzone` global, `ZONEID` / `MEM_ALIGN` /
//!   `MINFRAGMENT`, and the alloc / free / tag-range / retag /
//!   resize entry points, plus the baseline-vector test module
//! - `diagnose.rs` -- heap diagnostics: stdout dump, fatal and quiet
//!   invariant checkers, free-byte counter, stubbed file dumper
//! - `anchor.rs` -- `zone_link_anchor`, the dead-but-kept link anchor
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the path-stability re-exports and the upstream-name
//! shims below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern),
//! every function carries a plain-English internal name with
//! `#[doc(alias = "OriginalName")]`; the freeze-zone surface is held
//! by the `upstream-name shim` re-exports at this root. Functions
//! only: statics/consts/types keep their upstream names (data-tier
//! renaming comes with freeze-zone retirement). All renamed functions
//! keep their pre-move ABI kind (`pub unsafe extern "C"`, or plain
//! `pub unsafe` for `check_heap_after`, which was never extern) but
//! drop `#[no_mangle]` with the rename, shrinking the dead wasm
//! export surface (m_fixed precedent) -- except the two functions
//! freeze-zone legacy `extern "C"` blocks link by symbol, which are
//! re-pinned with `#[export_name]`.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `Z_ClearZone` | `zone::zone_clear` | glue | private helper; upstream inlines it in `Z_Init` (`z_zone.c:105-119`); private, so no shim |
//! | `Z_Init` | `zone::zone_init` | glue | shim; `#[no_mangle]` dropped (zero extern declarers); upstream `vendor/doomgeneric/z_zone.c:97` |
//! | `Z_Free` | `zone::zone_free` | glue | shim + `#[export_name = "Z_Free"]` pin (declared extern by `f_wipe.rs`, `r_draw.rs`); both-side coalescing pinned by the baseline tests; upstream `z_zone.c:126` |
//! | `Z_Malloc` | `zone::zone_alloc` | glue | shim + `#[export_name = "Z_Malloc"]` pin (same declarers); next-fit rover walk + purge eviction + `MINFRAGMENT` split; zero-fills the returned buffer (upstream C does not -- port addition, doc-commented at the definition, pinned by the baseline tests); upstream `z_zone.c:184` |
//! | `Z_FreeTags` | `zone::zone_free_tags` | glue | shim; the 2000-block walk cap and `[Z_FreeTags]` stderr trace are Rust-only additions absent from C `z_zone.c:297-319`; upstream `z_zone.c:297` |
//! | `Z_DumpHeap` | `diagnose::dump_heap` | glue | shim; zero external callers; upstream `z_zone.c:327` |
//! | `Z_CheckHeap` | `diagnose::check_heap` | glue | shim; sole live caller `g_game.rs`; upstream `z_zone.c:400` |
//! | `Z_CheckHeapQuiet` | `diagnose::check_heap_quiet` | glue | shim; Rust-only (no C counterpart); 2000-cap walk + 10 MB size heuristic (flags legitimately merged large free blocks -- known quirk, documented at the definition) |
//! | `Z_CheckHeapAfter` | `diagnose::check_heap_after` | glue | shim; Rust-only, stays `pub unsafe fn` (never extern) |
//! | `Z_ChangeTag2` | `zone::change_tag` | glue | shim; `file`/`line` args ignored (macro callsite rewritten on the C side); upstream `z_zone.c:429` |
//! | `Z_ChangeUser` | `zone::change_user` | glue | shim; sole caller `w_wad/file.rs`; upstream `z_zone.c:446` |
//! | `Z_FreeMemory` | `diagnose::free_memory` | glue | shim; counts free + purgeable bytes; upstream `z_zone.c:466` |
//! | `Z_FileDumpHeap` | `diagnose::file_dump_heap` | glue | shim; stubbed no-op (no Rust call site; C body `z_zone.c:367-393` never ported) |
//! | `Z_ZoneSize` | `zone::zone_size` | glue | shim; zone SIZE ruled not demo-observable (`docs/vanilla-workarounds.md:1183-1192`), stays consistent with `i_system::DEFAULT_RAM`; upstream `z_zone.c:484` |
//! | `Z_Zone_Link_Anchor` | `anchor::zone_link_anchor` | glue | dead-but-kept link anchor (zero callers; `doomgeneric.rs`'s anchor list has no z_zone entry); renamed + shimmed, `#[no_mangle]` dropped -- only the two live symbols stay pinned; retires with the freeze zone (p_spec precedent) |
//! | `mainzone` | `zone::mainzone` (static) | data | name kept (statics ruling; zero extern declarers today); re-exported at the root for path stability |
//! | `memblock_t` / `memzone_t` | `zone.rs` | data | `#[repr(C)]` layouts kept, `pub(super)` to the module subtree (were file-private) |
//! | `ZONEID` / `MINFRAGMENT` / `MEM_ALIGN` | `zone.rs` | data | private consts kept; `ZONEID` `0x1d4a11` is byte-stable -- `docs/vanilla-workarounds.md` entry 4 quotes it in the REJECT pad words |
//! | `PU_STATIC` / `PU_FREE` / `PU_LEVEL` / `PU_LEVSPEC` / `PU_PURGELEVEL` / `PU_CACHE` | `tags.rs` | data | names and values kept; `PU_LEVEL` = 50-class header words in the workaround model read these literally |
//! | `PU_SOUND` / `PU_MUSIC` / `PU_NUM_TAGS` | -- (never ported) | -- | absent from this port with zero Rust consumers (`PU_SOUND`/`PU_MUSIC` upstream `z_zone.h:36-37`, `PU_NUM_TAGS` `:49`); deliberately not added |
//!
//! Provenance note: `c2rust-intermediate` declares `Z_Init` / `Z_Malloc`
//! / `Z_Free` / `Z_CheckHeap` by symbol, but that crate is
//! nightly-only and excluded from default builds -- recorded as
//! provenance, not a linkage constraint.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave B5, whole module):
//! the allocator determines WHERE bytes live, never WHAT the
//! simulation computes. Provided an allocation succeeds (same sizes
//! returned), no tic-level observable differs: no PRNG consumption,
//! no demo-visible statics, no state-hash words. The free-list
//! walk/coalesce/evict mechanics are memory-model surface, explicitly
//! not demo-sync; failure paths (`I_Error` on exhaustion or header
//! mismatch) are crash surfaces, not sync surfaces; and zone SIZE is
//! already ruled unobservable in `docs/vanilla-workarounds.md`
//! (:1183-1192). The one demo-ADJACENT value -- the DOS zone-header
//! shape vanilla reads past an undersized REJECT lump -- is modeled
//! as fixed constants in `p_setup::reject` (entry 4), not computed
//! here, which is why `ZONEID` and the tag values must stay
//! byte-stable. Accordingly `z_zone` has no `dtmc` submodule: every
//! function adjudicates to `glue` (allocator marshalling, crash-surface
//! guards, diagnostics) or `data` (tags, layouts, constants) in the
//! mapping table above, and nothing is a pure integer computation
//! feeding sim values that would qualify for extraction.

pub mod anchor;
pub mod diagnose;
pub mod tags;
pub mod zone;

//* path-stability re-export: the purge-level tags keep their
//* module-root paths (20+ freeze-zone consumer files).
pub use tags::{PU_CACHE, PU_FREE, PU_LEVSPEC, PU_LEVEL, PU_PURGELEVEL, PU_STATIC};

//* path-stability re-export: the zone global keeps its module-root
//* path.
pub use zone::mainzone;

//* upstream-name shim: freeze-zone callers keep the upstream names.
//* `Z_Malloc` / `Z_Free` additionally resolve by C symbol through the
//* `#[export_name]` pins at their definitions (`f_wipe.rs:31/:33`,
//* `r_draw.rs:41/:43` legacy `extern "C"` blocks).
pub use zone::zone_alloc as Z_Malloc;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use zone::zone_free as Z_Free;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use zone::zone_free_tags as Z_FreeTags;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use zone::zone_init as Z_Init;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use zone::change_tag as Z_ChangeTag2;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use zone::change_user as Z_ChangeUser;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use zone::zone_size as Z_ZoneSize;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use diagnose::check_heap as Z_CheckHeap;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use diagnose::check_heap_after as Z_CheckHeapAfter;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use diagnose::check_heap_quiet as Z_CheckHeapQuiet;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use diagnose::dump_heap as Z_DumpHeap;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use diagnose::file_dump_heap as Z_FileDumpHeap;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use diagnose::free_memory as Z_FreeMemory;
//* upstream-name shim: the dead-but-kept link anchor keeps its
//* module-root path (renamed at the definition; see anchor.rs).
pub use anchor::zone_link_anchor as Z_Zone_Link_Anchor;
