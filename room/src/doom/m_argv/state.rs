//! The process-wide argv pair and the private constants the other
//! submodules serve from.
//!
//! The statics keep their upstream names and `#[no_mangle]` linkage
//! (functions-only ruling): `doomgeneric.rs` and the `d_iwad` module
//! root (its verbatim extern block) declare them in legacy `extern "C"`
//! blocks and `doomgeneric_Create` writes them BY SYMBOL before
//! `D_DoomMain` runs. The web shell's
//! argv-anchor contract additionally requires the `myargv` array to
//! survive for the process lifetime (`shells/web/src/init_pipeline.rs`).

use std::ffi::c_char;

/// Path separator used to extract the executable basename in
/// `M_GetExecutableName`. Matches `DIR_SEPARATOR` from the C headers
/// (always `'/'` in this port; the upstream code uses `'\\'` on Windows).
pub(super) const DIR_SEPARATOR: c_char = b'/' as c_char;

/// Maximum number of arguments produced by a response-file expansion,
/// matching `MAXARGVS` in `m_argv.c`.
pub(super) const MAXARGVS: usize = 100;

/// `int myargc` — process argument count. Exported with C linkage so the
/// vendored C TUs share the same definition.
#[no_mangle]
pub static mut myargc: std::ffi::c_int = 0;

/// `char **myargv` — process argument vector. Exported with C linkage so
/// the vendored C TUs share the same definition.
#[no_mangle]
pub static mut myargv: *mut *mut c_char = std::ptr::null_mut();
