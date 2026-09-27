//! Self-contained SHA-1 streaming implementation (GnuPG lineage) used
//! by the net-consistency checksum: the 80-round compression function
//! and big-endian message schedule, byte-faithful to
//! `vendor/doomgeneric/sha1.c`.
//!
//! *Provenance:* the vendored `sha1.c` copy carries a GPL-3 GnuPG
//! license header; this project inherits GPL-2.0 via room. Recorded
//! as provenance -- do not relabel.
//!
//! ## Submodule Responsibility
//!
//! - `context.rs` -- the `SHA1Context` streaming state and the
//!   `sha1_digest_t` alias
//! - `compress.rs` -- the compression function (`transform`) with the
//!   `rol` helper, the `M!` schedule-extension and `R!` step macros,
//!   and the round functions / constants
//! - `stream.rs` -- the streaming API: `init`, `update`, `finalize`,
//!   `update_int32`, `update_string`, the private `update_stream`
//!   core, and the RFC-vector test module
//!
//! The module root is documentation + wiring only: the `mod`
//! declarations, the type re-exports, and the upstream-name shims
//! below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! Per the maintainer ruling of 2026-09-27 (m_fixed naming pattern):
//! plain-English internal names with `#[doc(alias = "OriginalName")]`,
//! upstream-name shims at this root, `#[no_mangle]` dropped with the
//! rename (no extern declarers anywhere -- `w_checksum.rs` imports by
//! path, `d_net.rs` declares `W_Checksum`, not the SHA1_ functions).
//! Each function keeps its pre-move ABI kind: `init` was SAFE
//! `pub extern "C"`, the other four were `pub unsafe extern "C"` and
//! stay unsafe (precedent parity). No pins.
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `SHA1_Init` | `stream::init` | glue | shim; standard IV, leaves `buf` untouched; upstream `vendor/doomgeneric/sha1.c:40` |
//! | `SHA1_Update` | `stream::update` | glue | shim; null buffer = flush request (C convention carried); upstream `sha1.c:198` |
//! | `SHA1_Final` | `stream::finalize` | glue | shim; `final` is a Rust keyword -- `finalize` is the plain-English resolution; context is spent afterwards (re-init to reuse); upstream `sha1.c:238` |
//! | `SHA1_UpdateInt32` | `stream::update_int32` | glue | shim; big-endian u32 feed; upstream `sha1.c:303` |
//! | `SHA1_UpdateString` | `stream::update_string` | glue | shim; feeds the NUL terminator (intentional, C convention); upstream `sha1.c:315` |
//! | `sha1_update` (Rust-only safe core) | `stream::update_stream` | glue | private; Rust-only name, renamed per the functions ruling; the `Option<&[u8]>` flush shape and the aliasing `buf_copy` snapshots are load-bearing |
//! | `Transform` (file-static) | `compress::transform` | glue | pub(super) within the module (stream.rs is the only consumer); NO rename -- already plain English (m_random precedent); shape-identical to `sha1.c:55-195`: byte-assembled big-endian schedule, wrapping arithmetic -- the RFC vectors pin the digests, never "modernise" |
//! | `rol` / `M!` / `R!` / `f1..f4` / `K1..K4` | `compress.rs` | glue | private helpers kept; `R!` shuffles fixed-name variables where C rotates macro arguments (algorithmically equivalent, module doc) |
//! | `sha1_context_t` → `SHA1Context` | `context::SHA1Context` | data | type name kept; `#[repr(C)]` layout unchanged |
//! | `sha1_digest_t` | `context::sha1_digest_t` | data | `[u8; 20]` alias kept |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication (F10 wave B5, whole module): all
//! five public functions plus `transform` / `update_stream` are pure
//! functions -- the stated dtmc-candidate shape -- but the criterion
//! requires demo-observable CONSUMERS, and the only consumer chain
//! here terminates in the stubbed net-consistency cookie
//! (`d_net.rs`'s `NetConnectDataT.wad_sha1sum`, written at boot,
//! read only by the optional-multiplayer layer that must never shape
//! the core loop). No per-tic or demo-path function hashes anything.
//! Honest forward-looking caveat: this is the module most likely to
//! GAIN a dtmc surface if a future tier (MBF21/ID24 demo checksum
//! features, prboom-style demo-lump digest annotations) makes demo
//! inputs hash-observable -- at that point the pure core in
//! `compress.rs` / `stream.rs` is a clean dtmc extraction candidate;
//! do not pre-extract now. Accordingly `sha1` has no `dtmc`
//! submodule: every function adjudicates to `glue` (boot-time
//! integrity hashing) or `data` (types) in the mapping table above.

pub mod compress;
pub mod context;
pub mod stream;

//* path-stability re-export: the digest vocabulary keeps its
//* module-root paths (`w_checksum.rs`).
pub use context::{sha1_digest_t, SHA1Context};

//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use stream::finalize as SHA1_Final;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use stream::init as SHA1_Init;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use stream::update as SHA1_Update;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use stream::update_int32 as SHA1_UpdateInt32;
//* upstream-name shim: freeze-zone callers keep the upstream names.
pub use stream::update_string as SHA1_UpdateString;
