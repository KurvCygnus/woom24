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
mod init_pipeline;
mod launcher_ui;
pub mod present_c2d;
pub mod present_gl2;
pub mod profile;
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

/// 最小入口 (AGENTS.md): 设备元数据 + IWAD → launcher 模式.
#[wasm_bindgen]
pub fn woom24_minimal_start(
    max_render_res: u32,
    iwad_name: &str,
    iwad: &[u8],
) -> Result<(), JsValue> {
    wasm_vfs::vfs_register(iwad_name, iwad.to_vec());
    //? JsValue::from_str 收 &str, String 得先取引用 -- 不能直接把方法路径喂给 map_err.
    launcher_ui::show(max_render_res, iwad_name).map_err(|e| JsValue::from_str(&e))
}

/// 宿主侧注册文件 (standard 入口的 PWAD/SF2 全部走这里).
#[wasm_bindgen]
pub fn woom24_register_file(name: &str, bytes: &[u8]) {
    wasm_vfs::vfs_register(name, bytes.to_vec());
}

/// 标准入口 (AGENTS.md): 完整启动档案 → 直接开局, 无配置 UI.
#[wasm_bindgen]
pub fn woom24_standard_start(profile_json: &str) -> Result<(), JsValue> {
    let p = profile::parse_profile(profile_json).map_err(|e| JsValue::from_str(&e))?;
    init_pipeline::run(&p).map_err(|e| JsValue::from_str(&e))
}

/// rAF 每帧调用一次: 推进引擎一 tic, 呈现, 并按需补调音乐块 (D2/D4).
#[wasm_bindgen]
pub fn woom24_tick() {
    room::doom::d_main::doomgeneric_Tick();
    web_audio::pump_current_backend();
}
