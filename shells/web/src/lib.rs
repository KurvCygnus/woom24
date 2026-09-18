//! woom24 的 wasm shell: 把 room 引擎接进浏览器 (spec ②).
//!
//! 模块职责 (随任务逐步落地):
//! - `wasm_vfs`: CRT 垫片 + 内存 VFS (Task 2)
//! - `web_audio`: WebAudioBackend (Task 4)
//! - `clock` / `dg` / `present_c2d`: 平台回调与 Canvas2D 呈现 (Task 5)
//! - `present_gl2`: WebGL2 呈现器 (Task 6)
//! - `profile` / `init_pipeline` / `launcher_ui`: 两入口契约 (Task 7)

use wasm_bindgen::prelude::*;

mod clock;
mod dg;
pub mod present_c2d;
pub mod present_gl2;
pub mod wasm_vfs;
mod web_audio;

/// 返回 shell 构建版本号 (脚手架冒烟用; 避免链接器裁掉整个 crate).
#[wasm_bindgen]
pub fn woom24_version() -> u32 {
    1
}

/// 由 loader JS 调用: 交入目标画布 (engine 分辨率 640×400).
#[wasm_bindgen]
pub fn woom24_attach_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
    dg::attach_canvas(&canvas).map_err(|e| JsValue::from_str(&e))
}

/// JS 键盘事件 → 队列 (DG_GetKey 在 tick 中弹出).
#[wasm_bindgen]
pub fn woom24_push_key(pressed: bool, doom_key: u8) {
    dg::push_key(pressed, doom_key);
}

/// rAF 每帧调用一次: 推进引擎一 tic 并呈现 (D2 数据流).
#[wasm_bindgen]
pub fn woom24_tick() {
    room::doom::d_main::doomgeneric_Tick();
}
