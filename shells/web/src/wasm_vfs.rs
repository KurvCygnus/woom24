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
// sscanf 子集: M_StrToInt 的四个格式串所需的最小解析器 (纯逻辑, 宿主机可测)
// ---------------------------------------------------------------------------

/// C `isspace` 子集 (空格/制表/换行等).
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// 吃掉一个可选符号, 返回 (是否负号, 剩余输入).
fn take_sign(input: &[u8]) -> (bool, &[u8]) {
    match input.first() {
        Some(b'-') => (true, &input[1..]),
        Some(b'+') => (false, &input[1..]),
        _ => (false, input),
    }
}

/// 按给定进制吃数字, 返回 (值, 剩余输入); 一个数字都没有则返回 None.
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

/// `sscanf` 单转换最小子集: 只实现引擎审计过的格式语法 --
/// 空格 = 跳过输入空白, 其余字面量逐字节匹配, `%x`/`%o`/`%d` 整数转换
/// (M_StrToInt 的 ` 0x%x` / ` 0X%x` / ` 0%o` / ` %d` 全部覆盖).
/// 返回成功赋值的转换数 (0 或 1); 失败时不动 `out`, 绝不 panic.
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
                        // C %x 接受输入侧可选 0x/0X 前缀.
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
                    // 未审计转换: 降级为匹配失败 (D1: log + degrade, never trap).
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
    //* 4096 是垫片自定的扫描上限 (沿用 Task 2 首版 %s 约定, 非 C 语义), 缺 NUL 的输入至多读 4096 字节即止.
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
// 第二批 CRT 符号: 字符/字符串/atof/calloc (fix round 1; 纯逻辑宿主机可测)
// ---------------------------------------------------------------------------

/// C `toupper` ASCII 子集 (引擎输入恒为 ASCII).
fn c_toupper(c: c_int) -> c_int {
    let b = c as u8;
    if b.is_ascii_lowercase() {
        (b - 32) as c_int
    } else {
        c
    }
}

/// C `tolower` ASCII 子集.
fn c_tolower(c: c_int) -> c_int {
    let b = c as u8;
    if b.is_ascii_uppercase() {
        (b + 32) as c_int
    } else {
        c
    }
}

/// C `isspace` ("C" locale): 空格/\t/\n/\v/\f/\r, 非 0 表示真.
fn c_isspace(c: c_int) -> c_int {
    c_int::from(is_c_space(c as u8))
}

/// Ordering → C 比较约定的 <0/0/>0.
fn order_to_c_int(o: std::cmp::Ordering) -> c_int {
    match o {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// C `strcmp`: 按无符号字节逐位比较; 一方先到 NUL 则短者小.
fn c_strcmp(a: &[u8], b: &[u8]) -> c_int {
    let n = a.len().min(b.len());
    match a[..n].iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => order_to_c_int(a[i].cmp(&b[i])),
        None => order_to_c_int(a.len().cmp(&b.len())),
    }
}

/// C `strncmp`: 最多比较 n 字节; 截断后即 strcmp 语义 (NUL 终止保证等价).
fn c_strncmp(a: &[u8], b: &[u8], n: usize) -> c_int {
    c_strcmp(&a[..n.min(a.len())], &b[..n.min(b.len())])
}

/// C `strncpy` 核心: 复制 min(src.len, n) 字节, 不足补 NUL 到 n;
/// src 不短于 n 时恰复制 n 字节、不写终止符. `dst.len()` 即 n.
fn c_strncpy_into(dst: &mut [u8], src: &[u8]) {
    let copy = src.len().min(dst.len());
    dst[..copy].copy_from_slice(&src[..copy]);
    dst[copy..].fill(0);
}

/// C `strrchr`: 最后一次出现 `(c as u8)` 的偏移; c=0 命中结束 NUL 槽;
/// 未找到返回 None (导出层转 null).
fn c_strrchr(s: &[u8], c: c_int) -> Option<usize> {
    let target = c as u8;
    if target == 0 {
        return Some(s.len());
    }
    s.iter().rposition(|&b| b == target)
}

/// C `strstr`: 最左匹配偏移; 空针 = 0; 未找到返回 None.
fn c_strstr(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// C `atof` (strtod-lite): 前导 C 空白 + 可选符号 + 整数/小数数字,
/// 取最长合法前缀, 无数字 → 0.0. 审计结论 (fix round 1): 引擎唯一调用点
/// m_config.rs:688 解析 .cfg 值, 引擎侧写入恒为普通十进制 (Rust 格式化
/// 从不产出指数), 故不实现指数形式.
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

/// C `calloc`: 布局跟踪分配器 + 清零; 乘法溢出或 size=0 返回 null.
/// 清零必须走 volatile 路径 (shm_memset 的注释: 普通 fill 会被 LLVM
/// 降为 `call memset`, 与本 crate 同名导出构成自递归).
fn shm_calloc(nmemb: usize, size: usize) -> *mut c_void {
    let Some(total) = nmemb.checked_mul(size) else {
        return std::ptr::null_mut();
    };
    let p = shm_malloc(total);
    if !p.is_null() && total > 0 {
        // SAFETY: p 指向 shm_malloc 的 total 字节可写内存.
        unsafe { shm_memset(p, 0, total) };
    }
    p
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

/// sscanf 单转换导出: 对应 M_StrToInt 的调用形状 (1 个输出指针槽,
/// 见 [`sscanf_parse1`]). C 语义: 返回成功赋值的转换数, 不匹配为 0.
///
/// # Safety
/// `s`/`fmt` 必须是 NUL 结尾 C 字符串; `a0` 必须指向可写的 `c_int`.
#[no_mangle]
pub unsafe extern "C" fn sscanf1(s: *const c_char, fmt: *const c_char, a0: usize) -> c_int {
    if s.is_null() || fmt.is_null() || a0 == 0 {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let input = unsafe { copy_cstr(s as u32) };
    let f = unsafe { copy_cstr(fmt as u32) };
    let mut v: i64 = 0;
    let n = sscanf_parse1(&f, &input, &mut v);
    if n == 1 {
        // SAFETY: a0 是调用方提供的可写 c_int 槽.
        unsafe {
            *(a0 as *mut c_int) = v as c_int;
        }
    }
    n as c_int
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

// ---------------------------------------------------------------------------
// 第二批 CRT 导出 (fix round 1): 声明见 woom24-libc
// ---------------------------------------------------------------------------

/// C `toupper`: ASCII 语义 (引擎输入恒为 ASCII).
#[no_mangle]
pub extern "C" fn toupper(c: c_int) -> c_int {
    c_toupper(c)
}

/// C `tolower`: ASCII 语义.
#[no_mangle]
pub extern "C" fn tolower(c: c_int) -> c_int {
    c_tolower(c)
}

/// C `isspace` ("C" locale 空白集合).
#[no_mangle]
pub extern "C" fn isspace(c: c_int) -> c_int {
    c_isspace(c)
}

/// C `strcmp`.
///
/// # Safety
/// 两个参数都必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let (a, b) = unsafe { (copy_cstr(s1 as u32), copy_cstr(s2 as u32)) };
    c_strcmp(&a, &b)
}

/// C `strncmp`.
///
/// # Safety
/// 两个参数都必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let (a, b) = unsafe { (copy_cstr(s1 as u32), copy_cstr(s2 as u32)) };
    c_strncmp(&a, &b, n)
}

/// C `strncpy`: 返回 `dst`; src 不短于 n 时恰复制 n 字节、不写终止符.
///
/// # Safety
/// `dst` 必须可写 `n` 字节; `src` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    if dst.is_null() || n == 0 {
        return dst;
    }
    //* 与 C 的偏差: src=NULL 在 C 里是 UB, 本垫片按"空 src"降级为向 dst 补零 n 字节; 引擎调用点不会传 NULL.
    let bytes = if src.is_null() {
        Vec::new()
    } else {
        // SAFETY: 调用方保证 NUL 结尾.
        unsafe { copy_cstr(src as u32) }
    };
    // SAFETY: 调用方保证 dst 至少可写 n 字节.
    unsafe {
        c_strncpy_into(std::slice::from_raw_parts_mut(dst as *mut u8, n), &bytes);
    }
    dst
}

/// C `strrchr`: 未找到返回 null; c=0 返回指向结束 NUL 的指针.
///
/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn strrchr(s: *const c_char, c: c_int) -> *mut c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let bytes = unsafe { copy_cstr(s as u32) };
    match c_strrchr(&bytes, c) {
        // SAFETY: off <= strlen, add 后仍在对象存储内 (含结束 NUL 槽);
        // C 约定 strrchr 返回可写字符指针, 故 const→mut 转换.
        Some(off) => unsafe { s.add(off) as *mut c_char },
        None => std::ptr::null_mut(),
    }
}

/// C `strstr`: 未找到返回 null.
///
/// # Safety
/// 两个参数都必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    if haystack.is_null() || needle.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let (hay, nee) = unsafe { (copy_cstr(haystack as u32), copy_cstr(needle as u32)) };
    match c_strstr(&hay, &nee) {
        // SAFETY: off <= strlen; C 约定 strstr 返回可写指针 (const→mut).
        Some(off) => unsafe { haystack.add(off) as *mut c_char },
        None => std::ptr::null_mut(),
    }
}

/// C `getenv`: wasm 无进程环境, 恒返回 NULL. 审计: 引擎仅查
/// HOME / XDG_CONFIG_HOME (m_misc.rs), 调用方均有未设回退路径.
/// 仅 wasm32 导出: 宿主测试二进制的 CRT 正常终止路径会调到本符号,
/// 交给宿主 CRT 才能保持 `cargo test` 全绿.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn getenv(_name: *const c_char) -> *mut c_char {
    std::ptr::null_mut()
}

/// C `atof` (strtod-lite, 无指数 -- 见 [`c_atof`] 审计注释).
///
/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn atof(s: *const c_char) -> f64 {
    if s.is_null() {
        return 0.0;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    unsafe { c_atof(&copy_cstr(s as u32)) }
}

/// C `calloc`: 布局跟踪分配器 + 清零 (清零走 volatile, 防自递归).
#[no_mangle]
pub extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    shm_calloc(nmemb, size)
}

/// C `exit`: wasm 上页面生命周期即进程生命周期, 引擎无正常退出路径
/// (d_main.rs:1504 在 I_Endoom 后调用) -- 到此即显式 trap (D1 降级点).
/// 仅 wasm32 导出: 宿主二进制的 CRT 在 main 返回后的正常终止也会调
/// `exit(0)`, 若被本符号截获会把宿主 `cargo test` 变成 trap.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn exit(status: c_int) -> ! {
    unreachable!("wasm shell: engine called exit({status})")
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

    // ---- sscanf 子集 (M_StrToInt 的四个 golden 格式串) ----

    fn scan1(fmt: &str, input: &str) -> Option<i64> {
        let mut v: i64 = 0;
        let n = sscanf_parse1(fmt.as_bytes(), input.as_bytes(), &mut v);
        (n == 1).then_some(v)
    }

    #[test]
    fn sscanf_hex_lower_and_upper_prefix() {
        // m_misc.rs M_StrToInt 的两条十六进制路径.
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
        // 字面量不匹配: 输入没有 0x 前缀.
        assert_eq!(scan1(" 0x%x", "12"), None);
        // 转换前输入耗尽.
        assert_eq!(scan1(" %d", "   "), None);
        // 前缀后没有合法八进制数字.
        assert_eq!(scan1(" 0%o", "0Z"), None);
    }

    // ---- 第二批 CRT 符号 (fix round 1): 字符/字符串/atof/calloc 纯逻辑 ----

    #[test]
    fn toupper_tolower_ascii_only() {
        assert_eq!(c_toupper(b'a' as c_int), b'A' as c_int);
        assert_eq!(c_toupper(b'z' as c_int), b'Z' as c_int);
        // 已大写/非字母原样返回.
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
        // 前缀短者小 (结束 NUL = 0 < 任意非零字节).
        assert!(c_strcmp(b"ab", b"abc") < 0);
        assert_eq!(c_strcmp(b"", b""), 0);
        // C strcmp 按无符号字节比较: 0x80 > 'a' (0x61).
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
        // src 短于 n: 复制全部 + NUL 填充到 n.
        let mut d = [b'#'; 6];
        c_strncpy_into(&mut d, b"ab");
        assert_eq!(&d, b"ab\0\0\0\0");
        // src 不短于 n: 恰好复制 n 字节, 不写终止符 (C 语义).
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
        // m_config.rs:688 的全部输入形态: 空白/符号/整数/小数/最长合法前缀.
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
        // 乘法溢出 → NULL (C 语义).
        assert!(shm_calloc(usize::MAX, 2).is_null());
        // nmemb = 0 → 与 malloc(0) 一致返回 null.
        assert!(shm_calloc(0, 8).is_null());
    }
}
