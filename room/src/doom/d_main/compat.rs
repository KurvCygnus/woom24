//! The C-string helpers shared by `display`, `sequencing`, `identify`
//! and `boot`: the `DEH_String` identity seam (upstream
//! `FEATURE_DEHACKED` is undefined, so the function returns its
//! argument unchanged) and the case-insensitive comparison/formatting
//! wrappers over the libc string functions.

use std::ffi::c_char;

use crate::doom::crt::{strcasecmp, strncasecmp};

extern "C" {
    /// Returns the length of the null-terminated C string `s`, excluding the null terminator.
    fn strlen(s: *const c_char) -> usize;
}

/// Return the string as-is.  In the original C code this could be replaced
/// by dehacked patches, but since FEATURE_DEHACKED is not defined we
/// simply return the original pointer.
#[inline]
#[doc(alias = "DEH_String")]
pub(super) fn deh_string(s: *const c_char) -> *const c_char { s }

/// Return `true` if the C string `s` ends with `suffix` (case-insensitive).
/// Returns `false` for null pointers.
///
/// # Safety
///
/// Both `s` and `suffix`, if non-null, must point to valid, null-terminated C strings that remain
/// live for the duration of the call.
#[doc(alias = "c_str_ends_with")]
pub(super) unsafe fn ends_with_ci(s: *const c_char, suffix: *const c_char) -> bool {
    if s.is_null() || suffix.is_null() { return false; }
    let s_len = strlen(s);
    let suf_len = strlen(suffix);
    if suf_len > s_len { return false; }
    strncasecmp(s.add(s_len.wrapping_sub(suf_len)), suffix, suf_len) == 0
}

/// Return `true` if the two C strings are equal under a case-insensitive comparison.
/// Returns `false` if either pointer is null.
///
/// # Safety
///
/// Both `s1` and `s2`, if non-null, must point to valid, null-terminated C strings that remain
/// live for the duration of the call.
#[doc(alias = "c_str_eq")]
pub(super) unsafe fn eq_ci(s1: *const c_char, s2: *const c_char) -> bool {
    if s1.is_null() || s2.is_null() { return false; }
    strcasecmp(s1, s2) == 0
}

/// Return `true` if the first `n` bytes of the two C strings differ under a case-insensitive
/// comparison. Returns `true` (not-equal) if either pointer is null.
///
/// # Safety
///
/// Both `s1` and `s2`, if non-null, must point to valid C strings with at least `n` accessible
/// bytes that remain live for the duration of the call.
#[doc(alias = "c_str_ne_n")]
pub(super) unsafe fn ne_ci_n(s1: *const c_char, s2: *const c_char, n: usize) -> bool {
    if s1.is_null() || s2.is_null() { return true; }
    strncasecmp(s1, s2, n) != 0
}

/// Convert a raw C string to an owned `String`, returning empty string for null pointers.
///
/// # Safety
///
/// `s`, if non-null, must point to a valid, null-terminated C string that remains live for the
/// duration of the call.
#[doc(alias = "c_str_to_str")]
pub(super) unsafe fn to_lossy_string(s: *const c_char) -> String {
    if s.is_null() { return String::new(); }
    std::ffi::CStr::from_ptr(s).to_string_lossy().into_owned()
}
