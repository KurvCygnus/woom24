//! C-string helpers: the `M_String*` family (safe copy/concat/replace/join,
//! prefix/suffix probes, strdup), case-insensitive substring search, and the
//! in-place case converters. All operate on null-terminated `c_char`
//! pointers exactly as the C originals do.

use std::ffi::{c_char, c_int};

use super::files::{malloc, strcmp, strlen, strncmp, strncpy, strstr, tolower, toupper};
use crate::doom::crt::{strdup, strncasecmp};
use crate::i_error;
use crate::types::Boolean;

/// Case-insensitive `strstr`: find `needle` inside `haystack`.
///
/// Returns a pointer into `haystack` at the first match, or NULL if not
/// found or if `needle` is longer than `haystack`. Mirrors C `M_StrCaseStr`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_StrCaseStr")]
#[export_name = "M_StrCaseStr"]
pub extern "C" fn str_case_str(haystack: *mut c_char, needle: *mut c_char) -> *mut c_char {
    unsafe {
        let haystack_len = strlen(haystack);
        let needle_len = strlen(needle);
        if haystack_len < needle_len {
            return std::ptr::null_mut();
        }
        let len = haystack_len - needle_len;
        for i in 0..=len {
            if strncasecmp(haystack.add(i), needle, needle_len) == 0 {
                return haystack.add(i);
            }
        }
        std::ptr::null_mut()
    }
}

/// Safe `strdup` that aborts via `I_Error` if allocation fails.
///
/// Returns a malloc'd copy of `orig` that the caller must `free`.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_StringDuplicate")]
#[export_name = "M_StringDuplicate"]
pub extern "C" fn string_duplicate(orig: *const c_char) -> *mut c_char {
    unsafe {
        let result = strdup(orig);
        if result.is_null() {
            i_error!(
                "Failed to duplicate string (length {})\n",
                strlen(orig) as c_int
            );
        }
        result
    }
}

/// Replace every occurrence of `needle` in `haystack` with `replacement`.
///
/// Computes the final length in a first pass, allocates a single malloc'd
/// buffer, then performs the substitution in a second pass. The returned
/// pointer must be freed with `free`. On allocation failure the function
/// calls `I_Error` and returns NULL (unreachable).
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_StringReplace")]
#[export_name = "M_StringReplace"]
pub extern "C" fn string_replace(
    haystack: *const c_char,
    needle: *const c_char,
    replacement: *const c_char,
) -> *mut c_char {
    unsafe {
        let needle_len = strlen(needle);
        let mut result_len = strlen(haystack) + 1;
        let mut p = haystack;
        loop {
            p = strstr(p, needle);
            if p.is_null() {
                break;
            }
            p = p.add(needle_len);
            result_len = result_len
                .wrapping_add(strlen(replacement))
                .wrapping_sub(needle_len);
        }
        let result = malloc(result_len) as *mut c_char;
        if result.is_null() {
            i_error!("M_StringReplace: Failed to allocate new string");
        }
        let mut dst = result;
        let mut dst_len = result_len;
        p = haystack;
        while *p != 0 {
            if strncmp(p, needle, needle_len) == 0 {
                string_copy(dst, replacement, dst_len);
                p = p.add(needle_len);
                let rep_len = strlen(replacement);
                dst = dst.add(rep_len);
                dst_len = dst_len.wrapping_sub(rep_len);
            } else {
                *dst = *p;
                dst = dst.add(1);
                dst_len -= 1;
                p = p.add(1);
            }
        }
        *dst = 0;
        result
    }
}

/// `strlcpy`-style copy: writes at most `dest_size - 1` bytes plus a NUL.
///
/// Returns `TRUE` if the entire source string fit, `FALSE` if it was
/// truncated (or if `dest_size == 0`). Mirrors OpenBSD `strlcpy` semantics.
///
/// The pre-move export symbol is kept with `#[export_name]` below; the
/// freeze-zone callers import the upstream name through the root shim.
#[doc(alias = "M_StringCopy")]
#[export_name = "M_StringCopy"]
pub extern "C" fn string_copy(dest: *mut c_char, src: *const c_char, dest_size: usize) -> Boolean {
    unsafe {
        if dest_size >= 1 {
            *dest.add(dest_size - 1) = 0;
            strncpy(dest, src, dest_size - 1);
        } else {
            return Boolean::FALSE;
        }
        let len = strlen(dest);
        Boolean::from(*src.add(len) == 0)
    }
}

/// `strlcat`-style concat: appends `src` to `dest` without overrunning
/// `dest_size`, always leaving the destination NUL-terminated.
///
/// Returns `TRUE` if the entire source string fit, `FALSE` if it was
/// truncated. Mirrors OpenBSD `strlcat` semantics.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_StringConcat")]
#[export_name = "M_StringConcat"]
pub extern "C" fn string_concat(
    dest: *mut c_char,
    src: *const c_char,
    dest_size: usize,
) -> Boolean {
    unsafe {
        let mut offset = strlen(dest);
        if offset > dest_size {
            offset = dest_size;
        }
        string_copy(dest.add(offset), src, dest_size - offset)
    }
}

/// Returns `TRUE` if `s` starts with `prefix`.
///
/// Note: uses strict `>` against the prefix length (matching the C source),
/// so an exact-length match returns `FALSE`. See the pinned test
/// `test_string_starts_with_exact_match_broken` for details.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_StringStartsWith")]
#[export_name = "M_StringStartsWith"]
pub extern "C" fn string_starts_with(s: *const c_char, prefix: *const c_char) -> Boolean {
    unsafe {
        let s_len = strlen(s);
        let prefix_len = strlen(prefix);
        Boolean::from(s_len > prefix_len && strncmp(s, prefix, prefix_len) == 0)
    }
}

/// Returns `TRUE` if `s` ends with `suffix` (exact match also returns `TRUE`).
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_StringEndsWith")]
#[export_name = "M_StringEndsWith"]
pub extern "C" fn string_ends_with(s: *const c_char, suffix: *const c_char) -> Boolean {
    unsafe {
        let s_len = strlen(s);
        let suffix_len = strlen(suffix);
        Boolean::from(s_len >= suffix_len && strcmp(s.add(s_len - suffix_len), suffix) == 0)
    }
}

/// Array-form replacement for the C variadic `M_StringJoin`.
///
/// Takes a pointer to a NULL-terminated array of C-string pointers and
/// concatenates them into a single freshly malloc'd string. The result
/// must be freed by the caller. Aborts via `I_Error` on allocation
/// failure. Rust callers use this in place of the C variadic API; for the
/// few sites where the C entry point is still needed, a separate shim
/// wraps it.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `m_config`, `d_iwad` and `m_misc` itself import the upstream name
/// through the root shim.
#[doc(alias = "M_StringJoinA")]
#[export_name = "M_StringJoinA"]
pub extern "C" fn string_join_array(strs: *const *const c_char) -> *mut c_char {
    unsafe {
        let mut result_len: usize = 1;
        let mut p = strs;
        while !(*p).is_null() {
            result_len += strlen(*p);
            p = p.add(1);
        }

        let result = malloc(result_len) as *mut c_char;
        if result.is_null() {
            i_error!("M_StringJoinA: Failed to allocate new string");
        }

        let mut dst = result;
        p = strs;
        while !(*p).is_null() {
            let src = *p;
            let len = strlen(src);
            std::ptr::copy_nonoverlapping(src, dst, len);
            dst = dst.add(len);
            p = p.add(1);
        }
        *dst = 0;
        result
    }
}

/// Convert the null-terminated string `text` to upper case in place.
///
/// Iterates byte-by-byte using libc `toupper`. The C original only
/// implements the uppercase variant; the lowercase variant below is a
/// symmetric helper added by chocolate-doom / this port.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_ForceUppercase")]
#[export_name = "M_ForceUppercase"]
pub extern "C" fn force_uppercase(text: *mut c_char) {
    unsafe {
        let mut p = text;
        while *p != 0 {
            *p = toupper(*p as c_int) as c_char;
            p = p.add(1);
        }
    }
}

/// Convert the null-terminated string `text` to lower case in place.
///
/// Symmetric counterpart to [`force_uppercase`].
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_ForceLowercase")]
#[export_name = "M_ForceLowercase"]
pub extern "C" fn force_lowercase(text: *mut c_char) {
    unsafe {
        let mut p = text;
        while *p != 0 {
            *p = tolower(*p as c_int) as c_char;
            p = p.add(1);
        }
    }
}

/// Unit tests for the string helpers in this subfile.
///
/// The suite focuses on the routines whose semantics are easy to express in
/// pure-Rust fixtures - the `M_String*` family - plus pinned tests that
/// document known-broken behaviour inherited from the C source.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doom::m_misc::files::free;
    use std::ffi::{c_void, CString, CStr};

    // -----------------------------------------------------------------------
    // M_StringCopy
    // -----------------------------------------------------------------------

    /// Copy fits exactly within the destination - returns `TRUE` and the
    /// full source string is present.
    #[test]
    fn test_string_copy_fits() {
        let src = CString::new("Hello").unwrap();
        let mut dest: Vec<c_char> = vec![0; 10];
        let result = unsafe { string_copy(dest.as_mut_ptr(), src.as_ptr(), dest.len()) };
        assert_eq!(result, Boolean::TRUE);
        let cstr = unsafe { CStr::from_ptr(dest.as_ptr()) };
        assert_eq!(cstr.to_str().unwrap(), "Hello");
    }

    /// Copy is truncated to `dest_size - 1` bytes - returns `FALSE` and
    /// the destination is NUL-terminated.
    #[test]
    fn test_string_copy_truncation() {
        let src = CString::new("Hello, World!").unwrap();
        let mut dest: Vec<c_char> = vec![0; 6];
        let result = unsafe { string_copy(dest.as_mut_ptr(), src.as_ptr(), dest.len()) };
        assert_eq!(result, Boolean::FALSE);
        let cstr = unsafe { CStr::from_ptr(dest.as_ptr()) };
        assert_eq!(cstr.to_str().unwrap(), "Hello");
    }

    // -----------------------------------------------------------------------
    // M_StringConcat
    // -----------------------------------------------------------------------

    /// `M_StringConcat` succeeds when the combined string fits in the
    /// destination buffer.
    #[test]
    fn test_string_concat_fits() {
        let mut buf: Vec<c_char> = CString::new("Hello, ")
            .unwrap()
            .into_bytes_with_nul()
            .into_iter()
            .map(|b| b as c_char)
            .collect();
        buf.resize(20, 0);
        let suffix = CString::new("World!").unwrap();
        let result = unsafe { string_concat(buf.as_mut_ptr(), suffix.as_ptr(), buf.len()) };
        assert_eq!(result, Boolean::TRUE);
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        assert_eq!(cstr.to_str().unwrap(), "Hello, World!");
    }

    /// `M_StringConcat` truncates and returns `FALSE` when the combined
    /// length exceeds the destination buffer.
    #[test]
    fn test_string_concat_truncation() {
        let mut buf: Vec<c_char> = CString::new("Hello, ")
            .unwrap()
            .into_bytes_with_nul()
            .into_iter()
            .map(|b| b as c_char)
            .collect();
        buf.resize(11, 0);
        let suffix = CString::new("World!").unwrap();
        let result = unsafe { string_concat(buf.as_mut_ptr(), suffix.as_ptr(), buf.len()) };
        // "Hello, " (7) + "World!" (6) = 13 chars, but buffer is 11 => truncated to "Hello, Wor"
        assert_eq!(result, Boolean::FALSE);
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        assert_eq!(cstr.to_str().unwrap(), "Hello, Wor");
    }

    // -----------------------------------------------------------------------
    // M_StringStartsWith
    // -----------------------------------------------------------------------

    /// Basic prefix match: `"hello world"` begins with `"hello"`.
    #[test]
    fn test_string_starts_with_basic() {
        let s = CString::new("hello world").unwrap();
        let prefix = CString::new("hello").unwrap();
        let result = unsafe { string_starts_with(s.as_ptr(), prefix.as_ptr()) };
        assert_eq!(result, Boolean::TRUE);
    }

    /// BUG: M_StringStartsWith uses `s_len > prefix_len` (strictly greater
    /// than) instead of `>=`, so an exact-length match incorrectly returns 0.
    /// The correct return value for `M_StringStartsWith("hello", "hello")`
    /// would be 1 (true).  The same bug exists in the original C source; this
    /// test pins the current behaviour so that any future fix is immediately
    /// visible as a test failure that needs updating.
    #[test]
    fn test_string_starts_with_exact_match_broken() {
        let s = CString::new("hello").unwrap();
        let prefix = CString::new("hello").unwrap();
        let result = unsafe { string_starts_with(s.as_ptr(), prefix.as_ptr()) };
        // BUG: should be 1 — a string starts with itself.
        assert_eq!(result, Boolean::FALSE);
    }

    /// Negative case: `"world"` does not begin with `"hello"`.
    #[test]
    fn test_string_starts_with_no_match() {
        let s = CString::new("world").unwrap();
        let prefix = CString::new("hello").unwrap();
        let result = unsafe { string_starts_with(s.as_ptr(), prefix.as_ptr()) };
        assert_eq!(result, Boolean::FALSE);
    }

    /// A prefix longer than the string itself returns `FALSE`.
    #[test]
    fn test_string_starts_with_prefix_longer_than_string() {
        let s = CString::new("hi").unwrap();
        let prefix = CString::new("hello").unwrap();
        let result = unsafe { string_starts_with(s.as_ptr(), prefix.as_ptr()) };
        assert_eq!(result, Boolean::FALSE);
    }

    // -----------------------------------------------------------------------
    // M_StringEndsWith
    // -----------------------------------------------------------------------

    /// Basic suffix match: `"hello world"` ends with `"world"`.
    #[test]
    fn test_string_ends_with_basic() {
        let s = CString::new("hello world").unwrap();
        let suffix = CString::new("world").unwrap();
        let result = unsafe { string_ends_with(s.as_ptr(), suffix.as_ptr()) };
        assert_eq!(result, Boolean::TRUE);
    }

    /// Exact-length suffix match returns `TRUE` (unlike the starts-with
    /// counterpart, this case is intentionally allowed by the C source).
    #[test]
    fn test_string_ends_with_exact_match() {
        let s = CString::new("hello").unwrap();
        let suffix = CString::new("hello").unwrap();
        let result = unsafe { string_ends_with(s.as_ptr(), suffix.as_ptr()) };
        assert_eq!(result, Boolean::TRUE);
    }

    /// Negative case: `"hello"` does not end with `"world"`.
    #[test]
    fn test_string_ends_with_no_match() {
        let s = CString::new("hello").unwrap();
        let suffix = CString::new("world").unwrap();
        let result = unsafe { string_ends_with(s.as_ptr(), suffix.as_ptr()) };
        assert_eq!(result, Boolean::FALSE);
    }

    /// A suffix longer than the string itself returns `FALSE`.
    #[test]
    fn test_string_ends_with_suffix_longer_than_string() {
        let s = CString::new("hi").unwrap();
        let suffix = CString::new("hello world").unwrap();
        let result = unsafe { string_ends_with(s.as_ptr(), suffix.as_ptr()) };
        assert_eq!(result, Boolean::FALSE);
    }

    // -----------------------------------------------------------------------
    // M_StringReplace
    // -----------------------------------------------------------------------

    /// Test helper: release a malloc'd C string with libc `free`.
    ///
    /// # Safety
    ///
    /// `p` must be a non-null pointer returned by `malloc` (or
    /// `M_String*` routines that wrap it) and not previously freed.
    unsafe fn free_cstring(p: *mut c_char) {
        free(p as *mut c_void);
    }

    /// Single-occurrence replacement: `"world"` -> `"earth"` in `"hello world"`.
    #[test]
    fn test_string_replace_basic() {
        let haystack = CString::new("hello world").unwrap();
        let needle = CString::new("world").unwrap();
        let replacement = CString::new("earth").unwrap();
        let result = unsafe {
            string_replace(haystack.as_ptr(), needle.as_ptr(), replacement.as_ptr())
        };
        assert!(!result.is_null());
        let s = unsafe { CStr::from_ptr(result).to_str().unwrap().to_owned() };
        unsafe { free_cstring(result) };
        assert_eq!(s, "hello earth");
    }

    /// When the needle is absent, the haystack is returned verbatim
    /// in a fresh allocation.
    #[test]
    fn test_string_replace_not_found() {
        let haystack = CString::new("hello").unwrap();
        let needle = CString::new("world").unwrap();
        let replacement = CString::new("earth").unwrap();
        let result = unsafe {
            string_replace(haystack.as_ptr(), needle.as_ptr(), replacement.as_ptr())
        };
        assert!(!result.is_null());
        let s = unsafe { CStr::from_ptr(result).to_str().unwrap().to_owned() };
        unsafe { free_cstring(result) };
        assert_eq!(s, "hello");
    }

    /// Multiple non-overlapping occurrences are all replaced left-to-right.
    #[test]
    fn test_string_replace_multiple_occurrences() {
        let haystack = CString::new("aababab").unwrap();
        let needle = CString::new("ab").unwrap();
        let replacement = CString::new("cd").unwrap();
        let result = unsafe {
            string_replace(haystack.as_ptr(), needle.as_ptr(), replacement.as_ptr())
        };
        assert!(!result.is_null());
        let s = unsafe { CStr::from_ptr(result).to_str().unwrap().to_owned() };
        unsafe { free_cstring(result) };
        assert_eq!(s, "acdcdcd");
    }

    // -----------------------------------------------------------------------
    // M_StringJoinA
    // -----------------------------------------------------------------------

    /// Joining a single-element array reproduces the input string.
    #[test]
    fn test_string_join_a_single() {
        let s = CString::new("hello").unwrap();
        let strs: [*const c_char; 2] = [s.as_ptr(), std::ptr::null()];
        // SAFETY: null-terminated pointer array; result freed below with free_cstring.
        let result = unsafe { string_join_array(strs.as_ptr()) };
        assert!(!result.is_null());
        let out = unsafe { CStr::from_ptr(result).to_str().unwrap().to_owned() };
        unsafe { free_cstring(result) };
        assert_eq!(out, "hello");
    }

    /// Joining multiple elements concatenates them in order with no
    /// separator inserted.
    #[test]
    fn test_string_join_a_multiple() {
        let a = CString::new("hello").unwrap();
        let b = CString::new(", ").unwrap();
        let c = CString::new("world").unwrap();
        let strs: [*const c_char; 4] = [a.as_ptr(), b.as_ptr(), c.as_ptr(), std::ptr::null()];
        // SAFETY: null-terminated pointer array; result freed below with free_cstring.
        let result = unsafe { string_join_array(strs.as_ptr()) };
        assert!(!result.is_null());
        let out = unsafe { CStr::from_ptr(result).to_str().unwrap().to_owned() };
        unsafe { free_cstring(result) };
        assert_eq!(out, "hello, world");
    }

    /// An empty (NULL-only) list still returns a fresh allocation
    /// containing just the trailing NUL.
    #[test]
    fn test_string_join_a_empty_list() {
        let strs: [*const c_char; 1] = [std::ptr::null()];
        // SAFETY: null-terminated pointer array; result freed below with free_cstring.
        let result = unsafe { string_join_array(strs.as_ptr()) };
        assert!(!result.is_null());
        let out = unsafe { CStr::from_ptr(result).to_str().unwrap().to_owned() };
        unsafe { free_cstring(result) };
        assert_eq!(out, "");
    }

    // -----------------------------------------------------------------------
    // M_ForceUppercase / M_ForceLowercase
    // -----------------------------------------------------------------------

    /// `M_ForceUppercase` upper-cases an ASCII mixed-case string in place.
    #[test]
    fn test_force_uppercase() {
        let mut buf: Vec<c_char> = b"Hello, World!\0".iter().map(|&b| b as c_char).collect();
        unsafe { force_uppercase(buf.as_mut_ptr()) };
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        assert_eq!(s, "HELLO, WORLD!");
    }

    /// `M_ForceLowercase` lower-cases an ASCII mixed-case string in place.
    #[test]
    fn test_force_lowercase() {
        let mut buf: Vec<c_char> = b"Hello, World!\0".iter().map(|&b| b as c_char).collect();
        unsafe { force_lowercase(buf.as_mut_ptr()) };
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        assert_eq!(s, "hello, world!");
    }
}
