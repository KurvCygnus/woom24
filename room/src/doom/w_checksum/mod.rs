//! Rust port of `vendor/doomgeneric/w_checksum.c`.
//!
//! Computes a SHA-1 hash over the WAD directory: for every lump it folds
//! the name, owning-WAD index, file offset and size into a `SHA1Context`.
//! The resulting digest is used by chocolate-doom for netgame consistency
//! checks (so all peers verify they loaded the same lumps). In this build
//! only `d_net`'s stub net path reaches it, through the extern-by-symbol
//! declaration in `d_net/mod.rs` (`fn W_Checksum(digest: *mut u8)` --
//! ABI-compatible with the Rust `*mut sha1_digest_t` parameter).
//!
//! ## Submodule Responsibility
//!
//! - `digest.rs` -- the `LumpInfo` layout-parallel directory mirror, the
//!   first-encounter per-file index assignment (`get_file_number`), the
//!   per-lump SHA-1 fold (`checksum_add_lump`), the
//!   `wad_directory_checksum` entry point, and the captured baseline
//!   vectors
//!
//! The module root is documentation + wiring only: the `mod` declaration
//! and the upstream-name shim below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `GetFileNumber` (file-static) | `digest::get_file_number` | glue | private in C too; first-encounter file numbering keeps the digest dependent only on directory order, never pointer values; upstream `vendor/doomgeneric/w_checksum.c:31` |
//! | `ChecksumAddLump` (file-static) | `digest::checksum_add_lump` | glue | private in C too; folds the 9-byte NUL-padded name, the file index, the position and the size, in that order and width -- byte-for-byte C parity is the digest contract; upstream `vendor/doomgeneric/w_checksum.c:57` |
//! | `W_Checksum` | `digest::wad_directory_checksum` | glue | the C symbol is pinned via `#[export_name]` at the renamed definition (`d_net/mod.rs:186` extern-declares it -- the one hard pin of this module); upstream `vendor/doomgeneric/w_checksum.c:68` |
//! | `lumpinfo_t` (C type) | `digest::LumpInfo` | data | layout-parallel mirror of `w_wad`'s graduated `lumpinfo_t`; only the checksummed fields are read, `cache`/`next` exist for layout parity; size parity pinned by a const assert plus the baseline test building real `w_wad::lumpinfo_t` entries |
//!
//! All three functions keep their `pub extern "C"`-shaped ABI
//! (`W_Checksum` was the only exported one); the two C file-statics were
//! already snake_case in the port and keep their names. The digest fold
//! uses the graduated `sha1` module's upstream-name functions
//! (`SHA1_Init` / `SHA1_UpdateInt32` / `SHA1_UpdateString` / `SHA1_Final`)
//! and `m_misc`'s `M_StringCopy` through their root shims.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: the digest is netgame-consistency
//! surface, not the demo synchronization surface (vanilla demo files
//! carry input only). Worth recording: the digest is order-stable by
//! construction -- `get_file_number` assigns per-WAD indices by
//! first-encounter order while iterating the directory, never by pointer
//! value, so the hash is stable across runs and hosts (baseline-pinned).
//! The C version keeps its `open_wadfiles` registry in a `realloc`-grown
//! global; this port allocates a local `Vec` inside
//! `wad_directory_checksum` for the duration of the call -- digest-neutral
//! (captured baseline vectors in `digest.rs` pin the bytes both ways).

pub mod digest;

//* upstream-name shim: freeze-zone callers keep the upstream names. The
//* `d_net` extern block links the C symbol directly; the shim keeps the
//* module-root Rust path stable for path callers.
pub use digest::wad_directory_checksum as W_Checksum;
