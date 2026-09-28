//! The extracted demo-synchronization surface of `g_game` (F10 wave C3
//! adjudication, criterion: "does this function's observable behavior
//! belong to the demo synchronization surface?"). Seven pure cores whose
//! outputs ARE demo bytes or workaround-emulation values:
//!
//! - `ticcmd_checksum` -- the netgame/demo ticcmd checksum
//! - `read_demo_ticcmd_bytes` -- the parameterised demo-read cursor math
//! - `vanilla_version_code_for` -- the demo-header version-code table
//! - `lowres_turn_round` -- the 256-BAM lowres-turn rounding step
//! - `teleport_fog_offset` -- the TeleportFogAngleOverrun case table
//! - `commercial_partime` + `gammalvl0_prefix_i32` -- the map33
//!   GAMMALVL0 par-time overrun model
//!
//! The stateful halves (cursor writes, census records, `I_Error` edges,
//! the callers) stay at their call sites in `ticcmd` / `demo` / `spawn` /
//! `actions`. Every core here is pinned by baseline vectors that were
//! captured against the pre-move bodies BEFORE the split (F10 §2.3) and
//! re-pointed at these names after it -- same vectors, same results.

use std::ffi::c_int;

use crate::doom::d_mode::{exe_doom_1_2, exe_doom_1_666, exe_doom_1_7, exe_doom_1_8};
use crate::doom::d_player::TiccmdT;
use crate::doom::i_system::I_Error;
use crate::doom::m_menu::gammamsg;
use crate::doom::tables::{finecosine, finesine, finetangent, tantoangle, ANG45, ANGLETOFINESHIFT};
use crate::doom::violations::{self, VanillaViolation};

use super::consts::fixed_t;
use super::state::cpars;

/// Compute a checksum over the first `sizeof(ticcmd_t)/4 - 1` 4-byte words
/// of `cmd` using wrapping integer addition; the whole-body pure core of
/// upstream `G_CmdChecksum` (`vendor/doomgeneric/g_game.c`).
///
/// ## Technical Details
///
/// Vanilla Doom uses this checksum for netgame and demo-validation
/// purposes. The final word is excluded so that fields stored at the tail
/// of the struct (e.g. inventory padding) do not affect compatibility.
/// The sum wraps (`wrapping_add`): checksums are compared for equality,
/// never ordered, and the vanilla C sum overflowed silently.
///
/// ## On Calling
///
/// # Safety
/// `cmd` must point to a fully-initialised `TiccmdT` with at least
/// `sizeof::<TiccmdT>()` valid bytes; reads through the pointer as a raw
/// `c_int` array. The C symbol is pinned (`G_CmdChecksum`) so the wasm
/// export set stays byte-identical.
#[doc(alias = "G_CmdChecksum")]
#[export_name = "G_CmdChecksum"]
pub unsafe extern "C" fn ticcmd_checksum(cmd: *const TiccmdT) -> c_int
{
    let n = std::mem::size_of::<TiccmdT>() / 4 - 1;
    let words = cmd as *const c_int;
    let mut sum: c_int = 0;
    for i in 0..n
    {
        sum = sum.wrapping_add(*words.add(i));
    }
    sum
}

/// Pure implementation of the demo ticcmd read, parameterised over the
/// demo-pointer and longtics flag; the cursor math that decides every
/// byte assignment during demo playback.
///
/// ## Technical Details
///
/// The `angleturn` encoding matches vanilla exactly:
///
/// * Non-longtics: the byte is read as **unsigned**, then shifted left 8
///   -- fits `[-32768, 32512]`.
/// * Longtics: two bytes little-endian, with each byte read unsigned (no
///   sign extension on the individual bytes).
///
/// Never debug-assert on the shifts: `(0xFF as u32) << 8 as i16` is
/// deliberately `-256`, the vanilla sign model.
///
/// ## On Calling
///
/// # Safety
///
/// `*p` must point into a valid, sufficiently-long demo buffer: at least 4 bytes
/// remaining for non-longtics demos, or 5 bytes for longtics. The pointer is advanced
/// past the bytes consumed. `cmd` must be a valid, writable `TiccmdT`.
#[doc(alias = "read_demo_ticcmd_inner")]
#[inline]
unsafe fn read_demo_ticcmd_bytes(p: &mut *mut u8, cmd: &mut TiccmdT, is_longtics: bool)
{
    cmd.forwardmove = (**p) as i8;
    *p = p.add(1);

    cmd.sidemove = (**p) as i8;
    *p = p.add(1);

    if is_longtics
    {
        let lo = **p;
        *p = p.add(1);
        let hi = **p;
        *p = p.add(1);
        // C: angleturn = lo; angleturn |= (hi << 8);  — both lo and hi unsigned
        cmd.angleturn = (lo as i16) | ((hi as i16) << 8);
    }
    else
    {
        let byte = **p;
        *p = p.add(1);
        // C: cmd->angleturn = ((unsigned char)*demo_p++) << 8
        cmd.angleturn = ((byte as u32) << 8) as i16;
    }

    cmd.buttons = **p;
    *p = p.add(1);
}

/// Pure helper for [`super::demo::vanilla_version_code`]: maps a
/// `gameversion` value to its 1-byte demo header code without touching
/// globals, so unit tests can exercise the table directly.
///
/// ## Technical Details
///
/// v1.2 has no demo version code and aborts via `I_Error`; v1.9's code
/// (109) is the shared answer for every later vanilla variant. The
/// match-arm order mirrors `G_VanillaVersionCode` in `g_game.c`.
///
/// ## On Calling
///
/// The `exe_doom_1_2` arm diverges (noreturn) -- never reachable for a
/// demo that carries a version byte, since v1.2 wrote no demo header.
pub fn vanilla_version_code_for(gv: c_int) -> c_int
{
    match gv
    {
        v if v == exe_doom_1_2 => I_Error(c"Doom 1.2 does not have a version code!".as_ptr()),
        v if v == exe_doom_1_666 => 106,
        v if v == exe_doom_1_7 => 107,
        v if v == exe_doom_1_8 => 108,
        _ => 109, // exe_doom_1_9 and all later variants
    }
}

/// One step of the low-resolution turn rounding applied when recording a
/// vanilla (non-longtics) demo: fold the previous tic's residual into
/// this tic's `angleturn`, round to a 256-BAM boundary, and return the
/// residual for the next tic.
///
/// ## Technical Details
///
/// Extracted from the inline `lowres_turn` block of `G_BuildTiccmd`
/// (pre-split `g_game.rs:1211-1215`) -- this arithmetic IS on the
/// recorded demo stream: 1-byte-per-tic demos can only carry
/// 256-BAM-quantized turns, and the carry is what makes successive small
/// turns accumulate accurately. The wrap semantics are load-bearing:
/// `wrapping_add` / `wrapping_sub` (the vanilla C `int` arithmetic
/// overflowed silently), and `(desired as i32 + 128) & 0xff00` implements
/// round-half-up to the next 256 boundary in 32-bit space before the
/// narrowing cast. Baseline vectors were captured against the inline
/// body before extraction and re-pointed here (`lowres_turn_round_baseline_vectors`).
///
/// ## On Calling
///
/// Returns `(rounded_angleturn, next_carry)`; the caller assigns both in
/// order (ticcmd byte, then the static carry). Inputs may be any `i16`
/// pair -- the function never panics, including `0x7FFF + 0x7FFF`.
pub fn lowres_turn_round(angleturn: i16, carry: i16) -> (i16, i16)
{
    let desired: i16 = angleturn.wrapping_add(carry);
    let rounded = ((desired as i32 + 128) & 0xff00) as i16;
    (rounded, desired.wrapping_sub(rounded))
}

//* Teleport-fog offset lookup, extracted from `G_CheckSpot` so the
//* vanilla-overflow case table is unit-testable without a live level
//* (the caller needs blockmap, subsectors and the mobj zone).
//*
//* Vanilla compiled `(ANG45 * (angle/45)) >> ANGLETOFINESHIFT` with a signed
//* shift: for angles >= 180 the multiply overflows into the sign bit, so
//* `an` goes negative and the table lookups land in `finetangent[]`
//* (docs/vanilla-workarounds.md #6). Chocolate reproduces the observable
//* values with the switch below, transcribed from
//* reference/chocolate-doom/src/doom/g_game.c:1223-1268: it deliberately
//* avoids the overflow and switches on the positive scale
//* `an = (ANG45 >> ANGLETOFINESHIFT) * (angle/45)` = `1024 * angle/45`,
//* whose 4096/5120/6144/7168 cases name the overrun indices explicitly.
//* `None` is chocolate's `default:` arm (`I_Error` in the caller).
///
/// ## Technical Details
///
/// Every legal mapthing angle `0..=360 step 45` must resolve through one
/// of the arms below -- the four 180..=315 arms ARE the vanilla-defect
/// emulation and record `TeleportFogAngleOverrun` when replayed.
///
/// ## On Calling
///
/// `None` routes the caller to `I_Error("G_CheckSpot: unexpected angle
/// %d\n")` (chocolate's default arm); the offset pair is in `fixed_t`
/// map units to be scaled by the caller's `20 *` fog distance.
pub fn teleport_fog_offset(angle: c_int) -> Option<(fixed_t, fixed_t)>
{
    let an = ((ANG45 >> ANGLETOFINESHIFT) as i32).wrapping_mul(angle / 45);
    match an
    {
        4096 =>
        {
            // Vanilla -4096: finecosine[-4096] / finesine[-4096]
            violations::record(VanillaViolation::TeleportFogAngleOverrun);
            Some((finetangent[2048], finetangent[0]))
        }
        5120 =>
        {
            // Vanilla -3072: finecosine[-3072] / finesine[-3072]
            violations::record(VanillaViolation::TeleportFogAngleOverrun);
            Some((finetangent[3072], finetangent[1024]))
        }
        6144 =>
        {
            // Vanilla -2048: finecosine[-2048] / finesine[-2048]
            violations::record(VanillaViolation::TeleportFogAngleOverrun);
            Some((finesine[0], finetangent[2048]))
        }
        7168 =>
        {
            // Vanilla -1024: finecosine[-1024] / finesine[-1024]
            violations::record(VanillaViolation::TeleportFogAngleOverrun);
            Some((finesine[1024], finetangent[3072]))
        }
        0 | 1024 | 2048 | 3072 =>
        {
            // SAFETY: `an` is 0..=3072; `finecosine` aims at
            // `finesine[FINEANGLES/4]`, so the reads stay inside
            // `finesine`'s 10240 entries.
            Some((
                unsafe { *finecosine.0.add(an as usize) },
                finesine[an as usize],
            ))
        }
        8192 =>
        {
            // 360 degrees: finecosine[8192] overran one past `finesine`,
            // into `tantoangle[0]` in the DOS binary's adjacent layout;
            // `finesine[8192]` itself is in-range (sine of 360 deg = 0).
            Some((tantoangle[0] as fixed_t, finesine[8192]))
        }
        _ => None,
    }
}

//* Commercial par-time selection for `G_DoCompleted`, extracted so the map33
//* emulation is unit-testable without a live level completion (same pattern
//* as `g_check_spot_fog_offset`).
//*
//* Chocolate has no special case either: doom2.exe just evaluates
//* `cpars[gamemap-1]`, and for map 33 that index lands one int past the
//* array, in the first four bytes of the GAMMALVL0 rodata string adjacent to
//* `cpars` in the DOS binary. Rust's bounds check would turn that read into
//* a panic, so the overrun is reproduced explicitly
//* (docs/vanilla-workarounds.md #9). `None` is the guard arm for maps the
//* references assign no value to (map <= 0 or > 33): chocolate does a plain
//* unguarded read there, which we cannot model; the caller raises I_Error.
//* (The parameter cannot be named `gamemap`: that would shadow the static.)
///
/// ## Technical Details
///
/// Maps 1-32 return `35 * cpars[map-1]`; map 33 returns
/// `35 * gammalvl0_prefix_i32()` with C wrap semantics (the product
/// overflows i32 -- `wrapping_mul`, never a debug panic) and records
/// `ParTimeOverrun` when replayed.
///
/// ## On Calling
///
/// `None` is not an error value the caller may swallow: the caller
/// (`do_completed`) raises `I_Error` naming the map.
pub fn commercial_partime(map: c_int) -> Option<c_int>
{
    match map
    {
        1..=32 => Some(35 * unsafe {
            // SAFETY: plain read of a compile-time-initialized table.
            cpars[(map - 1) as usize]
        }),
        33 =>
        {
            violations::record(VanillaViolation::ParTimeOverrun);
            Some(35i32.wrapping_mul(gammalvl0_prefix_i32()))
        }
        _ => None,
    }
}

//* The port's GAMMALVL0 equivalent: the first gamma message ("Gamma
//* correction OFF", `gammamsg[0]` in m_menu.rs). Reading the live static --
//* not a frozen copy of the text -- mirrors chocolate's
//* `DEH_String(GAMMALVL0)` indirection, so a future DSDHacked string
//* replacement would move map33's par time exactly as chocolate's does.
//* Chocolate loads the first `sizeof(int)` bytes of the string and runs the
//* result through `LONG()`, i.e. it interprets the four bytes as
//* little-endian on every host; `from_le_bytes` is the same
//* host-independent model.
///
/// ## Technical Details
///
/// The constant pin `0x6D6D6147` ("Gamm" little-endian) in the unit test
/// keeps this honest: a silent `gammamsg[0]` text change must fail the
/// test, not slide the map33 par-time expectation.
///
/// ## On Calling
///
/// Reads only; safe under the single-threaded engine convention.
fn gammalvl0_prefix_i32() -> c_int
{
    let bytes = unsafe {
        // SAFETY: read-only access to a const-initialized table; nothing
        // writes `gammamsg` after initialization.
        [
            gammamsg[0][0] as u8,
            gammamsg[0][1] as u8,
            gammamsg[0][2] as u8,
            gammamsg[0][3] as u8,
        ]
    };
    i32::from_le_bytes(bytes)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

/// Vectors for the demo I/O byte layout, checksumming, version-code
/// table, the lowres-turn rounding core and the two workaround case
/// tables. The demo read tests exercise `read_demo_ticcmd_bytes`
/// directly so they avoid mutating the global `demo_p` cursor and the
/// `DEMOMARKER` end-of-stream shortcut.
#[cfg(test)]
mod tests
{
    use super::{
        commercial_partime, gammalvl0_prefix_i32, lowres_turn_round, read_demo_ticcmd_bytes,
        teleport_fog_offset, ticcmd_checksum, vanilla_version_code_for,
    };
    use crate::doom::d_mode::{
        exe_doom_1_666, exe_doom_1_7, exe_doom_1_8, exe_doom_1_9, exe_final2, exe_ultimate,
    };
    use crate::doom::d_player::TiccmdT;
    use crate::doom::g_game::consts::fixed_t;
    use crate::doom::g_game::cpars;
    use crate::doom::m_menu::gammamsg;
    use crate::doom::tables::{finecosine, finesine, finetangent, tantoangle};
    use crate::doom::violations::{self, VanillaViolation};

    /// Helper returning a freshly zeroed [`TiccmdT`] for test assembly.
    fn zeroed_cmd() -> TiccmdT
    {
        unsafe { std::mem::zeroed() }
    }

    // --- G_CmdChecksum ---

    /// A fully-zero ticcmd checksums to zero.
    #[test]
    fn cmdchecksum_all_zeros_is_zero()
    {
        let cmd = zeroed_cmd();
        unsafe
        {
            assert_eq!(ticcmd_checksum(&cmd as *const TiccmdT), 0);
        }
    }

    /// `forwardmove = 1` produces a single bit in the first 4-byte word.
    #[test]
    fn cmdchecksum_forwardmove_one()
    {
        // sizeof(TiccmdT) = 16 bytes = 4 ints; loop runs for 3 ints.
        // First int = bytes [forwardmove(i8), sidemove(i8), angleturn_lo(u8), angleturn_hi(u8)]
        // With forwardmove=1, rest zero: first int = 0x00_00_00_01 = 1 (little-endian).
        let mut cmd = zeroed_cmd();
        cmd.forwardmove = 1;
        unsafe
        {
            assert_eq!(ticcmd_checksum(&cmd as *const TiccmdT), 1);
        }
    }

    /// The tail-word `lookfly` / `arti` / `_pad` fields are excluded from the sum.
    #[test]
    fn cmdchecksum_excludes_last_int()
    {
        // G_CmdChecksum sums sizeof(ticcmd_t)/4 - 1 = 3 words (bytes 0-11).
        // The last word (bytes 12-15: lookfly + arti + _pad) is NOT summed.
        let mut cmd = zeroed_cmd();
        cmd.lookfly = 0x12;
        cmd.arti = 0x34;
        unsafe
        {
            assert_eq!(ticcmd_checksum(&cmd as *const TiccmdT), 0);
        }
    }

    /// `inventory` sits in word 2 and is included in the checksum.
    #[test]
    fn cmdchecksum_includes_inventory()
    {
        // inventory is at bytes 8-11 (word 2), which IS included in the sum.
        let mut cmd = zeroed_cmd();
        cmd.inventory = 1;
        unsafe
        {
            assert_eq!(ticcmd_checksum(&cmd as *const TiccmdT), 1);
        }
    }

    // --- G_ReadDemoTiccmd (non-longtics) ---

    /// Test that G_ReadDemoTiccmd reads the non-longtics format correctly.
    /// Byte layout: [forwardmove, sidemove, angleturn_byte, buttons]
    #[test]
    fn read_demo_ticcmd_non_longtics_basic()
    {
        unsafe
        {
            let buf: [u8; 4] = [0x10, 0x20, 0x30, 0x01];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            // Don't call the full read (it calls G_CheckDemoStatus on DEMOMARKER),
            // instead test the inner read logic directly.
            read_demo_ticcmd_bytes(&mut p, &mut cmd, false);
            assert_eq!(cmd.forwardmove, 0x10i8);
            assert_eq!(cmd.sidemove, 0x20i8);
            // angleturn: (0x30 as u8 as u32) << 8 = 0x3000 = 12288 as i16
            assert_eq!(cmd.angleturn, 0x3000u16 as i16);
            assert_eq!(cmd.buttons, 0x01);
        }
    }

    /// Negative forwardmove/sidemove: byte 0xFF → -1 as i8.
    #[test]
    fn read_demo_ticcmd_negative_moves()
    {
        unsafe
        {
            let buf: [u8; 4] = [0xFF, 0x80, 0x00, 0x00];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            read_demo_ticcmd_bytes(&mut p, &mut cmd, false);
            assert_eq!(cmd.forwardmove, -1i8);
            assert_eq!(cmd.sidemove, -128i8);
        }
    }

    /// High angleturn byte 0xFF: must produce (0xFF as u32) << 8 = 0xFF00 = -256 as i16.
    /// This is the key signed-unsigned handling that must match vanilla C.
    #[test]
    fn read_demo_ticcmd_high_angleturn_byte()
    {
        unsafe
        {
            let buf: [u8; 4] = [0x00, 0x00, 0xFF, 0x00];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            read_demo_ticcmd_bytes(&mut p, &mut cmd, false);
            // (0xFF as u32) << 8 = 0xFF00; as i16 = -256
            assert_eq!(cmd.angleturn, -256i16);
        }
    }

    /// Angleturn byte 0x80 → 0x8000 = -32768 as i16 (maximum left turn).
    #[test]
    fn read_demo_ticcmd_angleturn_0x80()
    {
        unsafe
        {
            let buf: [u8; 4] = [0x00, 0x00, 0x80, 0x00];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            read_demo_ticcmd_bytes(&mut p, &mut cmd, false);
            assert_eq!(cmd.angleturn, i16::MIN); // 0x8000 = -32768
        }
    }

    // --- G_ReadDemoTiccmd (longtics) ---

    /// Longtics: two-byte little-endian angleturn, each byte treated as unsigned.
    #[test]
    fn read_demo_ticcmd_longtics_basic()
    {
        unsafe
        {
            let buf: [u8; 5] = [0x10, 0x20, 0xAB, 0xCD, 0x01];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            read_demo_ticcmd_bytes(&mut p, &mut cmd, true);
            assert_eq!(cmd.forwardmove, 0x10i8);
            assert_eq!(cmd.sidemove, 0x20i8);
            // angleturn = 0xAB | (0xCD << 8) = 0xCDAB as i16
            assert_eq!(cmd.angleturn, 0xCDABu16 as i16);
            assert_eq!(cmd.buttons, 0x01);
        }
    }

    /// Longtics with bytes [0x00, 0x80]: angleturn = 0 | (0x80 << 8) = 0x8000 = -32768.
    #[test]
    fn read_demo_ticcmd_longtics_high_hi_byte()
    {
        unsafe
        {
            let buf: [u8; 5] = [0x00, 0x00, 0x00, 0x80, 0x00];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            read_demo_ticcmd_bytes(&mut p, &mut cmd, true);
            assert_eq!(cmd.angleturn, i16::MIN);
        }
    }

    /// Longtics zero angleturn.
    #[test]
    fn read_demo_ticcmd_longtics_zero_angleturn()
    {
        unsafe
        {
            let buf: [u8; 5] = [0x00, 0x00, 0x00, 0x00, 0x00];
            let mut p = buf.as_ptr() as *mut u8;
            let mut cmd = zeroed_cmd();
            read_demo_ticcmd_bytes(&mut p, &mut cmd, true);
            assert_eq!(cmd.angleturn, 0);
        }
    }

    // --- G_VanillaVersionCode ---

    /// Doom v1.6/v1.666 maps to demo version code 106.
    #[test]
    fn vanilla_version_exe_doom_1_666_is_106()
    {
        assert_eq!(vanilla_version_code_for(exe_doom_1_666), 106);
    }

    /// Doom v1.7/v1.7a maps to demo version code 107.
    #[test]
    fn vanilla_version_exe_doom_1_7_is_107()
    {
        assert_eq!(vanilla_version_code_for(exe_doom_1_7), 107);
    }

    /// Doom v1.8 maps to demo version code 108.
    #[test]
    fn vanilla_version_exe_doom_1_8_is_108()
    {
        assert_eq!(vanilla_version_code_for(exe_doom_1_8), 108);
    }

    /// Doom v1.9 maps to demo version code 109.
    #[test]
    fn vanilla_version_exe_doom_1_9_is_109()
    {
        assert_eq!(vanilla_version_code_for(exe_doom_1_9), 109);
    }

    /// Ultimate Doom and later variants share v1.9's demo code (109).
    #[test]
    fn vanilla_version_ultimate_is_109()
    {
        // exe_ultimate and all later variants map to 109
        assert_eq!(vanilla_version_code_for(exe_ultimate), 109);
    }

    /// Final Doom (`exe_final2`) shares v1.9's demo code (109).
    #[test]
    fn vanilla_version_exe_final2_is_109()
    {
        assert_eq!(vanilla_version_code_for(exe_final2), 109);
    }

    // --- Low-resolution turn rounding (G_BuildTiccmd dtmc extraction) ---

    /// Baseline vectors for the lowres-turn rounding core, captured
    /// against the pre-move inline body (F10 wave C3, §2.3 of the
    /// process spec; commit `1c92fdd`) and re-pointed at the extracted
    /// `lowres_turn_round` after the split -- same vectors, same
    /// results. Small turns accumulate: two consecutive 100-BAM pushes
    /// must round to a single 256-BAM step with the residual carried;
    /// the `0x7FFF + 0x7FFF` pair must wrap through `wrapping_add`
    /// without panicking.
    #[test]
    fn lowres_turn_round_baseline_vectors()
    {
        assert_eq!(lowres_turn_round(100, 0), (0, 100));
        assert_eq!(lowres_turn_round(200, 0), (256, -56));
        // Two-tic accumulation: 100 + 100 rounds to one 256-BAM step.
        assert_eq!(lowres_turn_round(100, 100), (256, -56));
        // Already-aligned value passes through with zero carry.
        assert_eq!(lowres_turn_round(256, 0), (256, 0));
        // Negative turns accumulate downward without rounding up.
        assert_eq!(lowres_turn_round(-100, 0), (0, -100));
        // i16 wrap edges: no panic, wrap semantics preserved.
        assert_eq!(lowres_turn_round(0x7FFF, 0x7FFF), (0, -2));
        assert_eq!(lowres_turn_round(0x7FFF, 1), (-32768, 0));
    }

    // --- G_CheckSpot teleport-fog offset (docs/vanilla-workarounds.md #6) ---

    /// For every legal mapthing angle the fog offset pair must come from the
    /// chocolate switch (reference/chocolate-doom/src/doom/g_game.c:1223-1268),
    /// never from the `I_Error` default arm. The offset index is
    /// `an = (ANG45 >> ANGLETOFINESHIFT) * (angle/45)` = `1024 * angle/45`:
    /// 0/45/90/135 land in the in-range arms (finecosine/finesine at
    /// an = 0/1024/2048/3072); 180/225/270/315 land in the vanilla
    /// signed-index overrun, so those arms read `finetangent[]` at the
    /// exact indices chocolate's case bodies name; 360 is chocolate's
    /// `case 8192` (`finecosine[8192]` overran into `tantoangle[0]` in the
    /// DOS binary's adjacent table layout).
    #[test]
    fn g_check_spot_fog_matches_chocolate_case_table()
    {
        // In-range arms.
        let (xa0, ya0) = unsafe { (*finecosine.0.add(0), finesine[0]) };
        assert_eq!(teleport_fog_offset(0), Some((xa0, ya0)));
        let (xa45, ya45) = unsafe { (*finecosine.0.add(1024), finesine[1024]) };
        assert_eq!(teleport_fog_offset(45), Some((xa45, ya45)));
        let (xa90, ya90) = unsafe { (*finecosine.0.add(2048), finesine[2048]) };
        assert_eq!(teleport_fog_offset(90), Some((xa90, ya90)));
        let (xa135, ya135) = unsafe { (*finecosine.0.add(3072), finesine[3072]) };
        assert_eq!(teleport_fog_offset(135), Some((xa135, ya135)));

        // Finetangent overrun arms (case 4096 / 5120 / 6144 / 7168).
        assert_eq!(
            teleport_fog_offset(180),
            Some((finetangent[2048], finetangent[0]))
        );
        assert_eq!(
            teleport_fog_offset(225),
            Some((finetangent[3072], finetangent[1024]))
        );
        assert_eq!(
            teleport_fog_offset(270),
            Some((finesine[0], finetangent[2048]))
        );
        assert_eq!(
            teleport_fog_offset(315),
            Some((finesine[1024], finetangent[3072]))
        );

        // 360 degrees: chocolate's `case 8192` body verbatim.
        assert_eq!(
            teleport_fog_offset(360),
            Some((tantoangle[0] as fixed_t, finesine[8192]))
        );

        // No legal mapthing angle may reach the `I_Error` default arm.
        for angle in (0..=360).step_by(45)
        {
            assert!(
                teleport_fog_offset(angle).is_some(),
                "angle {} must not hit the I_Error default arm",
                angle
            );
        }

        // The overrun arms ARE the vanilla-defect emulation being replayed,
        // so replaying one must leave a census hit behind (Violations
        // census, docs/vanilla-workarounds.md #6). The crate-wide census
        // lock spans the snapshot/assert window: no sibling census test's
        // reset_all() may zero the counter in between (see violations.rs).
        let _census = violations::CENSUS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let before = violations::hits(VanillaViolation::TeleportFogAngleOverrun);
        assert!(teleport_fog_offset(180).is_some());
        assert!(
            violations::hits(VanillaViolation::TeleportFogAngleOverrun) > before,
            "the finetangent overrun arm must record TeleportFogAngleOverrun"
        );
    }

    // --- G_DoCompleted commercial par time (docs/vanilla-workarounds.md #9) ---

    /// The commercial par-time switch must never index `cpars` out of bounds:
    /// maps 1-32 use `cpars[map-1]`, and map 33 uses chocolate's GAMMALVL0
    /// model (reference/chocolate-doom/src/doom/g_game.c:1526-1535): the
    /// first four bytes of the "Gamma correction OFF" message read as a
    /// little-endian int, scaled by TICRATE with C wrap semantics (the
    /// product overflows i32). The pre-fix code evaluated `cpars[32]` here,
    /// a bounds panic on WAD-reachable input. Map 33's arm IS the emulation,
    /// so it must also record a `ParTimeOverrun` census hit.
    #[test]
    fn commercial_map33_exit_uses_gammalvl0_model_not_panic()
    {
        unsafe
        {
            // Our GAMMALVL0 equivalent is the gamma-message table's first
            // entry ("Gamma correction OFF", m_menu.rs) -- the port's copy of
            // the doom2.exe rodata string the overrun reads into. Pin its
            // prefix so the expectation below stays honest if the text is
            // ever touched.
            let prefix: [u8; 4] = [
                gammamsg[0][0] as u8,
                gammamsg[0][1] as u8,
                gammamsg[0][2] as u8,
                gammamsg[0][3] as u8,
            ];
            let cpars32 = i32::from_le_bytes(prefix);

            // Constant pin (controller-verified: 'G','a','m','m'
            // little-endian): a silent `gammamsg[0]` text change must fail
            // here, not slide both sides of the map33 expectation below.
            assert_eq!(gammalvl0_prefix_i32(), 0x6D6D6147);

            // Maps 1-32: the ordinary cpars[map-1] path, untouched.
            for map in 1..=32
            {
                assert_eq!(
                    commercial_partime(map),
                    Some(35 * cpars[(map - 1) as usize]),
                    "commercial map {} must use cpars[{}]",
                    map,
                    map - 1
                );
            }

            // Guard arm: maps with no reference-defined par time must take
            // the caller's I_Error path, never an unguarded read.
            assert_eq!(commercial_partime(0), None);
            assert_eq!(commercial_partime(34), None);

            // Map 33: 35 * cpars32 with C's wrap (vanilla's product
            // overflows the int; debug Rust must wrap, not panic). The
            // crate-wide census lock spans the snapshot/assert window
            // (see violations.rs).
            let _census = violations::CENSUS_TEST_LOCK
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let before = violations::hits(VanillaViolation::ParTimeOverrun);
            assert_eq!(commercial_partime(33), Some(35i32.wrapping_mul(cpars32)));
            assert!(
                violations::hits(VanillaViolation::ParTimeOverrun) > before,
                "the map33 GAMMALVL0 emulation must record ParTimeOverrun"
            );
        }
    }
}
