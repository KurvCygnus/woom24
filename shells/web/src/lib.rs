//! woom24's wasm shell: wires the room engine into the browser (spec 2).
//!
//! Module map:
//! - `wasm_vfs`: CRT shim + in-memory VFS
//! - `web_audio`: WebAudioBackend
//! - `clock` / `dg` / `present_c2d`: platform callbacks and Canvas2D presentation
//! - `present_gl2`: WebGL2 presenter
//! - `profile` / `init_pipeline` / `launcher_ui`: the two-entry contract

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

/// Returns the shell build version (scaffolding smoke value; keeps the linker
/// from discarding the whole crate).
#[wasm_bindgen]
pub fn woom24_version() -> u32 {
    1
}

/// Called by the loader JS: hands in the target canvas (engine resolution 640×400).
#[wasm_bindgen]
pub fn woom24_attach_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
    dg::attach_canvas(&canvas).map_err(|e| JsValue::from_str(&e))
}

/// JS key events → queue (DG_GetKey drains it during tick).
#[wasm_bindgen]
pub fn woom24_push_key(pressed: bool, doom_key: u8) {
    dg::push_key(pressed, doom_key);
}

/// Minimal entry (AGENTS.md): device metadata + IWAD → launcher mode.
#[wasm_bindgen]
pub fn woom24_minimal_start(
    max_render_res: u32,
    iwad_name: &str,
    iwad: &[u8],
) -> Result<(), JsValue> {
    wasm_vfs::vfs_register(iwad_name, iwad.to_vec());
    //? JsValue::from_str takes a &str, so a String needs its reference taken
    //? first -- the method path cannot be fed to map_err directly.
    launcher_ui::show(max_render_res, iwad_name).map_err(|e| JsValue::from_str(&e))
}

/// Host-side file registration (every PWAD/SF2 of the standard entry goes through here).
#[wasm_bindgen]
pub fn woom24_register_file(name: &str, bytes: &[u8]) {
    wasm_vfs::vfs_register(name, bytes.to_vec());
}

/// Standard entry (AGENTS.md): complete boot profile → straight into the game, no config UI.
#[wasm_bindgen]
pub fn woom24_standard_start(profile_json: &str) -> Result<(), JsValue> {
    let p = profile::parse_profile(profile_json).map_err(|e| JsValue::from_str(&e))?;
    init_pipeline::run(&p).map_err(|e| JsValue::from_str(&e))
}

/// Called once per rAF frame: advances the engine one tic, presents, and
/// pumps music blocks on demand (D2/D4).
#[wasm_bindgen]
pub fn woom24_tick() {
    room::doom::d_main::doomgeneric_Tick();
    web_audio::pump_current_backend();
}
