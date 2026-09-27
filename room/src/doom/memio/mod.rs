//! In-memory replacement for the `stdio.h` `FILE *` family: a
//! `MEMFILE` wraps either a caller-provided read-only buffer or a
//! self-growing write buffer, with stdio-like read/write/seek/tell/
//! close over it. Rust port of `vendor/doomgeneric/memio.c`.
//!
//! ## Submodule Responsibility
//!
//! - `stream.rs` -- everything: the `memfile_mode_t` / `mem_rel_t`
//!   enums, the `_MEMFILE` handle, the cfg-split allocator helpers,
//!   all stream functions, and the baseline test module
//!
//! The split is deliberately minimal (mod.rs + stream.rs): this
//! module has ZERO consumers anywhere in the tree -- the brief's
//! savegame/mus2mid hypotheses were both refuted (p_saveg uses its
//! own byte-cursor archive code; `audio/music.rs` consumes slices,
//! not MEMFILEs). The C-world consumers (`mus2mid.c`, `deh_io.c`)
//! were replaced by pure-Rust code outside the freeze zone. The
//! module is an orphan candidate for retirement, but the two-entry/
//! launcher roadmap may reuse it for DeHacked/MUS work, so it
//! graduates as-is. Nothing lives at the module root beyond the `mod`
//! declaration, the type re-exports, and the C-name shims.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! plain-English internal names with `#[doc(alias)]`, C-name shims at
//! this root, `#[no_mangle]` dropped with the rename (zero link
//! consumers anywhere, so nothing can break), and no pins -- no C
//! symbols are referenced by anything. All functions keep their
//! pre-move SAFE `pub extern "C"` kind.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `mem_fopen_read` | `stream::mem_open_read` | glue | shim; borrows the caller's buffer; upstream `vendor/doomgeneric/memio.c:42` |
//! | `mem_fread` | `stream::mem_read` | glue | shim; wrong-mode returns 0 while C returns `(size_t)-1` (documented defensive deviation, module doc `memio.c` companion); upstream `memio.c:58` |
//! | `mem_fopen_write` | `stream::mem_open_write` | glue | shim; fresh 1 KiB buffer, doubles on overflow; upstream `memio.c:90` |
//! | `mem_fwrite` | `stream::mem_write` | glue | shim; same wrong-mode deviation; upstream `memio.c:107` |
//! | `mem_get_buf` | `stream::mem_get_buf` | glue | NO rename -- already plain-English snake_case (m_random `P_Random` precedent: name kept, so no shim; root re-export is path-stability wiring only); upstream `memio.c:143` |
//! | `mem_fclose` | `stream::mem_close` | glue | shim; frees the owned buffer in write mode; upstream `memio.c:149` |
//! | `mem_ftell` | `stream::mem_tell` | glue | shim; upstream `memio.c:159` |
//! | `mem_fseek` | `stream::mem_seek` | glue | shim; seek-to-exact-end rejected; carries C's 32-bit CUR/END wrap quirks verbatim -- never "fix" during a refactor; upstream `memio.c:164` |
//! | `mem_malloc` / `mem_free` / `mem_malloc_zero` | `stream::{alloc, dealloc, alloc_zeroed}` | glue | private helpers, Rust-only names (no C counterpart): cfg(test) global-allocator pair vs cfg(not(test)) zone-allocator pair; the cfg pairs must stay together or tests stop routing through the zone allocator |
//! | `memfile_mode_t` / `mem_rel_t` / `_MEMFILE` | `stream.rs` | data | type names kept; `#[repr(C)]` layouts unchanged |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave B5, whole module): every
//! function is memory-stream IO over caller buffers; no simulation
//! input ever passes through it in this tree, and the demo stream
//! never sees a MEMFILE. The module has no runtime consumers at all
//! today, which makes the verdict trivial; should the launcher/
//! DeHacked roadmap revive it, it stays on the load/asset side (boot-
//! fixed inputs), never on the per-tic path. Accordingly `memio` has
//! no `dtmc` submodule: every function adjudicates to `glue` (buffer
//! marshalling) or `data` (types) in the mapping table above.

pub mod stream;

//* path-stability re-export: the types keep their module-root paths.
pub use stream::{_MEMFILE, memfile_mode_t, mem_rel_t};

//* path-stability re-export: `mem_get_buf` was not renamed (already
//* plain English), so this is wiring, not a shim.
pub use stream::mem_get_buf;

//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_open_read as mem_fopen_read;
//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_read as mem_fread;
//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_open_write as mem_fopen_write;
//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_write as mem_fwrite;
//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_close as mem_fclose;
//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_tell as mem_ftell;
//* upstream-name shim: keeps the historical C stdio-like surface one
//* `pub use` away ("C parity surface"); nothing in-tree links these.
pub use stream::mem_seek as mem_fseek;
