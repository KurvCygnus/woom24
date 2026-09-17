//! CRT 垫片 + 内存 VFS (spec ② D1).
//!
//! 设计要点:
//! - `libc::` 路径的类型解析由 `shells/web/crt` (woom24-libc) 承担;
//!   本模块通过 `#[no_mangle]` / `#[export_name]` 提供同名符号的实现,
//!   在最终 cdylib 链接时闭合引擎的全部未解析引用.
//! - printf 家族: wasm rust-lld 严格检查符号签名, 变参声明的调用点会
//!   按实参数量生成不同签名, 与固定槽位实现链接时会被 lld 换成
//!   `signature_mismatch` 陷阱桩 (已探针实证: 链接有 warning, 运行即 trap).
//!   因此声明与实现都按引擎审计过的固定参数形状逐一定义
//!   (printf0..4 / snprintf1..2, 见 woom24-libc 与计划附录 A);
//!   未用的槽是垃圾值, 但格式串里没有的说明符永远不去读它.
//! - 超出已审计说明符集合 (`%s %d %i %u %x %c %p %%` + 宽度) 时:
//!   记录日志并降级输出, 绝不 trap (D1).

use std::alloc::{alloc, dealloc, Layout};
use std::cell::RefCell;
use std::collections::HashMap;

use std::ffi::{c_char, c_int, c_long, c_void};

thread_local! {
    /// 进程级 VFS 表. JS 经 `woom24_register_file` 注册, 引擎经 fopen 消费.
    static VFS: RefCell<VfsTable> = RefCell::new(VfsTable::new());
}

/// 名字 → 字节 的内存文件表 (D1).
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

/// 供 shell 其它模块 (init_pipeline / launcher_ui) 注册文件.
pub fn vfs_register(name: &str, bytes: Vec<u8>) {
    VFS.with_borrow_mut(|t| t.register(name, bytes));
}

/// 供 shell 其它模块读取已注册文件 (SF2 预载等).
pub fn vfs_get(name: &str) -> Option<Vec<u8>> {
    VFS.with_borrow(|t| t.get(name).cloned())
}

// ---------------------------------------------------------------------------
// 格式化子集: 纯逻辑, 宿主机可测
// ---------------------------------------------------------------------------

/// 一个已解析的格式参数. 指针在 wasm 侧已被复制成字节串,
/// 所以 `format` 本身是纯函数 (宿主机测试不需要 wasm 内存).
// I/P 变体只在纯逻辑调用方 (单测/golden cases) 构造; printf 槽解析
// 只产 U/S (原始槽是 u32, 数值语义交给 format 按说明符解释).
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum FmtArg {
    I(i32),
    U(u32),
    P(u32),
    S(Vec<u8>),
}

/// 把 `fmt` 按 `args` 格式化进 `out`, 返回 C 语义的"本应写入"长度.
/// 超长截断 (`out` 里保留 min(len, out.len()-1) 字节 + 不写 NUL --
/// NUL 由 snprintf 外层负责); 未知/缺失参数降级为 `<na>`, 绝不 panic.
pub(crate) fn format(fmt: &[u8], args: &[FmtArg], out: &mut [u8]) -> usize {
    fn degrade() -> Vec<u8> {
        b"<na>".to_vec()
    }
    fn push_byte(would: &mut usize, written: &mut usize, out: &mut [u8], b: u8) {
        // 截断规则: 最多写 out.len()-1 字节 (留一位给调用方的 NUL 语义).
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
        // 宽度 (仅十进制右对齐, 如 %7i).
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
        // 先完整渲染本说明符, 再做宽度填充与截断写入.
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
            // 未审计说明符: 降级占位 (D1: log + degrade, never trap).
            other => {
                log::warn!("wasm_vfs: unsupported format specifier %{other} degraded");
                degrade()
            }
        };
        // 右对齐宽度填充.
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
// malloc / memset / free: 转发到 Rust 全局分配器 + 布局跟踪表 (D1)
// ---------------------------------------------------------------------------

thread_local! {
    /// 指针 → Layout, free 时需要.
    static LAYOUTS: RefCell<HashMap<usize, Layout>> = RefCell::new(HashMap::new());
}

/// C `malloc` 语义: 失败返回 null (这里几乎不会失败).
fn shm_malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: size > 0 且 align=8 满足 Layout::from_size_align 的约束.
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

/// C `free` 语义. 注意: 权威清单里引擎当前并未引用 free
/// (见计划附录 A), 这里随 malloc 成对提供, 防未来泄漏.
fn shm_free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    let layout = LAYOUTS.with_borrow_mut(|m| m.remove(&(p as usize)));
    if let Some(layout) = layout {
        // SAFETY: 指针来自 shm_malloc 且布局从表中取回, 未被重复释放.
        unsafe { dealloc(p as *mut u8, layout) };
    }
}

/// C `memset` 语义: 返回 `s`.
///
/// # Safety
/// `s` 必须指向至少 `n` 字节可写内存.
unsafe fn shm_memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    if !s.is_null() && n > 0 {
        // volatile 写循环: 普通 write_bytes 会被 LLVM 降为 `call memset`,
        // 而本 crate 导出同名强符号, 宿主测试二进制与 wasm cdylib 链接时
        // 都会绑回自身 → 无限递归爆栈; volatile 存储永不合并成 libcall.
        let p = s as *mut u8;
        for i in 0..n {
            p.add(i).write_volatile(c as u8);
        }
    }
    s
}

/// 从 wasm 线性内存复制 NUL 结尾字符串 (printf %s 用).
///
/// # Safety
/// `p` 必须指向可读内存且在 4096 字节内有 NUL (引擎内格式参数均满足).
unsafe fn copy_cstr(p: u32) -> Vec<u8> {
    if p == 0 {
        return b"(null)".to_vec();
    }
    let mut out = Vec::new();
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
// 导出符号面: 与附录 A 一一对应 (权威清单 = cargo check 捕获)
// ---------------------------------------------------------------------------

/// 打开的文件描述: fopen 时一次性拷贝文件字节 (几 MB 级 WAD 一次克隆,
/// 之后 fread 全部本地切片 -- 避免逐读克隆大文件) + 读游标.
/// fopen 返回它的裸指针作句柄, fclose 是唯一释放点 (CRT 语义).
struct OpenDesc {
    data: Vec<u8>,
    pos: usize,
}

/// # Safety
/// `path` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let name = unsafe { copy_cstr(path as u32) };
    let name = String::from_utf8_lossy(&name).into_owned();
    let Some(bytes) = wasm_vfs_lookup(&name) else {
        // 与 CRT 一致: 打开失败返回 NULL, 由引擎既有错误路径处理.
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(OpenDesc {
        data: bytes,
        pos: 0,
    })) as *mut c_void
}

/// 查 VFS 表 (thread_local 的薄封装, 供 fopen 用).
fn wasm_vfs_lookup(name: &str) -> Option<Vec<u8>> {
    VFS.with_borrow(|t| t.get(name).cloned())
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄.
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
    // C 语义: 返回完整读到的"项数" (= 字节数 / size, 向下取整).
    n / size
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄.
#[no_mangle]
pub unsafe extern "C" fn fwrite(
    _ptr: *const c_void,
    size: usize,
    nmemb: usize,
    _stream: *mut c_void,
) -> usize {
    // VFS 只读 (IWAD/PWAD/SF2 由宿主注册); 存档/演示写路径在 wasm 上
    // 本 spec 不持久化 -- 返回"已写满"以保持引擎状态机前进, 数据丢弃.
    let _ = size;
    nmemb
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄.
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
/// `stream` 必须是 fopen 返回的句柄.
#[no_mangle]
pub unsafe extern "C" fn ftell(stream: *mut c_void) -> c_long {
    if stream.is_null() {
        return -1;
    }
    let d = unsafe { &*(stream as *mut OpenDesc) };
    d.pos as c_long
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄.
#[no_mangle]
pub unsafe extern "C" fn fclose(stream: *mut c_void) -> c_int {
    if stream.is_null() {
        return -1;
    }
    // SAFETY: 句柄由 fopen 分配, fclose 即唯一释放点 (CRT 语义).
    drop(unsafe { Box::from_raw(stream as *mut OpenDesc) });
    0
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄或 null.
#[no_mangle]
pub unsafe extern "C" fn fflush(_stream: *mut c_void) -> c_int {
    0
}

/// printf 家族导出: 每个审计过的调用形状一个符号, 与 woom24-libc 的
/// 固定参数声明一一对应 (见模块头注释); 未传的槽不读.
///
/// # Safety
/// `fmt` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn printf0(fmt: *const c_char) -> c_int {
    let out = printf_impl(fmt, &[]);
    // stdout 在浏览器里落到 console (经 web 控制台可见).
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn printf1(fmt: *const c_char, a0: u32) -> c_int {
    let out = printf_impl(fmt, &[a0]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn printf2(fmt: *const c_char, a0: u32, a1: u32) -> c_int {
    let out = printf_impl(fmt, &[a0, a1]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn printf3(fmt: *const c_char, a0: u32, a1: u32, a2: u32) -> c_int {
    let out = printf_impl(fmt, &[a0, a1, a2]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

/// # Safety
/// `fmt` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn printf4(fmt: *const c_char, a0: u32, a1: u32, a2: u32, a3: u32) -> c_int {
    let out = printf_impl(fmt, &[a0, a1, a2, a3]);
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

unsafe fn printf_impl(fmt: *const c_char, slots: &[u32]) -> Vec<u8> {
    let f = copy_cstr(fmt as u32);
    // 逐说明符解析原始槽: %s 取指针解引用, 数值类直取槽值.
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
                // SAFETY: 引擎传入的 %s 实参都是有效 C 字符串.
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

/// snprintf 导出: 同 printf 家族, 写目标缓冲区并返回本应长度
/// (d_main.rs 依赖此值).
///
/// # Safety
/// `s`/`fmt` 必须有效; `s` 至少可写 `n` 字节.
#[no_mangle]
pub unsafe extern "C" fn snprintf1(
    s: *mut c_char,
    n: usize,
    fmt: *const c_char,
    a0: u32,
) -> c_int {
    let out = printf_impl(fmt, &[a0]);
    if !s.is_null() && n > 0 {
        let w = (out.len()).min(n - 1);
        std::ptr::copy_nonoverlapping(out.as_ptr(), s as *mut u8, w);
        *s.add(w) = 0;
    }
    out.len() as c_int
}

/// # Safety
/// `s`/`fmt` 必须有效; `s` 至少可写 `n` 字节.
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

/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    if s.is_null() {
        return -1;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let b = unsafe { copy_cstr(s as u32) };
    log::info!("{}", String::from_utf8_lossy(&b));
    b.len() as c_int + 1
}

/// # Safety
/// c 必须是合法字节值.
#[no_mangle]
pub unsafe extern "C" fn putchar(c: c_int) -> c_int {
    log::info!("{}", (c as u8) as char);
    c
}

/// C `malloc`: 失败返回 null.
///
/// # Safety
/// 返回指针须配对传给 `free`; `size` 为 0 时返回 null.
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    shm_malloc(size)
}

/// C `free`: 释放 `malloc` 返回的指针, null 是无害的 no-op.
///
/// # Safety
/// `p` 必须是尚未释放的 `malloc` 返回值.
#[no_mangle]
pub unsafe extern "C" fn free(p: *mut c_void) {
    shm_free(p)
}

/// # Safety
/// `s` 必须指向至少 `n` 字节可写内存.
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    shm_memset(s, c, n)
}

/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn atoi(s: *const c_char) -> c_int {
    if s.is_null() {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾.
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
/// `s` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    unsafe { copy_cstr(s as u32) }.len()
}

/// # Safety
/// `path` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn remove(_path: *const c_char) -> c_int {
    // VFS 不可变 (文件由宿主注册); 存档删除降级为成功 (与 fwrite 策略一致).
    0
}

/// # Safety
/// 两个参数都必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn rename(_old: *const c_char, _new: *const c_char) -> c_int {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- VFS 表 ----

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

    // ---- printf 子集 (golden cases) ----

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
        // z_zone.rs:349 的真实格式串.
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
        // 变参槽多于说明符: 多余的被忽略.
        assert_eq!(fmt_str("%i", &[FmtArg::I(1), FmtArg::I(2)]), "1");
        // 说明符多于变参: 降级 (不 trap), 用 `<na>` 占位并记日志.
        assert_eq!(fmt_str("%i %i", &[FmtArg::I(1)]), "1 <na>");
    }

    #[test]
    fn format_unknown_specifier_degrades_not_traps() {
        // 未审计的 %f 不在支持集合内: 原样降级输出.
        assert_eq!(fmt_str("v=%f", &[FmtArg::I(1)]), "v=<na>");
    }

    #[test]
    fn snprintf_returns_would_be_length_and_truncates() {
        // C 语义: 返回值是"本应写入"的长度; 缓冲区只收 min(len, n-1) + NUL.
        let mut out = [0u8; 8];
        let n = format(b"say %s", &[FmtArg::S(b"hello world".to_vec())], &mut out);
        assert_eq!(n, 15); // would-be
        assert_eq!(&out[..7], b"say hel");
        assert_eq!(out[7], 0);
    }

    // ---- malloc / memset (纯逻辑部分: 句柄表) ----

    #[test]
    fn allocator_roundtrip_via_heap() {
        let p = shm_malloc(64);
        assert!(!p.is_null());
        // 写入再读回, 确认可用.
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
}
