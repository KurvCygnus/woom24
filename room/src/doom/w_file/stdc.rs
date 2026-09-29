//! The stdio-backed WAD file backend: the vtable types, the descriptor, the
//! three stdio operations, the exported vtable static, and the dispatchers.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_long, c_uint, c_void};
use std::mem::size_of;
use std::ptr;

use libc::{fclose, fopen, fread, fseek, FILE, SEEK_SET};

/// Vtable describing how to open / close / read one flavour of WAD-file
/// backing store. Mirrors the C `wad_file_class_t`. `#[repr(C)]` so the
/// struct layout matches the C definition exactly; `OpenFile`,
/// `CloseFile` and `Read` are `unsafe extern "C" fn` so they call with
/// the C ABI.
#[repr(C)]
pub struct wad_file_class_t {
    /// Open the WAD at `path` and return a heap-allocated descriptor, or
    /// null on failure.
    pub OpenFile: unsafe extern "C" fn(*mut c_char) -> *mut wad_file_t,
    /// Close and free the descriptor returned by `OpenFile`.
    pub CloseFile: unsafe extern "C" fn(*mut wad_file_t),
    /// Read `buffer_len` bytes from `offset` in the WAD into `buffer`.
    /// Returns the number of bytes actually read.
    pub Read: unsafe extern "C" fn(*mut wad_file_t, c_uint, *mut c_void, usize) -> usize,
}

/// Public WAD-file descriptor base class. The stdio backend extends this
/// in-place via `stdc_wad_file_t`. `#[repr(C)]` keeps the layout
/// compatible with the C `wad_file_t` typedef so the lump subsystem can
/// access `mapped` and `length` regardless of which backend opened it.
#[repr(C)]
pub struct wad_file_t {
    /// Backend vtable; usually points at [`stdc_wad_file`].
    pub file_class: *mut wad_file_class_t,
    /// Pointer to a memory-mapped image of the WAD if the backend
    /// supports it; null for the stdio backend.
    pub mapped: *mut u8,
    /// Total file size in bytes.
    pub length: c_uint,
}

/// Stdio-backed WAD descriptor. Layout is `wad_file_t` followed by the
/// owning `FILE *`, mirroring the C `stdc_wad_file_t` struct so that a
/// `*mut wad_file_t` can be cast back to `*mut stdc_wad_file_t` whenever
/// the vtable methods need access to the underlying file handle.
#[repr(C)]
struct stdc_wad_file_t {
    /// Embedded base descriptor. Must be the first field so the cast
    /// from `*mut wad_file_t` is safe.
    wad: wad_file_t,
    /// Open C `FILE *` used by `fseek` / `fread` / `fclose`.
    fstream: *mut FILE,
}

use crate::doom::m_misc::{M_FileLength, FILE as MiscFILE};
use crate::doom::z_zone::{Z_Free, Z_Malloc, PU_STATIC};

/// Stdio-backed `OpenFile` implementation. Opens `path` for reading in
/// binary mode, allocates a [`stdc_wad_file_t`] in the static zone heap,
/// queries the file length and wires up the vtable. Returns null if
/// either the file cannot be opened or the allocation fails.
///
/// # Safety
///
/// `path` must point to a valid null-terminated C string. The returned
/// pointer (if non-null) is owned by the caller and must eventually be
/// passed to [`stdc_close_file`].
unsafe extern "C" fn stdc_open_file(path: *mut c_char) -> *mut wad_file_t {
    let fstream = fopen(path as *const c_char, c"rb".as_ptr());
    if fstream.is_null() {
        return ptr::null_mut();
    }

    let result = Z_Malloc(
        size_of::<stdc_wad_file_t>() as c_int,
        PU_STATIC,
        ptr::null_mut(),
    ) as *mut stdc_wad_file_t;

    if result.is_null() {
        fclose(fstream);
        return ptr::null_mut();
    }

    (*result).wad.file_class = std::ptr::addr_of_mut!(stdc_wad_file);
    (*result).wad.mapped = ptr::null_mut();
    (*result).wad.length = M_FileLength(fstream as *mut MiscFILE) as c_uint;
    (*result).fstream = fstream;

    &mut (*result).wad
}

/// Stdio-backed `CloseFile` implementation. Closes the owning `FILE *`
/// and frees the descriptor allocated by [`stdc_open_file`].
///
/// # Safety
///
/// `wad` must be a non-null pointer originally returned by
/// [`stdc_open_file`].
unsafe extern "C" fn stdc_close_file(wad: *mut wad_file_t) {
    let stdc_wad = wad as *mut stdc_wad_file_t;
    fclose((*stdc_wad).fstream);
    Z_Free(stdc_wad as *mut c_void);
}

/// Stdio-backed `Read` implementation. Seeks to `offset` from the start
/// of the file and reads up to `buffer_len` bytes into `buffer`. Returns
/// the number of bytes actually transferred (which may be less than
/// `buffer_len` at end of file or on error).
///
/// # Safety
///
/// `wad` must be a non-null pointer originally returned by
/// [`stdc_open_file`]. `buffer` must point to writable memory of at
/// least `buffer_len` bytes.
unsafe extern "C" fn stdc_read(
    wad: *mut wad_file_t,
    offset: c_uint,
    buffer: *mut c_void,
    buffer_len: usize,
) -> usize {
    let stdc_wad = wad as *mut stdc_wad_file_t;
    fseek((*stdc_wad).fstream, offset as c_long, SEEK_SET);
    fread(buffer, 1, buffer_len, (*stdc_wad).fstream)
}

/// Concrete vtable for the stdio backend. Exported with C linkage so
/// other translation units can reference it as `extern wad_file_class_t
/// stdc_wad_file`. This is the only backend the Rust port instantiates;
/// the chocolate-doom mmap backend is not ported.
#[no_mangle]
pub static mut stdc_wad_file: wad_file_class_t = wad_file_class_t {
    OpenFile: stdc_open_file,
    CloseFile: stdc_close_file,
    Read: stdc_read,
};

/// Open a WAD file by path, returning a heap-allocated descriptor (or
/// null on failure). Always uses the stdio backend; the C original
/// optionally tried a series of `wad_file_classes` based on the `-mmap`
/// command-line argument, but this port omits the mmap backend so the
/// dispatch collapses to a direct call into `stdc_open_file`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `w_wad/file.rs` imports the upstream name through the root shim.
#[doc(alias = "W_OpenFile")]
#[export_name = "W_OpenFile"]
pub extern "C" fn open_wad_file(path: *mut c_char) -> *mut wad_file_t {
    unsafe { (stdc_wad_file.OpenFile)(path) }
}

/// Close a WAD descriptor by dispatching through its vtable. The
/// descriptor must have been returned by [`open_wad_file`].
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export
/// symbol is kept with `#[export_name]` below.
#[doc(alias = "W_CloseFile")]
#[export_name = "W_CloseFile"]
pub extern "C" fn close_wad_file(wad: *mut wad_file_t) {
    unsafe {
        ((*(*wad).file_class).CloseFile)(wad);
    }
}

/// Read `buffer_len` bytes from `offset` in the given WAD into `buffer`
/// by dispatching through the descriptor's vtable. Returns the number
/// of bytes actually read.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `w_wad/file.rs` and `w_wad/cache.rs` import the upstream name through
/// the root shim.
#[doc(alias = "W_Read")]
#[export_name = "W_Read"]
pub extern "C" fn read_wad_bytes(
    wad: *mut wad_file_t,
    offset: c_uint,
    buffer: *mut c_void,
    buffer_len: usize,
) -> usize {
    unsafe { ((*(*wad).file_class).Read)(wad, offset, buffer, buffer_len) }
}
