# WASM Shell (`shells/web`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `shells/web` workspace package (`room-shell-web`, cdylib) so the engine runs to completion in the browser: CRT/VFS shim closing the 76-error wasm gap, six `DG_*` callbacks, Web Audio backend, Canvas2D + WebGL2 presenters, and the two-entry contract from AGENTS.md.

**Architecture:** A thin cdylib shell over the platform-agnostic `room` lib, exactly mirroring `shells/native`'s shape: `#[no_mangle] extern "C"` `DG_*` callbacks + a `#[wasm_bindgen]` JS surface (`woom24_*` exports) driven by `requestAnimationFrame`. The CRT gap is closed by (a) a declaration-only `libc`-shaped seam crate so `libc::*` paths type-check on wasm, and (b) real `#[no_mangle]` implementations (in-memory VFS, allocator-backed malloc, printf subset) in `shells/web/src/wasm_vfs.rs` that satisfy every symbol at final link. Music synthesis is extracted once into `room/src/audio/synth.rs` and consumed by both shells.

**Tech Stack:** Rust stable 1.98.1, `wasm32-unknown-unknown` (installed), `wasm-bindgen` + `web-sys` + `js-sys` (spec-approved deps), `wasm-bindgen-cli` (to be installed, version-matched), `rustysynth` (via `room`, not re-added to the shell), plain static JS/HTML in `shells/web/www/` (no CDN, no bundler, no npm).

**Spec:** `docs/specs/2026-09-17-wasm-shell-design.md` (spec ②, approved). Series: ① platform shell (done) → **② this plan** → ③ c_tests/LP64 strategy.

---

## Global Constraints

Copied verbatim from the spec and AGENTS.md. Every task implicitly includes these.

- **Self-contained artifact:** the final `.wasm` + static assets run standalone from any static host. At runtime the engine fetches nothing: no CDN, no `fetch()` of remote resources, no telemetry, no server component. (AGENTS constraint 1.)
- **WAD files are copyrighted — never commit, bundle, or upload them.** Same discipline for the local `soundfonts/` SF2 (present in the working tree, not committed).
- **Pure Rust on `wasm32-unknown-unknown`** via wasm-bindgen. No Emscripten, no C dependencies. (AGENTS constraint 2.)
- **New crates allowed this spec (spec-approved in D7/architecture):** `wasm-bindgen`, `web-sys`, `js-sys` for `shells/web`; the `wasm-bindgen-cli` tool. Everything else requires explicit user approval (AGENTS tooling protocol 5). `rustysynth` is **not** re-added to the shell — the synth core is consumed through `room::audio::synth`.
- **`room/src/doom/` gets zero edits except one mechanical class:** the three const-layout asserts in `info.rs` / `m_menu.rs` get `#[cfg(target_pointer_width = "64")]` gates (spec ② Goal 3 closes all 76 errors; the gates are the spec's allowed "mechanical cfg change" class; spec ③ refines per-width expectations). Before touching anything under `room/src/doom/`, read `docs/upstream/room-AGENTS.md` (AGENTS.md requirement).
- **Two-entry contract (AGENTS.md):** `woom24_minimal_start` (device metadata + IWAD → launcher UI) and `woom24_standard_start` (complete boot profile → direct start) are separate exports converging on one `init_pipeline`. One must never degrade into the other.
- **Determinism guard (D6):** the shell touches the engine only through `DG_*`, argv, and the audio control plane. No wall-clock data reaches the simulation.
- **Comment language:** new files (all of `shells/web/**`, `room/src/audio/synth.rs`) default to **Chinese** comments with half-width ASCII punctuation, using the `//*` / `//!` / `//?` prefix system. Files that already use English (e.g. `shells/native/src/audio_music.rs`, `room/src/doom/info.rs`) keep their file-local convention.
- **`rustfmt` authoritative; zero clippy warnings goal** (`cargo fmt` on touched files; `cargo clippy` before each commit).
- **Bounded timeouts on every command; exit 124 = FAILED.** Never retry blindly, never raise the limit; a command needing >300 s is a signal to decompose. The single known exception path (one-shot `cargo install wasm-bindgen-cli`) has a documented fallback (Task 1).
- **No concurrent heavy commands.** Never run two builds/tests in parallel, not across tool-call batches, not across subagents.
- **Conventional commits**; stage only intended files; never commit WADs/SF2s/secrets.
- **Host suite stays green: 376 unit tests + 2 `demo_playthrough` integration tests** (spec acceptance 5). Run `cargo test` (root; default-members) before every commit.
- **Engine resolution baseline:** simulation is fixed at 35 tics/s; the JS `requestAnimationFrame` cadence only drives `woom24_tick()`; engine throttling is driven by its own tick clock via `DG_GetTicksMs`.

## Pinned Toolchain (probe recorded 2026-09-17)

| Probe | Result | Consequence |
|---|---|---|
| `rustup target list --installed` | `wasm32-unknown-unknown`, `x86_64-pc-windows-msvc` | Target already installed; do not re-add. |
| `rustup toolchain list` | `stable-x86_64-pc-windows-msvc` (active, default), `nightly` also present | **Use stable.** The shim must not require nightly. |
| `which wasm-pack wasm-bindgen` | neither installed | Build via plain `cargo build --target` + `wasm-bindgen` CLI (D7 option B). `wasm-pack` would duplicate the same install cost for no gain. |
| `which node python` | `node` at `D:\Apps\nodejs\node`, `python` 3.10 on PATH (npm/npx not on PATH) | Static serving for smoke tests: `python -m http.server` (Task 8). No npm tooling assumed. |
| `cargo --version` | 1.98.1 | wasm-bindgen CLI version must be resolved to match the `wasm-bindgen` crate (Task 1 Step 3). |

**Pinned build commands (used from Task 8 on):**

```bash
# 编译（在仓库根目录执行）
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
# 生成 JS 胶水（版本必须与 crate 一致，见 Task 1）
wasm-bindgen --target web --out-dir shells/web/www/pkg --no-typescript \
  target/wasm32-unknown-unknown/release/room_shell_web.wasm
```

**Pinned workspace wasm check (acceptance command, Task 2 onward):**

```bash
cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown
```

Expected from Task 2 on: **exit 0**. (`doomgeneric-sys` cannot check on wasm — its build.rs needs a C toolchain, probe-verified — so the equivalent workspace-wide form is `cargo check --workspace --exclude room-shell-native --exclude doomgeneric-sys --exclude c2rust-intermediate --target wasm32-unknown-unknown`; use either, never plain `--workspace` on wasm.)

**Mechanism note (load-bearing, verified by probe):** the 73 CRT errors are `libc::…` *path-resolution* failures inside `room` (see Appendix A). `#[no_mangle]` exports alone cannot fix those — the paths must resolve at type-check time. Hence two parts: `shells/web/crt` (package `woom24-libc`, declaration-only, target-gated into `room` as `libc`) makes the paths resolve, and `shells/web/src/wasm_vfs.rs` provides the real bodies exported as symbols. The remaining 3 errors are E0080 const-assert panics (ILP32 layout), closed by the cfg gates. Variadic `printf` call sites (0–4 varargs, audited) type-check against a variadic declaration and link against **one fixed-max-arity `#[export_name]` definition** — this exact pattern was probe-verified to compile *and link* a wasm cdylib on stable 1.98.1 (Appendix B).

---

### Task 1: Toolchain wiring + `shells/web` scaffold

**Files:**
- Create: `shells/web/Cargo.toml`
- Create: `shells/web/src/lib.rs`
- Create: `shells/web/crt/Cargo.toml`
- Create: `shells/web/crt/src/lib.rs` (declaration-only this task; documented fully in Task 2)
- Modify: `Cargo.toml:4-9` (workspace members)
- Create: `shells/web/README.md` (one paragraph: what this package is, pointer to build script from Task 8)

**Interfaces:**
- Consumes: workspace root `Cargo.toml` (members list), installed `wasm32-unknown-unknown` target.
- Produces: package `room-shell-web` (cdylib, lib name `room_shell_web`), package `woom24-libc` (lib name `woom24_libc`), and one trivial export `woom24_version() -> u32`. All later tasks add modules to `shells/web/src/lib.rs`.

- [ ] **Step 1: Record the probe results**

Run (each with a 60 s timeout; record output in the task report):

```bash
rustup target list --installed
rustup toolchain list
which wasm-pack wasm-bindgen node python
```

Expected: `wasm32-unknown-unknown` installed; stable default; wasm-pack/wasm-bindgen absent. If anything differs from the Pinned Toolchain table, stop and record the deviation before proceeding.

- [ ] **Step 2: Scaffold the two packages**

Create `shells/web/Cargo.toml`:

```toml
[package]
name = "room-shell-web"
version = "0.1.0"
edition = "2021"
description = "Wasm platform shell for the room engine (wasm-bindgen + Web Audio + Canvas2D/WebGL2)"
repository = "https://github.com/KurvCygnus/woom24"
license = "GPL-2.0"

[lib]
name = "room_shell_web"
crate-type = ["cdylib"]

[dependencies]
room = { path = "../../room" }
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "Window", "Document", "Element", "HtmlElement", "HtmlCanvasElement",
    "HtmlInputElement", "HtmlButtonElement", "HtmlDivElement",
    "CanvasRenderingContext2d", "ImageData", "KeyboardEvent", "Event",
    "EventTarget", "Performance", "FileReader", "File", "Blob", "ProgressEvent",
    "WebGl2RenderingContext", "WebGlShader", "WebGlProgram", "WebGlTexture",
    "WebGlBuffer", "WebGlUniformLocation",
] }
js-sys = "0.3"
log = "0.4"
```

Create `shells/web/src/lib.rs`:

```rust
//! woom24 的 wasm shell：把 room 引擎接进浏览器（spec ②）。
//!
//! 模块职责（随任务逐步落地）：
//! - `wasm_vfs`：CRT 垫片 + 内存 VFS（Task 2）
//! - `web_audio`：WebAudioBackend（Task 4）
//! - `clock` / `dg` / `present_c2d` / `present_gl2`：平台回调与呈现（Task 5/6）
//! - `profile` / `init_pipeline` / `launcher_ui`：两入口契约（Task 7）

use wasm_bindgen::prelude::*;

/// 返回 shell 构建版本号（脚手架冒烟用；避免链接器裁掉整个 crate）。
#[wasm_bindgen]
pub fn woom24_version() -> u32 {
    1
}
```

Create `shells/web/crt/Cargo.toml`:

```toml
[package]
name = "woom24-libc"
version = "0.1.0"
edition = "2021"
description = "Declaration-only libc-shaped seam so room's libc:: paths type-check on wasm32; real bodies live in shells/web/src/wasm_vfs.rs"
repository = "https://github.com/KurvCygnus/woom24"
license = "GPL-2.0"

[lib]
name = "woom24_libc"
```

Create `shells/web/crt/src/lib.rs` (placeholder body this task, replaced in Task 2):

```rust
//! 声明层垫片：仅在 wasm32 上替代 crates.io `libc`，让 `room` 里的
//! `libc::fopen` / `libc::printf` 等路径通过类型检查。
//! 真正的实现（内存 VFS / malloc / printf 子集）在 `shells/web/src/wasm_vfs.rs`。
//!
//! //! 本 crate 不得包含任何实现逻辑；不得依赖任何其它 crate。
```

Modify root `Cargo.toml` members (keep `default-members` **unchanged** — host `cargo build/test` must not pull web-sys):

```toml
members = [
    "c2rust-intermediate",
    "doomgeneric-sys",
    "room",
    "shells/native",
    "shells/web",
    "shells/web/crt",
]
```

- [ ] **Step 3: Resolve the wasm-bindgen crate version and install the matching CLI**

```bash
cargo tree -p room-shell-web --depth 1 | grep wasm-bindgen
```

Expected: one line like `room-shell-web v0.1.0 (...) ─── wasm-bindgen v0.2.10X` — record `V` = that exact version. Then install the CLI (one-shot, may take several minutes):

```bash
cargo install wasm-bindgen-cli --version <V> --locked
```

Run with a 600 s timeout. Exit 124 = timed out = FAILED: record the stall, then use the documented fallback instead of retrying — download the prebuilt release binary for `V` from `https://github.com/rustwasm/wasm-bindgen/releases/tag/v<V>` (`wasm-bindgen-cli-<V>-x86_64-pc-windows-msvc.tar.gz`), unpack, and put `wasm-bindgen.exe` on PATH for the session. Verify: `wasm-bindgen --version` prints `V`.

- [ ] **Step 4: Verify host check is green**

Run: `cargo check -p room-shell-web -p woom24-libc` (timeout 300 s)
Expected: exit 0, no warnings (`cargo clippy -p room-shell-web -p woom24-libc -- -D warnings` also exit 0).

- [ ] **Step 5: Verify the wasm build + glue generation**

```bash
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir /tmp/woom24-scaffold-pkg --no-typescript \
  target/wasm32-unknown-unknown/release/room_shell_web.wasm
```

Expected: exit 0 both; `/tmp/woom24-scaffold-pkg/` contains `room_shell_web.js` + `room_shell_web_bg.wasm`. (Do not commit `pkg/` output; it is regenerated by the build script in Task 8.)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml shells/web/Cargo.toml shells/web/src/lib.rs shells/web/crt/Cargo.toml shells/web/crt/src/lib.rs shells/web/README.md Cargo.lock
git commit -m "feat(web): scaffold room-shell-web cdylib and wasm toolchain wiring"
```

---

### Task 2: CRT shim `wasm_vfs.rs` + `woom24-libc` seam (closes the 76-error wasm gap)

**Files:**
- Create: `shells/web/src/wasm_vfs.rs`
- Modify: `shells/web/src/lib.rs` (add `pub mod wasm_vfs;`)
- Modify: `shells/web/crt/src/lib.rs` (full declaration surface, replaces placeholder)
- Modify: `room/Cargo.toml:32-33` (target-gate the `libc` dependency)
- Modify: `room/src/doom/info.rs:910` (cfg-gate one assert)
- Modify: `room/src/doom/m_menu.rs:175-179` (cfg-gate two asserts)

**Interfaces:**
- Consumes: nothing from other tasks (room's `libc::*` call sites, audited in Appendix A).
- Produces:
  - Rust-internal, host-testable core: `wasm_vfs::VfsTable` (`new()`, `register(&mut self, name: &str, bytes: Vec<u8>)`, `get(&self, name: &str) -> Option<&Vec<u8>>`), `wasm_vfs::FmtArg { I(i32), U(u32), P(u32), S(Vec<u8>) }`, `wasm_vfs::format(fmt: &[u8], args: &[FmtArg], out: &mut [u8]) -> usize` (returns would-be length, C semantics).
  - Thread-local registration API used by Task 7: `wasm_vfs::vfs_register(name: &str, bytes: Vec<u8>)` and `wasm_vfs::vfs_get(name: &str) -> Option<Vec<u8>>`.
  - Exported C symbols (full table in Appendix A): `fopen fread fwrite fseek ftell fclose fflush printf snprintf puts putchar malloc free memset atoi strlen remove rename` — these satisfy `room`'s `libc::` calls and `m_misc.rs`'s own `extern "C"` block at link time.
  - `room`'s wasm graph resolves `libc::…` through `woom24-libc` (`pub use std::ffi::{c_char, c_int, c_long, c_uint, c_void}`, opaque `pub enum FILE {}`, `pub const SEEK_SET: c_int = 0;`, plus extern declarations with libc 0.2-identical signatures).

- [ ] **Step 1: Write the failing host tests for the VFS table and formatter**

Append to `shells/web/src/wasm_vfs.rs` (create the file with the test module first — TDD):

```rust
//! CRT 垫片 + 内存 VFS（spec ② D1）。
//!
//! //! 设计要点：
//! - `libc::` 路径的类型解析由 `shells/web/crt`（woom24-libc）承担；
//!   本模块通过 `#[no_mangle]` / `#[export_name]` 提供同名符号的实现，
//!   在最终 cdylib 链接时闭合引擎的全部未解析引用。
//! - printf 家族：声明是变参（调用点 0..4 个变参都存在），
//!   实现是固定 4 个参数槽的 `#[export_name]` 函数——已探针验证
//!   该组合在 stable 的 wasm32-unknown-unknown 上可编译可链接。
//!   未用的槽是垃圾值，但格式串里没有的说明符永远不去读它。
//! - 超出已审计说明符集合（`%s %d %i %u %x %c %p %%` + 宽度）时：
//!   记录日志并降级输出，绝不 trap（D1）。

// 实现将在后续步骤填入；先写测试。
```

Then the test module at the bottom of the same file:

```rust
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

    // ---- printf 子集（golden cases）----

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
        // z_zone.rs:349 的真实格式串。
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
        // 变参槽多于说明符：多余的被忽略。
        assert_eq!(fmt_str("%i", &[FmtArg::I(1), FmtArg::I(2)]), "1");
        // 说明符多于变参：降级（不 trap），用 `<na>` 占位并记日志。
        assert_eq!(fmt_str("%i %i", &[FmtArg::I(1)]), "1 <na>");
    }

    #[test]
    fn format_unknown_specifier_degrades_not_traps() {
        // 未审计的 %f 不在支持集合内：原样降级输出。
        assert_eq!(fmt_str("v=%f", &[FmtArg::I(1)]), "v=<na>");
    }

    #[test]
    fn snprintf_returns_would_be_length_and_truncates() {
        // C 语义：返回值是"本应写入"的长度；缓冲区只收 min(len, n-1) + NUL。
        let mut out = [0u8; 8];
        let n = format(b"say %s", &[FmtArg::S(b"hello world".to_vec())], &mut out);
        assert_eq!(n, 15); // would-be
        assert_eq!(&out[..7], b"say hel");
        assert_eq!(out[7], 0);
    }

    // ---- malloc / memset（纯逻辑部分：句柄表）----

    #[test]
    fn allocator_roundtrip_via_heap() {
        let p = shm_malloc(64);
        assert!(!p.is_null());
        // 写入再读回，确认可用。
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p room-shell-web --lib` (timeout 300 s)
Expected: FAIL — compile errors `cannot find type VfsTable` / `FmtArg` / `format` / `shm_malloc` etc. (TDD red).

- [ ] **Step 3: Write the implementation**

Full `shells/web/src/wasm_vfs.rs` (keep the test module from Step 1; the module doc comment from Step 1 stays at top):

```rust
use std::alloc::{alloc, dealloc, Layout};
use std::collections::HashMap;
use std::cell::RefCell;

use std::ffi::{c_char, c_int, c_long, c_void};

thread_local! {
    /// 进程级 VFS 表。JS 经 `woom24_register_file` 注册，引擎经 fopen 消费。
    static VFS: RefCell<VfsTable> = RefCell::new(VfsTable::new());
}

/// 名字 → 字节 的内存文件表（D1）。
pub(crate) struct VfsTable {
    files: HashMap<String, Vec<u8>>,
}

impl VfsTable {
    pub(crate) fn new() -> Self {
        Self { files: HashMap::new() }
    }

    pub(crate) fn register(&mut self, name: &str, bytes: Vec<u8>) {
        self.files.insert(name.to_string(), bytes);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.files.get(name)
    }
}

/// 供 shell 其它模块（init_pipeline / launcher_ui）注册文件。
pub fn vfs_register(name: &str, bytes: Vec<u8>) {
    VFS.with_borrow_mut(|t| t.register(name, bytes));
}

/// 供 shell 其它模块读取已注册文件（SF2 预载等）。
pub fn vfs_get(name: &str) -> Option<Vec<u8>> {
    VFS.with_borrow(|t| t.get(name).cloned())
}

// ---------------------------------------------------------------------------
// 格式化子集：纯逻辑，宿主机可测
// ---------------------------------------------------------------------------

/// 一个已解析的格式参数。指针在 wasm 侧已被复制成字节串，
/// 所以 `format` 本身是纯函数（宿主机测试不需要 wasm 内存）。
#[derive(Debug, PartialEq)]
pub(crate) enum FmtArg {
    I(i32),
    U(u32),
    P(u32),
    S(Vec<u8>),
}

/// 把 `fmt` 按 `args` 格式化进 `out`，返回 C 语义的"本应写入"长度。
/// 超长截断（`out` 里保留 min(len, out.len()-1) 字节 + 不写 NUL——
/// NUL 由 snprintf 外层负责）；未知/缺失参数降级为 `<na>`，绝不 panic。
pub(crate) fn format(fmt: &[u8], args: &[FmtArg], out: &mut [u8]) -> usize {
    fn degrade() -> Vec<u8> {
        b"<na>".to_vec()
    }
    fn push_byte(would: &mut usize, written: &mut usize, out: &mut [u8], b: u8) {
        // 截断规则：最多写 out.len()-1 字节（留一位给调用方的 NUL 语义）。
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
        // 宽度（仅十进制右对齐，如 %7i）。
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
        // 先完整渲染本说明符，再做宽度填充与截断写入。
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
            // 未审计说明符：降级占位（D1：log + degrade，never trap）。
            other => {
                log::warn!("wasm_vfs: unsupported format specifier %{other} degraded");
                degrade()
            }
        };
        // 右对齐宽度填充。
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
// malloc / memset / free：转发到 Rust 全局分配器 + 布局跟踪表（D1）
// ---------------------------------------------------------------------------

thread_local! {
    /// 指针 → Layout，free 时需要。
    static LAYOUTS: RefCell<HashMap<usize, Layout>> = RefCell::new(HashMap::new());
}

/// C `malloc` 语义：失败返回 null（这里几乎不会失败）。
fn shm_malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: size > 0 且 align=8 满足 Layout::from_size_align 的约束。
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

/// C `free` 语义。注意：权威清单里引擎当前并未引用 free
/// （见计划附录 A），这里随 malloc 成对提供，防未来泄漏。
fn shm_free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    let layout = LAYOUTS.with_borrow_mut(|m| m.remove(&(p as usize)));
    if let Some(layout) = layout {
        // SAFETY: 指针来自 shm_malloc 且布局从表中取回，未被重复释放。
        unsafe { dealloc(p as *mut u8, layout) };
    }
}

/// C `memset` 语义：返回 `s`。
///
/// # Safety
/// `s` 必须指向至少 `n` 字节可写内存。
unsafe fn shm_memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    if !s.is_null() && n > 0 {
        std::ptr::write_bytes(s as *mut u8, c as u8, n);
    }
    s
}

/// 从 wasm 线性内存复制 NUL 结尾字符串（printf %s 用）。
///
/// # Safety
/// `p` 必须指向可读内存且在 4096 字节内有 NUL（引擎内格式参数均满足）。
unsafe fn copy_cstr(p: u32) -> Vec<u8> {
    if p == 0 {
        return b"(null)".to_vec();
    }
    let mut out = Vec::new();
    let mut a = p;
    for _ in 0..4096 {
        let b = *(a as *const u8);
        if b == 0 {
            break;
        }
        out.push(b);
        a += 1;
    }
    out
}
```

Then append the `#[no_mangle]` / `#[export_name]` symbol surface at the bottom (still in `wasm_vfs.rs`, above the test module):

```rust
// ---------------------------------------------------------------------------
// 导出符号面：与附录 A 一一对应（权威清单 = cargo check 捕获）
// ---------------------------------------------------------------------------

/// 打开的文件描述：fopen 时一次性拷贝文件字节（几 MB 级 WAD 一次克隆，
/// 之后 fread 全部本地切片——避免逐读克隆大文件）+ 读游标。
/// fopen 返回它的裸指针作句柄，fclose 是唯一释放点（CRT 语义）。
struct OpenDesc {
    data: Vec<u8>,
    pos: usize,
}

/// # Safety
/// `path` 必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn fopen(path: *const c_char, _mode: *const c_char) -> *mut c_void {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: 调用方保证 NUL 结尾。
    let name = unsafe { copy_cstr(path as u32) };
    let name = String::from_utf8_lossy(&name).into_owned();
    let Some(bytes) = wasm_vfs_lookup(&name) else {
        // 与 CRT 一致：打开失败返回 NULL，由引擎既有错误路径处理。
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(OpenDesc { data: bytes, pos: 0 })) as *mut c_void
}

/// 查 VFS 表（thread_local 的薄封装，供 fopen 用）。
fn wasm_vfs_lookup(name: &str) -> Option<Vec<u8>> {
    VFS.with_borrow(|t| t.get(name).cloned())
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄。
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
    // C 语义：返回完整读到的"项数"（= 字节数 / size，向下取整）。
    n / size
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄。
#[no_mangle]
pub unsafe extern "C" fn fwrite(
    _ptr: *const c_void,
    size: usize,
    nmemb: usize,
    _stream: *mut c_void,
) -> usize {
    // VFS 只读（IWAD/PWAD/SF2 由宿主注册）；存档/演示写路径在 wasm 上
    // 本 spec 不持久化——返回"已写满"以保持引擎状态机前进，数据丢弃。
    let _ = size;
    nmemb
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄。
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
/// `stream` 必须是 fopen 返回的句柄。
#[no_mangle]
pub unsafe extern "C" fn ftell(stream: *mut c_void) -> c_long {
    if stream.is_null() {
        return -1;
    }
    let d = unsafe { &*(stream as *mut OpenDesc) };
    d.pos as c_long
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄。
#[no_mangle]
pub unsafe extern "C" fn fclose(stream: *mut c_void) -> c_int {
    if stream.is_null() {
        return -1;
    }
    // SAFETY: 句柄由 fopen 分配，fclose 即唯一释放点（CRT 语义）。
    drop(unsafe { Box::from_raw(stream as *mut OpenDesc) });
    0
}

/// # Safety
/// `stream` 必须是 fopen 返回的句柄或 null。
#[no_mangle]
pub unsafe extern "C" fn fflush(_stream: *mut c_void) -> c_int {
    0
}

/// printf：变参声明 + 固定 4 槽实现（见模块头注释）。
///
/// # Safety
/// `fmt` 必须是 NUL 结尾 C 字符串。
#[export_name = "printf"]
pub unsafe extern "C" fn printf_shim(
    fmt: *const c_char,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
) -> c_int {
    let out = printf_impl(fmt, &[a0, a1, a2, a3]);
    // stdout 在浏览器里落到 console（经 web 控制台可见）。
    log::info!("{}", String::from_utf8_lossy(&out));
    out.len() as c_int
}

unsafe fn printf_impl(fmt: *const c_char, slots: &[u32; 4]) -> Vec<u8> {
    let f = copy_cstr(fmt as u32);
    // 逐说明符解析原始槽：%s 取指针解引用，数值类直取槽值。
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
                // SAFETY: 引擎传入的 %s 实参都是有效 C 字符串。
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

/// snprintf：同 printf，写目标缓冲区并返回本应长度（d_main.rs 依赖此值）。
///
/// # Safety
/// `s`/`fmt` 必须有效；`s` 至少可写 `n` 字节。
#[export_name = "snprintf"]
pub unsafe extern "C" fn snprintf_shim(
    s: *mut c_char,
    n: usize,
    fmt: *const c_char,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
) -> c_int {
    let out = printf_impl(fmt, &[a0, a1, a2, a3]);
    if !s.is_null() && n > 0 {
        let w = (out.len()).min(n - 1);
        std::ptr::copy_nonoverlapping(out.as_ptr(), s as *mut u8, w);
        *s.add(w) = 0;
    }
    out.len() as c_int
}

/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    if s.is_null() {
        return -1;
    }
    // SAFETY: 调用方保证 NUL 结尾。
    let b = unsafe { copy_cstr(s as u32) };
    log::info!("{}", String::from_utf8_lossy(&b));
    b.len() as c_int + 1
}

/// # Safety
/// c 必须是合法字节值。
#[no_mangle]
pub unsafe extern "C" fn putchar(c: c_int) -> c_int {
    log::info!("{}", (c as u8) as char);
    c
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    shm_malloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn free(p: *mut c_void) {
    shm_free(p)
}

/// # Safety
/// `s` 必须指向至少 `n` 字节可写内存。
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    shm_memset(s, c, n)
}

/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn atoi(s: *const c_char) -> c_int {
    if s.is_null() {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾。
    let b = unsafe { copy_cstr(s as u32) };
    let t = String::from_utf8_lossy(&b);
    let t = t.trim_start();
    let (neg, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let v: i64 = digits.chars().take_while(|c| c.is_ascii_digit()).fold(0, |acc, c| {
        acc * 10 + (c as u8 - b'0') as i64
    });
    let v = if neg { -v } else { v };
    v.clamp(i32::MIN as i64, i32::MAX as i64) as c_int
}

/// # Safety
/// `s` 必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    // SAFETY: 调用方保证 NUL 结尾。
    unsafe { copy_cstr(s as u32) }.len()
}

/// # Safety
/// `path` 必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn remove(_path: *const c_char) -> c_int {
    // VFS 不可变（文件由宿主注册）；存档删除降级为成功（与 fwrite 策略一致）。
    0
}

/// # Safety
/// 两个参数都必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn rename(_old: *const c_char, _new: *const c_char) -> c_int {
    0
}
```

Add `pub mod wasm_vfs;` to `shells/web/src/lib.rs` (after the `use wasm_bindgen::prelude::*;` line):

```rust
pub mod wasm_vfs;
```

Replace `shells/web/crt/src/lib.rs` placeholder with the full declaration surface (signatures mirror crates.io `libc` 0.2 exactly — anything that compiled against real libc compiles against these):

```rust
//! 声明层垫片：仅在 wasm32 上替代 crates.io `libc`，让 `room` 里的
//! `libc::fopen` / `libc::printf` 等路径通过类型检查（spec ② D1）。
//! 真正的实现（内存 VFS / malloc / printf 子集）在 `shells/web/src/wasm_vfs.rs`，
//! 由 `#[no_mangle]` 导出同名符号，在最终 cdylib 链接时闭合引用。
//!
//! //! 规则：本 crate 只声明、不实现；签名与 libc 0.2 逐字一致；
//! 权威符号清单见 docs/plans/2026-09-17-wasm-shell-implementation.md 附录 A。

pub use std::ffi::{c_char, c_int, c_long, c_uint, c_void};

/// 不透明 FILE 句柄（同 m_misc.rs 的占位 enum 手法）。
#[repr(C)]
pub enum FILE {}

/// stdio seek 常量（wasm_vfs::fseek 只支持这三种）。
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
    // 变参声明：引擎调用点存在 0..4 个变参（附录 A），必须用 `...` 才能全部通过类型检查。
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
```

Modify `room/Cargo.toml` — replace the plain dependency (lines 32-33):

```toml
# libc bindings for C FFI (FILE, fopen, fclose, fseek, fread, printf, etc.)
# 宿主目标仍用真实 libc；wasm32 换成本仓库的声明垫片，
# 实现在 shells/web/src/wasm_vfs.rs（spec ② D1）。
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
libc = "0.2"

[target.'cfg(target_arch = "wasm32")'.dependencies]
libc = { package = "woom24-libc", path = "../shells/web/crt" }
```

Modify `room/src/doom/info.rs:910` (the failing `State` size assert only; leave the `MobjInfo` asserts and both `offset_of!` asserts untouched — they pass on ILP32):

```rust
//? ILP32（wasm32）上 `State` 布局与 C 原型不同，期望值需按位宽细化 = spec ③。
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<State>() == 40);
```

Modify `room/src/doom/m_menu.rs:175-179` (both failing asserts):

```rust
//? ILP32（wasm32）上菜单结构布局与 C 原型不同，期望值需按位宽细化 = spec ③。
#[cfg(target_pointer_width = "64")]
const _: () = assert!(
    std::mem::size_of::<menuitem_t>() == 32,
    "menuitem_t size mismatch"
);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<menu_t>() == 40, "menu_t size mismatch");
```

- [ ] **Step 4: Run the shim's own tests (host) to green**

Run: `cargo test -p room-shell-web --lib` (timeout 300 s)
Expected: PASS — all 11 tests from Step 1. If `format_width_and_u_x_c` or `snprintf_returns_would_be_length_and_truncates` fail, fix `format` until golden cases pass — do not weaken the tests.

- [ ] **Step 5: Run the pinned workspace wasm check — the acceptance gate**

Run: `cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown` (timeout 300 s)
Expected: **exit 0**, zero errors (73 CRT resolution errors closed by the shim; 3 E0080 closed by the cfg gates). If any `error[E0425]/[E0432]` remain, a symbol from Appendix A is missing — add the missing declaration+definition pair and re-run. If an `error[E0308]` (type mismatch) appears at a call site, the declaration signature does not match the audited call shape — fix the declaration, never the call site.

- [ ] **Step 6: Verify the full host suite is still green**

Run: `cargo test` (root; timeout 300 s) and `cargo check --workspace` (timeout 300 s)
Expected: exit 0 — 376 unit + 2 demo tests pass; native shell unaffected by the Cargo.toml target-gating.

- [ ] **Step 7: Commit**

```bash
git add shells/web/src/wasm_vfs.rs shells/web/src/lib.rs shells/web/crt/src/lib.rs room/Cargo.toml room/src/doom/info.rs room/src/doom/m_menu.rs Cargo.lock
git commit -m "feat(web): wasm CRT shim with in-memory VFS closes the 76-error wasm gap"
```

---

### Task 3: Extract `SynthEngine` into `room/src/audio/synth.rs` (lib work, no web code)

**Files:**
- Create: `room/src/audio/synth.rs`
- Modify: `room/src/audio/mod.rs:17-21` (register module)
- Modify: `shells/native/src/audio_music.rs` (refactor `MusicSource`/`MusicState` onto the new core)

**Interfaces:**
- Consumes: `room::audio::SAMPLE_RATE` (`pub const SAMPLE_RATE: i32 = 44100;`), `room::audio::mus2midi` (for tests), `rustysynth` (already a `room` dependency).
- Produces (consumed by Task 4 and by the refactored native shell):
  - `room::audio::synth::SynthFont::parse(sf2_bytes: &[u8]) -> Option<SynthFont>`
  - `room::audio::synth::SynthEngine::new(font: &SynthFont, midi_bytes: &[u8], looping: bool) -> Option<SynthEngine>`
  - `room::audio::synth::SynthEngine::next_interleaved(&mut self) -> Option<f32>` (interleaved L,R,… at 44100 Hz; `None` after end-of-sequence is drained)
  - `room::audio::synth::SynthEngine::is_finished(&self) -> bool`
  - `room::audio::synth::BLOCK_SIZE: usize = 512`
  - `SynthFont` is opaque (rustysynth no longer appears in either shell's API surface).

- [ ] **Step 1: Write the failing tests**

Create `room/src/audio/synth.rs` containing only the doc comment and test module:

```rust
//! 拉取式合成核心：rustysynth 的薄封装（spec ② D4）。
//!
//! 把"MIDI 字节 → 交错立体声 f32 采样"的唯一实现放进 lib，
//! native（rodio Source）与 web（AudioBuffer 分块）各自做输出适配，
//! 合成逻辑绝不在两个 shell 里重复（AGENTS.md）。
//!
//! //! 命令式状态：512 帧块渲染 + 交错游标 + 结束标志。
//! 音量不在本模块（输出适配层各自缩放），保持核心纯净。

/// 每次 sequencer render 渲染的帧数（左右各 BLOCK_SIZE 个采样）。
pub const BLOCK_SIZE: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mus2midi;

    /// 与 music.rs 测试同款的最小 MUS，但带一个 note-on，
    /// 让合成器有真实事件可渲染。
    fn note_on_mus() -> Vec<u8> {
        let mut d = vec![0u8; 19];
        d[0..4].copy_from_slice(b"MUS\x1a");
        d[4] = 3; // score_len（低字节）
        d[6] = 16; // score_start
        d[16] = 0x10; // note-on, channel 0
        d[17] = 0x3c; // note 60
        d[18] = 0x60; // score end（非 last，仅占位）
        d
    }

    #[test]
    fn synth_font_rejects_garbage() {
        assert!(SynthFont::parse(b"not a soundfont").is_none());
        assert!(SynthFont::parse(&[]).is_none());
    }

    #[test]
    fn synth_engine_rejects_bad_midi() {
        // 没有 SF2 时构造不出引擎，先造一个拒绝路径：坏 MIDI 必须在
        // 有字体前置下也返回 None —— 这里退而验证 mus2midi 的守门：
        assert!(mus2midi(b"garbage").is_none());
    }

    #[test]
    fn synth_engine_renders_deterministic_samples() {
        // 需要本地 SF2（soundfonts/ 下任一，不入库）；缺失则跳过并说明。
        let Some(sf2_bytes) = find_local_sf2() else {
            eprintln!("skip: no local .sf2 under soundfonts/ — determinism not exercised");
            return;
        };
        let font = SynthFont::parse(&sf2_bytes).expect("local sf2 must parse");
        let midi = mus2midi(&note_on_mus()).expect("fixture MUS must convert");
        let mut a = SynthEngine::new(&font, &midi, false).expect("engine a");
        let mut b = SynthEngine::new(&font, &midi, false).expect("engine b");
        let sa: Vec<f32> = (0..4096).filter_map(|_| a.next_interleaved()).collect();
        let sb: Vec<f32> = (0..4096).filter_map(|_| b.next_interleaved()).collect();
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert_eq!(x.to_bits(), y.to_bits(), "同一输入必须逐位一致");
        }
        // note-on 后不应全零（事件真的被合成了）。
        assert!(sa.iter().any(|&s| s != 0.0));
    }

    /// 在 soundfonts/ 下找任一 .sf2（开发机有 SC-55 字体；CI/无字体环境跳过）。
    fn find_local_sf2() -> Option<Vec<u8>> {
        for entry in std::fs::read_dir("soundfonts").ok()?.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "sf2") {
                return std::fs::read(&p).ok();
            }
            // 允许一级子目录（仓库现状：soundfonts/SC55Soundfont-1.2b/*.sf2）。
            if p.is_dir() {
                for sub in std::fs::read_dir(&p).ok()?.flatten() {
                    let sp = sub.path();
                    if sp.extension().is_some_and(|e| e == "sf2") {
                        return std::fs::read(&sp).ok();
                    }
                }
            }
        }
        None
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p room --lib audio::synth` (timeout 300 s)
Expected: FAIL — compile error: `SynthFont` / `SynthEngine` not defined.

- [ ] **Step 3: Implement `SynthEngine`**

Add above the test module in `room/src/audio/synth.rs`:

```rust
use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};
use std::io::Cursor;
use std::sync::Arc;

use super::SAMPLE_RATE;

/// 已解析的 SF2 字体。对外不透明：rustysynth 类型不再出现在任何 shell 的 API 上。
/// Clone = Arc 克隆（廉价），供后端在多次 play_music 间复用。
#[derive(Clone)]
pub struct SynthFont(Arc<SoundFont>);

impl SynthFont {
    /// 从字节解析 SF2；失败返回 None（调用方记日志并保持静音）。
    pub fn parse(sf2_bytes: &[u8]) -> Option<Self> {
        SoundFont::new(&mut Cursor::new(sf2_bytes))
            .ok()
            .map(Arc::new)
            .map(Self)
    }
}

/// 一次播放会话：sequencer + 双声道暂存块 + 交错游标。
/// 字体本身不进来（Synthesizer 内部持有 Arc<SoundFont>），避免每次播放重解析。
pub struct SynthEngine {
    sequencer: MidiFileSequencer,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    finished: bool,
}

impl SynthEngine {
    /// 由字体 + SMF 字节构建播放会话。MIDI 解析失败返回 None。
    pub fn new(font: &SynthFont, midi_bytes: &[u8], looping: bool) -> Option<Self> {
        let settings = SynthesizerSettings::new(SAMPLE_RATE);
        let synthesizer = Synthesizer::new(font.0.as_ref(), &settings).ok()?;
        let midi_file = Arc::new(MidiFile::new(&mut Cursor::new(midi_bytes)).ok()?);
        let mut sequencer = MidiFileSequencer::new(synthesizer);
        sequencer.play(&midi_file, looping);
        Some(Self {
            sequencer,
            buf_l: vec![0.0f32; BLOCK_SIZE],
            buf_r: vec![0.0f32; BLOCK_SIZE],
            pos: BLOCK_SIZE * 2, // 首次 next_interleaved 触发渲染
            finished: false,
        })
    }

    /// 交错输出下一个采样（L,R,L,R…）。序列结束且当前块排空后返回 None。
    pub fn next_interleaved(&mut self) -> Option<f32> {
        if self.pos >= BLOCK_SIZE * 2 {
            if self.finished {
                return None;
            }
            self.sequencer.render(&mut self.buf_l, &mut self.buf_r);
            self.pos = 0;
            if self.sequencer.end_of_sequence() {
                self.finished = true;
            }
        }
        let s = if self.pos % 2 == 0 {
            self.buf_l[self.pos / 2]
        } else {
            self.buf_r[self.pos / 2]
        };
        self.pos += 1;
        Some(s)
    }

    /// 是否已到序列末尾（当前块可能仍在排空）。
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}
```

Register the module in `room/src/audio/mod.rs` — change line 17-18 region to:

```rust
pub(crate) mod music;
pub mod sfx;
pub mod synth;
```

and extend the re-export line 20-21 region:

```rust
pub use music::{mus2midi, SAMPLE_RATE};
pub use sfx::{decode_doom_sfx, gains_from, PanState};
pub use synth::{SynthEngine, SynthFont};
```

- [ ] **Step 4: Run the new tests to green**

Run: `cargo test -p room --lib audio::synth` (timeout 300 s)
Expected: PASS — 3 tests (the determinism test runs on this machine because `soundfonts/SC55Soundfont-1.2b/SC-55 SoundFont v1.2b.sf2` exists locally; it skips with a printed reason where no SF2 exists).

- [ ] **Step 5: Refactor the native `MusicSource`/`MusicState` onto `SynthEngine` — behavior-identical**

In `shells/native/src/audio_music.rs`:

Replace the `MusicSource` struct and its impls (lines 50-145) with:

```rust
/// rodio 适配层：SynthEngine 输出 × 共享音量。渲染逻辑全部在 lib 侧。
pub(crate) struct MusicSource {
    engine: SynthEngine,
    volume: Arc<AtomicU32>,
}

impl MusicSource {
    /// 由字体与 SMF 字节构建；`looping` 交给引擎循环播放。
    pub(crate) fn new(
        sound_font: &SynthFont,
        midi_bytes: &[u8],
        looping: bool,
        volume: Arc<AtomicU32>,
    ) -> Option<Self> {
        Some(Self {
            engine: SynthEngine::new(sound_font, midi_bytes, looping)?,
            volume,
        })
    }
}

impl Iterator for MusicSource {
    type Item = f32;

    /// 与重构前逐位一致：SynthEngine.next_interleaved() × 音量位图。
    fn next(&mut self) -> Option<f32> {
        let vol = f32::from_bits(self.volume.load(Ordering::Relaxed));
        self.engine.next_interleaved().map(|s| s * vol)
    }
}

/// rodio 元数据：2 声道 / 44100 Hz / 无固定长度（与重构前一致）。
impl Source for MusicSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        std::num::NonZero::new(2u16).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        std::num::NonZero::new(SAMPLE_RATE as u32).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
```

In the same file change the import line:

```rust
use rodio::{Player, Source};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use room::audio::{SynthFont, SAMPLE_RATE};
```

and change `MusicState::sound_font` type (line ~156) from `Option<Arc<SoundFont>>` to `Option<SynthFont>`, plus the two touchpoints:

```rust
    /// 已加载字体，供后续每次 play 复用（不再直接持有 rustysynth 类型）。
    pub(crate) sound_font: Option<SynthFont>,
```

`load_sound_font` body becomes byte-based (disk read stays native-side):

```rust
    pub(crate) fn load_sound_font(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => match SynthFont::parse(&bytes) {
                Some(f) => {
                    log::info!("Soundfont loaded: {}", path.display());
                    self.sound_font = Some(f);
                }
                None => log::warn!("Soundfont parse error ({})", path.display()),
            },
            Err(e) => log::warn!("Soundfont not found ({}): {e}", path.display()),
        }
    }
```

`MusicState::play` keeps its signature (`&SynthFont` derefs from `Option<SynthFont>` as before). The now-unused `rustysynth` imports (`MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings, BufReader, Cursor`) are deleted from `audio_music.rs`. Keep `BLOCK_SIZE` deleted from the native file too (it moved to lib).

- [ ] **Step 6: Verify the full host suite + native behavior**

Run: `cargo test` (root; timeout 300 s) and `cargo clippy --workspace --exclude c2rust-intermediate` (timeout 300 s)
Expected: exit 0 — 376 unit + 2 demo tests pass (music-path tests included), zero clippy warnings.

Also remove `rustysynth = "1"` from `shells/native/Cargo.toml` — nothing in the native shell uses it anymore (`audio_music.rs` now goes through `room::audio::synth`). This **resolves the spec-② follow-up** ("decide `rustysynth` keep/reword/drop in `room`"): it stays as a `room` dependency because the synth core itself now lives in `room/src/audio/synth.rs`. Verify with `cargo tree -p room-shell-native | grep rustysynth` printing nothing.

- [ ] **Step 7: Commit**

```bash
git add room/src/audio/synth.rs room/src/audio/mod.rs shells/native/src/audio_music.rs shells/native/Cargo.toml Cargo.lock
git commit -m "refactor(audio): extract pull-based SynthEngine core into room lib"
```

---

### Task 4: `web_audio.rs` — `WebAudioBackend` over the control plane

**Files:**
- Create: `shells/web/src/web_audio.rs`
- Modify: `shells/web/src/lib.rs` (add `mod web_audio;`)

**Interfaces:**
- Consumes: `room::audio::{decode_doom_sfx, gains_from, AudioBackend, PanState}` (signatures pinned in the spec-relevant sources: `start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool`, `load_sound_font(&mut self, path: &std::path::Path)`, `play_music(&mut self, midi_bytes: &[u8], looping: bool)`, 8 SFX channels, `sep` 0-255 with 128 centred); `room::audio::{SynthEngine, SynthFont, BLOCK_SIZE}` (Task 3); `wasm_vfs::vfs_get` (Task 2).
- Produces: `web_audio::WebAudioBackend::new() -> Result<WebAudioBackend, String>` (factory-ready) and `web_audio::pump_current_backend()` (shell-reachable music pump, called by `woom24_tick` in Task 7; the handle bookkeeping lives in this module).

Design decisions locked here (spec D4 + engine-reality notes):
- SF2 reaches the backend at **construction**: `find_soundfont_path()` in `room/src/doom/i_sound.rs` relies on `std::path::Path::exists()` which is always `false` on wasm, so `I_InitMusic` will never call `load_sound_font`. The shell therefore preloads the font from the VFS (`vfs_get(PENDING_SF2_NAME)`) in `WebAudioBackend::new()`; the trait's `load_sound_font` is implemented as a logged no-op (engine may still call it — with `None` from `find_soundfont_path` it won't). Engine code stays untouched.
- Music is a **chunked pull** (D4): `woom24_tick` (Task 7) calls `WebAudioBackend::pump_music()`, which renders `BLOCK_SIZE`-frame chunks into `AudioBuffer`s and schedules `AudioBufferSourceNode`s 0.2 s ahead on the `AudioContext` timeline.
- SFX: one `AudioBuffer` (mono) + `GainNode` + `StereoPannerNode` per logical channel; `is_playing` = slot presence, and slots are cleared by `pump_music` when `AudioContext.current_time()` passes the channel's recorded `end_time` (mirrors native `!player.empty()` semantics; `AudioBufferSourceNode` has no pollable `ended` property and an `onended` closure cannot reach `&mut self`).
- `vol` (0..=127) → gain `vol/127`; `sep` (0..=254, 128 centred) → `pan` in `[-1, 1]` via `room::audio::gains_from` for left/right, mapped to a single pan value `(right - left) / (right + left)` (0 when silent).

- [ ] **Step 1: Write the failing test for the pure volume mapping**

Create `shells/web/src/web_audio.rs` with module doc + one pure helper + test:

```rust
//! WebAudioBackend：引擎 AudioBackend 控制面在浏览器上的实现（spec ② D4）。
//!
//! //! 要点：
//! - SF2 在构造时从 VFS 预载（i_sound 的 exists() 在 wasm 上恒 false，
//!   I_InitMusic 不会调 load_sound_font —— 见计划 Task 4 设计说明）。
//! - 音乐走"分块拉取"：woom24_tick 每帧调 pump_music()，提前 0.2s 调度。
//! - SFX 每逻辑声道一对 source+gain+panner；end_time 时刻簿记清槽 = is_playing。

use std::cell::RefCell;

use room::audio::{decode_doom_sfx, AudioBackend, SynthEngine, SynthFont, BLOCK_SIZE};

use crate::wasm_vfs;

/// Doom 音量(0..=127) → 线性增益。
fn vol_gain(vol: i32) -> f32 {
    todo!("Step 3 实现")
}

/// sep(0..=254, 128 居中) → StereoPannerNode.pan ∈ [-1, 1]。
fn sep_to_pan(sep: i32) -> f32 {
    todo!("Step 3 实现")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vol_gain_matches_doom_scale() {
        assert_eq!(vol_gain(0), 0.0);
        assert!((vol_gain(127) - 1.0).abs() < 1e-6);
        assert!((vol_gain(64) - 64.0 / 127.0).abs() < 1e-6);
        assert_eq!(vol_gain(200), 1.0, "超范围钳制");
    }

    #[test]
    fn sep_to_pan_is_centred_at_128() {
        assert!(sep_to_pan(128).abs() < 1e-6);
        assert!(sep_to_pan(254) > 0.0, "全右为正");
        assert!(sep_to_pan(0) < 0.0, "全左为负");
        assert!(sep_to_pan(-5).abs() <= 1.0, "越界钳制后仍在 [-1,1]");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p room-shell-web --lib` (timeout 300 s)
Expected: FAIL — both tests panic at the `todo!()` bodies (compile succeeds; TDD red).

- [ ] **Step 3: Implement the backend**

First replace the two `todo!()` bodies with the real mappings:

```rust
/// Doom 音量(0..=127) → 线性增益。
fn vol_gain(vol: i32) -> f32 {
    vol.clamp(0, 127) as f32 / 127.0
}

/// sep(0..=254, 128 居中) → StereoPannerNode.pan ∈ [-1, 1]。
/// 直接复用引擎的 gains_from（左/右增益），折算成单一 pan 值。
fn sep_to_pan(sep: i32) -> f32 {
    let (l, r) = room::audio::gains_from(127, sep.clamp(0, 254));
    let sum = l + r;
    if sum <= f32::EPSILON {
        0.0
    } else {
        (r - l) / sum
    }
}
```

Add below the helpers (same file):

```rust
thread_local! {
    /// 构造时要预载的 SF2 名（由 init_pipeline 在创建后端之前设置）。
    static PENDING_SF2: RefCell<Option<String>> = const { RefCell::new(None) };
    /// shell 侧保留的后端句柄：引擎把工厂产物存进 lib 私有的 AUDIO 单元，
    /// shell 拿不到 —— 工厂每次构造成功都在这里留一份 Rc 引用，
    /// 专供 woom24_tick 的音乐泵使用（Task 7）。
    static LAST_BUILT: RefCell<Option<std::rc::Rc<RefCell<WebAudioBackend>>>> =
        const { RefCell::new(None) };
}

/// init_pipeline 在 doomgeneric_Create 之前调用：记住 SF2 名。
pub fn set_pending_sf2(name: Option<String>) {
    PENDING_SF2.with_borrow_mut(|s| *s = name);
}

/// 供 woom24_tick 每帧调用的音乐泵（engine 的 AUDIO 单元 shell 摸不到，
/// 所以泵走这里保留的句柄；两个持有者各司其职，互不共享可变状态）。
pub fn pump_current_backend() {
    LAST_BUILT.with_borrow(|slot| {
        if let Some(b) = slot.as_ref() {
            b.borrow_mut().pump_music();
        }
    });
}

/// 单个 SFX 逻辑声道的活动节点组。
/// `end_time` = 声源结束的 AudioContext 绝对时刻（AudioBufferSourceNode
/// 没有 ended 轮询属性；用时间簿记代替 onended 回调，无全局句柄）。
struct SfxChannel {
    source: web_sys::AudioBufferSourceNode,
    gain: web_sys::GainNode,
    panner: web_sys::StereoPannerNode,
    end_time: f64,
}

pub struct WebAudioBackend {
    ctx: web_sys::AudioContext,
    /// 8 个逻辑声道（与引擎 I_UpdateSoundParams 的 0..8 对应）。
    channels: [Option<SfxChannel>; 8],
    music_gain: web_sys::GainNode,
    /// 已预载字体（构造时从 VFS 读取；Clone = Arc 克隆，随取随用）。
    font: Option<SynthFont>,
    /// 当前音乐引擎；None = 无字体/未播放/已结束。
    music: Option<SynthEngine>,
    /// 下一个音乐块应调度的绝对时间（AudioContext 时钟，秒）。
    music_next_time: f64,
    /// 已调度但未播完的源节点（stop_music 时统一 stop）。
    scheduled: Vec<web_sys::AudioBufferSourceNode>,
    /// 音乐是否被显式暂停（pause/resume 用 ctx 级 suspend/resume）。
    paused: bool,
}

impl WebAudioBackend {
    /// 工厂入口：Web Audio 不可用时返回 Err（room 记日志并走 NoopBackend，
    /// 即 spec 的"降级静音"契约）。构造成功即在 LAST_BUILT 留泵句柄。
    pub fn new() -> Result<Self, String> {
        let ctx = web_sys::AudioContext::new().map_err(|e| format!("AudioContext: {e:?}"))?;
        let music_gain = ctx
            .create_gain()
            .map_err(|e| format!("create_gain: {e:?}"))?;
        music_gain
            .connect_with_audio_node(&ctx.destination())
            .map_err(|e| format!("connect: {e:?}"))?;
        let mut backend = Self {
            ctx,
            channels: std::array::from_fn(|_| None),
            music_gain,
            font: None,
            music: None,
            music_next_time: 0.0,
            scheduled: Vec::new(),
            paused: false,
        };
        backend.preload_sound_font();
        let shared = std::rc::Rc::new(RefCell::new(backend));
        LAST_BUILT.with_borrow_mut(|slot| *slot = Some(shared.clone()));
        // Rc::try_unwrap 拿回唯一所有权：LAST_BUILT 只留一个 Rc 引用。
        let backend = std::rc::Rc::try_unwrap(shared).map_err(|_| "backend aliasing")?;
        Ok(backend)
    }

    /// 从 VFS 预载 SF2（名字由 init_pipeline 经 set_pending_sf2 提供）。
    /// 失败只降级为"音乐静音"，绝不 panic（AGENTS 约束 2）。
    fn preload_sound_font(&mut self) {
        let Some(name) = PENDING_SF2.with_borrow(|s| s.clone()) else {
            return;
        };
        let Some(bytes) = wasm_vfs::vfs_get(&name) else {
            log::warn!("SF2 '{name}' 未注册，音乐将静音");
            return;
        };
        match SynthFont::parse(&bytes) {
            Some(f) => self.font = Some(f),
            None => log::warn!("SF2 '{name}' 解析失败，音乐将静音"),
        }
    }

    /// pump：SFX 槽清理 + 音乐块调度。由 woom24_tick 每帧调用（D4 pull 模型）。
    pub fn pump_music(&mut self) {
        if self.paused {
            return;
        }
        let now = self.ctx.current_time();
        // SFX 槽清理：声源到达 end_time 即视为空闲（对齐原生 !player.empty()）。
        for slot in self.channels.iter_mut() {
            if let Some(ch) = slot {
                if now >= ch.end_time {
                    *slot = None;
                }
            }
        }
        // 音乐调度：时间线上保持 0.2s 领先。
        while self.music.is_some() && self.music_next_time < now + 0.2 {
            let Some(buf) = self.render_block_buffer() else {
                break; // 序列结束且已排空
            };
            let Ok(src) = self.ctx.create_buffer_source() else {
                break;
            };
            src.set_buffer(Some(&buf));
            let _ = src.connect_with_audio_node(&self.music_gain);
            let _ = src.start_with_when(self.music_next_time.max(now));
            self.scheduled.push(src);
            self.music_next_time += BLOCK_SIZE as f64 / 44100.0;
        }
        if self.music.is_none() && !self.scheduled.is_empty() {
            // 序列结束后仅当尾块播完才真正归位。
            self.scheduled.clear();
        }
    }

    /// 渲染一个 BLOCK_SIZE 帧块（交错的 L/R）进新 AudioBuffer。
    /// 序列已排空（第一个采样都取不到）时返回 None；提前结束时尾部补零。
    fn render_block_buffer(&mut self) -> Option<web_sys::AudioBuffer> {
        let engine = self.music.as_mut()?;
        let first = engine.next_interleaved()?;
        let buf = self
            .ctx
            .create_buffer(2, BLOCK_SIZE as u32, 44100.0)
            .ok()?;
        let l = buf.get_channel_data(0).ok()?;
        let r = buf.get_channel_data(1).ok()?;
        // 消费序：L0, R0, L1, R1, …, R(BLOCK-1)，恰好 2*BLOCK 个采样。
        l[0] = first;
        for i in 0..BLOCK_SIZE {
            let Some(s) = engine.next_interleaved() else { break };
            r[i] = s;
            if i + 1 < BLOCK_SIZE {
                let Some(s2) = engine.next_interleaved() else { break };
                l[i + 1] = s2;
            }
        }
        Some(buf)
    }
}

Add `font: Option<SynthFont>` to the struct and constructor (`font: None` initially, set in `preload_sound_font`). Then the trait impl:

```rust
impl AudioBackend for WebAudioBackend {
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        if channel >= 8 {
            return false;
        }
        let Some((sample_rate, samples)) = decode_doom_sfx(data) else {
            return false;
        };
        let (Ok(buf), Ok(gain), Ok(panner)) = (
            self.ctx.create_buffer(1, samples.len() as u32, sample_rate as f32),
            self.ctx.create_gain(),
            self.ctx.create_stereo_panner(),
        ) else {
            return false;
        };
        let Ok(ch) = buf.get_channel_data(0) else {
            return false;
        };
        ch.copy_from_slice(&samples);
        gain.gain().set_value(vol_gain(vol));
        panner.pan().set_value(sep_to_pan(sep));
        let Ok(src) = self.ctx.create_buffer_source() else {
            return false;
        };
        src.set_buffer(Some(&buf));
        let _ = src.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&panner);
        let _ = panner.connect_with_audio_node(&self.ctx.destination());
        let _ = src.start();
        // 结束时刻簿记：pump_music 里到点清槽（替代 onended 回调，
        // 因为 AudioBufferSourceNode 没有 ended 轮询属性，且回调拿不到 &mut self）。
        let end_time = self.ctx.current_time() + samples.len() as f64 / sample_rate as f64;
        self.channels[channel] = Some(SfxChannel { source: src, gain, panner, end_time });
        true
    }

    fn stop_sound(&mut self, channel: usize) {
        if channel >= 8 {
            return;
        }
        if let Some(ch) = self.channels[channel].take() {
            let _ = ch.source.stop();
        }
    }

    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        if channel >= 8 {
            return;
        }
        if let Some(ch) = &self.channels[channel] {
            ch.gain.gain().set_value(vol_gain(vol));
            ch.panner.pan().set_value(sep_to_pan(sep));
        }
    }

    /// 槽非空 = 声源未到 end_time（pump_music 每帧清槽）。
    /// 语义与原生 `!player.empty()` 对齐；8 之外的声道恒 false。
    fn is_playing(&self, channel: usize) -> bool {
        channel < 8 && self.channels[channel].is_some()
    }

    /// 引擎侧不会在 wasm 上调用（find_soundfont_path 依赖的 Path::exists()
    /// 恒 false）；保留 trait 完整性，字体在构造时已从 VFS 预载。
    fn load_sound_font(&mut self, _path: &std::path::Path) {
        log::info!("load_sound_font 在 wasm 上为 no-op：字体已在构造时从 VFS 预载");
    }

    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) {
        let Some(font) = self.font.clone() else {
            log::warn!("无已预载 SF2，play_music 忽略");
            return;
        };
        match SynthEngine::new(&font, midi_bytes, looping) {
            Some(engine) => {
                for s in self.scheduled.drain(..) {
                    let _ = s.stop();
                }
                self.music = Some(engine);
                self.music_next_time = self.ctx.current_time() + 0.05;
            }
            None => log::warn!("I_PlaySong: SynthEngine 构建失败"),
        }
    }

    fn stop_music(&mut self) {
        for s in self.scheduled.drain(..) {
            let _ = s.stop();
        }
        self.music = None;
    }

    fn set_music_volume(&self, vol: i32) {
        self.music_gain.gain().set_value(vol_gain(vol));
    }

    fn pause_music(&self) {
        self.paused = true;
        let _ = self.ctx.suspend();
    }

    fn resume_music(&self) {
        self.paused = false;
        let _ = self.ctx.resume();
    }

    fn is_music_playing(&self) -> bool {
        self.music.is_some() || !self.scheduled.is_empty()
    }
}
```

Finish the task by adding to `shells/web/src/lib.rs`:

```rust
mod web_audio;
```

- [ ] **Step 4: Verify host tests + wasm build**

```bash
cargo test -p room-shell-web --lib
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
```

(timeout 300 s each) Expected: host tests PASS (2 tests); wasm build exit 0. If web-sys reports a missing feature (e.g. `StereoPannerNode`, `AudioBuffer`, `AudioContext`, `AudioBufferSourceNode`, `GainNode`), add those exact feature names to `web-sys.features` in `shells/web/Cargo.toml` — expected additions: `"AudioContext"`, `"AudioBuffer"`, `"AudioBufferSourceNode"`, `"GainNode"`, `"StereoPannerNode"`, `"AudioParam"`, `"AudioNode"` (add all seven now to avoid a second round-trip).

- [ ] **Step 5: Commit**

```bash
git add shells/web/src/web_audio.rs shells/web/src/lib.rs shells/web/Cargo.toml Cargo.lock
git commit -m "feat(web): WebAudioBackend over the room AudioBackend control plane"
```

---

### Task 5: `clock.rs` + `dg.rs` + `present_c2d.rs` (first visible frame)

**Files:**
- Create: `shells/web/src/clock.rs`
- Create: `shells/web/src/dg.rs`
- Create: `shells/web/src/present_c2d.rs`
- Modify: `shells/web/src/lib.rs` (module list + `woom24_attach_canvas`, `woom24_push_key`, `woom24_tick`)

**Interfaces:**
- Consumes: `room::doom::doomgeneric::{DG_ScreenBuffer, DOOMGENERIC_PIXELS, DOOMGENERIC_RESX, DOOMGENERIC_RESY}` (`pub static mut DG_ScreenBuffer: *mut u32`; `DOOMGENERIC_RESX=640`, `DOOMGENERIC_RESY=400`, `DOOMGENERIC_PIXELS=256000`), `room::doom::d_main::doomgeneric_Tick()` (`pub extern "C" fn`, line 897).
- Produces:
  - The six `#[no_mangle] extern "C"` callbacks with the exact native shapes: `DG_Init()`, `DG_DrawFrame()`, `DG_SleepMs(ms: u32)` (no-op), `DG_GetTicksMs() -> u32`, `DG_GetKey(pressed: *mut i32, doom_key: *mut u8) -> i32` (1 = event, 0 = empty), `DG_SetWindowTitle(title: *const c_char)`.
  - `present_c2d::bgra_to_rgba(src: &[u8], dst: &mut [u8])` (pure, host-tested) and `present_c2d::Canvas2dPresenter::new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String>` + `draw_frame(&mut self, bgra: &[u8]) -> Result<(), String>` (the `Presenter` shape Task 6 also implements).
  - `clock::init_start_time()`, `clock::now_ms() -> u32` (`performance.now()` minus start).
  - Exports: `woom24_attach_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue>`, `woom24_push_key(pressed: bool, doom_key: u8)`, `woom24_tick()`.
  - Note: there is deliberately **no Rust key-mapping module** — D2's contract has JS push the already-mapped Doom key byte (`woom24_push_key(pressed, doom_key)`), mirroring doomgeneric's Emscripten backend; the DOM-code→Doom-key table lives in `www/woom24.js` (platform mapping, not engine logic — the native shell's winit table is likewise platform-local).

- [ ] **Step 1: Write the failing test for the pixel conversion**

Create `shells/web/src/present_c2d.rs` with the converter stub + tests:

```rust
//! Canvas2D 呈现器（D3 基线）：ImageData + putImageData。

/// 引擎帧缓冲是 BGRA 字节序（native gpu.rs 同源）；
/// Canvas ImageData 是 RGBA 字节序。逐像素换位。
/// Step 3 填入真实实现（下方测试即其规格）。
pub fn bgra_to_rgba(src: &[u8], dst: &mut [u8]) {
    let _ = (src, dst);
    todo!("Step 3 实现")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_blue_and_red_keeps_green_alpha() {
        let src = [1u8, 2, 3, 4];
        let mut dst = [0u8; 4];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [3, 2, 1, 4]);
    }

    #[test]
    fn full_frame_conversion_is_reversible_in_shape() {
        let src = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let mut dst = vec![0u8; 8];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [30, 20, 10, 255, 60, 50, 40, 255]);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p room-shell-web --lib` (timeout 300 s)
Expected: FAIL — both new tests panic at the `todo!()` body (TDD red).

- [ ] **Step 3: Implement clock, presenter, and the DG surface**

First replace the `todo!()` body with the real converter:

`shells/web/src/present_c2d.rs`:

```rust
pub fn bgra_to_rgba(src: &[u8], dst: &mut [u8]) {
    assert_eq!(src.len(), dst.len(), "缓冲区必须等长");
    for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        d[0] = s[2]; // R
        d[1] = s[1]; // G
        d[2] = s[0]; // B
        d[3] = s[3]; // A
    }
}
```

Complete `shells/web/src/present_c2d.rs` (add below the converter):

```rust
use wasm_bindgen::Clamped;

/// Canvas2D 呈现器：常驻复用的 RGBA 缓冲，每帧包一次 ImageData。
pub struct Canvas2dPresenter {
    ctx: web_sys::CanvasRenderingContext2d,
    rgba: Vec<u8>,
}

impl Canvas2dPresenter {
    /// 画布被设为引擎分辨率（640×400）；失败返回 Err 由上层降级。
    pub fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String> {
        canvas.set_width(room::doom::doomgeneric::DOOMGENERIC_RESX as u32);
        canvas.set_height(room::doom::doomgeneric::DOOMGENERIC_RESY as u32);
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("2d context: {e:?}"))?
            .ok_or("2d context unavailable")?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| "2d context cast failed")?;
        let len = room::doom::doomgeneric::DOOMGENERIC_PIXELS * 4;
        Ok(Self {
            ctx,
            rgba: vec![0u8; len],
        })
    }
}

/// Presenter 统一形状（Task 6 的 WebGL2 实现同名同签名）。
pub trait Presenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String>;
}

impl Presenter for Canvas2dPresenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String> {
        bgra_to_rgba(bgra, &mut self.rgba);
        // ImageData 按 (宽, 高) 像素计；字节缓冲用 Clamped 包装成视图。
        let img = web_sys::ImageData::new_with_u8_clamped_array_and_width_and_height(
            Clamped(&self.rgba),
            room::doom::doomgeneric::DOOMGENERIC_RESX as u32,
            room::doom::doomgeneric::DOOMGENERIC_RESY as u32,
        )
        .map_err(|e| format!("ImageData: {e:?}"))?;
        self.ctx
            .put_image_data(&img, 0.0, 0.0)
            .map_err(|e| format!("putImageData: {e:?}"))
    }
}
```

Create `shells/web/src/clock.rs`:

```rust
//! performance.now 时钟（D2）：shell 起点之后的毫秒数。
//!
//! //! 只进 DG_GetTicksMs（引擎节拍），绝不进模拟状态（D6 确定性护栏）。

use std::cell::Cell;

thread_local! {
    static START_MS: Cell<f64> = const { Cell::new(0.0) };
}

/// 在 doomgeneric_Create 之前调用一次。
pub fn init_start_time() {
    START_MS.with(|t| t.set(performance_now()));
}

/// 引擎节拍时钟：起点后毫秒数（u32 截断 = 原生 Instant 行为一致）。
pub fn now_ms() -> u32 {
    START_MS.with(|t| (performance_now() - t.get()) as u32)
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}
```

Create `shells/web/src/dg.rs` (mirrors `shells/native/src/platform.rs` line-for-line where possible):

```rust
//! 六个 DG_* 回调的 wasm 实现（D2）——与 shells/native/src/platform.rs 同形。
//!
//! | 回调 | wasm 行为 |
//! |---|---|
//! | DG_Init | no-op（画布/呈现器由 woom24_attach_canvas 先行装好） |
//! | DG_DrawFrame | 从 DG_ScreenBuffer 读 BGRA 帧 → Presenter |
//! | DG_SleepMs | no-op（rAF 节奏主导；引擎节流走自身 tick 时钟） |
//! | DG_GetTicksMs | clock::now_ms() |
//! | DG_GetKey | 弹出 JS 经 woom24_push_key 压入的队列 |
//! | DG_SetWindowTitle | 写 document.title |

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::CStr;

use crate::clock;
use crate::present_c2d::{Canvas2dPresenter, Presenter};

thread_local! {
    /// 装好的呈现器（Task 5 用 Canvas2D；Task 6 升级为运行时选择，
    /// 静态类型不变，始终是 trait 对象）。
    static PRESENTER: RefCell<Option<Box<dyn Presenter>>> = const { RefCell::new(None) };
    /// JS 压入的按键队列，条目 = (pressed, doom_key)。
    static KEY_QUEUE: RefCell<VecDeque<(bool, u8)>> = const { RefCell::new(VecDeque::new()) };
}

/// woom24_attach_canvas 的落点。
pub fn attach_canvas(canvas: &web_sys::HtmlCanvasElement) -> Result<(), String> {
    let p = Canvas2dPresenter::new(canvas)?;
    PRESENTER.with_borrow_mut(|s| *s = Some(Box::new(p) as Box<dyn Presenter>));
    clock::init_start_time();
    Ok(())
}

/// woom24_push_key 的落点。
pub fn push_key(pressed: bool, doom_key: u8) {
    KEY_QUEUE.with_borrow_mut(|q| q.push_back((pressed, doom_key)));
}

#[no_mangle]
pub extern "C" fn DG_Init() {
    log::debug!("DG_Init: 平台已由 attach_canvas 备好");
}

/// # Safety
/// DG_ScreenBuffer 由 doomgeneric_Create 分配，容量 = DOOMGENERIC_PIXELS * 4。
#[no_mangle]
pub unsafe extern "C" fn DG_DrawFrame() {
    let pixel_bytes = {
        let ptr = room::doom::doomgeneric::DG_ScreenBuffer as *const u8;
        if ptr.is_null() {
            log::warn!("DG_DrawFrame: DG_ScreenBuffer 为空，跳过本帧");
            return;
        }
        std::slice::from_raw_parts(ptr, room::doom::doomgeneric::DOOMGENERIC_PIXELS * 4)
    };
    PRESENTER.with_borrow_mut(|s| {
        match s.as_mut() {
            Some(p) => {
                if let Err(e) = p.draw_frame(pixel_bytes) {
                    log::warn!("DG_DrawFrame: {e}");
                }
            }
            None => log::warn!("DG_DrawFrame: 呈现器未就绪"),
        }
    });
}

#[no_mangle]
pub extern "C" fn DG_SleepMs(_ms: u32) {
    // no-op：rAF 驱动节奏（D2）。
}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    clock::now_ms()
}

/// # Safety
/// pressed/doom_key 必须是有效可写指针（doomgeneric 的调用约定保证）。
#[no_mangle]
pub unsafe extern "C" fn DG_GetKey(pressed: *mut i32, doom_key: *mut u8) -> i32 {
    KEY_QUEUE.with_borrow_mut(|q| {
        if let Some((is_pressed, key)) = q.pop_front() {
            // SAFETY: 调用方保证指针有效。
            unsafe {
                *pressed = i32::from(is_pressed);
                *doom_key = key;
            }
            1
        } else {
            0
        }
    })
}

/// # Safety
/// title 必须是 NUL 结尾 C 字符串。
#[no_mangle]
pub unsafe extern "C" fn DG_SetWindowTitle(title: *const std::ffi::c_char) {
    if title.is_null() {
        return;
    }
    // SAFETY: 调用方保证 NUL 结尾。
    let s = unsafe { CStr::from_ptr(title) }.to_string_lossy().into_owned();
    if let Some(doc) = web_sys::window().map(|w| w.document()).flatten() {
        doc.set_title(&s);
    }
}
```

Add to `shells/web/src/lib.rs` the module list and the three JS-facing exports (this is the first cut of `woom24_tick`; Task 7 extends it with the music pump):

```rust
mod clock;
mod dg;
pub mod present_c2d;

/// 由 loader JS 调用：交入目标画布（engine 分辨率 640×400）。
#[wasm_bindgen]
pub fn woom24_attach_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
    dg::attach_canvas(&canvas).map_err(JsValue::from_str)
}

/// JS 键盘事件 → 队列（DG_GetKey 在 tick 中弹出）。
#[wasm_bindgen]
pub fn woom24_push_key(pressed: bool, doom_key: u8) {
    dg::push_key(pressed, doom_key);
}

/// rAF 每帧调用一次：推进引擎一 tic 并呈现（D2 数据流）。
#[wasm_bindgen]
pub fn woom24_tick() {
    room::doom::d_main::doomgeneric_Tick();
}
```

- [ ] **Step 4: Verify tests + wasm build**

```bash
cargo test -p room-shell-web --lib
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
```

(timeout 300 s each) Expected: all lib tests PASS (present_c2d: 2, web_audio: 2 from Task 4, wasm_vfs: 11 from Task 2); wasm build exit 0 (the `DG_*` symbols must also be exported — verify with `wasm-bindgen` glue generation from Task 1 Step 5 still succeeding; `DG_*` symbols appear in the wasm export table, checked via Task 8's loader).

- [ ] **Step 5: Commit**

```bash
git add shells/web/src/clock.rs shells/web/src/dg.rs shells/web/src/present_c2d.rs shells/web/src/lib.rs
git commit -m "feat(web): DG_* callbacks, clock, and Canvas2D presenter"
```

---

### Task 6: `present_gl2.rs` — WebGL2 presenter + runtime selection

**Files:**
- Create: `shells/web/src/present_gl2.rs`
- Modify: `shells/web/src/present_c2d.rs` (add the selection vocabulary next to the `Presenter` trait)
- Modify: `shells/web/src/dg.rs` (attach chooses WebGL2, falls back to Canvas2D — D3)

**Interfaces:**
- Consumes: `present_c2d::Presenter` (`fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String>`), `present_c2d::bgra_to_rgba`, `present_c2d::Canvas2dPresenter`.
- Produces:
  - `pub fn choose_presenter(has_webgl2: bool) -> PresenterKind` where `pub enum PresenterKind { Canvas2D, WebGL2 }` (pure, host-tested).
  - `present_gl2::WebGl2Presenter::new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String>` + `impl Presenter` (one texture upload + blit per frame).
  - `dg::attach_canvas` now returns `Box<dyn Presenter>`-style runtime selection: Canvas2D always compiled in as fallback (D3).

- [ ] **Step 1: Write the failing selection test**

The `Presenter` trait already lives in `present_c2d.rs` (Task 5); extend that module with the selection vocabulary:

```rust
/// 呈现后端种类（D3：WebGL2 默认，Canvas2D 恒为兜底）。
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PresenterKind {
    Canvas2D,
    WebGL2,
}

/// 运行时选择：有 WebGL2 用 WebGL2，否则 Canvas2D。
/// WebGPU 不在本 spec（非目标）。
pub fn choose_presenter(has_webgl2: bool) -> PresenterKind {
    if has_webgl2 {
        PresenterKind::WebGL2
    } else {
        PresenterKind::Canvas2D
    }
}

#[cfg(test)]
mod presenter_kind_tests {
    use super::*;

    #[test]
    fn webgl2_preferred_when_available() {
        assert_eq!(choose_presenter(true), PresenterKind::WebGL2);
        assert_eq!(choose_presenter(false), PresenterKind::Canvas2D);
    }
}
```

Then swap `dg.rs`'s import line to the new vocabulary (this is what turns the build red — `present_gl2` does not exist yet):

```rust
use crate::present_c2d::{choose_presenter, Canvas2dPresenter, Presenter, PresenterKind};
use crate::present_gl2::WebGl2Presenter;
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p room-shell-web --lib` (timeout 300 s)
Expected: FAIL — compile error: `present_gl2` module does not exist yet (referenced by `dg.rs`).

- [ ] **Step 3: Implement the WebGL2 presenter**

Create `shells/web/src/present_gl2.rs`:

```rust
//! WebGL2 呈现器（D3 默认路径）：整帧一张纹理 + 全屏三角 blit。
//!
//! //! 引擎帧是 BGRA 字节序、WebGL2 核心不吃 BGRA——
//! //! 复用 present_c2d::bgra_to_rgba，单一换序代码路径。

use wasm_bindgen::JsCast;

use crate::present_c2d::{bgra_to_rgba, Presenter};

const VS_SRC: &str = r#"
#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = vec2((a_pos.x + 1.0) * 0.5, 1.0 - (a_pos.y + 1.0) * 0.5);
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

const FS_SRC: &str = r#"
#version 300 es
precision mediump float;
in vec2 v_uv;
uniform sampler2D u_frame;
out vec4 out_color;
void main() {
    out_color = texture(u_frame, v_uv);
}
"#;

pub struct WebGl2Presenter {
    gl: web_sys::WebGl2RenderingContext,
    texture: web_sys::WebGlTexture,
    rgba: Vec<u8>,
}

impl WebGl2Presenter {
    pub fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String> {
        canvas.set_width(room::doom::doomgeneric::DOOMGENERIC_RESX as u32);
        canvas.set_height(room::doom::doomgeneric::DOOMGENERIC_RESY as u32);
        let gl = canvas
            .get_context("webgl2")
            .map_err(|e| format!("webgl2: {e:?}"))?
            .ok_or("webgl2 unavailable")?
            .dyn_into::<web_sys::WebGl2RenderingContext>()
            .map_err(|_| "webgl2 cast failed")?;
        let program = compile_program(&gl)?;
        gl.use_program(Some(&program));
        // 全屏三角：一条大三角形覆盖 clip 空间。
        let verts: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
        let vbo = gl
            .create_buffer()
            .ok_or("create_buffer failed")?;
        gl.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
        unsafe {
            // SAFETY: 静态数组，长度精确。
            let slice = js_sys::Float32Array::view(&verts);
            gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &slice,
                web_sys::WebGl2RenderingContext::STATIC_DRAW,
            );
        }
        let loc = gl
            .get_attrib_location(&program, "a_pos")
            .ok_or("a_pos location")? as u32;
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, 2, web_sys::WebGl2RenderingContext::FLOAT, false, 0, 0);
        let texture = gl.create_texture().ok_or("create_texture failed")?;
        gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        // 像素对齐 + 最近邻（整数放大交给 CSS，保持像素风）。
        gl.tex_parameteri(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            web_sys::WebGl2RenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            web_sys::WebGl2RenderingContext::NEAREST as i32,
        );
        let tex_loc = gl.get_uniform_location(&program, "u_frame").ok_or("u_frame loc")?;
        gl.uniform1i(Some(&tex_loc), 0);
        let len = room::doom::doomgeneric::DOOMGENERIC_PIXELS * 4;
        Ok(Self {
            gl,
            texture,
            rgba: vec![0u8; len],
        })
    }
}

fn compile_shader(
    gl: &web_sys::WebGl2RenderingContext,
    kind: u32,
    src: &str,
) -> Result<web_sys::WebGlShader, String> {
    let sh = gl.create_shader(kind).ok_or("create_shader failed")?;
    gl.shader_source(&sh, src);
    gl.compile_shader(&sh);
    let ok = gl
        .get_shader_parameter(&sh, web_sys::WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false);
    if ok {
        Ok(sh)
    } else {
        Err(format!(
            "shader compile: {}",
            gl.get_shader_info_log(&sh)
        ))
    }
}

fn compile_program(gl: &web_sys::WebGl2RenderingContext) -> Result<web_sys::WebGlProgram, String> {
    let vs = compile_shader(gl, web_sys::WebGl2RenderingContext::VERTEX_SHADER, VS_SRC)?;
    let fs = compile_shader(gl, web_sys::WebGl2RenderingContext::FRAGMENT_SHADER, FS_SRC)?;
    let p = gl.create_program().ok_or("create_program failed")?;
    gl.attach_shader(&p, &vs);
    gl.attach_shader(&p, &fs);
    gl.link_program(&p);
    let ok = gl
        .get_program_parameter(&p, web_sys::WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false);
    if ok {
        Ok(p)
    } else {
        Err(format!("program link: {}", gl.get_program_info_log(&p)))
    }
}

impl Presenter for WebGl2Presenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String> {
        bgra_to_rgba(bgra, &mut self.rgba);
        let gl = &self.gl;
        gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&self.texture));
        unsafe {
            // SAFETY: rgba 长度 = RESX*RESY*4，与纹理尺寸一致。
            let view = js_sys::Uint8Array::view(&self.rgba);
            gl.tex_image_2d_with_u32_and_u32_and_html_image_element_or_canvas_or_video(
                web_sys::WebGl2RenderingContext::TEXTURE_2D,
                0,
                web_sys::WebGl2RenderingContext::RGBA as i32,
                room::doom::doomgeneric::DOOMGENERIC_RESX as i32,
                room::doom::doomgeneric::DOOMGENERIC_RESY as i32,
                0,
                web_sys::WebGl2RenderingContext::RGBA,
                web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(&view),
            )
            .map_err(|e| format!("texImage2D: {e:?}"))?;
        }
        gl.draw_arrays(
            web_sys::WebGl2RenderingContext::TRIANGLES,
            0,
            3,
        );
        Ok(())
    }
}
```

If `tex_image_2d_with_u32_and_u32_and_html_image_element_or_canvas_or_video` is not generated by the enabled web-sys features, use the plain `tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_u8_array_and_opt_u32` variant instead (same arguments minus the element slot) — whichever the pinned web-sys version generates; verify by compiling. Add to `shells/web/src/lib.rs`: `pub mod present_gl2;`. Then finish `dg.rs` — its import line was already swapped in Step 1; replace only `attach_canvas`'s body (the `PRESENTER` static type from Task 5 — `RefCell<Option<Box<dyn Presenter>>>` — stays unchanged):

```rust
pub fn attach_canvas(canvas: &web_sys::HtmlCanvasElement) -> Result<(), String> {
    // D3：WebGL2 默认，失败落回 Canvas2D（两者恒编译在内）。
    let kind = choose_presenter(
        canvas.get_context("webgl2").map(|c| c.is_some()).unwrap_or(false),
    );
    let presenter: Box<dyn Presenter> = match kind {
        PresenterKind::WebGL2 => WebGl2Presenter::new(canvas)
            .map(|p| Box::new(p) as Box<dyn Presenter>)
            .or_else(|e| {
                log::warn!("WebGL2 初始化失败，回退 Canvas2D: {e}");
                Canvas2dPresenter::new(canvas).map(|p| Box::new(p) as Box<dyn Presenter>)
            })?,
        PresenterKind::Canvas2D => {
            Box::new(Canvas2dPresenter::new(canvas)?)
        }
    };
    PRESENTER.with_borrow_mut(|s| *s = Some(presenter));
    clock::init_start_time();
    Ok(())
}
```

`DG_DrawFrame` keeps its Task 5 body unchanged (it calls `draw_frame` through the trait object).

- [ ] **Step 4: Verify tests + wasm build**

```bash
cargo test -p room-shell-web --lib
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
```

(timeout 300 s each) Expected: PASS (adds 1 selection test; total shell lib tests: 16 = 11 wasm_vfs + 2 web_audio + 2 present_c2d + 1 selection), wasm build exit 0. Add `"js_sys"` usage note: `js-sys` is now genuinely used (`Float32Array::view` / `Uint8Array::view`) — it is already a declared dependency from Task 1.

- [ ] **Step 5: Commit**

```bash
git add shells/web/src/present_gl2.rs shells/web/src/present_c2d.rs shells/web/src/dg.rs shells/web/src/lib.rs
git commit -m "feat(web): WebGL2 presenter with runtime backend selection"
```

---

### Task 7: Entry contract — `profile.rs`, `init_pipeline.rs`, `launcher_ui.rs`, exports

**Files:**
- Create: `shells/web/src/profile.rs`
- Create: `shells/web/src/init_pipeline.rs`
- Create: `shells/web/src/launcher_ui.rs`
- Modify: `shells/web/src/lib.rs` (final export surface)

**Interfaces:**
- Consumes: `wasm_vfs::{vfs_register, vfs_get}` (Task 2), `web_audio::{set_pending_sf2, WebAudioBackend}` (Task 4), `dg::attach_canvas` (Tasks 5/6), `room::audio::set_backend_factory` (`pub fn set_backend_factory(factory: BackendFactory)` with `pub type BackendFactory = fn() -> Result<Box<dyn AudioBackend>, String>`), `room::doom::doomgeneric::doomgeneric_Create(argc: c_int, argv: *mut *mut c_char)` (`pub unsafe extern "C"`), `launcher_ui` DOM APIs.
- Produces (the AGENTS.md contract surface, final):
  - `#[wasm_bindgen] pub fn woom24_minimal_start(max_render_res: u32, iwad_name: &str, iwad: &[u8]) -> Result<(), JsValue>` — registers the IWAD, caps the framebuffer, enters launcher mode (DOM config UI; game starts only on user confirmation).
  - `#[wasm_bindgen] pub fn woom24_standard_start(profile_json: &str) -> Result<(), JsValue>` — complete boot profile → direct start.
  - `#[wasm_bindgen] pub fn woom24_register_file(name: &str, bytes: &[u8])` — host-side blob registration (PWADs/SF2 for the standard entry).
  - `profile::BootProfile { iwad: String, pwads: Vec<String>, sf2: Option<String>, max_render_res: Option<u32>, engine_args: Vec<String> }` and `profile::parse_profile(json: &str) -> Result<BootProfile, String>` (strict schema, no weakly-typed maps per AGENTS.md).
  - `init_pipeline::run(profile: &BootProfile) -> Result<(), String>` — the single shared pipeline.

- [ ] **Step 1: Write failing golden tests for the profile parser**

Create `shells/web/src/profile.rs` with the struct and test module:

```rust
//! 启动档案：标准入口的 JSON 契约（D5）。
//!
//! //! 严格解析：固定 schema、未知键报错、类型不匹配报错。
//! //! 不用 serde_json（依赖冻结 + AGENTS 反对弱类型 DTO）——
//! //! 这个手写解析器只服务本 schema，golden 全覆盖。

/// 完整启动档案。`pwads` 顺序即加载顺序（契约的一部分）。
#[derive(Debug, PartialEq, Clone)]
pub struct BootProfile {
    pub iwad: String,
    pub pwads: Vec<String>,
    pub sf2: Option<String>,
    pub max_render_res: Option<u32>,
    pub engine_args: Vec<String>,
}

pub fn parse_profile(json: &str) -> Result<BootProfile, String> {
    todo!("Step 3 实现")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_profile() {
        let p = parse_profile(
            r#"{"iwad":"doom1.wad","pwads":["a.wad","b.wad"],"sf2":"sc55.sf2","maxRenderRes":1080,"engineArgs":["-nomusic","-turbo 2"]}"#,
        )
        .unwrap();
        assert_eq!(p.iwad, "doom1.wad");
        assert_eq!(p.pwads, vec!["a.wad".to_string(), "b.wad".to_string()]);
        assert_eq!(p.sf2.as_deref(), Some("sc55.sf2"));
        assert_eq!(p.max_render_res, Some(1080));
        assert_eq!(p.engine_args, vec!["-nomusic".to_string(), "-turbo 2".to_string()]);
    }

    #[test]
    fn minimal_profile_defaults() {
        let p = parse_profile(r#"{"iwad":"doom.wad"}"#).unwrap();
        assert_eq!(p.iwad, "doom.wad");
        assert!(p.pwads.is_empty());
        assert_eq!(p.sf2, None);
        assert_eq!(p.max_render_res, None);
        assert!(p.engine_args.is_empty());
    }

    #[test]
    fn explicit_nulls_are_none() {
        let p = parse_profile(r#"{"iwad":"d.wad","sf2":null,"maxRenderRes":null}"#).unwrap();
        assert_eq!(p.sf2, None);
        assert_eq!(p.max_render_res, None);
    }

    #[test]
    fn unknown_key_rejected() {
        assert!(parse_profile(r#"{"iwad":"d.wad","cheat":1}"#).is_err());
    }

    #[test]
    fn missing_iwad_rejected() {
        assert!(parse_profile(r#"{"pwads":[]}"#).is_err());
    }

    #[test]
    fn wrong_value_type_rejected() {
        assert!(parse_profile(r#"{"iwad":42}"#).is_err());
        assert!(parse_profile(r#"{"pwads":"a.wad"}"#).is_err());
        assert!(parse_profile(r#"{"maxRenderRes":"big"}"#).is_err());
    }

    #[test]
    fn trailing_garbage_rejected() {
        assert!(parse_profile(r#"{"iwad":"d.wad"} oops"#).is_err());
        assert!(parse_profile("").is_err());
    }

    #[test]
    fn duplicate_key_rejected() {
        assert!(parse_profile(r#"{"iwad":"a.wad","iwad":"b.wad"}"#).is_err());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p room-shell-web --lib profile` (timeout 300 s)
Expected: FAIL — all `parse_profile` calls panic on `todo!()`.

- [ ] **Step 3: Implement the parser**

Replace `todo!` with a strict recursive-descent reader (complete code):

```rust
/// 严格 JSON 读取器（只支持本 schema 用到的字面量形态）。
struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn new(s: &'a str) -> Self {
        Self { b: s.as_bytes(), i: 0 }
    }

    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Result<u8, String> {
        self.ws();
        self.b.get(self.i).copied().ok_or_else(|| "unexpected end".to_string())
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        if self.peek()? == c {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}'", c as char))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let c = self.b.get(self.i).copied().ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.b.get(self.i).copied().ok_or("bad escape")?;
                    self.i += 1;
                    out.push(match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b't' => '\t',
                        _ => return Err("unsupported escape".to_string()),
                    });
                }
                _ => out.push(c as char),
            }
        }
    }

    fn literal(&mut self, want: &str) -> Result<(), String> {
        if self.b[self.i..].starts_with(want.as_bytes()) {
            self.i += want.len();
            Ok(())
        } else {
            Err(format!("expected {want}"))
        }
    }

    fn u32_or_null(&mut self) -> Result<Option<u32>, String> {
        self.ws();
        if self.b[self.i..].starts_with(b"null") {
            self.i += 4;
            return Ok(None);
        }
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if start == self.i {
            return Err("expected number or null".to_string());
        }
        std::str::from_utf8(&self.b[start..self.i])
            .unwrap()
            .parse::<u32>()
            .map(Some)
            .map_err(|e| e.to_string())
    }

    fn string_or_null(&mut self) -> Result<Option<String>, String> {
        self.ws();
        if self.b[self.i..].starts_with(b"null") {
            self.i += 4;
            return Ok(None);
        }
        self.string().map(Some)
    }

    fn string_array(&mut self) -> Result<Vec<String>, String> {
        self.eat(b'[')?;
        let mut out = Vec::new();
        if self.peek()? == b']' {
            self.i += 1;
            return Ok(out);
        }
        loop {
            out.push(self.string()?);
            match self.peek()? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => Err("expected ',' or ']'".to_string())?,
            }
        }
    }
}

pub fn parse_profile(json: &str) -> Result<BootProfile, String> {
    let mut r = Reader::new(json);
    let mut p = BootProfile {
        iwad: String::new(),
        pwads: Vec::new(),
        sf2: None,
        max_render_res: None,
        engine_args: Vec::new(),
    };
    r.eat(b'{')?;
    if r.peek()? == b'}' {
        r.i += 1;
        return Err("iwad is required".to_string());
    }
    loop {
        let key = r.string()?;
        // 重复键按契约拒绝。
        let dup = match key.as_str() {
            "iwad" => !p.iwad.is_empty(),
            "pwads" => !p.pwads.is_empty(),
            "sf2" => p.sf2.is_some(),
            "maxRenderRes" => p.max_render_res.is_some(),
            "engineArgs" => !p.engine_args.is_empty(),
            _ => false,
        };
        if dup {
            return Err(format!("duplicate key: {key}"));
        }
        r.eat(b':')?;
        match key.as_str() {
            "iwad" => p.iwad = r.string()?,
            "pwads" => p.pwads = r.string_array()?,
            "sf2" => p.sf2 = r.string_or_null()?,
            "maxRenderRes" => p.max_render_res = r.u32_or_null()?,
            "engineArgs" => p.engine_args = r.string_array()?,
            other => return Err(format!("unknown key: {other}")),
        }
        match r.peek()? {
            b',' => r.i += 1,
            b'}' => {
                r.i += 1;
                break;
            }
            _ => return Err("expected ',' or '}'".to_string()),
        }
    }
    r.ws();
    if r.i != r.b.len() {
        return Err("trailing characters".to_string());
    }
    if p.iwad.is_empty() {
        return Err("iwad is required".to_string());
    }
    Ok(p)
}
```

- [ ] **Step 4: Run parser tests to green**

Run: `cargo test -p room-shell-web --lib profile` (timeout 300 s)
Expected: PASS — 8 tests.

- [ ] **Step 5: Implement `init_pipeline` and the launcher UI**

Create `shells/web/src/init_pipeline.rs`:

```rust
//! 两入口共享的初始化管线（D5 / AGENTS.md 入口契约）。
//!
//! //! 档案 → argv（Rust 侧构造，无 JS argv）→ 工厂安装 → doomgeneric_Create。
//! //! 引擎会永久保存 myargv，CString 与指针数组必须活满整个进程：
//! //! 存进 thread_local，与 native main.rs 的 App.args 同一手法。

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CString};

use room::audio::AudioBackend;

use crate::profile::BootProfile;
use crate::wasm_vfs;
use crate::web_audio;

thread_local! {
    /// argv 的所有权锚点（引擎保存 myargv 指针，绝不可释放）。
    static ARG_STORAGE: RefCell<Vec<CString>> = const { RefCell::new(Vec::new()) };
}

/// 由档案构造引擎 argv。
/// 约定：argv[0] = "woom24"；-iwad <名>；PWAD 逐个 -file <名>；
/// SF2 不进 argv（i_sound 的 exists() 在 wasm 上恒 false，见 Task 4 说明）。
fn build_argv(profile: &BootProfile) -> Vec<CString> {
    let mut v = vec![CString::new("woom24").unwrap()];
    v.push(CString::new("-iwad").unwrap());
    v.push(CString::new(profile.iwad.clone()).unwrap());
    for pwad in &profile.pwads {
        v.push(CString::new("-file").unwrap());
        v.push(CString::new(pwad.clone()).unwrap());
    }
    for a in &profile.engine_args {
        // 引擎参数原样透传（宿主档案是信任边界内的输入）。
        v.push(CString::new(a.clone()).unwrap());
    }
    v
}

/// 管线主体：两入口在此汇合，导出面之外没有任何分支。
pub fn run(profile: &BootProfile) -> Result<(), String> {
    // 1. IWAD 必须已在 VFS（minimal 入口刚注册；standard 入口由宿主预注册）。
    if wasm_vfs::vfs_get(&profile.iwad).is_none() {
        return Err(format!("IWAD '{}' 未注册：先经 woom24_register_file 注册", profile.iwad));
    }
    // 2. 帧缓冲上限（D5）：引擎侧 cap 属于后续自定义分辨率工作；
    //    本 spec 先记录到呈现器画布尺寸钳制（Task 5/6 已按 640×400 落地）。
    if let Some(res) = profile.max_render_res {
        log::info!("宿主最大渲染分辨率: {res}px（引擎侧 cap 待自定义分辨率工作）");
    }
    // 3. 音频：记住 SF2 名（后端构造时从 VFS 预载），装工厂。
    web_audio::set_pending_sf2(profile.sf2.clone());
    room::audio::set_backend_factory(|| {
        web_audio::WebAudioBackend::new()
            .map(|b| Box::new(b) as Box<dyn AudioBackend>)
    });
    // 4. argv → doomgeneric_Create（D_DoomMain 由此返回，tick 交给 woom24_tick）。
    let args = build_argv(profile);
    let mut argv: Vec<*mut c_char> = args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut()); // C 约定 argv[argc] = NULL
    let argc = (argv.len() - 1) as c_int;
    ARG_STORAGE.with_borrow_mut(|s| *s = args);
    // SAFETY: argv 指向 ARG_STORAGE 持有的 NUL 结尾串，进程级存活；
    // doomgeneric_Create 在本进程至多调用一次（入口契约保证）。
    unsafe {
        room::doom::doomgeneric::doomgeneric_Create(argc, argv.as_mut_ptr());
    }
    Ok(())
}
```

Create `shells/web/src/launcher_ui.rs` (complete file):

```rust
//! 最小入口的 DOM 配置 UI（D5 / AGENTS.md launcher mode）。
//!
//! //! 全部用 web-sys 裸 DOM API，无框架无 CDN。
//! //! 文件读取用 FileReader 回调（闭包持有 reader 所有权），
//! //! 避开 wasm-bindgen-futures 新依赖；读到的字节直接注册进 VFS。
//! //! 用户点"开始"才构造档案并进入 init_pipeline —— 游戏绝不自启。

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::init_pipeline;
use crate::profile::BootProfile;
use crate::wasm_vfs;

thread_local! {
    /// change 事件里已读入并注册的 PWAD 名（顺序 = FileList 顺序）。
    static PWAD_NAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static SF2_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 在 body 上挂出配置面板：PWAD 多选 + SF2 单选 + 开始按钮 + 错误横幅。
pub fn show(max_render_res: u32, iwad_name: &str) -> Result<(), String> {
    let doc = web_sys::window()
        .ok_or("no window")?
        .document()
        .ok_or("no document")?;
    let body = doc.body().ok_or("no body")?;

    let panel = doc
        .create_element("div")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlDivElement>()
        .map_err(js_err)?;
    panel.set_id("woom24-launcher");

    let title = doc.create_element("p").map_err(js_err)?;
    title.set_text_content(Some(&format!("woom24 — IWAD: {iwad_name}")));
    let _ = panel.append_child(&title);

    // PWAD 多选（顺序即 -file 加载顺序）。
    let pwad_label = doc.create_element("label").map_err(js_err)?;
    pwad_label.set_text_content(Some("PWADs（可多选，按选择顺序加载）"));
    let pwads = file_input(&doc, true, ".wad")?;
    let _ = panel.append_child(&pwad_label);
    let _ = panel.append_child(&pwads);

    // SF2 单选（可选；不选则音乐静音）。
    let sf2_label = doc.create_element("label").map_err(js_err)?;
    sf2_label.set_text_content(Some("SoundFont（可选；不选则音乐静音）"));
    let sf2 = file_input(&doc, false, ".sf2")?;
    let _ = panel.append_child(&sf2_label);
    let _ = panel.append_child(&sf2);

    // 错误横幅：缺 IWAD / 引擎初始化失败显示于此，绝不裸 panic 进控制台。
    let banner = doc.create_element("p").map_err(js_err)?;
    banner.set_id("woom24-banner");

    let start = doc
        .create_element("button")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlButtonElement>()
        .map_err(js_err)?;
    start.set_text_content(Some("Start"));

    let iwad = iwad_name.to_string();
    let doc_for_cb = doc.clone();
    let banner_for_cb = banner.clone();
    let on_click = Closure::<dyn FnMut()>::new(move || {
        banner_for_cb.set_text_content(None);
        // 字节已在各自 change→onload 链里注册进 VFS；
        // start 只需要把名字列表拼进档案。
        let pwad_names = PWAD_NAMES.with_borrow(|n| n.clone());
        let sf2_name = SF2_NAME.with_borrow(|n| n.clone());
        let profile = BootProfile {
            iwad: iwad.clone(),
            pwads: pwad_names,
            sf2: sf2_name,
            max_render_res: Some(max_render_res),
            engine_args: Vec::new(),
        };
        if let Err(e) = init_pipeline::run(&profile) {
            banner_for_cb.set_text_content(Some(&format!("启动失败：{e}")));
        } else {
            // 启动成功：拆掉配置面板（呈现画布由 loader 提供）。
            let _ = doc_for_cb
                .get_element_by_id("woom24-launcher")
                .map(|n| n.remove());
        }
    });
    start.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget();
    let _ = panel.append_child(&start);
    let _ = panel.append_child(&banner);
    let _ = body.append_child(&panel);
    Ok(())
}

/// 构造文件选择框；change 事件里逐文件读字节并注册 VFS。
fn file_input(
    doc: &web_sys::Document,
    multiple: bool,
    accept: &str,
) -> Result<web_sys::HtmlInputElement, JsValue> {
    let input = doc
        .create_element("input")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(js_err)?;
    input.set_type("file");
    input.set_accept(accept);
    input.set_multiple(multiple);
    let is_sf2 = accept == ".sf2";
    let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        if let Err(e) = on_files_picked(&ev, is_sf2) {
            web_sys::console::error_1(&JsValue::from_str(&e));
        }
    });
    input
        .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
        .map_err(js_err)?;
    on_change.forget();
    Ok(input)
}

/// change 事件：逐文件建 FileReader，onload 里取字节并注册 VFS + 记名。
fn on_files_picked(ev: &web_sys::Event, is_sf2: bool) -> Result<(), String> {
    let input = ev
        .target()
        .ok_or("no target")?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|e| format!("cast: {e:?}"))?;
    let files = input.files().ok_or("no files")?;
    for i in 0..files.length() {
        let f = files.get(i).ok_or("file vanished")?;
        let name = f.name();
        let reader = web_sys::FileReader::new().map_err(js_err)?;
        // 闭包持有 reader/fname/is_sf2 所有权；onload 时 result 已就绪。
        let fname = name.clone();
        let on_load = Closure::<dyn FnMut()>::new(move || {
            match reader.result() {
                Ok(v) if !v.is_null() => {
                    let bytes = js_sys::Uint8Array::new(&v).to_vec();
                    wasm_vfs::vfs_register(&fname, bytes);
                    if is_sf2 {
                        SF2_NAME.with_borrow_mut(|n| *n = Some(fname.clone()));
                    } else {
                        PWAD_NAMES.with_borrow_mut(|n| n.push(fname.clone()));
                    }
                }
                _ => web_sys::console::error_1(&JsValue::from_str(&format!(
                    "读取 {fname} 失败"
                ))),
            }
        });
        reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
        on_load.forget();
        reader
            .read_as_array_buffer(&f)
            .map_err(|e| format!("read {name}: {e:?}"))?;
    }
    Ok(())
}

fn js_err(e: JsValue) -> String {
    format!("DOM: {e:?}")
}
```

- [ ] **Step 6: Finalize the export surface in `lib.rs`**

Add (after the existing `woom24_attach_canvas` / `woom24_push_key` from Task 5):

```rust
mod init_pipeline;
mod launcher_ui;
pub mod profile;

/// 最小入口（AGENTS.md）：设备元数据 + IWAD → launcher 模式。
#[wasm_bindgen]
pub fn woom24_minimal_start(
    max_render_res: u32,
    iwad_name: &str,
    iwad: &[u8],
) -> Result<(), JsValue> {
    wasm_vfs::vfs_register(iwad_name, iwad.to_vec());
    launcher_ui::show(max_render_res, iwad_name).map_err(JsValue::from_str)
}

/// 宿主侧注册文件（standard 入口的 PWAD/SF2 全部走这里）。
#[wasm_bindgen]
pub fn woom24_register_file(name: &str, bytes: &[u8]) {
    wasm_vfs::vfs_register(name, bytes.to_vec());
}

/// 标准入口（AGENTS.md）：完整启动档案 → 直接开局，无配置 UI。
#[wasm_bindgen]
pub fn woom24_standard_start(profile_json: &str) -> Result<(), JsValue> {
    let p = profile::parse_profile(profile_json).map_err(JsValue::from_str)?;
    init_pipeline::run(&p).map_err(JsValue::from_str)
}
```

And replace the Task 5 stub of `woom24_tick` with the pump-wired version (`pump_current_backend` and its `LAST_BUILT` handle were shipped in Task 4's `web_audio.rs` — the engine's own `AUDIO` cell is lib-private, so the pump uses the shell-retained handle):

```rust
/// rAF 每帧调用一次：推进引擎一 tic、呈现，并按需补调音乐块（D2/D4）。
#[wasm_bindgen]
pub fn woom24_tick() {
    room::doom::d_main::doomgeneric_Tick();
    web_audio::pump_current_backend();
}
```

`init_pipeline::run`'s factory closure stays exactly as written in Step 5 — `WebAudioBackend::new()` registers the pump handle itself, so no extra wiring exists between the two tasks.

- [ ] **Step 7: Verify tests + wasm build + clippy**

```bash
cargo test -p room-shell-web --lib
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
cargo clippy -p room-shell-web -- -D warnings
```

(timeout 300 s each) Expected: all lib tests PASS (8 profile + 16 prior); wasm build exit 0; clippy clean.

- [ ] **Step 8: Commit**

```bash
git add shells/web/src/profile.rs shells/web/src/init_pipeline.rs shells/web/src/launcher_ui.rs shells/web/src/lib.rs
git commit -m "feat(web): two-entry contract (minimal/standard) with launcher UI"
```

---

### Task 8: `www/` static loader + build script + browser smoke checklist

**Files:**
- Create: `shells/web/www/index.html`
- Create: `shells/web/www/woom24.js`
- Create: `shells/web/scripts/build-www.sh`
- Modify: `shells/web/README.md` (build + serve + smoke checklist)

**Interfaces:**
- Consumes: the wasm-bindgen `--target web` glue (`www/pkg/room_shell_web.js` + `www/pkg/room_shell_web_bg.wasm`, generated, git-ignored) exporting `woom24_version`, `woom24_minimal_start`, `woom24_standard_start`, `woom24_register_file`, `woom24_attach_canvas`, `woom24_push_key`, `woom24_tick`.
- Produces: a self-contained static page (Task 8 deliverable) and the pinned one-command build.

- [ ] **Step 1: Write `www/index.html`**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <title>woom24</title>
  <style>
    /* 自包含：无外链字体/样式/脚本。 */
    html, body { margin: 0; height: 100%; background: #000; color: #ddd;
                 font-family: monospace; }
    body { display: flex; flex-direction: column; align-items: center; }
    canvas { image-rendering: pixelated; width: min(100vw, calc(100vh * 1.6));
             aspect-ratio: 8 / 5; background: #111; }
    #status { margin: 8px; }
  </style>
</head>
<body>
  <canvas id="screen" width="640" height="400"></canvas>
  <div id="status">loading wasm…</div>
  <script type="module" src="./woom24.js"></script>
</body>
</html>
```

- [ ] **Step 2: Write `www/woom24.js` (the loader)**

```javascript
// woom24 静态 loader：rAF 驱动 woom24_tick，DOM 键盘 → woom24_push_key。
// 自包含：只 import 本目录生成的 wasm-bindgen 胶水，运行时零网络请求
// （wasm 由打包后的同目录相对路径加载；file:// 直开由后续单文件化工作处理）。
import init, {
  woom24_version,
  woom24_minimal_start,
  woom24_standard_start,
  woom24_register_file,
  woom24_attach_canvas,
  woom24_push_key,
  woom24_tick,
} from "./pkg/room_shell_web.js";

const status = document.getElementById("status");
const canvas = document.getElementById("screen");

// 键盘映射：DOM code → Doom 键字节（此表为 wasm shell 的平台映射，
// 与 native 的 platform/keys.rs 平级；D2 契约要求 push 的是已映射的字节）。
const DOM2DOOM = {
  Enter: 13, NumpadEnter: 13, Escape: 27, ArrowLeft: 0xac, ArrowRight: 0xae,
  ArrowUp: 0xad, ArrowDown: 0xaf, ControlLeft: 0xa3, ControlRight: 0xa3,
  Space: 0xa2, ShiftLeft: 0x80 + 0x36, ShiftRight: 0x80 + 0x36,
  AltLeft: 0x80 + 0x38, AltRight: 0x80 + 0x38, Tab: 9, Backspace: 0x7f,
  Pause: 0xff, Equal: 61, Minus: 45,
  F1: 0x80 + 0x3b, F2: 0x80 + 0x3c, F3: 0x80 + 0x3d, F4: 0x80 + 0x3e,
  F5: 0x80 + 0x3f, F6: 0x80 + 0x40, F7: 0x80 + 0x41, F8: 0x80 + 0x42,
  F9: 0x80 + 0x43, F10: 0x80 + 0x44, F11: 0x80 + 0x57, F12: 0x80 + 0x58,
};

function doomKeyFor(code) {
  if (DOM2DOOM[code] !== undefined) return DOM2DOOM[code];
  if (code.startsWith("Key") && code.length === 4) {
    return code.charCodeAt(3) | 0x20; // 小写 ASCII
  }
  if (code.startsWith("Digit") && code.length === 6) {
    return code.charCodeAt(5);
  }
  return null;
}

window.addEventListener("keydown", (e) => {
  const k = doomKeyFor(e.code);
  if (k !== null) { e.preventDefault(); woom24_push_key(true, k); }
});
window.addEventListener("keyup", (e) => {
  const k = doomKeyFor(e.code);
  if (k !== null) { e.preventDefault(); woom24_push_key(false, k); }
});

function loop() {
  woom24_tick();
  requestAnimationFrame(loop);
}

try {
  await init();
  status.textContent = "wasm ok (v" + woom24_version() + ")";
  woom24_attach_canvas(canvas);

  // 演示流程 A（最小入口）：从本地选 IWAD → 配置面板。
  document.addEventListener("iwad-picked", async (ev) => {
    const bytes = new Uint8Array(await ev.detail.arrayBuffer());
    woom24_minimal_start(1080, ev.detail.name, bytes);
    requestAnimationFrame(loop);
  });

  // 演示流程 B（标准入口）：控制台调用——
  //   woom24_demo_standard(doom1.wad 文件对象)
  window.woom24_demo_standard = async (file) => {
    const bytes = new Uint8Array(await file.arrayBuffer());
    woom24_register_file(file.name, bytes);
    woom24_standard_start(JSON.stringify({
      iwad: file.name, pwads: [], sf2: null, maxRenderRes: 1080, engineArgs: [],
    }));
    requestAnimationFrame(loop);
  };

  // 页面上给一个 IWAD 选择器（演示 A 的触发器）。
  const pick = document.createElement("input");
  pick.type = "file"; pick.accept = ".wad";
  pick.onchange = () => {
    if (pick.files[0]) {
      document.dispatchEvent(new CustomEvent("iwad-picked", { detail: pick.files[0] }));
    }
  };
  document.body.appendChild(pick);
  status.textContent = "选择本地 IWAD 开始（或控制台调 woom24_demo_standard(file)）";
} catch (e) {
  status.textContent = "wasm 加载失败: " + e;
  console.error(e);
}
```

- [ ] **Step 3: Write the build script**

Create `shells/web/scripts/build-www.sh`:

```bash
#!/usr/bin/env bash
# 构建静态产物到 shells/web/www/pkg/（自包含；无 CDN）。
# 用法（仓库根目录）: bash shells/web/scripts/build-www.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."

echo "[1/3] cargo build (wasm32, release)"
cargo build -p room-shell-web --target wasm32-unknown-unknown --release

echo "[2/3] wasm-bindgen --target web"
wasm-bindgen --target web --out-dir shells/web/www/pkg --no-typescript \
  target/wasm32-unknown-unknown/release/room_shell_web.wasm

echo "[3/3] 产物尺寸"
ls -l shells/web/www/pkg/room_shell_web_bg.wasm
echo "本地预览: python -m http.server 8000 --directory shells/web/www"
```

Create `shells/web/www/.gitignore`:

```
pkg/
```

- [ ] **Step 4: Run the build script**

Run: `bash shells/web/scripts/build-www.sh` (timeout 300 s)
Expected: exit 0; `www/pkg/room_shell_web.js` + `room_shell_web_bg.wasm` exist; `.wasm` size printed (record it — spec acceptance 4 requires reporting it in the implementation summary).

- [ ] **Step 5: Browser smoke checklist (manual, documented)**

Run: `python -m http.server 8000 --directory shells/web/www` (leave running with a bounded window; stop with Ctrl+C)
Open `http://localhost:8000/` in Chromium and Firefox. Checklist (record pass/fail per line in the task report):

1. Page loads, status shows `wasm ok (v1)` — no console errors.
2. Pick a local `doom1.wad` (shareware, user-supplied) → launcher panel appears (PWAD/SF2 inputs + Start).
3. Click Start (no PWADs) → demo playback visible on canvas, 35 Hz feel, no console panics.
4. SFX audible (pistol/door during demo). Music silent without SF2 (expected degradation), audible with a user-picked `.sf2`.
5. Keys work: arrows move, Ctrl fires, Esc responds.
6. Reload, then in DevTools console: `woom24_demo_standard(<File object of doom1.wad>)` → boots directly, no launcher.
7. Re-run 3-6 with a WebGL2-less context blocked (e.g. `--disable-webgl2` flag in Chromium) → Canvas2D fallback shows the same demo.
8. `file://` open of `www/index.html` may fail on module/wasm fetch — record the exact browser error text; single-file embedding is a follow-up, static-host serving is the contract.

Update `shells/web/README.md` with: the build command, the serve command, and this checklist verbatim (Chinese, half-width punctuation).

- [ ] **Step 6: Commit**

```bash
git add shells/web/www/index.html shells/web/www/woom24.js shells/web/www/.gitignore shells/web/scripts/build-www.sh shells/web/README.md
git commit -m "feat(web): static www loader and build script"
```

---

### Task 9: Docs sync + final verification (spec acceptance audit)

**Files:**
- Modify: `README.md` (Project layout + Building sections)
- Modify: `AGENTS.md:14` (status line)

**Interfaces:**
- Consumes: everything shipped in Tasks 1-8; the spec acceptance list.
- Produces: synchronized docs and the recorded verification evidence (`.wasm` size, error-count baseline, suite status).

- [ ] **Step 1: Update `README.md`**

In "## Project layout" (line 39 section), after the `shells/native` bullet, add:

```markdown
- `shells/web/` — wasm shell (`room-shell-web`, cdylib): wasm-bindgen exports, in-memory VFS
  CRT shim, Web Audio backend, Canvas2D/WebGL2 presenters, static `www/` loader. Its
  declaration-only `libc` seam crate lives at `shells/web/crt/` (`woom24-libc`).
```

In "## Building" (line 78 section), add (the outer fence below is four backticks so the inner block survives verbatim):

````markdown
### Web shell

```bash
bash shells/web/scripts/build-www.sh
python -m http.server 8000 --directory shells/web/www
```

Open `http://localhost:8000/` and supply a local IWAD. The wasm check used during
development: `cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown`.
````

- [ ] **Step 2: Update the `AGENTS.md` status line (line 14)**

Replace:

```
Status: **spec ① implemented** — platform layer lives in `shells/native`; `room` lib carries no windowing/GPU/audio deps; audio goes through the `AudioBackend` control plane. Wasm gap (CRT externs + LP64 guards) enumerated in the spec and handed to specs ②/③. GitHub-side fork: KurvCygnus/woom24 (origin); upstream: sunsided/room.
```

with:

```
Status: **spec ① implemented** — platform layer lives in `shells/native`; `room` lib carries no windowing/GPU/audio deps; audio goes through the `AudioBackend` control plane. **Spec ② implemented** — `shells/web` closes the wasm CRT gap (in-memory VFS + `woom24-libc` seam) and ships the two-entry wasm contract; ILP32 layout-guard refinement remains with spec ③. GitHub-side fork: KurvCygnus/woom24 (origin); upstream: sunsided/room.
```

- [ ] **Step 3: Run the full verification battery (evidence before done)**

Run in sequence, each with a 300 s timeout, none in parallel (AGENTS protocol 2/4):

```bash
cargo build
cargo clippy --workspace --exclude c2rust-intermediate
cargo test
cargo check -p room -p woom24-libc -p room-shell-web --target wasm32-unknown-unknown
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
bash shells/web/scripts/build-www.sh
```

Expected, recorded verbatim in the task report:
1. Host build: exit 0.
2. Clippy: zero warnings.
3. Tests: 376 unit + 2 demo, all pass.
4. Wasm check: exit 0 (spec acceptance 2).
5. Wasm release build: exit 0.
6. `room_shell_web_bg.wasm` byte size printed (spec acceptance 4).

- [ ] **Step 4: Commit**

```bash
git add README.md AGENTS.md
git commit -m "docs: sync README/AGENTS for the wasm shell (spec 2)"
```

---

## Appendix A: Authoritative CRT symbol list (regenerated 2026-09-17)

Regeneration command (run at repo root):

```bash
cargo check -p room --target wasm32-unknown-unknown 2>&1 | grep -E "error\[E0432|error\[E0425|cannot find"
```

**Total: 76 errors** = 71 × E0425 + 2 × E0432 + 3 × E0080. Files: 13 `doom/` files (w_file.rs, w_main.rs, d_main.rs, g_game.rs, i_scale.rs, i_system.rs, p_saveg.rs, p_setup.rs, r_data.rs, r_main.rs, w_wad.rs, wi_stuff.rs, z_zone.rs) + the 3 guard sites (info.rs, m_menu.rs).

Types and constants (must exist on `libc` for wasm = the `woom24-libc` seam):

| Item | Kind | Definition | Used at |
|---|---|---|---|
| `c_char` | type | `pub use std::ffi::c_char` | g_game.rs ×5 (2190/2267/2272/2716/2900) |
| `c_long` | type | `pub use std::ffi::c_long` | g_game.rs:72 |
| `FILE` | type | opaque `#[repr(C)] pub enum FILE {}` | w_file.rs:18, i_scale.rs:67, p_saveg.rs:69, z_zone.rs:621 |
| `SEEK_SET` | const | `pub const SEEK_SET: c_int = 0;` | w_file.rs:18 |

Functions (E0425 count = call sites; signatures mirror crates.io `libc` 0.2 verbatim):

| Symbol | Signature | Sites |
|---|---|---|
| `printf` | `fn(*const c_char, ...) -> c_int` (variadic decl) | 32 |
| `puts` | `fn(*const c_char) -> c_int` | 3 |
| `putchar` | `fn(c_int) -> c_int` | 3 |
| `ftell` | `fn(*mut FILE) -> c_long` | 3 |
| `fopen` | `fn(*const c_char, *const c_char) -> *mut FILE` | 3 |
| `fflush` | `fn(*mut FILE) -> c_int` | 3 |
| `fclose` | `fn(*mut FILE) -> c_int` | 3 |
| `strlen` | `fn(*const c_char) -> usize` | 2 |
| `malloc` | `fn(usize) -> *mut c_void` | 2 |
| `atoi` | `fn(*const c_char) -> c_int` | 2 |
| `snprintf` | `fn(*mut c_char, usize, *const c_char, ...) -> c_int` (variadic decl) | 1 |
| `rename` | `fn(*const c_char, *const c_char) -> c_int` | 1 |
| `remove` | `fn(*const c_char) -> c_int` | 1 |
| `memset` | `fn(*mut c_void, c_int, usize) -> *mut c_void` | 1 |
| `fwrite` | `fn(*const c_void, usize, usize, *mut FILE) -> usize` | 1 |
| `fread` | `fn(*mut c_void, usize, usize, *mut FILE) -> usize` | 1 |
| `fseek` | `fn(*mut FILE, c_long, c_int) -> c_int` | E0432 import only (used via `use` in w_file.rs) |

E0432 unresolved imports (2): `w_file.rs:18` (`fclose, fopen, fread, fseek, FILE, SEEK_SET`), `w_main.rs:15` (`printf`).

E0080 const-assert panics (3, spec ② closes via `#[cfg(target_pointer_width = "64")]` gates; spec ③ refines per-width expectations): `info.rs:910` (`size_of::<State>() == 40`), `m_menu.rs:175` (`size_of::<menuitem_t>() == 32`), `m_menu.rs:179` (`size_of::<menu_t>() == 40`).

printf format-string audit (specifier set the shim must support): `%s %d %i %u %x %c %p %%` plus decimal width (`%7i`). Arity span 0–4 varargs (4: z_zone.rs:358 `msg3`; 2: z_zone.rs:349/352, g_game.rs:2899, i_system.rs:134, d_main.rs:1132 snprintf, wi_stuff.rs:870; 1: w_wad.rs:194, w_main.rs:49, r_data.rs:618; 0: i_scale/r_data/r_main dots-and-banners, z_zone msg4-6). **Not in the regenerated list** (spec D1 prose supersets): `calloc`, `free`, `sscanf`, `vsnprintf`, `exit`, `strcmp` — not declared, not implemented (except `free`, shipped paired with `malloc` as belt-and-braces).

If a future regen diverges from this table, the command output is authoritative.

## Appendix B: Variadic-shim probe (2026-09-17, stable 1.98.1)

Scratch crate (`crate-type = ["cdylib"]`), `extern "C" { pub fn printf(fmt: *const c_char, ...) -> i32; }` declared at crate root plus `#[export_name = "printf"] pub extern "C" fn printf_shim(fmt: *const c_char, a: usize, b: usize, c: usize, d: usize) -> i32`, with call sites of 0/1/2/3/4 varargs — `cargo build --target wasm32-unknown-unknown` **links successfully**. This validates Task 2's declaration/definition split without nightly or `c_variadic`. (Direct `#[no_mangle] fn printf` + same-name declaration in one module is E0428 — hence `#[export_name]` on the shim body.)
