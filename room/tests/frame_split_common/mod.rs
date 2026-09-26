//! Shared harness for the F1 M1 frame/pump-split determinism binaries.
//!
//! Two integration-test binaries (`frame_split_exact`, `frame_split_jittered`)
//! boot the same engine, drive it through `doomgeneric_frame` with a fully
//! controlled clock — one at an exact 35 Hz cadence, one with jitter, skipped
//! frames, and a simulated tab-suspend — and must land on identical final
//! simulation state at the same gametic. This is the spec's red-line proof
//! that render timing (the frame pump cap and the interpolation fraction)
//! never leaks into the simulation.
//!
//! `mod.rs` inside `tests/frame_split_common/` is not its own test target; it
//! is included by each binary via `mod frame_split_common;`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::{c_char, c_int, CString};
use std::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Controllable engine clock (the only time source the engine sees)
// ---------------------------------------------------------------------------

/// Monotone fake timeline in milliseconds. Fully deterministic: the only
/// things that advance it are the harness's per-frame cadence steps and the
/// engine's own `I_Sleep` requests (sleep = time travel on a fake clock).
/// Vanilla's tic-build protocol (`LASTTIME`/`newtics` in `NetUpdate`) paces
/// tic creation to newly elapsed clock time, so a no-op sleep would deadlock
/// its stall loop on a frozen clock; advancing the timeline per sleep keeps
/// the stall paths (engine boot, wipe screen) progressing deterministically.
static CLOCK_MS: AtomicU32 = AtomicU32::new(1000);

pub fn set_clock(ms: u32)
{
    let cur = CLOCK_MS.load(Ordering::SeqCst);
    if ms > cur
    {
        CLOCK_MS.store(ms, Ordering::SeqCst);
    }
}

pub fn clock() -> u32 { CLOCK_MS.load(Ordering::SeqCst) }

// ---------------------------------------------------------------------------
// DG_* stubs (C-callable; replaces the platform layer in these binaries)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn DG_Init() {}

#[no_mangle]
pub extern "C" fn DG_DrawFrame() {}

#[no_mangle]
pub extern "C" fn DG_SleepMs(ms: u32)
{
    // Sleep = advance the fake timeline; the engine cannot tell the
    // difference, and the harness stays deterministic.
    set_clock(clock().saturating_add(ms));
}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 { clock() }

#[no_mangle]
pub extern "C" fn DG_GetKey(_pressed: *mut i32, _doom_key: *mut u8) -> i32 { 0 }

#[no_mangle]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}

// ---------------------------------------------------------------------------
// Simulation-state snapshot (the red-line comparison surface)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FinalState
{
    pub gametic: c_int,
    pub leveltime: c_int,
    pub gamestate: c_int,
    pub rndindex: c_int,
    pub prndindex: c_int,
    pub health: c_int,
    pub armorpoints: c_int,
    pub killcount: c_int,
    pub readyweapon: c_int,
    pub ammo: [c_int; 4],
    pub mo_x: c_int,
    pub mo_y: c_int,
    pub mo_z: c_int,
    pub mo_angle: u32,
}

impl FinalState
{
    #[allow(dead_code)] // BLESS-mode helper; used only in BLESS runs
    pub fn print_bless(&self)
    {
        println!("FinalState {{");
        println!("    gametic: {},", self.gametic);
        println!("    leveltime: {},", self.leveltime);
        println!("    gamestate: {},", self.gamestate);
        println!("    rndindex: {},", self.rndindex);
        println!("    prndindex: {},", self.prndindex);
        println!("    health: {},", self.health);
        println!("    armorpoints: {},", self.armorpoints);
        println!("    killcount: {},", self.killcount);
        println!("    readyweapon: {},", self.readyweapon);
        println!("    ammo: {:?},", self.ammo);
        println!("    mo_x: {},", self.mo_x);
        println!("    mo_y: {},", self.mo_y);
        println!("    mo_z: {},", self.mo_z);
        println!("    mo_angle: {},", self.mo_angle);
        println!("}};");
    }
}

/// The expected final state at `TARGET_TICS`. Both binaries must land here;
/// paste a BLESS run's output over the values.
pub static EXPECTED: FinalState = FinalState {
    gametic: 1200,
    leveltime: 1029,
    gamestate: 0,
    rndindex: 69,
    prndindex: 122,
    health: 65,
    armorpoints: 85,
    killcount: 11,
    readyweapon: 2,
    ammo: [39, 8, 0, 0],
    mo_x: 70314354,
    mo_y: 60036173,
    mo_z: 3670016,
    mo_angle: 2248146944,
};

/// How long the controlled cadences run before the 1 ms landing crawl.
pub const TARGET_TICS: c_int = 1200;

// ---------------------------------------------------------------------------
// Boot + drive
// ---------------------------------------------------------------------------

extern "C"
{
    static gametic: c_int;
    static gamestate: c_int;
    static mut prndindex: c_int;
}

fn capture() -> FinalState
{
    use room::doom::d_player::{consoleplayer, players, MAXPLAYERS};

    let cp = unsafe { consoleplayer };
    let pidx = if cp >= 0 && (cp as usize) < MAXPLAYERS { cp as usize } else { 0 };
    let p = unsafe { &players[pidx] };
    let mo_ptr = p.mo as *const room::doom::c_ffi::mobj_t;
    let (mo_x, mo_y, mo_z, mo_angle) = if mo_ptr.is_null()
    {
        (0, 0, 0, 0)
    }
    else
    {
        let mo = unsafe { &*mo_ptr };
        (mo.x, mo.y, mo.z, mo.angle)
    };

    FinalState
    {
        gametic: unsafe { gametic },
        leveltime: unsafe { room::doom::p_tick::leveltime },
        gamestate: unsafe { gamestate },
        rndindex: unsafe { room::doom::m_random::rndindex },
        prndindex: unsafe { prndindex },
        health: p.health,
        armorpoints: p.armorpoints,
        killcount: p.killcount,
        readyweapon: p.readyweapon,
        ammo: p.ammo,
        mo_x,
        mo_y,
        mo_z,
        mo_angle,
    }
}

/// Boot the engine headlessly (same shape as the demo_playthrough harness).
//* Idempotent on purpose: [`doomgeneric_Create`] must run exactly once per
//* process (a second run re-enters `D_DoomMain` on a live engine, pumping a
//* few tics and leaving a hybrid state), yet each test in a binary calls
//* `boot`. Which test wins that race is thread-scheduling luck, and the
//* loser's re-boot silently shifted the demo golden's pre-level tic count
//* (video_raster anchor, ~1-in-8 runs). The guard pins first-boot wins.
pub fn boot()
{
    use std::sync::Once;
    static BOOT: Once = Once::new();
    BOOT.call_once(boot_once);
}

fn boot_once()
{
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let wad_path = manifest
        .join("../doom1.wad")
        .canonicalize()
        .unwrap_or_else(|_| manifest.parent().unwrap().join("doom1.wad"));
    assert!(wad_path.exists(), "doom1.wad not found at {}", wad_path.display());
    let wad_str = wad_path.to_str().expect("WAD path is not valid UTF-8");

    let argv: Vec<CString> = vec![
        CString::new("room").unwrap(),
        CString::new("-iwad").unwrap(),
        CString::new(wad_str).unwrap(),
        CString::new("-nomusic").unwrap(),
        CString::new("-nosound").unwrap(),
        CString::new("-nomouse").unwrap(),
        CString::new("-nojoy").unwrap(),
        CString::new("-nograbmouse").unwrap(),
    ];
    let mut c_argv: Vec<*mut c_char> = argv.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    c_argv.push(std::ptr::null_mut());

    // SAFETY: exactly-once by the BOOT guard above. The argv backing store
    // is leaked because `doomgeneric_Create` keeps `myargv` pointing into it
    // for the lifetime of the process.
    let c_argv = Box::leak(c_argv.into_boxed_slice());
    std::mem::forget(argv);
    unsafe
    {
        doomgeneric_sys::doomgeneric_Create((c_argv.len() - 1) as c_int, c_argv.as_mut_ptr());
    }
}

/// xorshift32: deterministic jitter source for the jittered cadence.
pub struct Rng(u32);

impl Rng
{
    pub fn new(seed: u32) -> Self { Rng(seed) }

    pub fn next(&mut self) -> u32
    {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

/// Drive `doomgeneric_frame` to exactly `TARGET_TICS` gametics.
///
/// Phase 1 uses the requested cadence (exact ~28.571 ms steps, or jittered:
/// 0..29 ms jitter, an eighth of frames withheld so the next frame pumps two
/// tics, and a 5 s suspend every ~64 frames — the tab-suspend shape). Phase 2
/// crawls the clock by 1 ms frames so both runs land on exactly the same
/// gametic; each such frame pumps at most one tic, so the landing is precise
/// in both runs.
pub fn run(jittered: bool) -> FinalState
{
    let mut rng = Rng::new(0xC0FFEE);
    let mut k: u64 = 0;
    let mut num: u32 = 0;
    // gametic lives behind the C ABI; mirror it into a local so the loop
    // conditions satisfy Rust's mutation analysis.
    let mut g = unsafe { gametic };

    while g < TARGET_TICS - 16
    {
        // Bresenham step of 2000/70 ms = 28.571 (true 35 Hz cadence).
        num += 2000;
        let mut step = 0u32;
        while num >= 70
        {
            num -= 70;
            step += 1;
        }
        let frame_ms = if jittered
        {
            let mut ms = clock() + step + rng.next() % 30;
            if rng.next() & 7 == 0
            {
                ms += step; // withheld frame: next frame catches up
            }
            if rng.next() & 63 == 0
            {
                ms += 5000; // tab-suspend shape
            }
            ms
        }
        else
        {
            clock() + step
        };

        set_clock(frame_ms);
        room::doom::d_main::doomgeneric_frame(frame_ms);
        k += 1;
        g = unsafe { gametic };
        assert!(k < 100_000, "cadence run did not converge (frame {k})");
    }

    // Exact landing: pump capped to exactly one tic per frame and one extra
    // millisecond of timeline per frame, so gametic walks up to TARGET_TICS
    // in single steps in both runs (any suspend debt from the jittered
    // cadence is consumed at the same 1 tic per frame).
    unsafe
    {
        room::doom::d_loop::pump_tic_cap = 1;
    }
    while g < TARGET_TICS
    {
        let frame_ms = clock() + 1;
        set_clock(frame_ms);
        room::doom::d_main::doomgeneric_frame(frame_ms);
        k += 1;
        g = unsafe { gametic };
        assert!(k < 200_000, "landing crawl did not converge (frame {k})");
    }
    unsafe
    {
        room::doom::d_loop::pump_tic_cap = room::doom::d_main::MAX_TICS_PER_FRAME;
    }

    let got = capture();
    assert_eq!(got.gametic, TARGET_TICS, "must land exactly on the target tic");
    got
}

/// Compare a run's final state against the shared EXPECTED constant.
pub fn assert_matches_expected(got: &FinalState)
{
    assert_eq!(got.gametic, EXPECTED.gametic, "gametic");
    assert_eq!(got.leveltime, EXPECTED.leveltime, "leveltime");
    assert_eq!(got.gamestate, EXPECTED.gamestate, "gamestate");
    assert_eq!(got.rndindex, EXPECTED.rndindex, "rndindex");
    assert_eq!(got.prndindex, EXPECTED.prndindex, "prndindex");
    assert_eq!(got.health, EXPECTED.health, "health");
    assert_eq!(got.armorpoints, EXPECTED.armorpoints, "armorpoints");
    assert_eq!(got.killcount, EXPECTED.killcount, "killcount");
    assert_eq!(got.readyweapon, EXPECTED.readyweapon, "readyweapon");
    assert_eq!(got.ammo, EXPECTED.ammo, "ammo");
    assert_eq!(got.mo_x, EXPECTED.mo_x, "mo_x");
    assert_eq!(got.mo_y, EXPECTED.mo_y, "mo_y");
    assert_eq!(got.mo_z, EXPECTED.mo_z, "mo_z");
    assert_eq!(got.mo_angle, EXPECTED.mo_angle, "mo_angle");
}
