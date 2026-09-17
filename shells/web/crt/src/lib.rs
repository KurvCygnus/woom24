//! 声明层垫片: 仅在 wasm32 上替代 crates.io `libc`, 让 `room` 里的
//! `libc::fopen` / `libc::printf` 等路径通过类型检查 (spec ② D1).
//! 真正的实现 (内存 VFS / malloc / printf 子集) 在 `shells/web/src/wasm_vfs.rs`,
//! 由 `#[no_mangle]` 导出同名符号, 在最终 cdylib 链接时闭合引用.
//!
//! 规则: 本 crate 只声明, 不实现; 除 printf 家族按 wasm 链接器要求
//! 拆成固定参数形状外, 签名与 libc 0.2 逐字一致;
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
    // printf 家族不用 `...`: wasm rust-lld 严格检查符号签名, 变参声明的
    // 每个调用点按实参数量生成不同的 wasm 签名, 与任何单个实现都对不上,
    // lld 会把对不上的调用换成 `signature_mismatch` 陷阱桩 (运行时 trap).
    // 故按引擎审计过的固定参数形状声明 (printf 0..4 个变参, snprintf 1..2 个,
    // 见计划附录 A); usize 槽 = 指针/整数统一宽度, wasm_vfs 提供同名导出.
    pub fn printf0(fmt: *const c_char) -> c_int;
    pub fn printf1(fmt: *const c_char, a0: usize) -> c_int;
    pub fn printf2(fmt: *const c_char, a0: usize, a1: usize) -> c_int;
    pub fn printf3(fmt: *const c_char, a0: usize, a1: usize, a2: usize) -> c_int;
    pub fn printf4(fmt: *const c_char, a0: usize, a1: usize, a2: usize, a3: usize) -> c_int;
    pub fn snprintf1(s: *mut c_char, n: usize, fmt: *const c_char, a0: usize) -> c_int;
    pub fn snprintf2(s: *mut c_char, n: usize, fmt: *const c_char, a0: usize, a1: usize) -> c_int;
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
