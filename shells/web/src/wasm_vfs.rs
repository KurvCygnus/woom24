//! CRT shim + in-memory VFS (spec 2 D1).
//!
//! Design notes:
//! - Type resolution for `libc::` paths is handled by `shells/web/crt`
//!   (woom24-libc); this module provides the implementations under the same
//!   names via `#[no_mangle]` / `#[export_name]`, closing every unresolved
//!   engine reference at final cdylib link time.
//! - printf family: wasm rust-lld strictly checks symbol signatures, and call
//!   sites of variadic declarations generate a different signature per
//!   argument count, so linking such a call to a fixed-slot implementation
//!   makes lld swap it for a `signature_mismatch` trap stub (probe-verified:
//!   the link warns, running traps). Declarations and implementations are
//!   therefore defined one by one in the engine-audited fixed parameter
//!   shapes (printf0..4 / snprintf1..2, see woom24-libc and plan appendix A);
//!   unused slots hold garbage, but a specifier absent from the format string
//!   is never read.
//! - Beyond the audited specifier set (`%s %d %i %u %x %c %p %%` + width):
//!   log and degrade the output, never trap (D1).

use std::alloc::{alloc, dealloc, Layout};
use std::cell::RefCell;
use std::collections::HashMap;

use std::ffi::{c_char, c_int, c_long, c_void};

thread_local! {
    /// Process-wide VFS table. JS registers via `woom24_register_file`; the
    /// engine consumes via fopen.
    static VFS: RefCell<VfsTable> = RefCell::new(VfsTable::new());
}

/// In-memory name → bytes file table (D1).
pub(crate) struct VfsTable {
    files: HashMap<String, Vec<u8>>,
}

impl VfsTable {
    pub(crate) fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub(crate) fn register(&mut self, name: &str, bytes: Vec<u8>) {
        self.files.insert(name.to_string(), bytes);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.files.get(name)
    }
}

/// Registration hook for the shell's other modules (init_pipeline / launcher_ui).
pub fn vfs_register(name: &str, bytes: Vec<u8>) {
    VFS.with_borrow_mut(|t| t.register(name, bytes));
}

/// Read hook for the shell's other modules (SF2 preload etc.).
pub fn vfs_get(name: &str) -> Option<Vec<u8>> {
    VFS.with_borrow(|t| t.get(name).cloned())
}

// ---------------------------------------------------------------------------
// Formatting subset: pure logic, testable on the host
// ---------------------------------------------------------------------------

/// One parsed format argument. Pointers have already been copied into byte
/// strings on the wasm side, so `format` itself is a pure function (host
/// tests need no wasm memory).
// The I/P variants are only built by pure-logic callers (unit tests / golden
// cases); printf slot parsing produces only U/S (raw slots are u32; the
// numeric meaning is left to `format` to interpret per specifier).
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum FmtArg {
    I(i32),
    U(u32),
    P(u32),
    S(Vec<u8>),
}

/// Formats `fmt` with `args` into `out`, returning the C-semantic "would-be"
/// written length. Overlong output is truncated (`out` keeps
/// min(len, out.len()-1) bytes + no NUL -- the snprintf wrapper owns the NUL);
/// unknown/missing args degrade to `<na>`, never panic.
pub(crate) fn format(fmt: &[u8], args: &[FmtArg], out: &mut [u8]) -> usize {
    fn degrade() -> Vec<u8> {
        b"<na>".to_vec()
    }
    fn push_byte(would: &mut usize, written: &mut usize, out: &mut [u8], b: u8) {
        // Truncation rule: write at most out.len()-1 bytes (one byte reserved
        // for the caller's NUL semantics).
        if *would + 1 < out.len() {
            out[*written] = b;
            *written += 1;
        }
        *would += 1;
    }

    let mut it = args.iter();
    let mut written = 0usize;
    let mut would = 0usize;
    let mut i = 0usize;
    while i < fmt.len() {
        if fmt[i] != b'%' {
            push_byte(&mut would, &mut written, out, fmt[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= fmt.len() {
            break;
        }
        // Width (decimal right-align only, e.g. %7i).
        let mut width = 0usize;
        while i < fmt.len() && fmt[i].is_ascii_digit() {
            width = width * 10 + (fmt[i] - b'0') as usize;
            i += 1;
        }
        if i >= fmt.len() {
            break;
        }
        let spec = fmt[i];
        i += 1;
        // Render this specifier fully first, then apply width padding and
        // truncated writes.
        let rendered: Vec<u8> = match spec {
            b'%' => vec![b'%'],
            b's' => match it.next() {
                Some(FmtArg::S(s)) => s.clone(),
                _ => degrade(),
            },
            b'd' | b'i' => match it.next() {
                Some(FmtArg::I(v)) => v.to_string().into_bytes(),
                Some(FmtArg::U(v)) => (*v as i32).to_string().into_bytes(),
                _ => degrade(),
            },
            b'u' => match it.next() {
                Some(FmtArg::U(v)) => v.to_string().into_bytes(),
                Some(FmtArg::I(v)) => (*v as u32).to_string().into_bytes(),
                _ => degrade(),
            },
            b'x' => match it.next() {
                Some(FmtArg::U(v)) => format!("{v:x}").into_bytes(),
                Some(FmtArg::I(v)) => format!("{:x}", *v as u32).into_bytes(),
                _ => degrade(),
            },
            b'c' => match it.next() {
                Some(FmtArg::U(v)) => vec![*v as u8],
                Some(FmtArg::I(v)) => vec![*v as u8],
                _ => degrade(),
            },
            b'p' => match it.next() {
                Some(FmtArg::P(v)) => format!("0x{v:x}").into_bytes(),
                Some(FmtArg::U(v)) => format!("0x{v:x}").into_bytes(),
                _ => degrade(),
            },
            // Unaudited specifier: degrade to a placeholder (D1: log + degrade, never trap).
            other => {
                log::warn!("wasm_vfs: unsupported format specifier %{other} degraded");
                degrade()
            }
        };
        // Right-align width padding.
        let mut buf = rendered;
        while buf.len() < width {
            buf.insert(0, b' ');
        }
        for b in buf {
            push_byte(&mut would, &mut written, out, b);
        }
    }
    would
}

// ---------------------------------------------------------------------------
// sscanf subset: minimal parser for M_StrToInt's four format strings
// (pure logic, testable on the host)
// ---------------------------------------------------------------------------

/// C `isspace` subset (space/tab/newline etc.).
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// Consumes an optional sign, returning (is_negative, remaining input).
fn take_sign(input: &[u8]) -> (bool, &[u8]) {
    match input.first() {
        Some(b'-') => (true, &input[1..]),
        Some(b'+') => (false, &input[1..]),
        _ => (false, input),
    }
}

/// Consumes digits in the given base, returning (value, remaining input);
/// None if not a single digit was consumed.
fn take_digits(mut input: &[u8], base: u32) -> Option<(i64, &[u8])> {
    let mut val: i64 = 0;
    let mut any = false;
    while let Some(&b) = input.first() {
        let d = match b {
            b'0'..=b'9' => (b - b'0') as u32,
            b'a'..=b'f' if base == 16 => (b - b'a' + 10) as u32,
            b'A'..=b'F' if base == 16 => (b - b'A' + 10) as u32,
            _ => break,
        };
        if d >= base {
            break;
        }
        val = val.saturating_mul(base as i64).saturating_add(d as i64);
        any = true;
        input = &input[1..];
    }
    if any {
        Some((val, input))
    } else {
        None
    }
}

/// Minimal single-conversion `sscanf` subset: implements only the
/// engine-audited format syntax -- a space skips input whitespace, other
/// literals match byte for byte, plus the `%x`/`%o`/`%d` integer conversions
/// (covers all of M_StrToInt's ` 0x%x` / ` 0X%x` / ` 0%o` / ` %d`).
/// Returns the number of successful assignments (0 or 1); on failure `out`
/// is untouched, never panics.
pub(crate) fn sscanf_parse1(fmt: &[u8], input: &[u8], out: &mut i64) -> usize {
    let mut inp = input;
    let mut i = 0usize;
    let mut assigned = 0usize;
    while i < fmt.len() {
        match fmt[i] {
            b' ' => {
                while inp.first().is_some_and(|&b| is_c_space(b)) {
                    inp = &inp[1..];
                }
                i += 1;
            }
            b'%' => {
                i += 1;
                let Some(&spec) = fmt.get(i) else { break };
                i += 1;
                let parsed = match spec {
                    b'x' | b'X' => {
                        let (neg, rest) = take_sign(inp);
                        // C %x accepts an optional 0x/0X prefix on the input side.
                        let rest = if rest.len() >= 2 && rest[0] == b'0' && (rest[1] | 0x20) == b'x'
                        {
                            &rest[2..]
                        } else {
                            rest
                        };
                        take_digits(rest, 16).map(|(v, r)| (if neg { -v } else { v }, r))
                    }
                    b'o' => {
                        let (neg, rest) = take_sign(inp);
                        take_digits(rest, 8).map(|(v, r)| (if neg { -v } else { v }, r))
                    }
                    b'd' | b'i' => {
                        let (neg, rest) = take_sign(inp);
                        take_digits(rest, 10).map(|(v, r)| (if neg { -v } else { v }, r))
                    }
                    // Unaudited conversion: degrade to a match failure (D1: log + degrade, never trap).
                    _ => None,
                };
                match parsed {
                    Some((v, rest)) => {
                        *out = v;
                        inp = rest;
                        assigned += 1;
                    }
                    None => return 0,
                }
            }
            lit => {
                if inp.first() != Some(&lit) {
                    return 0;
                }
                inp = &inp[1..];
                i += 1;
            }
        }
    }
    assigned
}

// ---------------------------------------------------------------------------
// malloc / memset / free: forwarded to the Rust global allocator + a layout
// tracking table (D1)
// ---------------------------------------------------------------------------

thread_local! {
    /// Pointer → Layout, needed by free.
    static LAYOUTS: RefCell<HashMap<usize, Layout>> = RefCell::new(HashMap::new());
}

/// C `malloc` semantics: returns null on failure (which practically never
/// happens here).
fn shm_malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: size > 0 and align=8 satisfy Layout::from_size_align's constraints.
    let layout = match Layout::from_size_align(size, 8) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    let p = unsafe { alloc(layout) };
    if !p.is_null() {
        LAYOUTS.with_borrow_mut(|m| {
            m.insert(p as usize, layout);
        });
    }
    p as *mut c_void
}

/// C `free` semantics. The engine does call `free` (d_iwad frees
/// FFI-allocated strings), so this is real deallocation, paired with `malloc`
/// via the layout table.
fn shm_free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    let layout = LAYOUTS.with_borrow_mut(|m| m.remove(&(p as usize)));
    if let Some(layout) = layout {
        // SAFETY: the pointer came from shm_malloc with its layout fetched
        // back from the table; not a double free.
        unsafe { dealloc(p as *mut u8, layout) };
    }
}

/// C `memset` semantics: returns `s`.
///
/// # Safety
/// `s` must point to at least `n` bytes of writable memory.
unsafe fn shm_memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    if !s.is_null() && n > 0 {
        // volatile write loop: a plain write_bytes gets lowered by LLVM to
        // `call memset`, and this crate exports a strong symbol of that very
        // name -- both the host test binary and the wasm cdylib would bind the
        // call back to itself → infinite recursion, stack overflow. Volatile
        // stores are never folded into a libcall.
        let p = s as *mut u8;
        for i in 0..n {
            p.add(i).write_volatile(c as u8);
        }
    }
    s
}

/// Copies a NUL-terminated string out of wasm linear memory (used by printf %s).
///
/// # Safety
/// `p` must point to readable memory with a NUL within 4096 bytes (true for
/// every engine format argument).
unsafe fn copy_cstr(p: u32) -> Vec<u8> {
    if p == 0 {
        return b"(null)".to_vec();
    }
    let mut out = Vec::new();
    //* 4096 is the shim's own scan cap (carried over from the Task 2 first-pass
    //* %s convention, not C semantics): an input missing its NUL is read for at
    //* most 4096 bytes, then stops.
    for i in 0..4096 {
        let b = *(p as *const u8).add(i);
        if b == 0 {
            break;
        }
        out.push(b);
    }
    out
}

// ---------------------------------------------------------------------------
// Second batch of CRT symbols: ctype/string/atof/calloc (fix round 1; pure
// logic, host-testable)
// ---------------------------------------------------------------------------

/// C `toupper` ASCII subset (engine input is always ASCII).
fn c_toupper(c: c_int) -> c_int {
    let b = c as u8;
    if b.is_ascii_lowercase() {
        (b - 32) as c_int
    } else {
        c
    }
}

/// C `tolower` ASCII subset.
fn c_tolower(c: c_int) -> c_int {
    let b = c as u8;
    if b.is_ascii_uppercase() {
        (b + 32) as c_int
    } else {
        c
    }
}

/// C `isspace` ("C" locale): space/\t/\n/\v/\f/\r, non-zero means true.
fn c_isspace(c: c_int) -> c_int {
    c_int::from(is_c_space(c as u8))
}

/// Ordering → the C comparison convention <0/0/>0.
fn order_to_c_int(o: std::cmp::Ordering) -> c_int {
    match o {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// C `strcmp`: compares as unsigned bytes; the shorter string is lesser when
/// one hits NUL first.
fn c_strcmp(a: &[u8], b: &[u8]) -> c_int {
    let n = a.len().min(b.len());
    match a[..n].iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => order_to_c_int(a[i].cmp(&b[i])),
        None => order_to_c_int(a.len().cmp(&b.len())),
    }
}

/// C `strncmp`: compares at most n bytes; once truncated this is plain
/// strcmp semantics (NUL termination guarantees equivalence).
fn c_strncmp(a: &[u8], b: &[u8], n: usize) -> c_int {
    c_strcmp(&a[..n.min(a.len())], &b[..n.min(b.len())])
}

/// C `strncpy` core: copies min(src.len, n) bytes, NUL-padding to n when
/// short; when src is at least n bytes, exactly n bytes are copied and no
/// terminator is written. `dst.len()` is n.
fn c_strncpy_into(dst: &mut [u8], src: &[u8]) {
    let copy = src.len().min(dst.len());
    dst[..copy].copy_from_slice(&src[..copy]);
    dst[copy..].fill(0);
}

/// C `strrchr`: offset of the last occurrence of `(c as u8)`; c=0 hits the
/// terminating NUL slot; None when absent (the export layer maps it to null).
fn c_strrchr(s: &[u8], c: c_int) -> Option<usize> {
    let target = c as u8;
    if target == 0 {
        return Some(s.len());
    }
    s.iter().rposition(|&b| b == target)
}

/// C `strstr`: leftmost match offset; empty needle = 0; None when absent.
fn c_strstr(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// C `atof` (strtod-lite): leading C whitespace + optional sign +
/// integer/fraction digits, taking the longest valid prefix; no digits → 0.0.
/// Audit conclusion (fix round 1): the engine's only call site, m_config.rs:688,
/// parses .cfg values, and the engine only ever writes plain decimals there
/// (Rust formatting never emits exponents), so exponent forms are not
/// implemented.
fn c_atof(s: &[u8]) -> f64 {
    let mut i = 0usize;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    let neg = match s.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let num_start = i;
    let mut int_digits = 0usize;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
        int_digits += 1;
    }
    let mut frac_digits = 0usize;
    if i < s.len() && s[i] == b'.' {
        let mut j = i + 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        frac_digits = j - i - 1;
        if int_digits + frac_digits > 0 {
            i = j;
        }
    }
    if int_digits + frac_digits == 0 {
        return 0.0;
    }
    let text = std::str::from_utf8(&s[num_start..i]).unwrap_or("");
    let v = text.parse::<f64>().unwrap_or(0.0);
    if neg {
        -v
    } else {
        v
    }
}

/// C `calloc`: layout-tracking allocator + zeroing; multiplication overflow
/// or size=0 returns null. Zeroing must take the volatile path (see
/// shm_memset's comment: a plain fill is lowered by LLVM to `call memset`,
/// which self-recurses against this crate's same-name export).
fn shm_calloc(nmemb: usize, size: usize) -> *mut c_void {
    let Some(total) = nmemb.checked_mul(size) else {
        return std::ptr::null_mut();
    };
    let p = shm_malloc(total);
    if !p.is_null() && total > 0 {
        // SAFETY: p points at total bytes of writable memory from shm_malloc.
        unsafe { shm_memset(p, 0, total) };
    }
    p
}

// ---------------------------------------------------------------------------
// Export surface: one-to-one with appendix A (authoritative list = captured
// by cargo check)
// ---------------------------------------------------------------------------

/// An open file: fopen clones the file bytes once up front (a multi-MB WAD is
/// cloned a single time; fread afterwards slices locally -- avoiding per-read
/// clones of large files) plus a read cursor. fopen returns its raw pointer as
/// the handle; fclose is the sole release point (CRT semantics).
struct OpenDesc {
    data: Vec<u8>,
    pos: usize,
}

/// # Safety
/// `path` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the caller guarantees NUL termination.
    let name = unsafe { copy_cstr(path as u32) };
    let name = String::from_utf8_lossy(&name).into_owned();
    let Some(bytes) = wasm_vfs_lookup(&name) else {
        // Matches CRT: a failed open returns NULL; the engine's existing error
        // path handles it.
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(OpenDesc {
        data: bytes,
        pos: 0,
    })) as *mut c_void
}

/// VFS table lookup (thin thread_local wrapper, used by fopen).
fn wasm_vfs_lookup(name: &str) -> Option<Vec<u8>> {
    VFS.with_borrow(|t| t.get(name).cloned())
}

/// # Safety
/// `stream` must be a handle returned by fopen.
#[no_mangle]
pub unsafe extern "C" fn fread(
    ptr: *mut c_void,
    size: usize,
    nmemb: usize,
    stream: *mut c_void,
) -> usize {
    if ptr.is_null() || stream.is_null() || size == 0 {
        return 0;
    }
    let want = size.saturating_mul(nmemb);
    let d = unsafe { &mut *(stream as *mut OpenDesc) };
    let avail = d.data.len().saturating_sub(d.pos);
    let n = want.min(avail);
    if n > 0 {
        std::ptr::copy_nonoverlapping(d.data[d.pos..d.pos + n].as_ptr(), ptr as *mut u8, n);
        d.pos += n;
    }
    // C semantics: returns the number of whole items read (= bytes / size, floor).
    n / size
}

/// # Safety
/// `stream` must be a handle returned by fopen.
#[no_mangle]
pub unsafe extern "C" fn fwrite(
    _ptr: *const c_void,
    size: usize,
    nmemb: usize,
    _stream: *mut c_void,
) -> usize {
    // The VFS is read-only (IWAD/PWAD/SF2 are host-registered); save/demo
    // write paths are not persisted on wasm in this spec -- return "fully
    // written" to keep the engine's state machine moving, and drop the data.
    let _ = size;
    nmemb
}

/// # Safety
/// `stream` must be a handle returned by fopen.
#[no_mangle]
pub unsafe extern "C" fn fseek(stream: *mut c_void, offset: c_long, whence: c_int) -> c_int {
    if stream.is_null() {
        return -1;
    }
    let d = unsafe { &mut *(stream as *mut OpenDesc) };
    let len = d.data.len() as c_long;
    let new = match whence {
        0 => offset,                   // SEEK_SET
        1 => d.pos as c_long + offset, // SEEK_CUR
        2 => len + offset,             // SEEK_END
        _ => return -1,
    };
    if new < 0 || new > len {
        return -1;
    }
    d.pos = new as usize;
    0
}

/// # Safety
/// `stream` must be a handle returned by fopen.
#[no_mangle]
pub unsafe extern "C" fn ftell(stream: *mut c_void) -> c_long {
    if stream.is_null() {
        return -1;
    }
    let d = unsafe { &*(stream as *mut OpenDesc) };
    d.pos as c_long
}

/// # Safety
/// `stream` must be a handle returned by fopen.
#[no_mangle]
pub unsafe extern "C" fn fclose(stream: *mut c_void) -> c_int {
    if stream.is_null() {
        return -1;
    }
    // SAFETY: the handle was allocated by fopen; fclose is the sole release
    // point (CRT semantics).
    drop(unsafe { Box::from_raw(stream as *mut OpenDesc) });
    0
}

/// # Safety
/// `stream` must be a handle returned by fopen, or null.
#[no_mangle]
pub unsafe extern "C" fn fflush(_stream: *mut c_void) -> c_int {
    0
}

/// printf family exports: one symbol per audited call shape, one-to-one with
/// woom24-libc's fixed-arity declarations (see the module header comment);
/// unfilled slots are never read.
///
/// # Safety
/// `fmt` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn printf0(fmt: *const c_char) -> c_int {
    let out = printf_impl(fmt, &[]);
    // stdout lands in the browser console (visible via the web console).
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn printf1(fmt: *const c_char, a0: u32) -> c_int {
    let out = printf_impl(fmt, &[a0]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn printf2(fmt: *const c_char, a0: u32, a1: u32) -> c_int {
    let out = printf_impl(fmt, &[a0, a1]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn printf3(fmt: *const c_char, a0: u32, a1: u32, a2: u32) -> c_int {
    let out = printf_impl(fmt, &[a0, a1, a2]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn printf4(fmt: *const c_char, a0: u32, a1: u32, a2: u32, a3: u32) -> c_int {
    let out = printf_impl(fmt, &[a0, a1, a2, a3]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

unsafe fn printf_impl(fmt: *const c_char, slots: &[u32]) -> Vec<u8> {
    let f = copy_cstr(fmt as u32);
    // Parse raw slots per specifier: %s dereferences the pointer, numeric
    // classes take the slot value directly.
    let mut args: Vec<FmtArg> = Vec::new();
    let mut slot_i = 0usize;
    let mut i = 0usize;
    while i < f.len() {
        if f[i] != b'%' {
            i += 1;
            continue;
        }
        i += 1;
        while i < f.len() && f[i].is_ascii_digit() {
            i += 1;
        }
        if i >= f.len() {
            break;
        }
        match f[i] {
            b'%' => {}
            b's' => {
                let p = slots.get(slot_i).copied().unwrap_or(0);
                slot_i += 1;
                // SAFETY: the engine's %s arguments are all valid C strings.
                args.push(FmtArg::S(copy_cstr(p)));
            }
            b'c' | b'd' | b'i' | b'u' | b'x' | b'p' => {
                let v = slots.get(slot_i).copied().unwrap_or(0);
                slot_i += 1;
                args.push(FmtArg::U(v));
            }
            _ => {}
        }
        i += 1;
    }
    let mut out = vec![0u8; 4096];
    let n = format(&f, &args, &mut out);
    out.truncate(n);
    out
}

/// snprintf export: like the printf family, writes the target buffer and
/// returns the would-be length (d_main.rs depends on this value).
///
/// # Safety
/// `s`/`fmt` must be valid; `s` must be writable for at least `n` bytes.
#[no_mangle]
pub unsafe extern "C" fn snprintf1(s: *mut c_char, n: usize, fmt: *const c_char, a0: u32) -> c_int {
    let out = printf_impl(fmt, &[a0]);
    if !s.is_null() && n > 0 {
        let w = (out.len()).min(n - 1);
        std::ptr::copy_nonoverlapping(out.as_ptr(), s as *mut u8, w);
        *s.add(w) = 0;
    }
    out.len() as c_int
}

/// # Safety
/// `s`/`fmt` must be valid; `s` must be writable for at least `n` bytes.
#[no_mangle]
pub unsafe extern "C" fn snprintf2(
    s: *mut c_char,
    n: usize,
    fmt: *const c_char,
    a0: u32,
    a1: u32,
) -> c_int {
    let out = printf_impl(fmt, &[a0, a1]);
    if !s.is_null() && n > 0 {
        let w = (out.len()).min(n - 1);
        std::ptr::copy_nonoverlapping(out.as_ptr(), s as *mut u8, w);
        *s.add(w) = 0;
    }
    out.len() as c_int
}

/// Single-conversion sscanf export: matches M_StrToInt's call shape (one
/// output pointer slot, see [`sscanf_parse1`]). C semantics: returns the
/// number of successful assignments, 0 on no match.
///
/// # Safety
/// `s`/`fmt` must be NUL-terminated C strings; `a0` must point to a writable
/// `c_int`.
#[no_mangle]
pub unsafe extern "C" fn sscanf1(s: *const c_char, fmt: *const c_char, a0: usize) -> c_int {
    if s.is_null() || fmt.is_null() || a0 == 0 {
        return 0;
    }
    // SAFETY: the caller guarantees NUL termination.
    let input = unsafe { copy_cstr(s as u32) };
    let f = unsafe { copy_cstr(fmt as u32) };
    let mut v: i64 = 0;
    let n = sscanf_parse1(&f, &input, &mut v);
    if n == 1 {
        // SAFETY: a0 is a caller-provided writable c_int slot.
        unsafe {
            *(a0 as *mut c_int) = v as c_int;
        }
    }
    n as c_int
}

/// # Safety
/// `s` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    if s.is_null() {
        return -1;
    }
    // SAFETY: the caller guarantees NUL termination.
    let b = unsafe { copy_cstr(s as u32) };
    log::info!("{}", String::from_utf8_lossy(&b));
    b.len() as c_int + 1
}

/// # Safety
/// c must be a valid byte value.
#[no_mangle]
pub unsafe extern "C" fn putchar(c: c_int) -> c_int {
    log::info!("{}", (c as u8) as char);
    c
}

/// C `malloc`: null on failure.
///
/// # Safety
/// The returned pointer must be paired with `free`; `size` of 0 returns null.
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    shm_malloc(size)
}

/// C `free`: releases a `malloc`-returned pointer; null is a harmless no-op.
///
/// # Safety
/// `p` must be an as-yet-unfreed `malloc` return value.
#[no_mangle]
pub unsafe extern "C" fn free(p: *mut c_void) {
    shm_free(p)
}

/// # Safety
/// `s` must point to at least `n` bytes of writable memory.
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    shm_memset(s, c, n)
}

/// # Safety
/// `s` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn atoi(s: *const c_char) -> c_int {
    if s.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees NUL termination.
    let b = unsafe { copy_cstr(s as u32) };
    let t = String::from_utf8_lossy(&b);
    let t = t.trim_start();
    let (neg, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let v: i64 = digits
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .fold(0, |acc, c| acc * 10 + (c as u8 - b'0') as i64);
    let v = if neg { -v } else { v };
    v.clamp(i32::MIN as i64, i32::MAX as i64) as c_int
}

/// # Safety
/// `s` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees NUL termination.
    unsafe { copy_cstr(s as u32) }.len()
}

/// # Safety
/// `path` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn remove(_path: *const c_char) -> c_int {
    // The VFS is immutable (files are host-registered); save deletion degrades
    // to success (same policy as fwrite).
    0
}

/// # Safety
/// Both arguments must be NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn rename(_old: *const c_char, _new: *const c_char) -> c_int {
    0
}

// ---------------------------------------------------------------------------
// Second batch of CRT exports (fix round 1): declarations live in woom24-libc
// ---------------------------------------------------------------------------

/// C `toupper`: ASCII semantics (engine input is always ASCII).
#[no_mangle]
pub extern "C" fn toupper(c: c_int) -> c_int {
    c_toupper(c)
}

/// C `tolower`: ASCII semantics.
#[no_mangle]
pub extern "C" fn tolower(c: c_int) -> c_int {
    c_tolower(c)
}

/// C `isspace` ("C" locale whitespace set).
#[no_mangle]
pub extern "C" fn isspace(c: c_int) -> c_int {
    c_isspace(c)
}

/// C `strcmp`.
///
/// # Safety
/// Both arguments must be NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees NUL termination.
    let (a, b) = unsafe { (copy_cstr(s1 as u32), copy_cstr(s2 as u32)) };
    c_strcmp(&a, &b)
}

/// C `strncmp`.
///
/// # Safety
/// Both arguments must be NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees NUL termination.
    let (a, b) = unsafe { (copy_cstr(s1 as u32), copy_cstr(s2 as u32)) };
    c_strncmp(&a, &b, n)
}

/// C `strncpy`: returns `dst`; when src is at least n bytes, exactly n bytes
/// are copied and no terminator is written.
///
/// # Safety
/// `dst` must be writable for `n` bytes; `src` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    if dst.is_null() || n == 0 {
        return dst;
    }
    //* Deviation from C: src=NULL is UB in C; this shim degrades it to "empty
    //* src", zero-filling dst for n bytes. Engine call sites never pass NULL.
    let bytes = if src.is_null() {
        Vec::new()
    } else {
        // SAFETY: the caller guarantees NUL termination.
        unsafe { copy_cstr(src as u32) }
    };
    // SAFETY: the caller guarantees dst is writable for at least n bytes.
    unsafe {
        c_strncpy_into(std::slice::from_raw_parts_mut(dst as *mut u8, n), &bytes);
    }
    dst
}

/// C `strrchr`: null when absent; c=0 returns a pointer to the terminating NUL.
///
/// # Safety
/// `s` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn strrchr(s: *const c_char, c: c_int) -> *mut c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the caller guarantees NUL termination.
    let bytes = unsafe { copy_cstr(s as u32) };
    match c_strrchr(&bytes, c) {
        // SAFETY: off <= strlen, so after add the pointer is still inside the
        // object (terminating NUL slot included); C has strrchr return a writable
        // char pointer, hence the const→mut cast.
        Some(off) => unsafe { s.add(off) as *mut c_char },
        None => std::ptr::null_mut(),
    }
}

/// C `strstr`: null when absent.
///
/// # Safety
/// Both arguments must be NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    if haystack.is_null() || needle.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the caller guarantees NUL termination.
    let (hay, nee) = unsafe { (copy_cstr(haystack as u32), copy_cstr(needle as u32)) };
    match c_strstr(&hay, &nee) {
        // SAFETY: off <= strlen; C has strstr return a writable pointer (const→mut).
        Some(off) => unsafe { haystack.add(off) as *mut c_char },
        None => std::ptr::null_mut(),
    }
}

/// C `getenv`: no process environment on wasm, always returns NULL. Audit:
/// the engine only queries HOME / XDG_CONFIG_HOME (m_misc.rs), and every
/// caller has an unset fallback path.
/// Exported on wasm32 only: the host test binary's normal CRT termination
/// path calls this symbol, and leaving it to the host CRT keeps `cargo test`
/// green.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn getenv(_name: *const c_char) -> *mut c_char {
    std::ptr::null_mut()
}

/// C `atof` (strtod-lite, no exponents -- see [`c_atof`]'s audit comment).
///
/// # Safety
/// `s` must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn atof(s: *const c_char) -> f64 {
    if s.is_null() {
        return 0.0;
    }
    // SAFETY: the caller guarantees NUL termination.
    unsafe { c_atof(&copy_cstr(s as u32)) }
}

/// C `calloc`: layout-tracking allocator + zeroing (zeroing goes through
/// volatile to prevent self-recursion).
#[no_mangle]
pub extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    shm_calloc(nmemb, size)
}

/// C `exit`: on wasm the page lifecycle is the process lifecycle, and the
/// engine has no normal exit path (d_main.rs calls this after I_Endoom) --
/// reaching it is an explicit trap (D1 degrade point).
/// Exported on wasm32 only: the host binary's CRT also calls `exit(0)` when
/// terminating normally after main returns; intercepting that would turn host
/// `cargo test` into a trap.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn exit(status: c_int) -> ! {
    unreachable!("wasm shell: engine called exit({status})")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- VFS table ----

    #[test]
    fn vfs_register_then_get_roundtrips_bytes() {
        let mut t = VfsTable::new();
        t.register("doom1.wad", vec![1, 2, 3]);
        assert_eq!(t.get("doom1.wad"), Some(&vec![1, 2, 3]));
        assert_eq!(t.get("missing.wad"), None);
    }

    #[test]
    fn vfs_register_overwrites_same_name() {
        let mut t = VfsTable::new();
        t.register("a.wad", vec![1]);
        t.register("a.wad", vec![9, 9]);
        assert_eq!(t.get("a.wad"), Some(&vec![9, 9]));
    }

    // ---- printf subset (golden cases) ----

    fn fmt_str(fmt: &str, args: &[FmtArg]) -> String {
        let mut out = vec![0u8; 256];
        let n = format(fmt.as_bytes(), args, &mut out);
        String::from_utf8(out[..n.min(256)].to_vec()).unwrap()
    }

    #[test]
    fn format_percent_s_and_d() {
        let s = fmt_str(" adding %s\n", &[FmtArg::S(b"doom1.wad".to_vec())]);
        assert_eq!(s, " adding doom1.wad\n");
    }

    #[test]
    fn format_zone_line_i_and_p() {
        // Real format string from z_zone.rs:349.
        let s = fmt_str(
            "zone size: %i  location: %p\n",
            &[FmtArg::I(65536), FmtArg::P(0x12340)],
        );
        assert_eq!(s, "zone size: 65536  location: 0x12340\n");
    }

    #[test]
    fn format_width_and_u_x_c() {
        assert_eq!(fmt_str("%7i!", &[FmtArg::I(42)]), "     42!");
        assert_eq!(fmt_str("%u %x", &[FmtArg::U(7), FmtArg::U(255)]), "7 ff");
        assert_eq!(fmt_str("%c%c", &[FmtArg::U(65), FmtArg::U(66)]), "AB");
    }

    #[test]
    fn format_escaped_percent() {
        assert_eq!(fmt_str("100%%\n", &[]), "100%\n");
    }

    #[test]
    fn format_extra_args_are_ignored_and_missing_args_degrade() {
        // More variadic slots than specifiers: extras are ignored.
        assert_eq!(fmt_str("%i", &[FmtArg::I(1), FmtArg::I(2)]), "1");
        // More specifiers than variadic slots: degrade (no trap) with an
        // `<na>` placeholder and a log line.
        assert_eq!(fmt_str("%i %i", &[FmtArg::I(1)]), "1 <na>");
    }

    #[test]
    fn format_unknown_specifier_degrades_not_traps() {
        // The unaudited %f is outside the supported set: degraded as-is.
        assert_eq!(fmt_str("v=%f", &[FmtArg::I(1)]), "v=<na>");
    }

    #[test]
    fn snprintf_returns_would_be_length_and_truncates() {
        // C semantics: the return value is the "would-be written" length; the
        // buffer only takes min(len, n-1) + NUL.
        let mut out = [0u8; 8];
        let n = format(b"say %s", &[FmtArg::S(b"hello world".to_vec())], &mut out);
        assert_eq!(n, 15); // would-be
        assert_eq!(&out[..7], b"say hel");
        assert_eq!(out[7], 0);
    }

    // ---- malloc / memset (pure-logic part: handle table) ----

    #[test]
    fn allocator_roundtrip_via_heap() {
        let p = shm_malloc(64);
        assert!(!p.is_null());
        // Write then read back to confirm usability.
        unsafe { std::ptr::write_bytes(p as *mut u8, 0xAB, 64) };
        let b = unsafe { std::slice::from_raw_parts(p as *const u8, 64) };
        assert!(b.iter().all(|&x| x == 0xAB));
        shm_free(p);
    }

    #[test]
    fn memset_fills_range() {
        let p = shm_malloc(16);
        unsafe {
            shm_memset(p, 0, 16);
            let b = std::slice::from_raw_parts(p as *const u8, 16);
            assert!(b.iter().all(|&x| x == 0));
        }
        shm_free(p);
    }

    // ---- sscanf subset (M_StrToInt's four golden format strings) ----

    fn scan1(fmt: &str, input: &str) -> Option<i64> {
        let mut v: i64 = 0;
        let n = sscanf_parse1(fmt.as_bytes(), input.as_bytes(), &mut v);
        (n == 1).then_some(v)
    }

    #[test]
    fn sscanf_hex_lower_and_upper_prefix() {
        // The two hex paths of m_misc.rs's M_StrToInt.
        assert_eq!(scan1(" 0x%x", "0x1f"), Some(0x1f));
        assert_eq!(scan1(" 0X%x", "0X10"), Some(0x10));
    }

    #[test]
    fn sscanf_octal_leading_zero() {
        assert_eq!(scan1(" 0%o", "0755"), Some(0o755));
    }

    #[test]
    fn sscanf_decimal_with_whitespace_and_sign() {
        assert_eq!(scan1(" %d", "   -42"), Some(-42));
        assert_eq!(scan1(" %d", "+7"), Some(7));
        assert_eq!(scan1(" %d", "0"), Some(0));
    }

    #[test]
    fn sscanf_no_match_returns_zero_conversions() {
        // Literal mismatch: input has no 0x prefix.
        assert_eq!(scan1(" 0x%x", "12"), None);
        // Input exhausted before the conversion.
        assert_eq!(scan1(" %d", "   "), None);
        // No valid octal digit after the prefix.
        assert_eq!(scan1(" 0%o", "0Z"), None);
    }

    // ---- Second batch of CRT symbols (fix round 1): ctype/string/atof/calloc pure logic ----

    #[test]
    fn toupper_tolower_ascii_only() {
        assert_eq!(c_toupper(b'a' as c_int), b'A' as c_int);
        assert_eq!(c_toupper(b'z' as c_int), b'Z' as c_int);
        // Already uppercase / non-letters return unchanged.
        assert_eq!(c_toupper(b'A' as c_int), b'A' as c_int);
        assert_eq!(c_toupper(b'1' as c_int), b'1' as c_int);
        assert_eq!(c_tolower(b'Q' as c_int), b'q' as c_int);
        assert_eq!(c_tolower(b'q' as c_int), b'q' as c_int);
        assert_eq!(c_tolower(b'!' as c_int), b'!' as c_int);
    }

    #[test]
    fn isspace_matches_c_space_set() {
        for c in [b' ', b'\t', b'\n', 0x0b, 0x0c, b'\r'] {
            assert_ne!(c_isspace(c as c_int), 0, "byte {c:#x} 应判为空白");
        }
        for c in [b'a', b'0', 0x00] {
            assert_eq!(c_isspace(c as c_int), 0);
        }
    }

    #[test]
    fn strcmp_orders_by_unsigned_bytes() {
        assert_eq!(c_strcmp(b"abc", b"abc"), 0);
        assert!(c_strcmp(b"abc", b"abd") < 0);
        assert!(c_strcmp(b"abd", b"abc") > 0);
        // The shorter prefix is lesser (terminating NUL = 0 < any non-zero byte).
        assert!(c_strcmp(b"ab", b"abc") < 0);
        assert_eq!(c_strcmp(b"", b""), 0);
        // C strcmp compares as unsigned bytes: 0x80 > 'a' (0x61).
        assert!(c_strcmp(b"\x80", b"a") > 0);
    }

    #[test]
    fn strncmp_compares_at_most_n_bytes() {
        assert_eq!(c_strncmp(b"abcdef", b"abcxyz", 3), 0);
        assert!(c_strncmp(b"abcdef", b"abcxyz", 4) < 0);
        assert_eq!(c_strncmp(b"abc", b"abc", 10), 0);
        assert!(c_strncmp(b"", b"a", 1) < 0);
        assert_eq!(c_strncmp(b"x", b"y", 0), 0, "n=0 恒相等");
    }

    #[test]
    fn strncpy_fills_and_pads_like_c() {
        // src shorter than n: copy everything + NUL-fill to n.
        let mut d = [b'#'; 6];
        c_strncpy_into(&mut d, b"ab");
        assert_eq!(&d, b"ab\0\0\0\0");
        // src at least n: exactly n bytes copied, no terminator written (C semantics).
        let mut d2 = [b'#'; 3];
        c_strncpy_into(&mut d2, b"abcdef");
        assert_eq!(&d2, b"abc");
    }

    #[test]
    fn strrchr_finds_last_and_nul_slot() {
        assert_eq!(c_strrchr(b"a/b/c", b'/' as c_int), Some(3));
        assert_eq!(c_strrchr(b"abc", b'x' as c_int), None);
        assert_eq!(c_strrchr(b"abc", 0), Some(3), "c=0 命中结束 NUL 槽");
        assert_eq!(c_strrchr(b"", b'a' as c_int), None);
    }

    #[test]
    fn strstr_finds_first_occurrence() {
        assert_eq!(c_strstr(b"hello world", b"world"), Some(6));
        assert_eq!(c_strstr(b"aaa", b"aa"), Some(0), "取最左匹配");
        assert_eq!(c_strstr(b"abc", b"xyz"), None);
        assert_eq!(c_strstr(b"abc", b""), Some(0), "空针 = 位置 0");
        assert_eq!(c_strstr(b"", b""), Some(0));
    }

    #[test]
    fn atof_parses_engine_config_shapes() {
        // Every input shape of m_config.rs:688: whitespace/sign/integer/
        // fraction/longest valid prefix.
        assert_eq!(c_atof(b"0"), 0.0);
        assert_eq!(c_atof(b"1"), 1.0);
        assert_eq!(c_atof(b"0.5"), 0.5);
        assert_eq!(c_atof(b"  -2.75"), -2.75);
        assert_eq!(c_atof(b"+3"), 3.0);
        assert_eq!(c_atof(b"42abc"), 42.0, "C atof 取最长合法前缀");
        assert_eq!(c_atof(b".5"), 0.5);
        assert_eq!(c_atof(b"5."), 5.0);
        assert_eq!(c_atof(b""), 0.0);
        assert_eq!(c_atof(b"  abc"), 0.0, "无数字 → 0.0");
        assert_eq!(c_atof(b"  \t-0.25 junk"), -0.25);
    }

    #[test]
    fn calloc_zeroes_and_overflows_to_null() {
        let p = shm_calloc(4, 8);
        assert!(!p.is_null());
        let b = unsafe { std::slice::from_raw_parts(p as *const u8, 32) };
        assert!(b.iter().all(|&x| x == 0), "calloc 必须清零");
        shm_free(p);
        // Multiplication overflow → NULL (C semantics).
        assert!(shm_calloc(usize::MAX, 2).is_null());
        // nmemb = 0 → null, consistent with malloc(0).
        assert!(shm_calloc(0, 8).is_null());
    }
}
