//! Demo I/O: recording (`record_demo`, `begin_recording`,
//! `write_demo_ticcmd`), playback (`defered_play_demo`, `do_play_demo`,
//! `read_demo_ticcmd`, `check_demo_status`) and benchmarking
//! (`time_demo`), plus the version-code wrapper. The byte layout matches
//! vanilla Doom exactly (4 bytes per tic non-longtics, 5 bytes longtics)
//! so demos remain bit-compatible with the original DOS engine.
//!
//! All of it is demo-synchronization surface whole-body (F10 wave C3
//! adjudication) -- this file IS the demo stream's reader and writer.
//! The bodies moved verbatim from pre-split `g_game.rs` (the pure read
//! core lives in `dtmc::read_demo_ticcmd_bytes`; `read_demo_ticcmd`
//! keeps its inlined body verbatim); see the module root for the
//! mapping table.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int, c_void};

use crate::doom::crt::{c_printf3, c_snprintf1, c_snprintf2};
use crate::doom::d_main::{fastparm, nomonsters, respawnparm, D_AdvanceDemo};
use crate::doom::d_player::{TiccmdT, MAXPLAYERS};
use crate::doom::doomstat::gameversion;
use crate::doom::i_system::I_Error;
use crate::doom::i_timer::I_GetTime;
use crate::doom::m_argv::{myargv, M_CheckParm, M_CheckParmWithArgs};
use crate::doom::m_misc::{M_snprintf_clamp, M_WriteFile};
use crate::doom::w_wad::{W_CacheLumpName, W_ReleaseLumpName};
use crate::doom::z_zone::{Z_Free, Z_Malloc, PU_STATIC};

use super::actions::init_new as G_InitNew;
use super::consts::{boolean, byte, ga_nothing, ga_playdemo, skill_t, DEMOMARKER};
use super::state::{
    consoleplayer, deathmatch, defdemoname, demobuffer, demoname, demoend, demo_p, demoplayback,
    demorecording, gameaction, gameepisode, gamemap, gameskill, longtics, lowres_turn, netdemo,
    netgame, nodrawers, playeringame, precache, singledemo, starttime, timingdemo, usergame,
    vanilla_demo_limit,
};

use self::{
    check_demo_status as G_CheckDemoStatus, read_demo_ticcmd as G_ReadDemoTiccmd,
    vanilla_version_code as G_VanillaVersionCode,
};

/// Buffer for `DemoVersionDescription` (static local in `G_DoPlayDemo`).
static mut DEMOVERSIONBUF: [c_char; 16] = [0; 16];

// ---------------------------------------------------------------------------
// External function and variable declarations
// ---------------------------------------------------------------------------

extern "C"
{
    /// Engine-wide clean shutdown from `i_system.c` (used by single demos).
    fn I_Quit() -> !;
}

// ---------------------------------------------------------------------------
// G_ReadDemoTiccmd  <- CRITICAL for demo accuracy
// ---------------------------------------------------------------------------

/// Read one tic command from the demo buffer into `cmd`.
///
/// * Non-longtics: 4 bytes (`forwardmove`, `sidemove`, `angleturn-hi`,
///   `buttons`).
/// * Longtics: 5 bytes (`forwardmove`, `sidemove`, `angleturn-lo`,
///   `angleturn-hi`, `buttons`).
///
/// The `angleturn` encoding matches vanilla exactly:
///
/// * Non-longtics: the byte is read as **unsigned**, then shifted left 8 -
///   fits `[-32768, 32512]`.
/// * Longtics: two bytes little-endian, with each byte read unsigned (no
///   sign extension on the individual bytes).
///
/// If the next byte is `DEMOMARKER` (0x80), the read instead calls
/// `G_CheckDemoStatus` to wrap up the demo and returns without touching
/// `cmd`. The parameterised cursor math is pinned in
/// `dtmc::read_demo_ticcmd_bytes`'s baseline vectors.
///
/// # Safety
/// Reads through `demo_p` and writes through `cmd`. Both must be valid.
/// The C symbol is pinned (`G_ReadDemoTiccmd`) so the wasm export set
/// stays byte-identical.
#[doc(alias = "G_ReadDemoTiccmd")]
#[export_name = "G_ReadDemoTiccmd"]
pub unsafe extern "C" fn read_demo_ticcmd(cmd: *mut TiccmdT)
{
    if *demo_p == DEMOMARKER
    {
        G_CheckDemoStatus();
        return;
    }

    let cmd = &mut *cmd;

    // forwardmove: byte reinterpreted as signed char
    cmd.forwardmove = (*demo_p) as i8;
    demo_p = demo_p.add(1);

    // sidemove: byte reinterpreted as signed char
    cmd.sidemove = (*demo_p) as i8;
    demo_p = demo_p.add(1);

    if longtics != 0
    {
        // Little-endian 16-bit value; each byte read as unsigned
        let lo = *demo_p;
        demo_p = demo_p.add(1);
        let hi = *demo_p;
        demo_p = demo_p.add(1);
        // Matches C: angleturn = lo; angleturn |= hi << 8;
        cmd.angleturn = (lo as i16) | ((hi as i16) << 8);
    }
    else
    {
        // Non-longtics: one byte read as UNSIGNED, shifted left 8
        // C: cmd->angleturn = ((unsigned char)*demo_p++) << 8
        let byte = *demo_p;
        demo_p = demo_p.add(1);
        cmd.angleturn = ((byte as u32) << 8) as i16;
    }

    // buttons: unsigned byte
    cmd.buttons = *demo_p;
    demo_p = demo_p.add(1);
}

// ---------------------------------------------------------------------------
// IncreaseDemoBuffer (static)
// ---------------------------------------------------------------------------

/// Double the size of the demo buffer (used only when `vanilla_demo_limit`
/// is off, to allow recording arbitrarily long demos).
///
/// Allocates a new `Z_Malloc` block of twice the current size, copies the
/// existing demo data over, frees the old buffer and rebases `demobuffer`,
/// `demo_p` and `demoend` onto the new allocation.
///
/// # Safety
/// All three demo pointer globals must be in a consistent state pointing
/// into the same allocation before the call.
#[doc(alias = "IncreaseDemoBuffer")]
unsafe fn increase_demo_buffer()
{
    let current_length = demoend.offset_from(demobuffer) as c_int;
    let new_length = current_length * 2;
    let new_demobuffer = Z_Malloc(new_length, PU_STATIC, std::ptr::null_mut()) as *mut byte;
    let new_demop = new_demobuffer.add(demo_p.offset_from(demobuffer) as usize);
    std::ptr::copy_nonoverlapping(demobuffer, new_demobuffer, current_length as usize);
    Z_Free(demobuffer as *mut c_void);
    demobuffer = new_demobuffer;
    demo_p = new_demop;
    demoend = demobuffer.add(new_length as usize);
}

// ---------------------------------------------------------------------------
// G_WriteDemoTiccmd
// ---------------------------------------------------------------------------

/// Append `cmd` to the demo buffer using the same byte layout as
/// [`read_demo_ticcmd`] (4 bytes vanilla, 5 bytes longtics).
///
/// Pressing the `key_demo_quit` ends recording immediately via
/// `G_CheckDemoStatus`. After writing, the demo cursor is rewound and the
/// just-written record is read back through `G_ReadDemoTiccmd` so the
/// recorded value is exactly what playback will see (round-trip
/// consistency).
///
/// If the cursor approaches `demoend - 16`, either `G_CheckDemoStatus` ends
/// recording (vanilla limit on) or `increase_demo_buffer` grows the buffer
/// (vanilla limit off).
///
/// # Safety
/// Writes through `demo_p` and reads back through it; requires the demo
/// buffer to have at least 16 bytes of headroom or `vanilla_demo_limit == 0`.
/// The C symbol is pinned (`G_WriteDemoTiccmd`) so the wasm export set
/// stays byte-identical.
#[doc(alias = "G_WriteDemoTiccmd")]
#[export_name = "G_WriteDemoTiccmd"]
pub unsafe extern "C" fn write_demo_ticcmd(cmd: *mut TiccmdT)
{
    use crate::doom::m_controls::key_demo_quit;

    if super::ticcmd::GAMEKEYDOWN[key_demo_quit as usize] != 0 { G_CheckDemoStatus(); }

    let cmd = &*cmd;
    let demo_start = demo_p;

    *demo_p = cmd.forwardmove as byte;
    demo_p = demo_p.add(1);
    *demo_p = cmd.sidemove as byte;
    demo_p = demo_p.add(1);

    if longtics != 0
    {
        *demo_p = (cmd.angleturn & 0xff) as byte;
        demo_p = demo_p.add(1);
        *demo_p = ((cmd.angleturn >> 8) & 0xff) as byte;
        demo_p = demo_p.add(1);
    }
    else
    {
        *demo_p = (cmd.angleturn >> 8) as byte;
        demo_p = demo_p.add(1);
    }

    *demo_p = cmd.buttons;
    demo_p = demo_p.add(1);

    // Reset demo pointer — write then re-read to validate consistency
    demo_p = demo_start;

    if demo_p > demoend.sub(16)
    {
        if vanilla_demo_limit != 0
        {
            G_CheckDemoStatus();
            return;
        }
        else { increase_demo_buffer(); }
    }

    G_ReadDemoTiccmd(cmd as *const TiccmdT as *mut TiccmdT);
}

// ---------------------------------------------------------------------------
// G_RecordDemo / G_VanillaVersionCode / G_BeginRecording
// ---------------------------------------------------------------------------

/// Start recording a demo to `name.lmp`.
///
/// Allocates the demo buffer (default 128 KiB, overridable via the
/// `-maxdemo <kib>` command-line argument), constructs the output filename
/// by appending `.lmp`, and sets `demorecording = 1`. `usergame` is cleared
/// so save/load menus are disabled during recording.
///
/// # Safety
/// `name` must be a valid NUL-terminated C string. The C symbol is pinned
/// (`G_RecordDemo`) so the wasm export set stays byte-identical.
#[doc(alias = "G_RecordDemo")]
#[export_name = "G_RecordDemo"]
pub unsafe extern "C" fn record_demo(name: *mut c_char)
{
    usergame = 0;
    let name_len = libc::strlen(name);
    let demoname_size = name_len + 5;
    demoname = Z_Malloc(demoname_size as c_int, PU_STATIC, std::ptr::null_mut()) as *mut c_char;
    M_snprintf_clamp(
        demoname,
        demoname_size,
        c_snprintf1(demoname, demoname_size, c"%s.lmp".as_ptr(), name),
    );
    let mut maxsize: c_int = 0x20000;
    let i = M_CheckParmWithArgs(c"-maxdemo".as_ptr().cast_mut(), 1);
    if i != 0
    {
        maxsize = libc::atoi(*myargv.add(i as usize + 1) as *const c_char);
        maxsize *= 1024;
    }
    demobuffer = Z_Malloc(maxsize, PU_STATIC, std::ptr::null_mut()) as *mut byte;
    demoend = demobuffer.add(maxsize as usize);
    demorecording = 1;
}

/// Return the single-byte demo version code corresponding to the active
/// [`gameversion`] (e.g. 109 for v1.9 and every later vanilla variant).
///
/// Raises `I_Error` for v1.2, which never had a demo version code. Thin
/// wrapper over the pure table in [`super::dtmc::vanilla_version_code_for`].
///
/// # Safety
/// Reads the `gameversion` global. The C symbol is pinned
/// (`G_VanillaVersionCode`) so the wasm export set stays byte-identical.
#[doc(alias = "G_VanillaVersionCode")]
#[export_name = "G_VanillaVersionCode"]
pub unsafe extern "C" fn vanilla_version_code() -> c_int { super::dtmc::vanilla_version_code_for(gameversion) }

/// Write the demo file header (version byte, skill, episode, map,
/// deathmatch / respawn / fast / nomonsters flags, console player and
/// per-slot playeringame bytes) at the start of the demo buffer.
///
/// Honours the `-longtics` command-line flag: when set, writes the special
/// `DOOM_191_VERSION` marker and disables [`lowres_turn`], so each tic
/// stores `angleturn` in 2 bytes instead of 1.
///
/// # Safety
/// Mutates `longtics`, `lowres_turn`, the demo cursor and the demo buffer.
/// The C symbol is pinned (`G_BeginRecording`) so the wasm export set
/// stays byte-identical.
#[doc(alias = "G_BeginRecording")]
#[export_name = "G_BeginRecording"]
pub unsafe extern "C" fn begin_recording()
{
    use crate::doom::c_ffi::DOOM_191_VERSION;

    longtics = (M_CheckParm(c"-longtics".as_ptr().cast_mut()) != 0) as boolean;
    lowres_turn = (longtics == 0) as boolean;

    demo_p = demobuffer;

    if longtics != 0 { *demo_p = DOOM_191_VERSION as byte; }
    else { *demo_p = G_VanillaVersionCode() as byte; }
    demo_p = demo_p.add(1);

    *demo_p = gameskill as byte;
    demo_p = demo_p.add(1);
    *demo_p = gameepisode as byte;
    demo_p = demo_p.add(1);
    *demo_p = gamemap as byte;
    demo_p = demo_p.add(1);
    *demo_p = deathmatch as byte;
    demo_p = demo_p.add(1);
    *demo_p = respawnparm as byte;
    demo_p = demo_p.add(1);
    *demo_p = fastparm as byte;
    demo_p = demo_p.add(1);
    *demo_p = nomonsters as byte;
    demo_p = demo_p.add(1);
    *demo_p = consoleplayer as byte;
    demo_p = demo_p.add(1);

    for i in 0..MAXPLAYERS
    {
        *demo_p = playeringame[i] as byte;
        demo_p = demo_p.add(1);
    }
}

// ---------------------------------------------------------------------------
// G_DeferedPlayDemo / DemoVersionDescription / G_DoPlayDemo
// ---------------------------------------------------------------------------

/// Defer demo playback for `name`: latch `defdemoname` and queue
/// `ga_playdemo` for the next `G_Ticker` pass.
///
/// # Safety
/// `name` must point to a NUL-terminated C string that lives at least until
/// `G_DoPlayDemo` runs. The C symbol is pinned (`G_DeferedPlayDemo`) so
/// the wasm export set stays byte-identical.
#[doc(alias = "G_DeferedPlayDemo")]
#[export_name = "G_DeferedPlayDemo"]
pub unsafe extern "C" fn defered_play_demo(name: *const c_char)
{
    defdemoname = name as *mut c_char;
    gameaction = ga_playdemo;
}

/// Map a demo version byte to a human-readable engine identifier
/// (`"v1.9"`, `"v1.6/v1.666"`, etc.).
///
/// Unknown values in the range 0..=4 are treated as pre-v1.4 IWAD demos and
/// labelled `"v1.0/v1.1/v1.2"`. Anything else is formatted into the
/// `DEMOVERSIONBUF` static as `"major.minor (unknown)"` and a pointer into
/// that buffer is returned (mirroring the C function's use of a static
/// `resultbuf`).
///
/// # Safety
/// Writes the `DEMOVERSIONBUF` static; the returned pointer is invalidated
/// by the next call.
unsafe fn demo_version_description(version: c_int) -> *const c_char
{
    match version
    {
        104 => c"v1.4".as_ptr(),
        105 => c"v1.5".as_ptr(),
        106 => c"v1.6/v1.666".as_ptr(),
        107 => c"v1.7/v1.7a".as_ptr(),
        108 => c"v1.8".as_ptr(),
        109 => c"v1.9".as_ptr(),
        _ =>
        {
            if (0..=4).contains(&version) { c"v1.0/v1.1/v1.2".as_ptr() }
            else
            {
                M_snprintf_clamp(
                    std::ptr::addr_of_mut!(DEMOVERSIONBUF[0]),
                    16,
                    c_snprintf2(
                        std::ptr::addr_of_mut!(DEMOVERSIONBUF[0]),
                        16,
                        c"%i.%i (unknown)".as_ptr(),
                        version / 100,
                        version % 100,
                    ),
                );
                std::ptr::addr_of!(DEMOVERSIONBUF[0])
            }
        }
    }
}

/// Execute the deferred `ga_playdemo` action: cache the demo lump, parse
/// its header, configure netgame / netdemo / game parameters, then call
/// `G_InitNew` and flip `demoplayback = 1`.
///
/// The version handling matches vanilla exactly:
///
/// * If the version byte matches the current engine's vanilla code, clears
///   `longtics`.
/// * If it equals `DOOM_191_VERSION`, sets `longtics`.
/// * Otherwise prints a warning via `printf` (not `I_Error`) and continues
///   playback - this matches the C source, which deliberately allowed
///   wrong-version demos to attempt playback rather than aborting.
///
/// `precache` is temporarily cleared around `G_InitNew` so map loading
/// during timing demos does not skew the fps measurement.
///
/// # Safety
/// Mutates global engine state extensively. The C symbol is pinned
/// (`G_DoPlayDemo`) so the wasm export set stays byte-identical.
#[doc(alias = "G_DoPlayDemo")]
#[export_name = "G_DoPlayDemo"]
pub unsafe extern "C" fn do_play_demo()
{
    use crate::doom::c_ffi::DOOM_191_VERSION;

    gameaction = ga_nothing;
    demobuffer = W_CacheLumpName(defdemoname, PU_STATIC) as *mut byte;
    demo_p = demobuffer;

    let demoversion = *demo_p as c_int;
    demo_p = demo_p.add(1);

    if demoversion == G_VanillaVersionCode() { longtics = 0; }
    else if demoversion == DOOM_191_VERSION { longtics = 1; }
    else
    {
        let message = b"Demo is from a different game version!\n\
            (read %i, should be %i)\n\n\
            *** You may need to upgrade your version of Doom to v1.9. ***\n\
            See: https://www.doomworld.com/classicdoom/info/patches.php\n\
            This appears to be %s.\0";
        // C code uses printf (not I_Error) here so demo playback continues
        c_printf3(
            message.as_ptr() as *const c_char,
            demoversion,
            G_VanillaVersionCode(),
            demo_version_description(demoversion),
        );
    }

    let skill = *demo_p as skill_t;
    demo_p = demo_p.add(1);
    let episode = *demo_p as c_int;
    demo_p = demo_p.add(1);
    let map = *demo_p as c_int;
    demo_p = demo_p.add(1);
    deathmatch = *demo_p as c_int;
    demo_p = demo_p.add(1);
    respawnparm = *demo_p as c_int;
    demo_p = demo_p.add(1);
    fastparm = *demo_p as c_int;
    demo_p = demo_p.add(1);
    nomonsters = *demo_p as c_int;
    demo_p = demo_p.add(1);
    consoleplayer = *demo_p as c_int;
    demo_p = demo_p.add(1);
    if consoleplayer < 0 || consoleplayer >= MAXPLAYERS as c_int { I_Error(c"G_DoPlayDemo: consoleplayer %d out of range\n".as_ptr()); }

    for i in 0..MAXPLAYERS
    {
        playeringame[i] = *demo_p as boolean;
        demo_p = demo_p.add(1);
    }

    if playeringame[1] != 0 ||
        M_CheckParm(c"-solo-net".as_ptr().cast_mut()) > 0 ||
        M_CheckParm(c"-netdemo".as_ptr().cast_mut()) > 0
        {
            netgame = 1;
            netdemo = 1;
        }

    precache = 0;
    G_InitNew(skill, episode, map);
    precache = 1;
    starttime = I_GetTime();

    usergame = 0;
    demoplayback = 1;
}

// ---------------------------------------------------------------------------
// G_TimeDemo
// ---------------------------------------------------------------------------

/// Start a benchmark playback of demo `name`.
///
/// Honours `-nodraw` to suppress rendering, sets `singletics` so the engine
/// runs every tic immediately (no `I_GetTime`-pacing), then defers playback
/// the usual way through `ga_playdemo`. On demo end, `G_CheckDemoStatus`
/// prints the result via `I_Error("timed ... fps")`.
///
/// # Safety
/// Mutates `nodrawers`, `timingdemo`, `singletics`, `defdemoname`, `gameaction`.
/// The C symbol is pinned (`G_TimeDemo`) so the wasm export set stays
/// byte-identical.
#[doc(alias = "G_TimeDemo")]
#[export_name = "G_TimeDemo"]
pub unsafe extern "C" fn time_demo(name: *mut c_char)
{
    nodrawers = M_CheckParm(c"-nodraw".as_ptr().cast_mut());
    timingdemo = 1;
    use crate::doom::d_loop::singletics;
    singletics = 1;
    defdemoname = name;
    gameaction = ga_playdemo;
}

// ---------------------------------------------------------------------------
// G_CheckDemoStatus
// ---------------------------------------------------------------------------

/// End-of-demo cleanup; called by both reader and writer paths.
///
/// Three mutually-exclusive paths:
///
/// * **Timing demo**: compute fps, clear the timing/playback flags and
///   raise `I_Error("timed ... fps")` (which prints and exits).
/// * **Playback**: release the demo lump, clear demo / netgame / dm flags
///   and either `I_Quit()` (singledemo) or `D_AdvanceDemo()` (loop). Returns
///   `1`.
/// * **Recording**: append `DEMOMARKER`, flush the buffer to `demoname` via
///   `M_WriteFile`, free the buffer and raise `I_Error("Demo %s recorded")`.
///
/// Returns `0` when the call was a no-op (none of the conditions matched).
///
/// # Safety
/// Mutates demo / playback globals and performs file I/O. The C symbol is
/// pinned (`G_CheckDemoStatus`) so the wasm export set stays byte-identical.
#[doc(alias = "G_CheckDemoStatus")]
#[export_name = "G_CheckDemoStatus"]
pub unsafe extern "C" fn check_demo_status() -> boolean
{
    if timingdemo != 0
    {
        timingdemo = 0;
        demoplayback = 0;
        I_Error(c"timed %i gametics in %i realtics (%f fps)".as_ptr());
    }

    if demoplayback != 0
    {
        W_ReleaseLumpName(defdemoname);
        demoplayback = 0;
        netdemo = 0;
        netgame = 0;
        deathmatch = 0;
        playeringame[1] = 0;
        playeringame[2] = 0;
        playeringame[3] = 0;
        respawnparm = 0;
        fastparm = 0;
        nomonsters = 0;
        consoleplayer = 0;

        if singledemo != 0 { I_Quit(); }
        else { D_AdvanceDemo(); }
        return 1;
    }

    if demorecording != 0
    {
        *demo_p = DEMOMARKER;
        demo_p = demo_p.add(1);
        M_WriteFile(
            demoname,
            demobuffer as *mut c_void,
            demo_p.offset_from(demobuffer) as c_int,
        );
        Z_Free(demobuffer as *mut c_void);
        demorecording = 0;
        I_Error(c"Demo %s recorded".as_ptr());
    }

    0
}
