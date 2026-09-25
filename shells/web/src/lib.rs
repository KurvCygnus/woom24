//! woom24's wasm shell: wires the room engine into the browser (spec 2).
//!
//! Module map:
//! - `wasm_vfs`: CRT shim + in-memory VFS
//! - `web_audio`: WebAudioBackend
//! - `clock` / `dg` / `present_c2d`: platform callbacks and Canvas2D presentation
//! - `present_gl2`: WebGL2 presenter
//! - `profile` / `init_pipeline` / `launcher_ui`: the two-entry contract
//! - `console_log`: routes the `log` facade to the browser DevTools console
//! - `harness` (feature-gated): F9 e2e diagnostics -- frame/state digests + interp toggle

use std::cell::Cell;

use wasm_bindgen::prelude::*;

mod clock;
mod console_log;
mod dg;
#[cfg(feature = "harness")]
pub mod harness;
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
pub fn woom24_version() -> u32 { 1 }

/// Called by the loader JS: hands in the target canvas (engine resolution 640×400).
#[wasm_bindgen]
pub fn woom24_attach_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue>
{
    // Before anything else: without this, all `log::` output (boot diagnostics,
    // fallback warnings) is dropped silently.
    console_log::install_once();
    dg::attach_canvas(&canvas).map_err(|e| JsValue::from_str(&e))
}

/// JS key events → queue (DG_GetKey drains it during tick).
#[wasm_bindgen]
pub fn woom24_push_key(pressed: bool, doom_key: u8) { dg::push_key(pressed, doom_key); }

/// Minimal entry (AGENTS.md): device metadata + IWAD → launcher mode.
#[wasm_bindgen]
pub fn woom24_minimal_start(
    max_render_res: u32,
    iwad_name: &str,
    iwad: &[u8],
) -> Result<(), JsValue>
{
    wasm_vfs::vfs_register(iwad_name, iwad.to_vec());
    //? JsValue::from_str takes a &str, so a String needs its reference taken
    //? first -- the method path cannot be fed to map_err directly.
    launcher_ui::show(max_render_res, iwad_name).map_err(|e| JsValue::from_str(&e))
}

/// Host-side file registration (every PWAD/SF2 of the standard entry goes through here).
#[wasm_bindgen]
pub fn woom24_register_file(name: &str, bytes: &[u8]) { wasm_vfs::vfs_register(name, bytes.to_vec()); }

/// Standard entry (AGENTS.md): complete boot profile → straight into the game, no config UI.
#[wasm_bindgen]
pub fn woom24_standard_start(profile_json: &str) -> Result<(), JsValue>
{
    let p = profile::parse_profile(profile_json).map_err(|e| JsValue::from_str(&e))?;
    init_pipeline::run(&p).map_err(|e| JsValue::from_str(&e))
}

/// Legacy single-tic entry (pre-F1 shape): advances the engine one tic and
/// pumps music blocks. Kept for tests/compat; the rAF loader now calls
/// [`woom24_frame`], which presents uncapped interpolated frames.
#[wasm_bindgen]
pub fn woom24_tick()
{
    room::doom::d_main::doomgeneric_Tick();
    web_audio::pump_current_backend();
}

/// Called once per rAF frame with `performance.now()`: pumps 0..4 tics
/// through the engine's own ticker (browser catch-up cap, woom24 policy),
/// presents one interpolated frame, and pumps music blocks on demand (F1 M1
/// frame/pump split). The timestamp feeds the same clock domain
/// DG_GetTicksMs uses, so the interpolation fraction derives from the
/// engine's own heartbeat — no second time source.
#[wasm_bindgen]
pub fn woom24_frame(now_ms: f64)
{
    room::doom::d_main::doomgeneric_frame(clock::engine_ms(now_ms));
    web_audio::pump_current_backend();
    auto_diag_tick();
}

//? DIAGNOSTIC(spec-4): in-game frame counter for the auto-diag logger;
//? wraps at 2^32, which is harmless (only the modulo matters). Strip
//? together with woom24_diag_frame once the browser evidence closes the
//? sprite defect.
thread_local!
{
    static AUTO_DIAG_FRAMES: Cell<u32> = const { Cell::new(0) };
}

/// How often (in presented frames) the auto-diag logger fires while a map
/// is running: roughly every 8.5 wall seconds at 35 tics/s.
const AUTO_DIAG_INTERVAL: u32 = 300;

//? DIAGNOSTIC(spec-4): every AUTO_DIAG_INTERVAL-th frame in GS_LEVEL (0),
//? log [`woom24_diag_frame`] to the browser console -- hash, census and
//? tile map, no host action required. The maintainer's next gameplay
//? session yields the sprite-defect evidence series automatically; the
//? console lines start with the g=/gs= fields for easy grepping.
//? Maintainer capture: open the DevTools console, play one session, then
//? save the lines starting with "woom24-diag:".
//? The diag frames each perform two extra D_Display re-presents, which can
//? perturb an in-progress wipe by up to 3 render-side steps (the simulation
//? is untouched and the presented frame is correctly restored) -- a
//? momentary wipe glitch on a diag frame is an instrument artifact, not a
//? defect.
fn auto_diag_tick()
{
    let n = AUTO_DIAG_FRAMES.with(|c| c.get().wrapping_add(1));
    AUTO_DIAG_FRAMES.with(|c| c.set(n));
    let in_game = unsafe { room::doom::g_game::gamestate } == 0;
    if !in_game || !n.is_multiple_of(AUTO_DIAG_INTERVAL)
    {
        return;
    }
    let report = woom24_diag_frame();
    web_sys::console::log_1(&JsValue::from_str(&format!("woom24-diag: {report}")));
}

//? DEBUG(probe): Node-harness-only hooks for the ticdup-clobber hunt
//? (div-probe). `singletics` forces one tic per tick so the headless harness
//? advances past the frozen-clock wait loop; `gametic`/`ticdup` report
//? progress. Strip together with the `//? div-probe` markers in d_loop.rs
//? once the browser re-test passes.
#[wasm_bindgen]
pub fn woom24_probe_set_singletics(on: u32)
{
    unsafe { room::doom::d_loop::singletics = on as i32 };
}

/// # Safety
/// Reads a core `static mut`; single-threaded main-thread-only contract applies.
#[wasm_bindgen]
pub unsafe fn woom24_probe_gametic() -> u32
{
    room::doom::d_loop::gametic as u32
}

/// # Safety
/// Reads a core `static mut`; single-threaded main-thread-only contract applies.
#[wasm_bindgen]
pub unsafe fn woom24_probe_ticdup() -> u32
{
    room::doom::d_loop::ticdup as u32
}

/// Arms the harness fake clock (see clock.rs). Node-only diagnostic.
#[wasm_bindgen]
pub fn woom24_probe_set_clock_armed(on: u32, initial_ms: f64)
{
    clock::probe_set_armed(on != 0, initial_ms);
}

//? F1 M2 diagnostic (same strip-together family as the other probes): drives
//? the video_cfg reconfiguration mid-game so the resolution-switch smoke can
//? run from the console. Not part of the two-entry contract; the F7 settings
//? surface (M4) will own the user-facing path.
#[wasm_bindgen]
pub fn woom24_probe_set_video_resolution(width: u32, height: u32) -> Result<(), JsValue>
{
    let cfg = room::doom::video_cfg::VideoConfig::new(
        width,
        height,
        room::doom::video_cfg::AspectMode::VanillaStretch,
    );
    room::doom::video_cfg::apply(cfg).map_err(|e| e.into())
}

/// Last `I_Error` message of this process, if any. Hosts assert `None` on
/// healthy runs and render the text on crash; see `www/woom24.js` overlay.
#[wasm_bindgen]
pub fn woom24_last_error() -> Option<String>
{
    room::doom::i_system::last_i_error()
}

//? DIAGNOSTIC(spec-4): GRADUATED in the Task 5 fix (was: strip or graduate).
//? Sprite-invisibility classification readback, now wired into
//? [`woom24_frame`] through `auto_diag_tick` so a real browser gameplay
//? session produces the defect evidence automatically (the engine core and
//? the video_cfg::apply surface are exonerated/pinned; the remaining
//? suspect surface is the browser-only presenter/runtime, which only a
//? human session can reach). Strip this export, the auto-diag logger and
//? the harness diag mode together once the browser evidence closes the
//? defect. Format:
//?   "g=<gametic>;lt=<leveltime>;gs=<gamestate>;hash=<on fnv1a>;
//?    off=<off fnv1a>;mo=<max delta fixed>/<moved >2u>/<total>;
//?    tiles=<rows x cols ASCII map, '#' = >25% tile pixels differ>"
#[wasm_bindgen]
pub fn woom24_diag_frame() -> String
{
    // The export is directly JS-callable: before boot there is no present
    // buffer, and the frame reads below would index empty vectors into a
    // wasm trap.
    if unsafe { room::doom::doomgeneric::DG_ScreenBuffer.is_null() }
    {
        return String::from("not booted");
    }

    // -- current presented frame hash ---------------------------------------
    let (dg_w, dg_h) = room::doom::doomgeneric::dg_res();
    let frame_on = read_screen_buffer(dg_w, dg_h);
    let hash_on = fnv1a(&frame_on);

    // -- surgical interpolation-OFF re-present + tile map --------------------
    let saved_frac = room::doom::r_interp::fraction();
    room::doom::r_interp::set_enabled(false);
    room::doom::r_interp::begin_frame(room::doom::i_timer::I_GetTimeMS() as u32);
    room::doom::r_interp::set_fraction(saved_frac);
    room::doom::d_main::D_Display();
    let frame_off = read_screen_buffer(dg_w, dg_h);
    let hash_off = fnv1a(&frame_off);
    let tiles = diff_tile_map(&frame_on, &frame_off, dg_w, dg_h);

    // -- restore the ON present (fraction re-armed, gate re-opened) ----------
    room::doom::r_interp::set_enabled(true);
    room::doom::r_interp::set_fraction(saved_frac);
    room::doom::d_main::D_Display();

    // -- board census: sampled-vs-live mobj displacement right now -----------
    // Pass 1 at the live fraction (what the frame actually rendered); pass 2
    // at a forced half-tic fraction so every non-degenerate prev/curr pair
    // shows up as half its true displacement, immune to a ~0 live fraction.
    let (total, moved, max_delta) = unsafe
    {
        census_mobj_deltas(
            *std::ptr::addr_of!(room::doom::p_setup::sectors),
            *std::ptr::addr_of!(room::doom::p_setup::numsectors),
        )
    };
    room::doom::r_interp::set_fraction(32768);
    let (total2, moved2, max_delta2) = unsafe
    {
        census_mobj_deltas(
            *std::ptr::addr_of!(room::doom::p_setup::sectors),
            *std::ptr::addr_of!(room::doom::p_setup::numsectors),
        )
    };

    // -- camera pair displacement at the forced fraction ---------------------
    // Sampled INSIDE the same forced window as the pass-2 census (final
    // whole-branch review: the saved-fraction restore used to precede this
    // computation, so cam2 reported the live-fraction delta while the Node
    // harness reads it under forced-half-fraction logic -- the documented
    // bound did not apply). The restore below closes the window.
    let cam_delta: i64 = unsafe
    {
        use room::doom::c_ffi;
        use room::doom::d_player::{consoleplayer, players};
        let cp = consoleplayer as usize;
        let mo = players[cp].mo as *mut c_ffi::mobj_t;
        if mo.is_null()
        {
            -1
        }
        else
        {
            let live_x = (*mo).x;
            let live_y = (*mo).y;
            let s = room::doom::r_interp::sample_camera(&raw mut players[cp], mo);
            let dx = (s.x - live_x) as i64;
            let dy = (s.y - live_y) as i64;
            ((dx * dx + dy * dy) as f64).sqrt() as i64
        }
    };
    room::doom::r_interp::set_fraction(saved_frac);

    // -- live-scene motion tracker: did ANY mobj move since the last call? ---
    // Distinguishes "the board is inert while the scene moves" (capture
    // misses) from "the scene is genuinely static" (degenerate pairs).
    let (live_moved, live_total) = unsafe
    {
        movement_tracker(
            *std::ptr::addr_of!(room::doom::p_setup::sectors),
            *std::ptr::addr_of!(room::doom::p_setup::numsectors),
        )
    };
    let (hp, kills) = unsafe
    {
        use room::doom::d_player::{consoleplayer, players};
        let cp = consoleplayer as usize;
        (players[cp].health, players[cp].killcount)
    };
    let pmo = unsafe
    {
        use room::doom::d_player::{consoleplayer, players};
        let mo = players[consoleplayer as usize].mo;
        if mo.is_null()
        {
            String::from("none")
        }
        else
        {
            let m = mo as *mut room::doom::c_ffi::mobj_t;
            format!("{},{}", (*m).x, (*m).y)
        }
    };

    format!(
        "g={};lt={};gs={};f={};ba={};hp={hp};kills={kills};pmo={pmo};live={live_moved}/{live_total};\
         hash={hash_on:#018x};off={hash_off:#018x};\
         mo={max_delta}/{moved}/{total};mo2={max_delta2}/{moved2}/{total2};cam2={cam_delta};\
         tiles={tiles}",
        unsafe { room::doom::d_loop::gametic },
        unsafe { room::doom::p_tick::leveltime },
        unsafe { room::doom::g_game::gamestate },
        saved_frac,
        room::doom::r_interp::board_active() as u8,
    )
}

/// Remember every mobj's live (x, y) between calls (keyed by address) and
/// report how many moved more than one fixed unit since the last call.
///
/// # Safety
/// Walks the live sector/thing structures; single-threaded main-thread
/// contract applies.
unsafe fn movement_tracker(
    sec_base: *mut room::doom::c_ffi::sector_t,
    sec_count: i32,
) -> (usize, usize)
{
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local!
    {
        static PREV: RefCell<HashMap<usize, (i32, i32)>> = RefCell::new(HashMap::new());
    }

    let mut now: HashMap<usize, (i32, i32)> = HashMap::new();
    for si in 0..sec_count as usize
    {
        let mut thing = (*sec_base.add(si)).thinglist as *mut room::doom::c_ffi::mobj_t;
        while !thing.is_null()
        {
            now.insert(thing as usize, ((*thing).x, (*thing).y));
            thing = (*thing).snext as *mut room::doom::c_ffi::mobj_t;
        }
    }
    let mut moved = 0usize;
    PREV.with_borrow(|prev|
    {
        for (addr, &(x, y)) in &now
        {
            if let Some(&(px, py)) = prev.get(addr)
            {
                let dx = (x - px) as i64;
                let dy = (y - py) as i64;
                if dx * dx + dy * dy > 65536
                {
                    moved += 1;
                }
            }
        }
    });
    let total = now.len();
    PREV.with_borrow_mut(|p| *p = now);
    (moved, total)
}

/// Snapshot the doomgeneric present buffer as bytes.
fn read_screen_buffer(dg_w: usize, dg_h: usize) -> Vec<u8>
{
    unsafe
    {
        let ptr = room::doom::doomgeneric::DG_ScreenBuffer as *const u8;
        if ptr.is_null()
        {
            return Vec::new();
        }
        std::slice::from_raw_parts(ptr, dg_w * dg_h * 4).to_vec()
    }
}

/// FNV-1a 64 over the presented bytes (same hash the host probe uses).
fn fnv1a(bytes: &[u8]) -> u64
{
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes
    {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 16x16-tile diff-density map ('#' = more than 25% of the tile's pixels
/// differ in any BGRA byte); one pixel differs when any byte differs.
fn diff_tile_map(frame_on: &[u8], frame_off: &[u8], dg_w: usize, dg_h: usize) -> String
{
    const TILE: usize = 16;
    let stride = dg_w * 4;
    let cols = dg_w / TILE;
    let rows = dg_h / TILE;
    let mut map = String::with_capacity(rows * (cols + 1));
    for ty in 0..rows
    {
        for tx in 0..cols
        {
            let mut diff = 0usize;
            for y in ty * TILE..(ty + 1) * TILE
            {
                for x in tx * TILE..(tx + 1) * TILE
                {
                    let o = y * stride + x * 4;
                    if frame_on[o..o + 4] != frame_off[o..o + 4]
                    {
                        diff += 1;
                    }
                }
            }
            map.push(if diff > TILE * TILE / 4 { '#' } else { '.' });
        }
        map.push('\n');
    }
    map
}

/// Walk every sector's thing list and measure how far the interpolation
/// board's sampled position sits from the live simulation position for each
/// mobj. Returns (total mobjs, count displaced > 2 map units, max |delta|
/// in fixed units). This is the direct stale-slot aliasing detector: a
/// healthy board samples within one tic of motion of live; an aliased slot
/// throws the sample across the map.
///
/// # Safety
/// Walks the live sector/thing structures; single-threaded main-thread
/// contract applies.
unsafe fn census_mobj_deltas(
    sec_base: *mut room::doom::c_ffi::sector_t,
    sec_count: i32,
) -> (usize, usize, i64)
{
    let mut total = 0usize;
    let mut moved = 0usize;
    let mut max_delta: i64 = 0;
    for si in 0..sec_count as usize
    {
        let mut thing = (*sec_base.add(si)).thinglist as *mut room::doom::c_ffi::mobj_t;
        while !thing.is_null()
        {
            let live_x = (*thing).x;
            let live_y = (*thing).y;
            let s = room::doom::r_interp::sample_mobj(thing);
            let dx = (s.x - live_x) as i64;
            let dy = (s.y - live_y) as i64;
            let dist2 = dx * dx + dy * dy;
            if dist2 > 4 * 65536 * 65536
            {
                moved += 1;
            }
            let dist = (dist2 as f64).sqrt() as i64;
            if dist > max_delta
            {
                max_delta = dist;
            }
            total += 1;
            thing = (*thing).snext as *mut room::doom::c_ffi::mobj_t;
        }
    }
    (total, moved, max_delta)
}

#[cfg(test)]
mod tests
{
    #[test]
    fn last_error_export_is_none_on_healthy_process()
    {
        // Host-safe: this export is a plain read of the room-side
        // diagnostics static (no JS import involved), so it runs natively.
        // Pins the harness contract: a healthy run must end with null.
        assert_eq!(super::woom24_last_error(), None, "no I_Error fires in a test process");
    }
}
