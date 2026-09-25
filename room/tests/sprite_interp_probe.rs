//! Spec-4 Fix C diagnosis -- sprite-invisibility classification probe.
//!
//! The browser defect under diagnosis: enemies and decoration sprites did
//! not render during real gameplay while walls/flats stayed normal. The
//! prime suspect surface is the F1 M1 interpolation sampling that
//! `R_ProjectSprite` consults (`r_things.rs` reads `r_interp::sample_mobj`
//! for sprite placement). This probe is the Task 4 deliverable: it produces
//! the classification evidence (C-i not drawn / C-ii displaced / C-iii
//! wrong scale / not reproduced on host) WITHOUT fixing anything.
//!
//! Methodology (single boot, single drive, surgical re-presents):
//!
//! 1. Drive the standard exact-cadence harness with sampling enabled (the
//!    as-shipped configuration) and snapshot the drive's own final present --
//!    the exact `video_raster.rs` anchor methodology, asserted against the
//!    committed ON golden.
//! 2. Re-present the SAME final frame with `r_interp` sampling disabled:
//!    `begin_frame` + `set_fraction` + `D_Display` directly (never
//!    `doomgeneric_frame`, whose clock math could pump tics or resample the
//!    fraction). Zero tics are pumped, so simulation state, view and
//!    fraction are unchanged and only sprite/camera sampling differs.
//!    Fraction-independence of the OFF render makes 4259 (the blessed
//!    anchor fraction) as good as any.
//! 3. Re-present OFF a second time and require byte-identity -- proves the
//!    re-present is idempotent (no present-order render state such as fuzz
//!    positions leaks between presents).
//! 4. Print the 16x16-tile ASCII diff-density map of ON vs OFF plus the
//!    overall differing-pixel percentage and the five densest tiles.
//!
//! The OFF reference below needs an explanation. Task 4 brief step 1
//! expected the dc6b336 re-bless addendum value `0x25b8a31010313575`
//! ("with sampling disabled the pre- and post-latch trees render the
//! byte-identical frame"). That value does NOT reproduce -- and not because
//! of tree drift: it does not reproduce AT dc6b336 either. The measured
//! matrix on this harness (task-4 session, both commits checked out):
//!
//! - interp-ON drive present, fraction 4259 -> `0x841405eea75ee285` at HEAD
//!   and at dc6b336 (the committed golden reproduces byte-exactly);
//! - interp-ON re-present, fraction 2949 (the pre-latch fraction) ->
//!   `0xe1afbd39ea64da95` -- exactly the superseded pre-latch golden that
//!   `video_raster.rs` documents, so the harness is the blessed methodology;
//! - sampling-OFF (full-OFF drive and surgical re-present agree) ->
//!   `0xf3f8bc0c69cf6ca5`, fraction-independent (4259 / 2949 / 0 all
//!   identical), identical at HEAD and at dc6b336.
//!
//! The addendum value is therefore a stale artifact of a stripped temporary
//! harness (the dc6b336 evidence test no longer exists and cites an
//! unreachable task-2 report), not a render input this tree lost. Existing
//! goldens are untouched -- zero re-blesses; this probe records its own
//! cross-validated OFF value instead of the unreproducible addendum one.
//!
//! Like the other frame/drive harness binaries, this test boots the full
//! engine and touches process-global renderer state, so it holds the
//! shared serial lock and must not run concurrently with anything.

#![allow(non_snake_case, non_upper_case_globals)] // ! Reason: c2rust-mirror names + golden-struct field style -- the harness reads the ported engine's original C identifiers (same justification as the sibling harness binaries).

use std::sync::{Mutex, MutexGuard};

mod frame_split_common;

use frame_split_common::boot;

//* The engine's renderer state is a pile of process-global `static mut`s;
//* the drive and the re-presents below share them, so the serial lock is
//* mandatory (same discipline as video_raster.rs).
static SERIAL: Mutex<()> = Mutex::new(());

fn take_serial() -> MutexGuard<'static, ()>
{
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// FNV-1a 64-bit over the presented BGRA bytes. Hand-rolled (instead of
/// `std::hash::DefaultHasher`) so the reference hashes stay stable across
/// std versions -- same hash the video_raster anchor uses.
fn hash_frame(bytes: &[u8]) -> u64
{
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes
    {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// The committed blessed interp-ON anchor (`video_raster.rs`
/// GOLDEN_DEFAULT_640X400): the standard drive's final present at
/// interpolation fraction 4259/65536 with sampling enabled.
const GOLDEN_INTERP_ON: u64 = 0x841405eea75ee285;

//? DIAGNOSTIC(spec-4): this probe's own cross-validated sampling-OFF
//? reference: full-OFF drive and surgical re-present agree on it, it is
//? fraction-independent, and it is identical at HEAD and at dc6b336. The
//? dc6b336 addendum value 0x25b8a31010313575 is unreproducible even at its
//? own commit -- see the module doc and the task-4 report. GRADUATED (Task 5
//? decision): sprite_apply_regression.rs asserts this same reference as its
//? own harness-alignment check, so it stays.
const PROBE_INTERP_OFF: u64 = 0xf3f8bc0c69cf6ca5;

/// Tile side of the diff-density map. 16x16-pixel tiles over the 640x400
/// present buffer produce the 40-column x 25-row map the brief specifies.
const TILE: usize = 16;

/// Snapshot the doomgeneric present buffer (640x400x4 BGRA bytes).
fn read_present() -> Vec<u8>
{
    let (dg_w, dg_h) = room::doom::doomgeneric::dg_res();
    assert_eq!(
        (dg_w, dg_h),
        (640, 400),
        "probe expects the default 640x400 doomgeneric present"
    );
    unsafe
    {
        // SAFETY: DG_ScreenBuffer is the engine's process-lifetime
        // framebuffer, allocated by boot() and stable at the fixed 640x400
        // present (asserted non-null), so the pointer stays valid for the
        // full dg_w*dg_h*4 read below; single-threaded main-thread contract
        // applies (serial lock held).
        let ptr = room::doom::doomgeneric::DG_ScreenBuffer as *const u8;
        assert!(!ptr.is_null(), "DG_ScreenBuffer must be allocated after boot");
        std::slice::from_raw_parts(ptr, dg_w * dg_h * 4).to_vec()
    }
}

/// Re-present the current frame through `D_Display` with the interpolation
/// board armed at a fixed fraction and sampling explicitly set. Never pumps
/// tics and never reads the wall clock, so the simulation state, the view
/// and the fraction are exactly the caller's choice.
fn surgical_represent(enabled: bool, fraction: i32) -> Vec<u8>
{
    room::doom::r_interp::set_enabled(enabled);
    room::doom::r_interp::begin_frame(frame_split_common::clock());
    room::doom::r_interp::set_fraction(fraction);
    room::doom::d_main::D_Display();
    read_present()
}

/// Print the 25x40 ASCII diff-density map, the overall differing-pixel
/// percentage, and the per-tile counts for the five densest tiles.
///
/// A pixel differs when any of its 4 BGRA bytes differs; a tile prints '#'
/// when more than 25% of its 256 pixels differ, '.' otherwise.
fn print_diff_map(frame_on: &[u8], frame_off: &[u8], dg_w: usize, dg_h: usize)
{
    let stride = dg_w * 4;
    let cols = dg_w / TILE;
    let rows = dg_h / TILE;
    let mut tile_counts = vec![0usize; cols * rows];
    let mut diff_pixels = 0usize;

    for y in 0..dg_h
    {
        for x in 0..dg_w
        {
            let o = y * stride + x * 4;
            let differs = frame_on[o] != frame_off[o]
                || frame_on[o + 1] != frame_off[o + 1]
                || frame_on[o + 2] != frame_off[o + 2]
                || frame_on[o + 3] != frame_off[o + 3];
            if differs
            {
                diff_pixels += 1;
                tile_counts[(y / TILE) * cols + (x / TILE)] += 1;
            }
        }
    }

    let total = dg_w * dg_h;
    println!();
    println!(
        "diff: {diff_pixels}/{total} pixels ({:.4}%) differ between interp-ON and interp-OFF",
        diff_pixels as f64 * 100.0 / total as f64
    );

    println!("16x16-tile diff-density map ('#' = more than 25% of the tile's pixels differ):");
    for ty in 0..rows
    {
        let mut row = String::with_capacity(cols);
        for tx in 0..cols
        {
            let c = tile_counts[ty * cols + tx];
            row.push(if c > TILE * TILE / 4 { '#' } else { '.' });
        }
        println!("{row}");
    }

    let mut ranked: Vec<(usize, usize, usize)> = tile_counts
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, i % cols, i / cols))
        .collect();
    ranked.sort_by_key(|e| std::cmp::Reverse(e.0));
    println!("five densest tiles (tile_x, tile_y, differing pixels of 256):");
    for (c, tx, ty) in ranked.into_iter().take(5)
    {
        println!("  tile ({tx:2},{ty:2}): {c:3}/256 pixels differ");
    }

    //? DIAGNOSTIC(spec-4): temporary deep-dive evidence -- strip or graduate
    //? in the C fix. Channel histogram, 16-row band profile, sample pixels.
    let mut channel = [0usize; 4];
    for y in 0..dg_h
    {
        for x in 0..dg_w
        {
            let o = y * stride + x * 4;
            for ch in 0..4
            {
                if frame_on[o + ch] != frame_off[o + ch]
                {
                    channel[ch] += 1;
                }
            }
        }
    }
    println!("per-channel differing pixels [B, G, R, A] = {channel:?}");

    let bands = dg_h / 16;
    let mut band_counts = vec![0usize; bands];
    for y in 0..dg_h
    {
        for x in 0..dg_w
        {
            let o = y * stride + x * 4;
            if frame_on[o..o + 4] != frame_off[o..o + 4]
            {
                band_counts[y / 16] += 1;
            }
        }
    }
    println!("per-16-row-band differing pixel counts (25 bands, top to bottom):");
    for (i, c) in band_counts.iter().enumerate()
    {
        println!("  band {i:2} (y {:4}..{:4}): {c}", i * 16, i * 16 + 15);
    }

    for sy in [60usize, 150, 250, 340]
    {
        print!("scanline y={sy:3} ON :");
        for sx in (0..dg_w).step_by(80)
        {
            let o = sy * stride + sx * 4;
            print!(" {:02x}{:02x}{:02x}{:02x}", frame_on[o], frame_on[o + 1], frame_on[o + 2], frame_on[o + 3]);
        }
        println!();
        print!("scanline y={sy:3} OFF:");
        for sx in (0..dg_w).step_by(80)
        {
            let o = sy * stride + sx * 4;
            print!(" {:02x}{:02x}{:02x}{:02x}", frame_off[o], frame_off[o + 1], frame_off[o + 2], frame_off[o + 3]);
        }
        println!();
    }
}

/// The classification probe: ON capture from the drive's final present
/// (anchor methodology), OFF capture from surgical re-presents of the same
/// frame, then the diff map.
#[test]
fn probe_interp_on_vs_off_sprite_classification()
{
    let _g = take_serial();
    boot();

    // Capture 1 (interp ON, as-shipped): the drive's own final present --
    // the exact video_raster anchor methodology. The simulation red line
    // must hold inside this binary too.
    let got = frame_split_common::run(false);
    frame_split_common::assert_matches_expected(&got);
    let (dg_w, dg_h) = room::doom::doomgeneric::dg_res();
    let frame_on = read_present();
    let hash_on = hash_frame(&frame_on);

    // Capture 2 (interp OFF, live values): surgical re-present of the SAME
    // final frame at the blessed anchor fraction. The OFF-render comparison
    // relies on measured fraction-independence (4259 / 2949 / 0 all render
    // identical -- the probe's own module doc records the matrix); every
    // sampler short-circuits on enabled() before reading the fraction, so
    // the fraction choice is inert here.
    let frame_off = surgical_represent(false, 4259);
    let hash_off = hash_frame(&frame_off);

    // Capture 3: a second OFF re-present must be byte-identical -- the
    // re-present is idempotent, i.e. no present-order render state (fuzz
    // positions, wipe buffers, border latches) leaks between presents.
    let frame_off_again = surgical_represent(false, 4259);
    assert_eq!(
        frame_off, frame_off_again,
        "OFF re-present must be idempotent (present-order render state leaked)"
    );

    println!();
    println!(
        "gametic {} / interpolation fraction {} / clock {}",
        got.gametic,
        room::doom::r_interp::fraction(),
        frame_split_common::clock()
    );
    println!("hash_on  (interp enabled,  drive present)    = {hash_on:#018x}");
    println!("hash_off (interp disabled, surgical re-present) = {hash_off:#018x}");

    // Harness alignment: the ON frame must reproduce the committed blessed
    // anchor exactly (drive methodology identical to video_raster.rs).
    assert_eq!(
        hash_on, GOLDEN_INTERP_ON,
        "interp-ON frame must match the committed video_raster anchor ({hash_on:#018x})"
    );

    // Harness alignment: the OFF frame must match the cross-validated
    // sampling-OFF reference (full-OFF drive and surgical re-present agree;
    // fraction-independent; identical at HEAD and dc6b336). The dc6b336
    // addendum value 0x25b8a31010313575 is unreproducible even at its own
    // commit -- see the module doc; no committed golden was touched.
    assert_eq!(
        hash_off, PROBE_INTERP_OFF,
        "interp-OFF frame must match the probe's cross-validated reference ({hash_off:#018x})"
    );

    print_diff_map(&frame_on, &frame_off, dg_w, dg_h);

    //? DIAGNOSTIC(spec-4): displacement correlation -- does the ON view look
    //? like the OFF view shifted by (dx, dy)? Sparse global search first.
    correlation_profile(&frame_on, &frame_off, dg_w, dg_h);

    //? DIAGNOSTIC(spec-4): board census -- sampled vs live values for the
    //? camera, every mobj on the thinker lists and every sector, exactly
    //? what the final ON present rendered (fraction 4259 armed).
    board_census();
}

/// Sparse whole-frame 2D correlation: for candidate (dx, dy) displacements,
/// how many sampled pixels of the ON frame equal the OFF frame's displaced
/// pixel? Prints the top matches -- a strong uniform peak means the ON view
/// is a displaced copy of the OFF view.
fn correlation_profile(frame_on: &[u8], frame_off: &[u8], dg_w: usize, dg_h: usize)
{
    let stride = dg_w * 4;
    let mut best: Vec<(usize, usize, i32, i32)> = Vec::new();
    for dy in [-16i32, -8, 0, 8, 16]
    {
        for dx in (-160..=160).step_by(2)
        {
            let mut matched = 0usize;
            let mut tried = 0usize;
            let mut y = 0usize;
            while y < dg_h
            {
                let sy = y as i32 + dy;
                if sy < 0 || sy >= dg_h as i32
                {
                    y += 8;
                    continue;
                }
                let mut x = 0usize;
                while x < dg_w
                {
                    let sx = x as i32 + dx;
                    if sx < 0 || sx >= dg_w as i32
                    {
                        x += 8;
                        continue;
                    }
                    let o = y * stride + x * 4;
                    let p = (sy as usize) * stride + (sx as usize) * 4;
                    if frame_on[o..o + 4] == frame_off[p..p + 4]
                    {
                        matched += 1;
                    }
                    tried += 1;
                    x += 8;
                }
                y += 8;
            }
            best.push((matched, tried, dx, dy));
        }
    }
    best.sort_by_key(|e| std::cmp::Reverse(e.0));
    println!("correlation top-5 (matched of sampled, dx, dy present px):");
    for (m, tried, dx, dy) in best.into_iter().take(5)
    {
        println!("  dx={dx:4} dy={dy:3}: {m} of {tried}");
    }
}

/// Compare every board-sampled value against its live simulation value at
/// the final frame's state (fraction still armed at 4259): the player
/// camera, every mobj on every sector's thing list, every sector's floor
/// and ceiling. Deltas are printed in fixed-point map units; anything far
/// from live is what the ON frame rendered differently.
fn board_census()
{
    use room::doom::c_ffi;
    use room::doom::d_player::{consoleplayer, players};
    use room::doom::p_setup::{numsectors, sectors};
    use room::doom::p_tick::leveltime;

    unsafe
    {
        // SAFETY: reads the engine's process-global statics (players,
        // sectors, leveltime) and walks the live sector/thing lists -- all
        // process-lifetime state after boot(); single-threaded
        // main-thread-only contract applies (serial lock held by the
        // caller), mirroring the wasm shell's census helpers.
        // The last OFF re-present left the gate closed; the census must
        // sample through the board, so re-open it (render-only state).
        room::doom::r_interp::set_enabled(true);
        let lt = *std::ptr::addr_of!(leveltime);
        println!("census: leveltime={} board_active={} fraction={}",
            lt,
            room::doom::r_interp::board_active(),
            room::doom::r_interp::fraction()
        );

        // -- camera ---------------------------------------------------------
        let cp = consoleplayer;
        let mo = players[cp as usize].mo as *mut c_ffi::mobj_t;
        if !mo.is_null()
        {
            let live_x = (*mo).x;
            let live_y = (*mo).y;
            let live_a = (*mo).angle;
            let s = room::doom::r_interp::sample_camera(
                &raw mut players[cp as usize],
                mo,
            );
            println!(
                "camera: live=({},{}) sample=({},{}) dx={} dy={} live_deg={} sample_deg={}",
                live_x, live_y, s.x, s.y,
                s.x - live_x, s.y - live_y,
                (live_a as f64) * 360.0 / 4294967296.0,
                (s.angle as f64) * 360.0 / 4294967296.0
            );
        }

        // -- every mobj on every sector thing list ---------------------------
        let sec_base = *std::ptr::addr_of!(sectors);
        let sec_count = *std::ptr::addr_of!(numsectors) as usize;
        let mut total = 0usize;
        let mut moved = 0usize;
        let mut worst: [(i64, i32, i32, i32, u32); 8] = [(0, 0, 0, 0, 0); 8];
        for si in 0..sec_count
        {
            let mut thing = (*sec_base.add(si)).thinglist as *mut c_ffi::mobj_t;
            while !thing.is_null()
            {
                let live_x = (*thing).x as i64;
                let live_y = (*thing).y as i64;
                let live_z = (*thing).z as i64;
                let s = room::doom::r_interp::sample_mobj(thing);
                let dx = s.x as i64 - live_x;
                let dy = s.y as i64 - live_y;
                let dz = s.z as i64 - live_z;
                let dist2 = dx * dx + dy * dy;
                total += 1;
                if dist2 > (2 * 65536) * (2 * 65536)
                {
                    moved += 1;
                }
                // Keep the 8 largest displacements (insertion into a small
                // sorted-by-magnitude array).
                let rank = worst.iter().position(|w| dist2 > w.0).unwrap_or(8);
                if rank < 8
                {
                    let mut k = 7;
                    while k > rank
                    {
                        worst[k] = worst[k - 1];
                        k -= 1;
                    }
                    worst[rank] = (dist2, dx as i32, dy as i32, dz as i32, (*thing).sprite as u32);
                }
                thing = (*thing).snext as *mut c_ffi::mobj_t;
            }
        }
        println!("mobjs: {total} total, {moved} sampled >2 map units from live");
        for (dist2, dx, dy, dz, sprite) in worst
        {
            if dist2 > 0
            {
                println!(
                    "  sprite#{sprite}: dx={dx} dy={dy} dz={dz} (map units: {:.2},{:.2},{:.2})",
                    dx as f64 / 65536.0, dy as f64 / 65536.0, dz as f64 / 65536.0
                );
            }
        }

        // -- sectors ----------------------------------------------------------
        let mut sec_moved = 0usize;
        let mut sec_worst: (i64, i64, i64) = (0, 0, 0);
        for si in 0..sec_count
        {
            let sec = sec_base.add(si);
            let sf = room::doom::r_interp::sector_floor(sec) as i64;
            let sc = room::doom::r_interp::sector_ceiling(sec) as i64;
            let lf = (*sec).floorheight as i64;
            let lc = (*sec).ceilingheight as i64;
            let df = (sf - lf).abs();
            let dc = (sc - lc).abs();
            if df > 0 || dc > 0
            {
                sec_moved += 1;
                if df > sec_worst.0
                {
                    sec_worst = (df, sf - lf, sc - lc);
                }
            }
        }
        println!(
            "sectors: {sec_count} total, {sec_moved} with sampled!=live, worst floor delta {} ({:.2} units)",
            sec_worst.0,
            sec_worst.1 as f64 / 65536.0
        );
    }
}
