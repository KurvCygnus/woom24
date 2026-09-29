//! C-buffer formatting: the clamping `snprintf` post-processor, the
//! Rust-side buffer writers behind the `c_write!` / `DEH_snprintf!`
//! macros, and the `i_error!` fatal-error macro. The macros are
//! `#[macro_export]`-ed at the crate root exactly as before, so their
//! `use crate::...` consumers are untouched; their bodies call
//! `write_c_buf_ptr` through the module-root path kept by the root's
//! `pub(crate) use` re-export.

use std::ffi::{c_char, c_int};

/// Clamp the return value of a previously-invoked `snprintf` and ensure the
/// destination is NUL-terminated.
///
/// `result` is the value returned by the underlying `snprintf` call. When
/// the buffer was truncated (`result < 0` or `result >= len`), the last
/// byte of `buf` is set to NUL and `len - 1` is returned; otherwise the
/// original `result` is propagated unchanged. Passing `len == 0` returns
/// `0` without writing.
///
/// This is a clamping helper only - it does NOT perform variadic
/// formatting. Callers must call `snprintf` (or the `c_write!` /
/// `DEH_snprintf!` macros) first to fill `buf`.
pub(crate) fn m_snprintf_clamp(buf: *mut c_char, len: usize, result: c_int) -> c_int {
    if len == 0 {
        return 0;
    }
    if result < 0 || result >= len as c_int {
        unsafe {
            *buf.add(len - 1) = 0;
        }
        (len as c_int) - 1
    } else {
        result
    }
}

/// C-linkage shim around `m_snprintf_clamp` for `extern "C"` callers.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game` / `d_main` import the upstream name through the root shim.
#[doc(alias = "M_snprintf_clamp")]
#[export_name = "M_snprintf_clamp"]
pub extern "C" fn snprintf_clamp_c(buf: *mut c_char, len: usize, result: c_int) -> c_int {
    m_snprintf_clamp(buf, len, result)
}

/// Fill a `[c_char; N]` buffer using Rust format syntax.
///
/// Equivalent to `snprintf(buf, len, fmt, args...)` followed by
/// `m_snprintf_clamp`. No persistent heap allocation.
#[macro_export]
macro_rules! c_write {
    ($buf:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {{
        let ptr = ::std::ptr::addr_of_mut!($buf);
        let s = ::std::format!($fmt $(, $arg)*);
        #[allow(unused_unsafe)]
        unsafe { $crate::doom::m_misc::write_c_buf_ptr(ptr, &s) }
    }};
}

/// Rust replacement for the C `DEH_snprintf` helper.
///
/// Formats into a null-terminated `[c_char]` buffer using Rust format syntax.
/// `DEH_String` is identity in this build; the format string is passed as a
/// Rust literal instead of a `*const c_char`.
#[macro_export]
macro_rules! DEH_snprintf {
    ($buf:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {{
        let ptr = ::std::ptr::addr_of_mut!($buf);
        let s = ::std::format!($fmt $(, $arg)*);
        #[allow(unused_unsafe)]
        unsafe { $crate::doom::m_misc::write_c_buf_ptr(ptr, &s) }
    }};
}

/// Format a message and call `I_Error`, which exits the process.
///
/// The `CString` is a temporary; it is valid for the duration of the call
/// because `I_Error` never returns. Panics if the formatted string contains
/// an interior null byte (game strings never embed `\0`).
#[doc(alias = "I_Error")]
#[doc(alias = "I_ErrorV")]
#[macro_export]
macro_rules! i_error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::doom::i_system::I_Error(
            ::std::ffi::CString::new(::std::format!($fmt $(, $arg)*))
                .unwrap()
                .as_ptr()
        )
    };
}

/// Writes a Rust `&str` into a `*mut [c_char; N]` buffer with truncation
/// and NUL termination.
///
/// Intended to be invoked by the `c_write!` and `DEH_snprintf!` macros via
/// `addr_of_mut!`, which avoids creating a `&mut` reference to a mutable
/// static. Bytes beyond the `N - 1` boundary are dropped; the final slot is
/// always set to `0`.
///
/// # Safety
///
/// `ptr` must be a valid, properly aligned pointer to an array of `N`
/// `c_char`s for the duration of the call. Aliasing rules must be observed
/// by the caller: no other `&` or `&mut` reference to the buffer may exist
/// during the call.
pub(crate) unsafe fn write_c_buf_ptr<const N: usize>(ptr: *mut [c_char; N], s: &str) {
    // SAFETY: ptr is valid for N c_chars; obtained via addr_of_mut! to avoid
    // creating a reference to a mutable static.
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr.cast::<c_char>(), N) };
    write_c_buf(slice, s);
}

/// Copy `s` into the `c_char` slice `buf`, truncating and NUL-terminating.
///
/// Writes at most `buf.len() - 1` bytes from `s`, then stores `0` in the
/// next slot. If `buf` is empty, the call is a no-op (no panic, no write).
pub(crate) fn write_c_buf(buf: &mut [c_char], s: &str) {
    if buf.is_empty() {
        return;
    }
    let n = s.len().min(buf.len() - 1);
    for (dst, src) in buf[..n].iter_mut().zip(s.bytes()) {
        *dst = src as c_char;
    }
    buf[n] = 0;
}

/// Unit tests for the formatting helpers in this subfile.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doom::crt::c_snprintf1;
    use std::ffi::CStr;

    // -----------------------------------------------------------------------
    // m_snprintf_clamp / M_snprintf_clamp
    //
    // m_snprintf_clamp is a clamping-only helper — it null-terminates and
    // clamps the return value of snprintf but does NOT perform any format-
    // string substitution itself.  Callers must invoke snprintf (or equivalent)
    // first to fill the buffer.
    // -----------------------------------------------------------------------

    /// `len == 0`: the clamp returns `0` without writing anywhere.
    #[test]
    fn test_snprintf_clamp_zero_len() {
        let mut buf = [0i8; 16];
        let r = m_snprintf_clamp(buf.as_mut_ptr(), 0, 5);
        assert_eq!(r, 0);
    }

    /// `result < len`: the underlying `snprintf` result is propagated unchanged.
    #[test]
    fn test_snprintf_clamp_no_truncation() {
        // result < len: the value is returned unchanged.
        let mut buf = [0i8; 16];
        let r = m_snprintf_clamp(buf.as_mut_ptr(), buf.len(), 5);
        assert_eq!(r, 5);
    }

    /// `result == len - 1`: no truncation occurs; the boundary case is
    /// returned verbatim.
    #[test]
    fn test_snprintf_clamp_exact_fit() {
        // result == len - 1: no truncation, value returned unchanged.
        let mut buf = [0i8; 16];
        let r = m_snprintf_clamp(buf.as_mut_ptr(), buf.len(), (buf.len() - 1) as c_int);
        assert_eq!(r, (buf.len() - 1) as c_int);
    }

    /// `result >= len`: the buffer is forced to a NUL at `buf[len - 1]`
    /// and the clamped length `len - 1` is returned.
    #[test]
    fn test_snprintf_clamp_truncation() {
        // result >= len: buffer is null-terminated at len-1, clamped value returned.
        let mut buf: Vec<i8> = b"ABCDEFGHIJKLMNOP".iter().map(|&b| b as i8).collect();
        let len = buf.len();
        let r = m_snprintf_clamp(buf.as_mut_ptr(), len, len as c_int);
        assert_eq!(r, (len - 1) as c_int);
        assert_eq!(buf[len - 1], 0);
    }

    /// Encoding error path: a negative `result` triggers the same NUL
    /// terminator + clamp as the over-length case.
    #[test]
    fn test_snprintf_clamp_error_result() {
        // result < 0 (encoding error): buffer is null-terminated at len-1, clamped.
        let mut buf = [b'X' as i8; 8];
        let len = buf.len();
        let r = m_snprintf_clamp(buf.as_mut_ptr(), len, -1);
        assert_eq!(r, (len - 1) as c_int);
        assert_eq!(buf[len - 1], 0);
    }

    /// Demonstrates that m_snprintf_clamp alone cannot substitute format
    /// arguments.  If the buffer already contains a format string like
    /// `"say %s"` and m_snprintf_clamp is called without a prior snprintf
    /// call, the format specifier is returned literally.
    ///
    /// This pins the known limitation: snprintf must always be called first
    /// to perform the actual substitution.
    #[test]
    fn test_snprintf_clamp_does_not_substitute_format_args() {
        let fmt = b"say %s\0";
        let mut buf = [0i8; 32];
        buf[..fmt.len()].copy_from_slice(unsafe {
            std::slice::from_raw_parts(fmt.as_ptr() as *const i8, fmt.len())
        });
        // Call m_snprintf_clamp without first calling snprintf.
        let r = m_snprintf_clamp(buf.as_mut_ptr(), buf.len(), (fmt.len() - 1) as c_int);
        assert_eq!(r, (fmt.len() - 1) as c_int);
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        // LIMITATION: the format specifier %s is NOT substituted —
        // m_snprintf_clamp is a clamping-only helper with no variadic support
        // and cannot capture format arguments.  The raw format string is
        // returned as-is.  snprintf must always be called first.
        assert_eq!(s, "say %s");
    }

    /// Demonstrates the correct two-step pattern that DOES substitute format
    /// arguments: call snprintf first (which captures the variadic args), then
    /// m_snprintf_clamp to clamp and null-terminate the result.
    #[test]
    fn test_snprintf_then_clamp_substitutes_format_args() {
        let mut buf = [0i8; 32];
        let result = unsafe {
            c_snprintf1(
                buf.as_mut_ptr(),
                buf.len(),
                c"say %s".as_ptr(),
                c"hello".as_ptr(),
            )
        };
        let r = m_snprintf_clamp(buf.as_mut_ptr(), buf.len(), result);
        assert_eq!(r, 9); // "say hello" is 9 characters
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        assert_eq!(s, "say hello");
    }

    // -----------------------------------------------------------------------
    // write_c_buf
    // -----------------------------------------------------------------------

    /// `write_c_buf` copies the string verbatim when it fits.
    #[test]
    fn test_write_c_buf_fits() {
        let mut buf: [c_char; 16] = [0; 16];
        write_c_buf(&mut buf, "hello");
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        assert_eq!(s, "hello");
    }

    /// Boundary case: `s.len() == buf.len() - 1` writes the whole source
    /// and stores the NUL in the final slot.
    #[test]
    fn test_write_c_buf_exact_fit() {
        // s.len() == buf.len() - 1: no truncation, null at last position
        let mut buf: [c_char; 6] = [0; 6];
        write_c_buf(&mut buf, "hello");
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        assert_eq!(s, "hello");
        assert_eq!(buf[5], 0);
    }

    /// Truncation case: extra source bytes are dropped, NUL terminator
    /// stored at `buf[len - 1]`.
    #[test]
    fn test_write_c_buf_truncates() {
        // s.len() > buf.len() - 1: truncated, null at buf[len-1]
        let mut buf: [c_char; 4] = [0; 4];
        write_c_buf(&mut buf, "hello");
        let s = unsafe { CStr::from_ptr(buf.as_ptr()).to_str().unwrap() };
        assert_eq!(s, "hel");
        assert_eq!(buf[3], 0);
    }

    /// Empty source writes a single NUL terminator at `buf[0]`.
    #[test]
    fn test_write_c_buf_empty_str() {
        let mut buf: [c_char; 8] = [0x42; 8];
        write_c_buf(&mut buf, "");
        assert_eq!(buf[0], 0);
    }

    /// Zero-length destination: the helper exits without panicking and
    /// without writing.
    #[test]
    fn test_write_c_buf_empty_buf() {
        // zero-length buffer: no panic, no write
        let mut buf: [c_char; 0] = [];
        write_c_buf(&mut buf, "hello"); // must not panic
    }
}
