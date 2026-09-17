//! Portable C-runtime compatibility shims for the ported modules.
//!
//! The ported modules call the C runtime directly, mirroring the C
//! original. MSVC's UCRT, however, lacks the POSIX names `strcasecmp`,
//! `strncasecmp`, `strdup`, `mkdir`, and `__errno_location` — the vendored
//! C side solves the same problem in `doomtype.h`
//! (`#define strcasecmp _stricmp`), and the `wasm32-unknown-unknown`
//! target has no CRT at all. This module provides the portable names for
//! the Rust side:
//!
//! - The case-insensitive comparators are reimplemented in Rust. Every
//!   call site compares ASCII (IWAD names, lump names, argument words),
//!   so ASCII case folding matches the C originals' behavior in the
//!   "C" locale.
//! - `strdup`, `mkdir`, and `errno_location` forward to the POSIX or
//!   UCRT symbol depending on the target. On targets with no CRT at all
//!   (notably `wasm32-unknown-unknown`) they are still compiled so the
//!   crate type-checks, but panic with a clear message if ever called;
//!   the engine's wasm entry points must use the Rust-side I/O paths
//!   instead.

use std::ffi::{c_char, c_int};

#[cfg(not(target_arch = "wasm32"))]
extern "C" {
    #[cfg(unix)]
    #[link_name = "strdup"]
    fn posix_strdup(s: *const c_char) -> *mut c_char;
    #[cfg(windows)]
    #[link_name = "_strdup"]
    fn ucrt_strdup(s: *const c_char) -> *mut c_char;

    #[cfg(unix)]
    #[link_name = "mkdir"]
    fn posix_mkdir(path: *const c_char, mode: u32) -> c_int;
    #[cfg(windows)]
    #[link_name = "_mkdir"]
    fn ucrt_mkdir(path: *const c_char) -> c_int;

    #[cfg(unix)]
    #[link_name = "__errno_location"]
    fn posix_errno_location() -> *mut c_int;
    #[cfg(windows)]
    #[link_name = "_errno"]
    fn ucrt_errno() -> *mut c_int;
}

/// Case-insensitive comparison of two NUL-terminated C strings,
/// mirroring C `strcasecmp` (per-byte `tolower`, ASCII only).
///
/// Reimplemented in Rust rather than declared `extern "C"`: MSVC has no
/// `strcasecmp` symbol and `wasm32-unknown-unknown` has no libc.
pub fn strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int {
    strncasecmp(s1, s2, usize::MAX)
}

/// Case-insensitive comparison of at most `n` bytes of two
/// NUL-terminated C strings, mirroring C `strncasecmp` (ASCII only).
pub fn strncasecmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    unsafe {
        let mut i: usize = 0;
        while i < n {
            let b1 = *s1.add(i) as u8;
            let b2 = *s2.add(i) as u8;
            let l1 = b1.to_ascii_lowercase();
            let l2 = b2.to_ascii_lowercase();
            if l1 != l2 {
                return c_int::from(l1) - c_int::from(l2);
            }
            if b1 == 0 {
                return 0;
            }
            i += 1;
        }
    }
    0
}

/// Duplicates a NUL-terminated C string into a freshly allocated block
/// (POSIX `strdup` / UCRT `_strdup`).
///
/// # Safety
///
/// `s` must point to a valid, NUL-terminated C string.
pub unsafe fn strdup(s: *const c_char) -> *mut c_char {
    #[cfg(unix)]
    {
        posix_strdup(s)
    }
    #[cfg(windows)]
    {
        ucrt_strdup(s)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = s;
        panic!("no CRT on wasm: strdup() is unavailable on this target")
    }
}

/// Creates a directory (POSIX `mkdir` / UCRT `_mkdir`; the mode argument
/// is ignored on Windows).
///
/// # Safety
///
/// `path` must point to a valid, NUL-terminated C string.
pub unsafe fn mkdir(path: *const c_char, _mode: u32) -> c_int {
    #[cfg(unix)]
    {
        posix_mkdir(path, _mode)
    }
    #[cfg(windows)]
    {
        ucrt_mkdir(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        panic!("no CRT on wasm: mkdir() is unavailable on this target")
    }
}

/// Returns a pointer to the calling thread's C `errno`
/// (glibc `__errno_location` / UCRT `_errno`).
///
/// Panics on targets with no CRT (`wasm32-unknown-unknown`).
pub fn errno_location() -> *mut c_int {
    #[cfg(unix)]
    unsafe {
        posix_errno_location()
    }
    #[cfg(windows)]
    unsafe {
        ucrt_errno()
    }
    #[cfg(not(any(unix, windows)))]
    {
        panic!("no CRT on wasm: errno is unavailable on this target")
    }
}

#[cfg(test)]
mod tests {
    use super::{strcasecmp, strncasecmp};
    use std::ffi::c_char;

    /// Interprets a NUL-terminated byte literal as a C string pointer.
    fn cstr(b: &[u8]) -> *const c_char {
        b.as_ptr().cast::<c_char>()
    }

    #[test]
    fn strcasecmp_mirrors_c_semantics() {
        // Equal modulo ASCII case.
        assert_eq!(
            strcasecmp(cstr(b"ABCDEFGHIJKLMNOP\0"), cstr(b"abcdefghijklmnop\0")),
            0
        );
        // Ordering is by lowercased byte value.
        assert!(strcasecmp(cstr(b"ABC\0"), cstr(b"ABD\0")) < 0);
        assert!(strcasecmp(cstr(b"abd\0"), cstr(b"ABC\0")) > 0);
        // A prefix compares as smaller, matching strcmp.
        assert!(strcasecmp(cstr(b"ABC\0"), cstr(b"AB\0")) > 0);
        // The NUL terminator itself compares equal first.
        assert_eq!(strcasecmp(cstr(b"AB\0"), cstr(b"ab\0")), 0);
    }

    #[test]
    fn strncasecmp_compares_at_most_n_bytes() {
        assert_eq!(strncasecmp(cstr(b"ABCDE\0"), cstr(b"abcde\0"), 5), 0);
        // Differences past the n-byte window are ignored: with n = 3 only
        // "ABC" vs "abc" is inspected, so the later D/X mismatch is not seen.
        assert_eq!(strncasecmp(cstr(b"ABCDE\0"), cstr(b"abcX\0"), 3), 0);
        // Within the window the difference is the lowercased byte delta.
        assert!(strncasecmp(cstr(b"ABCDE\0"), cstr(b"abcX\0"), 4) < 0);
        // An n larger than the strings stops at the NUL bytes.
        assert_eq!(strncasecmp(cstr(b"AB\0"), cstr(b"ab\0"), 100), 0);
        // n == 0 always compares equal, mirroring C.
        assert_eq!(strncasecmp(cstr(b"ABCDE\0"), cstr(b"XXXXX\0"), 0), 0);
    }
}
