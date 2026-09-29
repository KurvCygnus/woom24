//! File I/O: the libc `FILE` handle stand-in, the shared libc extern
//! block, and the engine's `M_*File*` wrappers over `fopen`/`fread`/
//! `fwrite`/`fseek`. The libc declarations are the module-wide plumbing;
//! sibling subfiles import them from here.

use std::ffi::{c_char, c_int, c_long, c_void};

use super::strings::string_join_array;
use crate::doom::z_zone::{PU_STATIC, Z_Malloc};
use crate::i_error;

/// Opaque stand-in for libc's `FILE *`. Pointers are passed through to the
/// `extern "C"` shims below; the contents are never accessed from Rust.
pub enum FILE {}

/// Unix directory separator character (C: `DIR_SEPARATOR = '/'`).
pub(super) const DIR_SEPARATOR: c_char = b'/' as c_char;
/// Unix directory separator string (C: `DIR_SEPARATOR_S = "/"`), with a
/// trailing NUL so the buffer can be passed to C string APIs.
pub(super) const DIR_SEPARATOR_S: &[u8] = b"/\0";

/// Mirror of libc's `SEEK_END` constant for use with [`fseek`].
const SEEK_END: c_int = 2;
/// Mirror of libc's `SEEK_SET` constant for use with [`fseek`].
const SEEK_SET: c_int = 0;

/// errno value reported by Linux when `fopen` fails because the path is a
/// directory. Used by [`file_exists`] to treat directories as existing.
const EISDIR: c_int = 21;

extern "C" {
    /// libc `fopen`: open `path` with `mode`, returns NULL on error.
    pub(super) fn fopen(path: *const c_char, mode: *const c_char) -> *mut FILE;
    /// libc `fread`: read up to `nmemb * size` bytes from `stream` into `ptr`.
    pub(super) fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut FILE) -> usize;
    /// libc `fwrite`: write up to `nmemb * size` bytes from `ptr` to `stream`.
    pub(super) fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut FILE) -> usize;
    /// libc `fseek`: reposition stream offset.
    pub(super) fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int;
    /// libc `ftell`: return current stream offset.
    pub(super) fn ftell(stream: *mut FILE) -> c_long;
    /// libc `fclose`: close the stream and flush any pending output.
    pub(super) fn fclose(stream: *mut FILE) -> c_int;
    /// libc `getenv`: look up an environment variable.
    pub(super) fn getenv(name: *const c_char) -> *mut c_char;
    /// libc `malloc`: allocate `size` bytes.
    pub(super) fn malloc(size: usize) -> *mut c_void;
    /// libc `calloc`: allocate and zero `nmemb * size` bytes.
    pub(super) fn calloc(nmemb: usize, size: usize) -> *mut c_void;
    /// libc `free`: release memory previously returned by `malloc`/`calloc`.
    pub(super) fn free(ptr: *mut c_void);
    /// libc `toupper`: convert an ASCII character to upper case.
    pub(super) fn toupper(c: c_int) -> c_int;
    /// libc `tolower`: convert an ASCII character to lower case.
    pub(super) fn tolower(c: c_int) -> c_int;
    /// libc `strlen`: length of a null-terminated string.
    pub(super) fn strlen(s: *const c_char) -> usize;
    /// libc `strcmp`: compare two null-terminated strings.
    pub(super) fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
    /// libc `strncmp`: compare the first `n` bytes of two strings.
    pub(super) fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int;
    /// libc `strncpy`: copy at most `n` bytes (without guaranteeing a NUL).
    pub(super) fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char;
    /// libc `strrchr`: locate the last occurrence of a byte in a string.
    pub(super) fn strrchr(s: *const c_char, c: c_int) -> *mut c_char;
    /// libc `strchr`: locate the first occurrence of a byte in a string.
    pub(super) fn strchr(s: *const c_char, c: c_int) -> *mut c_char;
    /// libc `strstr`: locate the first occurrence of a substring.
    pub(super) fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char;
}

/// Returns the current value of libc `errno` for the calling thread.
///
/// # Safety
///
/// Must be called from a context where libc has been initialised. The
/// caller must not retain the returned `int` across operations that may
/// reset `errno`.
unsafe fn errno() -> c_int {
    *crate::doom::crt::errno_location()
}

/// Create the directory at `path` with permissions `0o755`.
///
/// Wrapper around libc `mkdir`; the return value is ignored, so calling on
/// an already-existing directory is a no-op. Mirrors the Unix branch of the
/// C `M_MakeDirectory`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `m_config` imports the upstream name through the root shim.
#[doc(alias = "M_MakeDirectory")]
#[export_name = "M_MakeDirectory"]
pub extern "C" fn make_directory(path: *mut c_char) {
    unsafe {
        crate::doom::crt::mkdir(path, 0o755);
    }
}

/// Returns `1` if `filename` names an existing file or directory, else `0`.
///
/// Tries `fopen("r")`; on success the stream is immediately closed. If the
/// open fails because the path is a directory (errno `EISDIR`), this still
/// reports the entry as existing, matching the C original.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `v_video` / `g_game` import the upstream name through the root shim.
#[doc(alias = "M_FileExists")]
#[export_name = "M_FileExists"]
pub extern "C" fn file_exists(filename: *mut c_char) -> c_int {
    unsafe {
        let fstream = fopen(filename as *const c_char, c"r".as_ptr());
        if !fstream.is_null() {
            fclose(fstream);
            return 1;
        }
        (errno() == EISDIR) as c_int
    }
}

/// Returns the total length in bytes of an open file `handle`.
///
/// Saves the current position, seeks to end to read the length, then
/// restores the original position. The stream is left in its prior state.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `w_file` / `m_argv` import the upstream name through the root shim.
#[doc(alias = "M_FileLength")]
#[export_name = "M_FileLength"]
pub extern "C" fn file_length(handle: *mut FILE) -> c_long {
    unsafe {
        let savedpos = ftell(handle);
        fseek(handle, 0, SEEK_END);
        let length = ftell(handle);
        fseek(handle, savedpos, SEEK_SET);
        length
    }
}

/// Write `length` bytes from `source` to the file named `name`.
///
/// Returns `1` on success, `0` if the file could not be opened for writing
/// or if the short-write case is hit. The file is created/truncated (`"wb"`).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `v_video` / `g_game` import the upstream name through the root shim.
#[doc(alias = "M_WriteFile")]
#[export_name = "M_WriteFile"]
pub extern "C" fn write_file(name: *mut c_char, source: *mut c_void, length: c_int) -> c_int {
    unsafe {
        let handle = fopen(name as *const c_char, c"wb".as_ptr());
        if handle.is_null() {
            return 0;
        }
        let count = fwrite(source, 1, length as usize, handle);
        fclose(handle);
        if count < length as usize {
            return 0;
        }
        1
    }
}

/// Read the entire file `name` into a freshly allocated Z_Malloc buffer.
///
/// On success, `*buffer` is set to point at the allocated buffer and the
/// file length (in bytes) is returned. On failure (file missing or short
/// read), `I_Error` is invoked and the process exits.
///
/// The pre-move export symbol is kept with `#[export_name]` below
/// (dead-but-exported: zero in-tree callers, kept for wasm symbol-set
/// parity).
#[doc(alias = "M_ReadFile")]
#[export_name = "M_ReadFile"]
pub extern "C" fn read_file(name: *mut c_char, buffer: *mut *mut c_char) -> c_int {
    unsafe {
        let handle = fopen(name as *const c_char, c"rb".as_ptr());
        if handle.is_null() {
            i_error!(
                "Couldn't read file {}",
                std::ffi::CStr::from_ptr(name).to_string_lossy()
            );
        }
        let length = file_length(handle);
        let buf = Z_Malloc(length as c_int, PU_STATIC, std::ptr::null_mut());
        let count = fread(buf, 1, length as usize, handle);
        fclose(handle);
        if count < length as usize {
            i_error!(
                "Couldn't read file {}",
                std::ffi::CStr::from_ptr(name).to_string_lossy()
            );
        }
        *buffer = buf as *mut c_char;
        length as c_int
    }
}

/// Returns a heap-allocated path of the form `"/tmp/<s>"`.
///
/// Always uses `/tmp` on this port (the C original probes `TEMP` on Windows
/// or `__DJGPP__`). The returned string is allocated by [`string_join_array`]
/// and must be freed by the caller.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `g_game` imports the upstream name through the root shim.
#[doc(alias = "M_TempFile")]
#[export_name = "M_TempFile"]
pub extern "C" fn temp_file(s: *mut c_char) -> *mut c_char {
    let tempdir = c"/tmp".as_ptr();
    let sep = DIR_SEPARATOR_S.as_ptr() as *const c_char;
    let strs: [*const c_char; 4] = [tempdir, sep, s as *const c_char, std::ptr::null()];
    // SAFETY: null-terminated pointer array; ownership transferred to caller via return.
    string_join_array(strs.as_ptr())
}
