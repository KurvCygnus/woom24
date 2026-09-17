//! 声明层垫片: 仅在 wasm32 上替代 crates.io `libc`, 让 `room` 里的
//! `libc::fopen` / `libc::printf` 等路径通过类型检查 (spec ② D1).
//! 真正的实现 (内存 VFS / malloc / printf 子集) 在 `shells/web/src/wasm_vfs.rs`,
//! 由 `#[no_mangle]` 导出同名符号, 在最终 cdylib 链接时闭合引用.
//!
//! 规则: 本 crate 只声明, 不实现; 签名与 libc 0.2 逐字一致;
//! 权威符号清单见 docs/plans/2026-09-17-wasm-shell-implementation.md 附录 A.

pub use std::ffi::{c_char, c_int, c_long, c_uint, c_void};

/// 不透明 FILE 句柄 (同 m_misc.rs 的占位 enum 手法; 零变体 enum 不允许 repr(C)).
pub enum FILE {}

/// stdio seek 常量 (wasm_vfs::fseek 只支持这三种).
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
    // 变参声明: 引擎调用点存在 0..4 个变参 (附录 A), 必须用 `...` 才能全部通过类型检查.
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn snprintf(s: *mut c_char, n: usize, format: *const c_char, ...) -> c_int;
    pub fn puts(s: *const c_char) -> c_int;
    pub fn putchar(c: c_int) -> c_int;
    pub fn malloc(size: usize) -> *mut c_void;
    pub fn free(p: *mut c_void);
    pub fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn strlen(s: *const c_char) -> usize;
    pub fn remove(path: *const c_char) -> c_int;
    pub fn rename(old: *const c_char, new: *const c_char) -> c_int;
}
