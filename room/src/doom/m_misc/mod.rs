//! Rust port of vendor/doomgeneric/m_misc.c.
//!
//! Miscellaneous helper routines used throughout the engine: file I/O
//! wrappers, string utilities (case-insensitive search, safe copy/concat,
//! variadic join, replace), filename helpers, integer parsing, the default
//! config directory helpers (HOME / XDG), and the Rust-side formatting
//! helpers `c_write!` / `DEH_snprintf!` / `i_error!` that replace the C
//! variadic helpers `M_snprintf` and `I_Error`.
//!
//! The C source supports both Windows and Unix paths; this port keeps only
//! the Unix branch (`DIR_SEPARATOR = '/'`). Several routines call back into
//! libc directly via `extern "C"` shims rather than reimplementing them.
//!
//! ## Submodule Responsibility
//!
//! - `files.rs` -- file I/O: the opaque `FILE` stand-in, the shared libc
//!   extern block, the directory/file wrappers, and `temp_file`
//! - `strings.rs` -- the `M_String*` family, case-insensitive search, and
//!   the in-place case converters
//! - `format.rs` -- the `snprintf` clamp post-processor, the buffer writers
//!   behind the crate-root macros, and the `c_write!` / `DEH_snprintf!` /
//!   `i_error!` macros themselves (`#[macro_export]`, so their
//!   `use crate::...` consumers are untouched by this split)
//! - `env.rs` -- integer parsing, the 8.3 lump-name extractor, and the
//!   HOME / XDG config-directory helpers
//!
//! The module root is documentation + wiring only: the `mod` declarations
//! and the re-exports below; no content lives here.
//!
//! ## Original Fn Name Mapping
//!
//! | Original (C) | New location | Surface | Notes |
//! |--------------|--------------|---------|-------|
//! | `M_MakeDirectory` | `files::make_directory` | glue | Unix branch only (`mkdir` 0o755, return ignored); C symbol pinned via `#[export_name]`; upstream `m_misc.c:55` |
//! | `M_FileExists` | `files::file_exists` | glue | directories count as existing via errno `EISDIR`; C symbol pinned; upstream `m_misc.c:66` |
//! | `M_FileLength` | `files::file_length` | glue | save / seek-end / restore; C symbol pinned; upstream `m_misc.c:90` |
//! | `M_WriteFile` | `files::write_file` | glue | `"wb"` truncate-on-open, short-write fails; C symbol pinned; upstream `m_misc.c:112` |
//! | `M_ReadFile` | `files::read_file` | glue | Z_Malloc'd buffer, `I_Error` on failure; C symbol pinned (dead-but-exported); upstream `m_misc.c:136` |
//! | `M_TempFile` | `files::temp_file` | glue | always `/tmp` on this port; joins via `string_join_array`; C symbol pinned; upstream `m_misc.c:167` |
//! | `M_StrToInt` | `env::str_to_int` | glue | `0x`/`0X` hex, leading-zero octal, decimal via `c_sscanf1`; C symbol pinned; upstream `m_misc.c:190` |
//! | `M_ExtractFileBase` | `env::extract_file_base` | glue | upper-cased 8.3 lump base with truncation warning; C symbol pinned; upstream `m_misc.c:198` |
//! | `M_ForceUppercase` | `strings::force_uppercase` | glue | byte-wise libc `toupper`; C symbol pinned; upstream `m_misc.c:243` |
//! | `M_ForceLowercase` | `strings::force_lowercase` | glue | port/chocolate addition (not in doomgeneric `m_misc.c`); C symbol pinned |
//! | `M_StrCaseStr` | `strings::str_case_str` | glue | case-insensitive `strstr` via `strncasecmp`; C symbol pinned (dead-but-exported); upstream `m_misc.c:259` |
//! | `M_StringDuplicate` | `strings::string_duplicate` | glue | `strdup` + `I_Error` on NULL; C symbol pinned (dead-but-exported); upstream `m_misc.c:292` |
//! | `M_StringReplace` | `strings::string_replace` | glue | two-pass length computation + substitution; C symbol pinned (dead-but-exported); upstream `m_misc.c:311` |
//! | `M_StringCopy` | `strings::string_copy` | glue | `strlcpy` semantics, `Boolean` result; C symbol pinned; upstream `m_misc.c:373` |
//! | `M_StringConcat` | `strings::string_concat` | glue | `strlcat` semantics; C symbol pinned (dead-but-exported); upstream `m_misc.c:394` |
//! | `M_StringStartsWith` | `strings::string_starts_with` | glue | strict `>` keeps the pinned C exact-match bug (see `strings.rs` tests); C symbol pinned (dead-but-exported); upstream `m_misc.c:409` |
//! | `M_StringEndsWith` | `strings::string_ends_with` | glue | `>=`, exact match is `TRUE`; C symbol pinned (dead-but-exported); upstream `m_misc.c:417` |
//! | `M_StringJoinA` | `strings::string_join_array` | glue | array-form port of the variadic `M_StringJoin` (`m_misc.c:426`); C symbol pinned |
//! | `M_snprintf_clamp` | `format::snprintf_clamp_c` | glue | C-linkage shim over `m_snprintf_clamp`, renamed per the naming ruling; C symbol pinned |
//! | `m_snprintf_clamp` (Rust-only) | `format::m_snprintf_clamp` | glue | name kept (already house style); `pub(crate)`, clamping only |
//! | `M_HomeDir` | `env::home_dir` | glue | Rust addition lifting the `getenv("HOME")` lookup; C symbol pinned (dead-but-exported) |
//! | `M_DefaultConfigDir` | `env::default_config_dir` | glue | Rust addition centralising XDG-aware resolution; C symbol pinned (dead-but-exported) |
//! | `M_OEMToUTF8` | `env::oem_to_utf8` | glue | Win32 stub, always NULL; dead-but-exported; upstream `m_misc.c:520` |
//! | `FILE` (enum) | `files::FILE` | data | opaque libc `FILE *` stand-in; name kept (`w_file` / `m_argv` alias it as `MiscFILE`) |
//! | `DIR_SEPARATOR` / `DIR_SEPARATOR_S` / `SEEK_END` / `SEEK_SET` / `EISDIR` | `files` | data | constants keep names, module-private |
//! | libc extern block (19 fns) | `files` | data | carried verbatim; `pub(super)` so the sibling subfiles import from it (the `w_wad::ffi` pattern) |
//! | `c_write!` / `DEH_snprintf!` | `format` (crate-root `#[macro_export]`) | glue | crate-root macro paths unchanged; bodies call `write_c_buf_ptr` through the module-root path kept by the `pub(crate) use` below |
//! | `i_error!` | `format` (crate-root `#[macro_export]`) | glue | routes to `i_system::I_Error`; the body's path updates ride `i_system`'s own graduation |
//!
//! ## Deterministic Aspects
//!
//! No dtmc surface by adjudication: every function here is string, file,
//! or format plumbing whose outputs are consumed at boot or by the
//! platform layer, never by the per-tic simulation. `str_to_int` feeds
//! cheat parsing and the `-setmem` boot parameter, which is config-ish
//! input parsing, still glue. The 32-test in-file suite reruns verbatim
//! after the split (20 string tests in `strings.rs`, 12 format tests in
//! `format.rs`), including the pinned upstream-bug test
//! `test_string_starts_with_exact_match_broken`.

pub mod env;
pub mod files;
pub mod format;
pub mod strings;

//* path-stability re-export: the opaque `FILE` stand-in keeps its
//* module-root path (`w_file` and `m_argv` alias it as `MiscFILE`).
pub use files::FILE;

//* path-stability re-export: the macros' bodies call this module-root
//* path (`$crate::doom::m_misc::write_c_buf_ptr`); stays `pub(crate)`
//* exactly like the definition.
pub(crate) use format::write_c_buf_ptr;

//* upstream-name shim: freeze-zone callers keep the upstream names. Each
//* shim is a plain `pub use` of ONE function item; the C symbol it
//* forwards to is re-pinned at the definition with
//* `#[export_name = "OriginalName"]`.
pub use env::{
    default_config_dir as M_DefaultConfigDir, extract_file_base as M_ExtractFileBase,
    home_dir as M_HomeDir, oem_to_utf8 as M_OEMToUTF8, str_to_int as M_StrToInt,
};
pub use files::{
    file_exists as M_FileExists, file_length as M_FileLength, make_directory as M_MakeDirectory,
    read_file as M_ReadFile, temp_file as M_TempFile, write_file as M_WriteFile,
};
pub use format::snprintf_clamp_c as M_snprintf_clamp;
pub use strings::{
    force_lowercase as M_ForceLowercase, force_uppercase as M_ForceUppercase,
    string_concat as M_StringConcat, string_copy as M_StringCopy,
    string_duplicate as M_StringDuplicate, string_ends_with as M_StringEndsWith,
    string_join_array as M_StringJoinA, string_replace as M_StringReplace,
    string_starts_with as M_StringStartsWith, str_case_str as M_StrCaseStr,
};
