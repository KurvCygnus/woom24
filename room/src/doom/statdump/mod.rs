//! End-of-level statistics capture: each time the intermission screen
//! posts its `wbstartstruct_t`, `StatCopy` records a copy in the
//! module-private `captured_stats` buffer (up to `MAX_CAPTURES`
//! entries) so a later `StatDump` invocation could dump them all --
//! bit-exact with `vendor/doomgeneric/statdump.c`.
//!
//! ## Submodule Responsibility
//!
//! - `types.rs` -- the trimmed `repr(C)` `wbstartstruct_t` /
//!   `wbplayerstruct_t` mirror structs and their size-pinning tests
//! - `capture.rs` -- the capture buffer (`captured_stats`,
//!   `num_captured_stats`, `MAX_CAPTURES`), the libc `memcpy`
//!   extern, `StatCopy`, the `StatDump` no-op stub, and their tests
//!
//! The module adjudicated with no `dtmc` surface: there is no
//! `dtmc.rs`, and the reasoning is stated under Deterministic
//! Aspects. The module root is documentation + wiring only: the `mod`
//! declarations and the re-exports below keep every existing consumer
//! path valid (`crate::doom::statdump::*` -- the `StatCopy` import in
//! `g_game/actions.rs` and the fully-qualified
//! `crate::doom::statdump::wbstartstruct_t` cast in
//! `g_game/actions.rs:do_completed`); no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `StatCopy` | `capture::StatCopy` | glue | write-only diagnostic side-channel gated by the `-statdump` CLI parm (`m_argv::M_ParmExists`, a one-way graduated-to-freeze-zone edge): mutates only module-private state nothing reads, so nothing here can reach the demo sequence; `#[no_mangle]` retained (path-referenced by `g_game/actions.rs`); upstream `statdump.c:333-341` |
//! | `StatDump` | `capture::StatDump` | glue | no-op stub -- the entire dump implementation (banner printing, par-time tables, `PrintFragsTable`, gamemode discovery) sits behind `#if ORIGCODE` upstream (`statdump.c:345-390`) and is correctly not ported; never called in-tree (`d_main.rs:1916-1919` only echoes a message when the parm is passed); `#[no_mangle]` retained so graduation changes nothing about the linker's current dead-strip decision; upstream `statdump.c:343` |
//! | `wbstartstruct_t` | `types::wbstartstruct_t` | data | trimmed 4-field `repr(C)` variant of the C `d_player.h:182-203` struct; see the `//?` layout-mismatch record in `types.rs` -- deliberately NOT repaired during graduation (move as-is, fix later) |
//! | `wbplayerstruct_t` | `types::wbplayerstruct_t` | data | lacks the C `score` field (36 B vs the C 40 B, `d_player.h:168-180`); same `//?` record |
//! | `captured_stats` / `num_captured_stats` / `MAX_CAPTURES` | `capture` private statics | data | capture ring, private exactly like the C file-statics (`statdump.c:56-58`) |
//!
//! No symbol was renamed, so there are no boundary shims and nothing
//! qualified for a `#[no_mangle]` drop or an `#[export_name]` pin;
//! no link anchor is added -- `StatDump` has zero references and
//! adding one could change the artifact (its current
//! dead-strip-or-not state is pre-existing and survives unchanged).
//! No compiled C translation unit references any of these symbols
//! (`doomgeneric-sys/build.rs` excludes statdump.c), so the retention
//! is pure conservatism (zero wasm export churn). The libc `memcpy`
//! extern stays an FFI call (an `std::ptr::copy_nonoverlapping`
//! rewrite is post-graduation territory): byte-identical, and it
//! documents the C provenance.
//!
//! ## Deterministic Aspects
//!
//! Module adjudicated with no `dtmc` surface: `StatCopy` copies
//! demo-relevant bytes (the intermission `wminfo`) into a buffer
//! nothing reads, so the copy changes no observable -- not the sim
//! state, not the state hash, not the demo stream. `StatDump` never
//! runs. The capture path is preserved byte-for-byte (including the
//! knowingly-wrong layout record) so a future hook can iterate
//! `captured_stats`; the F9 golden demo tests are the net under the
//! call site in `g_game/actions.rs` (the graduated demo driver).

pub mod capture;
pub mod types;

//* path-stability re-export: the two mirror structs keep their
//* module-root paths (the fully-qualified cast and the import neighborhood in
//* `g_game/actions.rs:do_completed`).
pub use types::{wbplayerstruct_t, wbstartstruct_t};

//* path-stability re-export: `StatCopy` keeps its module-root path
//* (g_game/actions.rs); `StatDump` stays exported for surface stability.
pub use capture::{StatCopy, StatDump};
