//! 声明层垫片：仅在 wasm32 上替代 crates.io `libc`，让 `room` 里的
//! `libc::fopen` / `libc::printf` 等路径通过类型检查。
//! 真正的实现（内存 VFS / malloc / printf 子集）在 `shells/web/src/wasm_vfs.rs`。
//!
//! //! 本 crate 不得包含任何实现逻辑；不得依赖任何其它 crate。
