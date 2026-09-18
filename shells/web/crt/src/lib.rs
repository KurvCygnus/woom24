//! Declaration-layer shim: stands in for the crates.io `libc` on wasm32 only,
//! so that paths like `libc::fopen` / `libc::printf` in `room` type-check
//! (spec 2 D1). The real implementations (in-memory VFS / malloc / printf
//! subset) live in `shells/web/src/wasm_vfs.rs`, exported under the same names
//! via `#[no_mangle]`, closing the references at final cdylib link time.
//!
//! Rule: this crate only declares, never implements; apart from the printf
//! family -- split into fixed-arity shapes at the wasm linker's demand --
//! signatures match libc 0.2 verbatim. The authoritative symbol list is
//! appendix A of docs/plans/2026-09-17-wasm-shell-implementation.md.

pub use std::ffi::{c_char, c_int, c_long, c_uint, c_void};

/// Opaque FILE handle (the same placeholder-enum trick as m_misc.rs; a
/// zero-variant enum cannot be repr(C)).
pub enum FILE {}

/// stdio seek constants (wasm_vfs::fseek supports only these three).
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;

extern "C" {
    pub fn fopen(path: *const c_char, mode: *const c_char) -> *mut FILE;
    pub fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut FILE) -> usize;
    pub fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut FILE) -> usize;
    pub fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int;
    pub fn ftell(stream: *mut FILE) -> c_long;
    pub fn fclose(stream: *mut FILE) -> c_int;
    pub fn fflush(stream: *mut FILE) -> c_int;
    // The printf family avoids `...`: wasm rust-lld strictly checks symbol
    // signatures, and each call site of a variadic declaration generates a
    // different wasm signature per argument count, matching no single
    // implementation -- lld swaps the mismatching call for a
    // `signature_mismatch` trap stub (runtime trap). So they are declared in
    // the engine-audited fixed-arity shapes (printf 0..4 variadics,
    // snprintf 1..2, see plan appendix A); usize slots = the common
    // pointer/integer width, with same-name exports provided by wasm_vfs.
    pub fn printf0(fmt: *const c_char) -> c_int;
    pub fn printf1(fmt: *const c_char, a0: usize) -> c_int;
    pub fn printf2(fmt: *const c_char, a0: usize, a1: usize) -> c_int;
    pub fn printf3(fmt: *const c_char, a0: usize, a1: usize, a2: usize) -> c_int;
    pub fn printf4(fmt: *const c_char, a0: usize, a1: usize, a2: usize, a3: usize) -> c_int;
    pub fn snprintf1(s: *mut c_char, n: usize, fmt: *const c_char, a0: usize) -> c_int;
    pub fn snprintf2(s: *mut c_char, n: usize, fmt: *const c_char, a0: usize, a1: usize) -> c_int;
    // sscanf likewise: the engine's only call shape is the single conversion
    // (M_StrToInt); a0 = the output pointer slot.
    pub fn sscanf1(s: *const c_char, fmt: *const c_char, a0: usize) -> c_int;
    pub fn puts(s: *const c_char) -> c_int;
    pub fn putchar(c: c_int) -> c_int;
    pub fn malloc(size: usize) -> *mut c_void;
    pub fn free(p: *mut c_void);
    pub fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn strlen(s: *const c_char) -> usize;
    pub fn remove(path: *const c_char) -> c_int;
    pub fn rename(old: *const c_char, new: *const c_char) -> c_int;
    // Second batch of CRT symbols (fix round 1): the authoritative roll-up of
    // the doom modules' local extern declarations; the implementations remain
    // wasm_vfs's same-name exports. atof is strtod-lite (no exponents -- the
    // engine's only call site, m_config.rs:688, always feeds plain decimals,
    // see the wasm_vfs audit); getenv always returns NULL (only
    // HOME/XDG_CONFIG_HOME, both with unset fallbacks); exit traps on wasm
    // (the engine has no normal exit path).
    pub fn toupper(c: c_int) -> c_int;
    pub fn tolower(c: c_int) -> c_int;
    pub fn isspace(c: c_int) -> c_int;
    pub fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
    pub fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int;
    pub fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char;
    pub fn strrchr(s: *const c_char, c: c_int) -> *mut c_char;
    pub fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char;
    pub fn getenv(name: *const c_char) -> *mut c_char;
    pub fn atof(s: *const c_char) -> f64;
    pub fn calloc(nmemb: usize, size: usize) -> *mut c_void;
    pub fn exit(status: c_int) -> !;
}
