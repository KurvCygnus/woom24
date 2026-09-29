//! Rust port of vendor/doomgeneric/w_file.c.
//!
//! Backing-store interface for WAD files. The original chocolate-doom
//! defines a `wad_file_class_t` vtable with three operations -
//! `W_StdC_OpenFile`, `W_StdC_CloseFile` and `W_StdC_Read` - and
//! optionally swaps in a `posix_wad_file` mmap-based class when
//! `-mmap` is passed and `HAVE_MMAP` is defined at build time. The
//! Rust port only ships the stdio-backed class: [`open_wad_file`] always
//! calls `stdc_open_file`, bypassing the `-mmap` switch entirely.
//! Behaviour is identical to a chocolate-doom build without
//! `HAVE_MMAP`.
//!
//! ## Submodule Responsibility
//!
//! - `stdc.rs` -- the `wad_file_class_t` / `wad_file_t` vtable types, the
//!   private `stdc_wad_file_t` descriptor, the three stdio backend
//!   functions, the exported `stdc_wad_file` vtable static, and the three
//!   dispatchers
//!
//! The module root is documentation + wiring only: the `mod` declaration
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `W_StdC_OpenFile` (file-static) | `stdc::stdc_open_file` | glue | stdio-backed vtable method; private file-static name kept in translated form (no shim needed); upstream `vendor/doomgeneric/w_file_stdc.c:33` |
//! | `W_StdC_CloseFile` (file-static) | `stdc::stdc_close_file` | glue | closes the `FILE *` and frees the descriptor; upstream `w_file_stdc.c` |
//! | `W_StdC_Read` (file-static) | `stdc::stdc_read` | glue | seek + fread through the descriptor; upstream `w_file_stdc.c` |
//! | `W_OpenFile` | `stdc::open_wad_file` | glue | dispatch collapses to the stdio backend (no mmap port); C symbol pinned via `#[export_name]` (callers `w_wad/file.rs` imports the upstream name through the root shim); upstream `vendor/doomgeneric/w_file.c` |
//! | `W_CloseFile` | `stdc::close_wad_file` | glue | vtable dispatch; C symbol pinned via `#[export_name]` (dead-but-exported: zero in-tree callers, kept for symbol-set byte-identity); upstream `w_file.c:85` |
//! | `W_Read` | `stdc::read_wad_bytes` | glue | vtable dispatch; C symbol pinned via `#[export_name]` (callers `w_wad/file.rs`, `w_wad/cache.rs` import the upstream name through the root shim); upstream `w_file.c` |
//! | `stdc_wad_file` (static) | `stdc::stdc_wad_file` | data | the exported vtable static keeps its upstream name and `#[no_mangle]` export |
//! | `wad_file_class_t` / `wad_file_t` / `stdc_wad_file_t` (types) | `stdc::` | data | FFI-shaped types keep names (`repr(C)` layouts unchanged) |
//!
//! The vtable-function-pointer pattern is load-bearing: `w_wad` casts
//! descriptors back to `stdc_wad_file_t` and calls through the vtable, so
//! the `unsafe extern "C"` signatures of the three backend functions are
//! kept in exact parity with the pre-move declarations.
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface: the module is WAD file I/O (open / close / byte reads),
//! the same load-time glue category `w_wad` adjudicates to. Given identical
//! WAD files the reads return identical bytes on every host; nothing here
//! participates in a tic's arithmetic or the demo synchronization surface.

mod stdc;

//* path-stability re-export: the FFI types and the exported vtable keep
//* their module-root paths (`w_wad/{file,state,cache}.rs`).
pub use stdc::{wad_file_class_t, wad_file_t, stdc_wad_file};

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use stdc::{close_wad_file as W_CloseFile, open_wad_file as W_OpenFile, read_wad_bytes as W_Read};
