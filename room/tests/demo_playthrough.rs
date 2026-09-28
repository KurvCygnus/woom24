//! Synthetic Demo Playthrough Test
//!
//! Drives the Doom engine headlessly for ~30 s of virtual time,
//! captures snapshots of player / RNG / game state at checkpoint tics,
//! and compares them against a frozen baseline.
//!
//! Usage:
//!   cargo test --test demo_playthrough        — compare vs BASELINE
//!   BLESS=1 cargo test --test demo_playthrough — print new BASELINE to stdout

#![allow(non_snake_case, non_upper_case_globals)]

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: room::dhat::Alloc = room::dhat::Alloc;

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, CString};
use std::mem::offset_of;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// headless:: TLS re-exports
// ---------------------------------------------------------------------------

thread_local! {
    static VIRTUAL_MS: Cell<u32> = const { Cell::new(1000) };
    static FRAMES: Cell<u64> = const { Cell::new(0) };
}

const TICK_MS: u32 = 1000 / 35;

fn virtual_ms() -> u32 {
    VIRTUAL_MS.with(|v| {
        let old = v.get();
        v.set(old + TICK_MS);
        old
    })
}

fn note_frame() {
    FRAMES.with(|f| f.set(f.get() + 1));
}

fn frame_count() -> u64 {
    FRAMES.with(|f| f.get())
}

// ---------------------------------------------------------------------------
// DG_* stubs (C-callable, replaces room/src/platform/mod.rs)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn DG_Init() {}

#[no_mangle]
pub extern "C" fn DG_DrawFrame() {
    note_frame();
}

#[no_mangle]
pub extern "C" fn DG_SleepMs(_ms: u32) {}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    virtual_ms()
}

#[no_mangle]
pub extern "C" fn DG_GetKey(_pressed: *mut i32, _doom_key: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}

// ---------------------------------------------------------------------------
// C global declarations
// ---------------------------------------------------------------------------

extern "C" {
    static gametic: c_int;
    static gamestate: c_int;
    static mut prndindex: c_int;
    static mut singletics: c_uint;
    static mut longtics: c_uint;
}

// ---------------------------------------------------------------------------
// MobjPrefix – repr-C mirror of the first fields of mobj_s
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct MobjPrefix {
    thinker_prev: *mut (),
    thinker_next: *mut (),
    thinker_fn: *mut (),
    x: i32,
    y: i32,
    z: i32,
    snext: *mut (),
    sprev: *mut (),
    angle: u32,
}

#[test]
fn mobj_prefix_offsets() {
    assert_eq!(offset_of!(MobjPrefix, x), 24, "thinker size mismatch");
    assert_eq!(offset_of!(MobjPrefix, y), 28);
    assert_eq!(offset_of!(MobjPrefix, z), 32);
    assert_eq!(offset_of!(MobjPrefix, angle), 56, "angle offset mismatch");
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Snapshot {
    tic: usize,
    virtual_ms: u32,
    frames: u64,
    gametic: c_int,
    gamestate: c_int,
    rndindex: c_int,
    prndindex: c_int,
    health: c_int,
    armorpoints: c_int,
    killcount: c_int,
    itemcount: c_int,
    secretcount: c_int,
    readyweapon: c_int,
    ammo: [c_int; 4],
    mo_x: i32,
    mo_y: i32,
    mo_z: i32,
    mo_angle: u32,
}

// ---------------------------------------------------------------------------
// BASELINE (paste BLESS output here, then re-run without BLESS)
// ---------------------------------------------------------------------------

const BASELINE: &[Snapshot] = &[
    Snapshot {
        tic: 35,
        virtual_ms: 2036,
        frames: 35,
        gametic: 36,
        gamestate: 3,
        rndindex: 0,
        prndindex: 0,
        health: 0,
        armorpoints: 0,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 0,
        ammo: [0, 0, 0, 0],
        mo_x: 0,
        mo_y: 0,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 70,
        virtual_ms: 3044,
        frames: 70,
        gametic: 71,
        gamestate: 3,
        rndindex: 0,
        prndindex: 0,
        health: 0,
        armorpoints: 0,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 0,
        ammo: [0, 0, 0, 0],
        mo_x: 0,
        mo_y: 0,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 140,
        virtual_ms: 5032,
        frames: 140,
        gametic: 141,
        gamestate: 3,
        rndindex: 0,
        prndindex: 0,
        health: 0,
        armorpoints: 0,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 0,
        ammo: [0, 0, 0, 0],
        mo_x: 0,
        mo_y: 0,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 280,
        virtual_ms: 10212,
        frames: 320,
        gametic: 281,
        gamestate: 0,
        rndindex: 174,
        prndindex: 53,
        health: 98,
        armorpoints: 99,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 1,
        ammo: [48, 0, 0, 0],
        mo_x: -19847754,
        mo_y: -9328776,
        mo_z: 0,
        mo_angle: 352321536,
    },
    Snapshot {
        tic: 560,
        virtual_ms: 18080,
        frames: 600,
        gametic: 561,
        gamestate: 0,
        rndindex: 198,
        prndindex: 226,
        health: 92,
        armorpoints: 99,
        killcount: 4,
        itemcount: 3,
        secretcount: 0,
        readyweapon: 1,
        ammo: [44, 0, 0, 0],
        mo_x: 20413025,
        mo_y: 9284681,
        mo_z: 0,
        mo_angle: 4278190080,
    },
    Snapshot {
        tic: 1050,
        virtual_ms: 31828,
        frames: 1090,
        gametic: 1051,
        gamestate: 0,
        rndindex: 176,
        prndindex: 53,
        health: 83,
        armorpoints: 94,
        killcount: 9,
        itemcount: 4,
        secretcount: 0,
        readyweapon: 2,
        ammo: [39, 10, 0, 0],
        mo_x: 33033067,
        mo_y: 34970087,
        mo_z: 3670016,
        mo_angle: 234881024,
    },
    Snapshot {
        tic: 1500,
        virtual_ms: 44456,
        frames: 1540,
        gametic: 1501,
        gamestate: 0,
        rndindex: 114,
        prndindex: 50,
        health: 90,
        armorpoints: 85,
        killcount: 15,
        itemcount: 4,
        secretcount: 0,
        readyweapon: 2,
        ammo: [49, 9, 0, 0],
        mo_x: 56903804,
        mo_y: 60287222,
        mo_z: 3670016,
        mo_angle: 4261412864,
    },
    Snapshot {
        tic: 1750,
        virtual_ms: 51484,
        frames: 1790,
        gametic: 1751,
        gamestate: 0,
        rndindex: 108,
        prndindex: 217,
        health: 100,
        armorpoints: 200,
        killcount: 15,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 4,
        ammo: [49, 9, 0, 7],
        mo_x: 60553099,
        mo_y: -2895518,
        mo_z: -1572864,
        mo_angle: 2113929216,
    },
    Snapshot {
        tic: 2000,
        virtual_ms: 58512,
        frames: 2040,
        gametic: 2001,
        gamestate: 0,
        rndindex: 102,
        prndindex: 152,
        health: 86,
        armorpoints: 188,
        killcount: 17,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 4,
        ammo: [54, 13, 0, 4],
        mo_x: -2001146,
        mo_y: 39680344,
        mo_z: -1572864,
        mo_angle: 1107296256,
    },
    Snapshot {
        tic: 2250,
        virtual_ms: 65540,
        frames: 2290,
        gametic: 2251,
        gamestate: 0,
        rndindex: 96,
        prndindex: 136,
        health: 13,
        armorpoints: 108,
        killcount: 24,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 4,
        ammo: [64, 13, 0, 1],
        mo_x: -37971708,
        mo_y: 54187776,
        mo_z: 0,
        mo_angle: 2130706432,
    },
    Snapshot {
        tic: 2500,
        virtual_ms: 72568,
        frames: 2540,
        gametic: 2501,
        gamestate: 0,
        rndindex: 90,
        prndindex: 153,
        health: 2,
        armorpoints: 98,
        killcount: 25,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 10, 0, 0],
        mo_x: -102829828,
        mo_y: 26562309,
        mo_z: -12058624,
        mo_angle: 419430400,
    },
    Snapshot {
        tic: 2750,
        virtual_ms: 79596,
        frames: 2790,
        gametic: 2751,
        gamestate: 0,
        rndindex: 84,
        prndindex: 218,
        health: 2,
        armorpoints: 98,
        killcount: 32,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 4, 0, 0],
        mo_x: -97222872,
        mo_y: 28998605,
        mo_z: -12058624,
        mo_angle: 4261412864,
    },
    Snapshot {
        tic: 2800,
        virtual_ms: 81024,
        frames: 2840,
        gametic: 2801,
        gamestate: 0,
        rndindex: 134,
        prndindex: 2,
        health: 2,
        armorpoints: 98,
        killcount: 33,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 19, 0, 0],
        mo_x: -83476903,
        mo_y: 22437029,
        mo_z: -11534336,
        mo_angle: 3405774848,
    },
    Snapshot {
        tic: 2850,
        virtual_ms: 82452,
        frames: 2890,
        gametic: 2851,
        gamestate: 0,
        rndindex: 184,
        prndindex: 177,
        health: 12,
        armorpoints: 98,
        killcount: 33,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 19, 0, 0],
        mo_x: -89408044,
        mo_y: 29036097,
        mo_z: -11534336,
        mo_angle: 2147483648,
    },
    Snapshot {
        tic: 2900,
        virtual_ms: 83880,
        frames: 2940,
        gametic: 2901,
        gamestate: 0,
        rndindex: 234,
        prndindex: 68,
        health: 12,
        armorpoints: 98,
        killcount: 33,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 19, 0, 0],
        mo_x: -109128119,
        mo_y: 36194724,
        mo_z: -12058624,
        mo_angle: 973078528,
    },
    Snapshot {
        tic: 2950,
        virtual_ms: 85308,
        frames: 2990,
        gametic: 2951,
        gamestate: 0,
        rndindex: 28,
        prndindex: 28,
        health: 6,
        armorpoints: 92,
        killcount: 36,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 17, 0, 0],
        mo_x: -108680942,
        mo_y: 56641696,
        mo_z: -12058624,
        mo_angle: 989855744,
    },
    Snapshot {
        tic: 3000,
        virtual_ms: 86736,
        frames: 3040,
        gametic: 3001,
        gamestate: 0,
        rndindex: 78,
        prndindex: 143,
        health: 6,
        armorpoints: 92,
        killcount: 36,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 24, 0, 0],
        mo_x: -107225184,
        mo_y: 72563252,
        mo_z: -12058624,
        mo_angle: 989855744,
    },
    Snapshot {
        tic: 3250,
        virtual_ms: 93764,
        frames: 3290,
        gametic: 3251,
        gamestate: 0,
        rndindex: 72,
        prndindex: 80,
        health: 3,
        armorpoints: 89,
        killcount: 40,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [69, 22, 0, 0],
        mo_x: -91553070,
        mo_y: 81295978,
        mo_z: -11534336,
        mo_angle: 67108864,
    },
    Snapshot {
        tic: 3500,
        virtual_ms: 100792,
        frames: 3540,
        gametic: 3501,
        gamestate: 0,
        rndindex: 66,
        prndindex: 36,
        health: 23,
        armorpoints: 89,
        killcount: 42,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [74, 19, 0, 0],
        mo_x: -107338746,
        mo_y: 61662351,
        mo_z: -12058624,
        mo_angle: 469762048,
    },
    Snapshot {
        tic: 3750,
        virtual_ms: 107820,
        frames: 3790,
        gametic: 3751,
        gamestate: 0,
        rndindex: 60,
        prndindex: 1,
        health: 45,
        armorpoints: 87,
        killcount: 45,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [74, 23, 0, 0],
        mo_x: -74515431,
        mo_y: 58094626,
        mo_z: -12582912,
        mo_angle: 1862270976,
    },
    Snapshot {
        tic: 4000,
        virtual_ms: 114848,
        frames: 4040,
        gametic: 4001,
        gamestate: 0,
        rndindex: 54,
        prndindex: 87,
        health: 34,
        armorpoints: 77,
        killcount: 47,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [79, 21, 0, 0],
        mo_x: -68107234,
        mo_y: 72596143,
        mo_z: -8912896,
        mo_angle: 3254779904,
    },
    Snapshot {
        tic: 4250,
        virtual_ms: 121876,
        frames: 4290,
        gametic: 4251,
        gamestate: 0,
        rndindex: 48,
        prndindex: 182,
        health: 25,
        armorpoints: 68,
        killcount: 48,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [89, 20, 0, 0],
        mo_x: -30462216,
        mo_y: 53511427,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 4500,
        virtual_ms: 128904,
        frames: 4540,
        gametic: 4501,
        gamestate: 0,
        rndindex: 42,
        prndindex: 69,
        health: 14,
        armorpoints: 59,
        killcount: 51,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [94, 14, 0, 0],
        mo_x: 29855097,
        mo_y: 72434923,
        mo_z: 3670016,
        mo_angle: 369098752,
    },
    Snapshot {
        tic: 4750,
        virtual_ms: 135932,
        frames: 4790,
        gametic: 4751,
        gamestate: 0,
        rndindex: 36,
        prndindex: 146,
        health: 11,
        armorpoints: 48,
        killcount: 56,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [99, 8, 0, 0],
        mo_x: 33029653,
        mo_y: 75298585,
        mo_z: 3670016,
        mo_angle: 0,
    },
    Snapshot {
        tic: 5000,
        virtual_ms: 142960,
        frames: 5040,
        gametic: 5001,
        gamestate: 0,
        rndindex: 30,
        prndindex: 73,
        health: 11,
        armorpoints: 48,
        killcount: 58,
        itemcount: 7,
        secretcount: 1,
        readyweapon: 2,
        ammo: [104, 2, 0, 0],
        mo_x: 54125562,
        mo_y: 71548044,
        mo_z: 5242880,
        mo_angle: 234881024,
    },
];

// ---------------------------------------------------------------------------
// Checkpoints
// ---------------------------------------------------------------------------
//
// Dense checkpoints during gameplay to catch subtle state drift.
// The region 2000–3500 is critical — this is where the ported g_game
// causes the player to die (health reaches 0 around tic 2500–3000),
// while the C version keeps them alive at 2–6 HP before they recover.
//
// Key health trajectory (C version, this commit):
//   tic 2250: health=13  (taking heavy damage)
//   tic 2500: health=2   (near death — ported version has health=0 here)
//   tic 2750: health=2   (still hanging on)
//   tic 3000: health=6   (picked up health, recovering)
//   tic 3500: health=23  (fully recovered)
//
// Early checkpoints (title screen → demo start): 35, 70, 140, 280
// Gameplay checkpoints every ~280 tics: 560, 1050
// Dense gameplay checkpoints every 250 tics: 1500..=5000

const CHECKPOINTS: &[usize] = &[
    35, 70, 140, 280, 560, 1050, 1500, 1750, 2000, 2250, 2500, 2750, 2800, 2850, 2900, 2950, 3000,
    3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000,
];
const TOTAL_TICS: usize = 5000;

fn is_checkpoint(tic: usize) -> bool {
    CHECKPOINTS.contains(&tic)
}

// ---------------------------------------------------------------------------
// Snapshot capture
// ---------------------------------------------------------------------------

fn capture_snapshot(tic: usize) -> Snapshot {
    use room::doom::d_player::{consoleplayer, players, MAXPLAYERS};
    use room::doom::m_random::rndindex;

    let cp = unsafe { consoleplayer };
    let pidx = if cp >= 0 && (cp as usize) < MAXPLAYERS {
        cp as usize
    } else {
        0
    };
    let p = unsafe { &players[pidx] };
    let mo_ptr = p.mo as *const MobjPrefix;
    let (mo_x, mo_y, mo_z, mo_angle) = if mo_ptr.is_null() {
        (0, 0, 0, 0)
    } else {
        let mo = unsafe { &*mo_ptr };
        (mo.x, mo.y, mo.z, mo.angle)
    };

    Snapshot {
        tic,
        virtual_ms: virtual_ms(),
        frames: frame_count(),
        gametic: unsafe { gametic },
        gamestate: unsafe { gamestate },
        rndindex: unsafe { rndindex },
        prndindex: unsafe { prndindex },
        health: p.health,
        armorpoints: p.armorpoints,
        killcount: p.killcount,
        itemcount: p.itemcount,
        secretcount: p.secretcount,
        readyweapon: p.readyweapon,
        ammo: p.ammo,
        mo_x,
        mo_y,
        mo_z,
        mo_angle,
    }
}

// ---------------------------------------------------------------------------
// WAD discovery
// ---------------------------------------------------------------------------

fn find_wad() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let wad = manifest
        .join("../doom1.wad")
        .canonicalize()
        .unwrap_or_else(|_| manifest.parent().unwrap_or(&manifest).join("doom1.wad"));
    wad
}

// ---------------------------------------------------------------------------
// The one-and-only test
// ---------------------------------------------------------------------------

#[test]
fn demo_playthrough() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = room::dhat::Profiler::new_heap();

    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .try_init();
    let wad_path = find_wad();
    if !wad_path.exists() {
        panic!(
            "doom1.wad not found at {}; the test requires the shareware IWAD",
            wad_path.display()
        );
    }

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

    let argc = (c_argv.len() - 1) as c_int;

    // Enable singletics mode: one game tic per TryRunTics() call,
    // bypassing the real-time waiting loop.
    unsafe {
        singletics = 1;
    }

    // Initialize the Doom engine.
    unsafe {
        doomgeneric_sys::doomgeneric_Create(argc, c_argv.as_mut_ptr());
    }

    // Sanity-check invariants that future agents often re-validate when
    // debugging random-tick divergence between C and Rust.
    assert_eq!(
        room::doom::c_ffi::DOOM_191_VERSION,
        unsafe { doomgeneric_sys::room_test_get_doom_191_version() },
        "Rust DOOM_191_VERSION must match the C #define"
    );
    assert_eq!(
        unsafe { longtics },
        0,
        "longtics should be false when neither -longtics nor a v1.91 demo is loaded"
    );

    let mut snapshots: Vec<Snapshot> = Vec::new();

    // Validate a single snapshot against the baseline, failing fast.
    fn validate_snapshot(i: usize, got: &Snapshot, expected: &Snapshot) {
        let n = i + 1; // 1-indexed for human-readable error messages
        assert_eq!(got.tic, expected.tic, "checkpoint {}: tic mismatch", n);
        assert_eq!(
            got.virtual_ms, expected.virtual_ms,
            "checkpoint {}: virtual_ms mismatch",
            n
        );
        assert_eq!(
            got.gametic, expected.gametic,
            "checkpoint {}: gametic mismatch",
            n
        );
        assert_eq!(
            got.gamestate, expected.gamestate,
            "checkpoint {}: gamestate mismatch",
            n
        );
        assert_eq!(
            got.rndindex, expected.rndindex,
            "checkpoint {}: rndindex mismatch",
            n
        );
        assert_eq!(
            got.prndindex, expected.prndindex,
            "checkpoint {}: prndindex mismatch",
            n
        );
        assert_eq!(
            got.health, expected.health,
            "checkpoint {}: health mismatch",
            n
        );
        assert_eq!(
            got.armorpoints, expected.armorpoints,
            "checkpoint {}: armorpoints mismatch",
            n
        );
        assert_eq!(
            got.killcount, expected.killcount,
            "checkpoint {}: killcount mismatch",
            n
        );
        assert_eq!(
            got.itemcount, expected.itemcount,
            "checkpoint {}: itemcount mismatch",
            n
        );
        assert_eq!(
            got.secretcount, expected.secretcount,
            "checkpoint {}: secretcount mismatch",
            n
        );
        assert_eq!(
            got.readyweapon, expected.readyweapon,
            "checkpoint {}: readyweapon mismatch",
            n
        );
        assert_eq!(got.ammo, expected.ammo, "checkpoint {}: ammo mismatch", n);
        assert_eq!(got.mo_x, expected.mo_x, "checkpoint {}: mo_x mismatch", n);
        assert_eq!(got.mo_y, expected.mo_y, "checkpoint {}: mo_y mismatch", n);
        assert_eq!(got.mo_z, expected.mo_z, "checkpoint {}: mo_z mismatch", n);
        assert_eq!(
            got.mo_angle, expected.mo_angle,
            "checkpoint {}: mo_angle mismatch",
            n
        );
    }

    let bless = std::env::var("BLESS").is_ok();

    // Drive the engine for TOTAL_TICS ticks.
    eprintln!(
        "demo_playthrough: {} checkpoints, {} total tics",
        CHECKPOINTS.len(),
        TOTAL_TICS
    );
    for tic in 0..TOTAL_TICS {
        unsafe {
            doomgeneric_sys::doomgeneric_Tick();
        }

        if is_checkpoint(tic + 1) {
            let checkpoint_idx = snapshots.len();
            let pct = ((checkpoint_idx + 1) * 100) / CHECKPOINTS.len();
            unsafe {
                eprintln!(
                    "  checkpoint {}/{} ({}%) at tic {} gametic={} leveltime={}",
                    checkpoint_idx + 1,
                    CHECKPOINTS.len(),
                    pct,
                    tic + 1,
                    gametic,
                    room::doom::p_tick::leveltime,
                );
            }
            let snap = capture_snapshot(tic + 1);
            if !bless {
                assert!(
                    checkpoint_idx < BASELINE.len(),
                    "checkpoint {}: no baseline entry (got {} snapshots, baseline has {})",
                    checkpoint_idx,
                    checkpoint_idx + 1,
                    BASELINE.len(),
                );
                validate_snapshot(checkpoint_idx, &snap, &BASELINE[checkpoint_idx]);
            }
            snapshots.push(snap);
        }
    }

    if bless {
        println!("/* BLESS output — paste into BASELINE */");
        println!("const BASELINE: &[Snapshot] = &[");
        for snap in &snapshots {
            println!("    Snapshot {{");
            println!("        tic: {},", snap.tic);
            println!("        virtual_ms: {},", snap.virtual_ms);
            println!("        frames: {},", snap.frames);
            println!("        gametic: {},", snap.gametic);
            println!("        gamestate: {},", snap.gamestate);
            println!("        rndindex: {},", snap.rndindex);
            println!("        prndindex: {},", snap.prndindex);
            println!("        health: {},", snap.health);
            println!("        armorpoints: {},", snap.armorpoints);
            println!("        killcount: {},", snap.killcount);
            println!("        itemcount: {},", snap.itemcount);
            println!("        secretcount: {},", snap.secretcount);
            println!("        readyweapon: {},", snap.readyweapon);
            println!("        ammo: {:?},", snap.ammo);
            println!("        mo_x: {},", snap.mo_x);
            println!("        mo_y: {},", snap.mo_y);
            println!("        mo_z: {},", snap.mo_z);
            println!("        mo_angle: {},", snap.mo_angle);
            println!("    }},");
        }
        println!("];");
        panic!("BLESS mode: paste the output above into BASELINE, then re-run without BLESS=1");
    } else {
        assert_eq!(
            snapshots.len(),
            BASELINE.len(),
            "snapshot count mismatch: got {}, expected {}",
            snapshots.len(),
            BASELINE.len()
        );
        for (i, (got, expected)) in snapshots.iter().zip(BASELINE.iter()).enumerate() {
            assert_eq!(got.tic, expected.tic, "checkpoint {}: tic mismatch", i);
            assert_eq!(
                got.virtual_ms, expected.virtual_ms,
                "checkpoint {}: virtual_ms mismatch",
                i
            );
            assert_eq!(
                got.gametic, expected.gametic,
                "checkpoint {}: gametic mismatch",
                i
            );
            assert_eq!(
                got.gamestate, expected.gamestate,
                "checkpoint {}: gamestate mismatch",
                i
            );
            assert_eq!(
                got.rndindex, expected.rndindex,
                "checkpoint {}: rndindex mismatch",
                i
            );
            assert_eq!(
                got.prndindex, expected.prndindex,
                "checkpoint {}: prndindex mismatch",
                i
            );
            assert_eq!(
                got.health, expected.health,
                "checkpoint {}: health mismatch",
                i
            );
            assert_eq!(
                got.armorpoints, expected.armorpoints,
                "checkpoint {}: armorpoints mismatch",
                i
            );
            assert_eq!(
                got.killcount, expected.killcount,
                "checkpoint {}: killcount mismatch",
                i
            );
            assert_eq!(
                got.itemcount, expected.itemcount,
                "checkpoint {}: itemcount mismatch",
                i
            );
            assert_eq!(
                got.secretcount, expected.secretcount,
                "checkpoint {}: secretcount mismatch",
                i
            );
            assert_eq!(
                got.readyweapon, expected.readyweapon,
                "checkpoint {}: readyweapon mismatch",
                i
            );
            assert_eq!(got.ammo, expected.ammo, "checkpoint {}: ammo mismatch", i);
            assert_eq!(got.mo_x, expected.mo_x, "checkpoint {}: mo_x mismatch", i);
            assert_eq!(got.mo_y, expected.mo_y, "checkpoint {}: mo_y mismatch", i);
            assert_eq!(got.mo_z, expected.mo_z, "checkpoint {}: mo_z mismatch", i);
            assert_eq!(
                got.mo_angle, expected.mo_angle,
                "checkpoint {}: mo_angle mismatch",
                i
            );
        }
    }
}
