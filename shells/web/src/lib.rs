//! woom24 的 wasm shell: 把 room 引擎接进浏览器 (spec ②).
//!
//! 模块职责 (随任务逐步落地):
//! - `wasm_vfs`: CRT 垫片 + 内存 VFS (Task 2)
//! - `web_audio`: WebAudioBackend (Task 4)
//! - `clock` / `dg` / `present_c2d` / `present_gl2`: 平台回调与呈现 (Task 5/6)
//! - `profile` / `init_pipeline` / `launcher_ui`: 两入口契约 (Task 7)

use wasm_bindgen::prelude::*;

pub mod wasm_vfs;

/// 返回 shell 构建版本号 (脚手架冒烟用; 避免链接器裁掉整个 crate).
#[wasm_bindgen]
pub fn woom24_version() -> u32 {
    1
}
