//! libc declarations for the WAD directory: the string and memory
//! primitives shared by `file.rs` (header parsing, directory growth)
//! and `lookup.rs` (case folding). Private module plumbing -- the
//! module root re-exports the names the sibling subfiles import.

use std::ffi::{c_char, c_int, c_void};

extern "C"
{
    /// libc: byte-equal compare of first `n` bytes.
    pub(super) fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int;
    /// libc: copy up to `n` bytes, NUL-padding the destination.
    pub(super) fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char;
    /// libc: NUL-terminated string length.
    pub(super) fn strlen(s: *const c_char) -> usize;
    /// libc: ASCII-upper-case.
    pub(super) fn toupper(c: c_int) -> c_int;

    /// libc: zero-initialised allocation.
    pub(super) fn calloc(nmemb: usize, size: usize) -> *mut c_void;
    /// libc: free a `malloc`/`calloc` block.
    pub(super) fn free(ptr: *mut c_void);
}
