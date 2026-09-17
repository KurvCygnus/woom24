//! woom24 的 wasm shell: 把 room 引擎接进浏览器 (spec ②).
//!
//! 模块职责 (随任务逐步落地):
//! - `wasm_vfs`: CRT 垫片 + 内存 VFS (Task 2)
//! - `web_audio`: WebAudioBackend (Task 4)
//! - `clock` / `dg` / `present_c2d` / `present_gl2`: 平台回调与呈现 (Task 5/6)
//! - `profile` / `init_pipeline` / `launcher_ui`: 两入口契约 (Task 7)

use wasm_bindgen::prelude::*;

pub mod wasm_vfs;
/// web_audio 只编译进 wasm32 与测试：宿主没有 Web Audio，且宿主 cdylib
/// 一旦引用 room 会把引擎对象拖进链接，在 Task 5 提供真实 DG_* 之前
/// 无法闭合符号 —— 用 cfg 维持宿主 `cargo build` 全绿（Task 5 落地
/// dg.rs 后如需宿主可见可移除本 cfg）。
#[cfg(any(test, target_arch = "wasm32"))]
mod web_audio;

/// 宿主测试二进制的 no-op doomgeneric 平台回调 (与 room/src/lib.rs 的
/// dg_test_stubs 同一手法). `web_audio` 是 shell 里第一个引用 room 的模块,
/// 它把引擎对象拖进宿主测试链接, 而 room rlib 里的 doom 代码引用六个
/// `DG_*` 外部符号 -- 真正的实现在 Task 5 的 `dg.rs` (wasm 侧);
/// 本模块只存在于 `cfg(test)`, 绝不进 shipped cdylib.
/// Task 5 落地真实 `DG_*` 时删除本模块.
#[cfg(test)]
mod dg_test_stubs {
    #[no_mangle]
    extern "C" fn DG_Init() {}
    #[no_mangle]
    extern "C" fn DG_DrawFrame() {}
    #[no_mangle]
    extern "C" fn DG_SleepMs(_ms: u32) {}
    #[no_mangle]
    extern "C" fn DG_GetTicksMs() -> u32 {
        0
    }
    #[no_mangle]
    extern "C" fn DG_GetKey(_pressed: *mut i32, _doom_key: *mut u8) -> i32 {
        0
    }
    #[no_mangle]
    extern "C" fn DG_SetWindowTitle(_title: *const std::ffi::c_char) {}
}

/// 返回 shell 构建版本号 (脚手架冒烟用; 避免链接器裁掉整个 crate).
#[wasm_bindgen]
pub fn woom24_version() -> u32 {
    1
}
