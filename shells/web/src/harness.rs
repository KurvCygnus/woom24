//! F9 e2e harness exports: canonical frame/state digests plus the interp
//! toggle, consumed by the Node scenario runner (F9 Task 3).
//!
//! Compiled only under the `harness` feature: a production wasm build
//! excludes this module entirely, so the generated JS glue carries zero
//! `harness_` symbols. Hash semantics live in `room::doom::harness_hash`;
//! this module only bridges the wasm boundary. Same single-threaded
//! main-thread export contract as every other export in this shell.

use wasm_bindgen::prelude::*;

/// Frame-ledger digest over the live doomgeneric present buffer: FNV-1a over
/// the `width * height * 4` BGRA bytes the renderer just drew. A null or
/// zero-size buffer (engine not yet booted) hashes as `0`, which the ledger
/// treats as "no frame" - this path never panics (wasm-reachable).
#[wasm_bindgen]
pub fn harness_frame_hash() -> u64 { harness_frame_hash_internal() }

/// Plain body of [`harness_frame_hash`], split out so the null-buffer
/// contract is testable on host without booting the engine.
pub fn harness_frame_hash_internal() -> u64
{
    // SAFETY: by-value read of the doomgeneric buffer pointer (no reference
    // is taken into the `static mut`), under the shell's single-threaded
    // main-thread export contract - the same regime dg.rs's DG_DrawFrame
    // reads the buffer under.
    let buf = unsafe { room::doom::doomgeneric::DG_ScreenBuffer };
    if buf.is_null()
    {
        return 0;
    }
    let (w, h) = room::doom::doomgeneric::dg_res();
    let Some(byte_len) = w.checked_mul(h).and_then(|px| px.checked_mul(4))
    else
    {
        return 0;
    };
    // SAFETY: `buf` is non-null and the engine allocates exactly
    // `width * height` u32 pixels for it (`dg_res` reports the same pair that
    // sized the allocation), so `byte_len` bytes are readable through it -
    // the same window DG_DrawFrame presents.
    let bytes = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), byte_len) };
    room::doom::harness_hash::frame_hash(bytes)
}

/// Canonical engine-state digest (`room::doom::harness_hash::state_hash`):
/// the 72-byte ledger field set behind the F9 anchors.
#[wasm_bindgen]
pub fn harness_state_hash() -> u64 { room::doom::harness_hash::state_hash() }

/// Load-comparable state digest (`room::doom::harness_hash::state_hash_load`):
/// the same 72-byte ledger with the three words a savegame does not carry
/// (gametic, rndindex, prndindex) zeroed. Consumed by the runner's
/// `expect_state_at_load` step for the save_load_roundtrip flow -- the full
/// `state_hash` can never agree across a load because the loop tic counter
/// keeps counting and `G_InitNew` resets the RNG indices.
#[wasm_bindgen]
pub fn harness_state_hash_load() -> u64 { room::doom::harness_hash::state_hash_load() }

/// F9 audio-health snapshot (spec §4) as a JSON object string:
/// `{"underruns":u32,"scheduled_seconds":f64,"music_active":bool,"voices":u32}`.
/// Serialization and the no-backend (all-zero) contract live in
/// `web_audio::audio_stats_json`; the harness only bridges the boundary,
/// same as the hash exports above.
#[wasm_bindgen]
pub fn harness_audio_stats() -> String { crate::web_audio::audio_stats_json() }

/// Node-runner toggle for the render interpolator's master gate (F7 owns the
/// user-facing path; the scenario runner flips it to pin frame-exact output).
#[wasm_bindgen]
pub fn harness_set_interp_enabled(on: u32) { room::doom::r_interp::set_enabled(on != 0); }

/// Live present resolution as `(width << 16) | height`, for the runner's
/// `--dump-frame` PPM writer. Returns `0` before the buffer exists.
#[wasm_bindgen]
pub fn harness_screen_res() -> u32
{
    let (w, h) = room::doom::doomgeneric::dg_res();
    ((w as u32) << 16) | (h as u32)
}

/// Copy of the live doomgeneric present buffer, `width * height * 4` bytes in
/// DG's BGRA byte order - the raw material for the runner's `--dump-frame`
/// PPM debug dumps (defect-C evidence). Empty when the buffer does not exist
/// (engine not yet booted); this path never panics (wasm-reachable).
#[wasm_bindgen]
pub fn harness_screen_bytes() -> Vec<u8>
{
    // SAFETY: by-value read of the doomgeneric buffer pointer (no reference
    // is taken into the `static mut`), under the shell's single-threaded
    // main-thread export contract - the same regime dg.rs's DG_DrawFrame
    // reads the buffer under.
    let buf = unsafe { room::doom::doomgeneric::DG_ScreenBuffer };
    if buf.is_null()
    {
        return Vec::new();
    }
    let (w, h) = room::doom::doomgeneric::dg_res();
    let Some(byte_len) = w.checked_mul(h).and_then(|px| px.checked_mul(4))
    else
    {
        return Vec::new();
    };
    // SAFETY: `buf` is non-null and the engine allocates exactly
    // `width * height` u32 pixels for it (`dg_res` reports the same pair that
    // sized the allocation), so `byte_len` bytes are readable through it -
    // the same window DG_DrawFrame presents.
    unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), byte_len) }.to_vec()
}

#[cfg(test)]
mod tests
{
    // On host, DG_ScreenBuffer is null until an engine boots; the export must
    // not panic - define and pin the null-buffer contract here:
    #[test]
    fn frame_hash_before_boot_is_zero_not_panic()
    {
        assert_eq!(super::harness_frame_hash_internal(), 0);
    }
}
