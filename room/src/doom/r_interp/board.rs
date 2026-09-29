//! The interpolation board: gate, storage, capture lifecycle, and the
//! hash-indexed mobj arena. Written only from the tic loop's two capture
//! entry points (`d_loop/tic_pump.rs`); sim state is read-only by
//! construction.

use std::ffi::{c_int, c_void};

use crate::doom::c_ffi;
use crate::doom::d_player::{players, MAXPLAYERS};
use crate::doom::m_fixed::{fixed_t, FRACUNIT};
use crate::doom::p_setup::{numsectors, sectors};
use crate::doom::p_tick::{leveltime, thinkercap};

/// Maximum per-axis momentum change of one simulated tic, in fixed-point map
/// units. Vanilla clamps `momx`/`momy` to `MAXMOVE = 30 * FRACUNIT` per axis
/// in `P_XYMovement`, so a legitimate one-tic XY displacement never exceeds
/// this; the teleport snap threshold is twice that.
const MAXMOVE: c_int = 30 * FRACUNIT as c_int;

/// Fixed capacity of the mobj board (prev/curr pairs). Overflow stops the
/// capture (renderer falls back to curr-only for unboarded mobjs); the arena
/// never grows at tic time.
pub const BOARD_MOBJ_SLOTS: usize = 4096;

/// Fixed capacity of the sector board, indexed directly by sector index.
pub const BOARD_SECTOR_SLOTS: usize = 4096;

/// Size of the open-addressing hash table mapping mobj pointers to board
/// slots (kept at 2x the slot count; linear probing).
const MOBJ_HASH_SIZE: usize = BOARD_MOBJ_SLOTS * 2;

/// Sentinel hash-table entry: no slot.
const MOBJ_HASH_EMPTY: u16 = u16::MAX;

/// Master gate (the future F7 "uncapped" setting lives here). When off, all
/// samplers return live simulation values and the render output is
/// bit-identical to the pre-interpolation engine.
pub static mut r_interp_enabled: c_int = 1;

/// One interpolated position quad (fixed-point map units, BAM angle).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PosSample
{
    pub x: c_int,
    pub y: c_int,
    pub z: c_int,
    pub angle: u32,
}

/// Per-mobj board slot: identity + one prev/curr position pair.
#[derive(Clone, Copy)]
pub(super) struct MobjSlot
{
    /// Key: the mobj's address. Pointer reuse across a free+alloc within the
    /// same board lifetime is caught by the teleport-snap displacement guard.
    pub(super) ptr: *mut c_ffi::mobj_t,
    pub(super) prev: PosSample,
    pub(super) curr: PosSample,
    pub(super) prev_valid: bool,
    pub(super) curr_valid: bool,
    /// How many captures this slot has seen. 1 = spawned into the board this
    /// tic (no prev pair -> snap); 2 = first pair.
    pub(super) age: u32,
}

/// Per-player camera pair. `z` is `player.viewz`; `x`/`y`/`angle` come from
/// the player's mobj (the same sources Woof!'s `p_user.c:351-363` captures).
#[derive(Clone, Copy)]
pub(super) struct CamSlot
{
    pub(super) prev: PosSample,
    pub(super) curr: PosSample,
    pub(super) prev_valid: bool,
    pub(super) curr_valid: bool,
}

/// Per-player weapon-sprite pair (slot 0 only; the flash slot mirrors the
/// weapon slot's position every tic, so it samples the weapon pair).
#[derive(Clone, Copy)]
pub(super) struct PsprSlot
{
    pub(super) prev_sx: fixed_t,
    pub(super) prev_sy: fixed_t,
    /// State pointer at capture time; a change between prev and curr means
    /// the sprite switched frames and must snap (Woof! `p_pspr.c:1221`).
    pub(super) prev_state: *mut c_void,
    pub(super) curr_sx: fixed_t,
    pub(super) curr_sy: fixed_t,
    pub(super) curr_state: *mut c_void,
    pub(super) prev_valid: bool,
    pub(super) curr_valid: bool,
}

pub(super) static mut MOBJ_SLOTS: [MobjSlot; BOARD_MOBJ_SLOTS] = [MobjSlot {
    ptr: std::ptr::null_mut(),
    prev: PosSample { x: 0, y: 0, z: 0, angle: 0 },
    curr: PosSample { x: 0, y: 0, z: 0, angle: 0 },
    prev_valid: false,
    curr_valid: false,
    age: 0,
}; BOARD_MOBJ_SLOTS];

static mut MOBJ_HASH: [u16; MOBJ_HASH_SIZE] = [MOBJ_HASH_EMPTY; MOBJ_HASH_SIZE];

/// Number of live mobj slots after the last capture.
static mut MOBJ_LEN: usize = 0;

/// Sector height pairs, indexed directly by sector index (sectors never
/// move in memory, so a hash would be waste).
pub(super) static mut SECTOR_PREV_FLOOR: [c_int; BOARD_SECTOR_SLOTS] = [0; BOARD_SECTOR_SLOTS];
pub(super) static mut SECTOR_PREV_CEIL: [c_int; BOARD_SECTOR_SLOTS] = [0; BOARD_SECTOR_SLOTS];
pub(super) static mut SECTOR_PREV_VALID: [bool; BOARD_SECTOR_SLOTS] = [false; BOARD_SECTOR_SLOTS];
pub(super) static mut SECTOR_CURR_FLOOR: [c_int; BOARD_SECTOR_SLOTS] = [0; BOARD_SECTOR_SLOTS];
pub(super) static mut SECTOR_CURR_CEIL: [c_int; BOARD_SECTOR_SLOTS] = [0; BOARD_SECTOR_SLOTS];
/// Number of sectors covered by the board (`min(numsectors, CAP)`).
pub(super) static mut SECTOR_LEN: usize = 0;

pub(super) static mut CAM_SLOTS: [CamSlot; MAXPLAYERS] = [CamSlot {
    prev: PosSample { x: 0, y: 0, z: 0, angle: 0 },
    curr: PosSample { x: 0, y: 0, z: 0, angle: 0 },
    prev_valid: false,
    curr_valid: false,
}; MAXPLAYERS];

pub(super) static mut PSPR_SLOTS: [PsprSlot; MAXPLAYERS] = [PsprSlot {
    prev_sx: 0,
    prev_sy: 0,
    prev_state: std::ptr::null_mut(),
    curr_sx: 0,
    curr_sy: 0,
    curr_state: std::ptr::null_mut(),
    prev_valid: false,
    curr_valid: false,
}; MAXPLAYERS];

/// `leveltime` latched at the start of the current simulated tic — the
/// render-side mirror of Woof!'s `oldleveltime` (`g_game.c:3519`). While
/// `leveltime <= oldleveltime` the game is paused (or between levels) and
/// nothing interpolates.
pub(super) static mut OLD_LEVELTIME: c_int = 0;

/// `leveltime` at the last capture; a decrease means the level changed and
/// the whole board is stale.
static mut LAST_CAPTURE_LEVELTIME: c_int = 0;

/// Set by [`begin_frame`] once per presented frame; samplers only interpolate
/// while it is set, which keeps the legacy `doomgeneric_Tick` test path
/// bit-exact.
static mut BOARD_ACTIVE: bool = false;

/// Fractional part of the current tic as a 16.16 fixed_t in `[0, FRACUNIT)`.
pub(super) static mut FRACTION: fixed_t = 0;

/// Overflow diagnostics are logged once per board reset, not once per tic.
static mut MOBJ_OVERFLOW_LOGGED: bool = false;

// ---------------------------------------------------------------------------
// Gating + fraction
// ---------------------------------------------------------------------------

/// Whether the render path consults the board. Off = today's output.
pub fn enabled() -> bool
{
    unsafe { r_interp_enabled != 0 }
}

/// Flip the master gate (F7 will own this; tests use it too).
pub fn set_enabled(on: bool)
{
    unsafe
    {
        r_interp_enabled = on as c_int;
    }
}

/// Whether [`begin_frame`] has run for the frame being presented. The legacy
/// `doomgeneric_Tick` path never sets it, keeping that path bit-exact.
pub fn board_active() -> bool
{
    unsafe { BOARD_ACTIVE }
}

/// Called once per presented frame by `doomgeneric_frame`. `now_ms` must come
/// from the same clock the shell feeds to `DG_GetTicksMs` — no new time
/// source is introduced. Also marks the board active so samplers start
/// interpolating (the legacy Tick path stays bit-exact).
pub fn begin_frame(now_ms: u32)
{
    unsafe
    {
        super::board::FRACTION =
            super::math::fraction_from_rel_ms(crate::doom::i_timer::elapsed_ms_from(now_ms));
        BOARD_ACTIVE = true;
    }
}

// ---------------------------------------------------------------------------
// Capture lifecycle — single call site in d_loop.rs (TryRunTics tic loop)
// ---------------------------------------------------------------------------

/// Begin one simulated tic. Mirrors Woof!'s `oldleveltime = leveltime` latch
/// (`g_game.c:3519`) and ages every board one generation (curr -> prev) so
/// the capture after the tic produces the next pair.
///
/// # Safety
/// Reads `leveltime`, `thinkercap` and the board statics; single-threaded
/// game-loop contract applies.
pub unsafe fn begin_tic()
{
    // Level change detection: `leveltime` went backwards.
    if leveltime < OLD_LEVELTIME
    {
        OLD_LEVELTIME = 0;
        reset_board();
    }

    OLD_LEVELTIME = leveltime;

    // Age every board one generation: curr -> prev. All board access goes
    // through raw pointers (a `&mut` into a `static mut` is UB-adjacent).
    let mobj_slots = std::ptr::addr_of_mut!(MOBJ_SLOTS);
    for s in (*mobj_slots).iter_mut()
    {
        if s.curr_valid
        {
            s.prev = s.curr;
            s.prev_valid = true;
            s.curr_valid = false;
        }
    }

    let sector_len = SECTOR_LEN;
    let prev_floor = std::ptr::addr_of_mut!(SECTOR_PREV_FLOOR) as *mut c_int;
    let prev_ceil = std::ptr::addr_of_mut!(SECTOR_PREV_CEIL) as *mut c_int;
    let prev_valid = std::ptr::addr_of_mut!(SECTOR_PREV_VALID) as *mut bool;
    let curr_floor = std::ptr::addr_of!(SECTOR_CURR_FLOOR) as *const c_int;
    let curr_ceil = std::ptr::addr_of!(SECTOR_CURR_CEIL) as *const c_int;
    std::ptr::copy_nonoverlapping(curr_floor, prev_floor, sector_len);
    std::ptr::copy_nonoverlapping(curr_ceil, prev_ceil, sector_len);
    for i in 0..sector_len
    {
        *prev_valid.add(i) = true;
    }

    let cam_slots = std::ptr::addr_of_mut!(CAM_SLOTS);
    for cam in (*cam_slots).iter_mut()
    {
        if cam.curr_valid
        {
            cam.prev = cam.curr;
            cam.prev_valid = true;
            cam.curr_valid = false;
        }
    }

    let pspr_slots = std::ptr::addr_of_mut!(PSPR_SLOTS);
    for ps in (*pspr_slots).iter_mut()
    {
        if ps.curr_valid
        {
            ps.prev_sx = ps.curr_sx;
            ps.prev_sy = ps.curr_sy;
            ps.prev_state = ps.curr_state;
            ps.prev_valid = true;
            ps.curr_valid = false;
        }
    }
}

/// End one simulated tic: movement is complete, capture the new generation.
/// A single centralized walker traverses the thinker list once (dsda
/// `r_fps.c:331-340` registry style, `p_mobj.c:1267-1280` order-independent
/// capture) plus the sector array — no per-mover hooks anywhere.
///
/// # Safety
/// Walks the live thinker list and the sector array; single-threaded
/// game-loop contract applies.
pub unsafe fn end_tic_and_capture()
{
    if leveltime < LAST_CAPTURE_LEVELTIME
    {
        reset_board();
    }
    LAST_CAPTURE_LEVELTIME = leveltime;

    capture_mobjs();
    capture_sectors();
    capture_cameras();
    capture_psprites();
}

/// Reset the whole board (level change; also used by tests). Prev/curr
/// validity is cleared everywhere so nothing interpolates across a level
/// boundary.
///
/// # Safety
/// Writes the board statics; single-threaded contract applies.
pub unsafe fn reset_board()
{
    MOBJ_HASH = [MOBJ_HASH_EMPTY; MOBJ_HASH_SIZE];
    MOBJ_LEN = 0;
    let mobj_slots = std::ptr::addr_of_mut!(MOBJ_SLOTS);
    for s in (*mobj_slots).iter_mut()
    {
        s.prev_valid = false;
        s.curr_valid = false;
        s.age = 0;
        s.ptr = std::ptr::null_mut();
    }
    SECTOR_LEN = 0;
    let sector_valid = std::ptr::addr_of_mut!(SECTOR_PREV_VALID);
    for v in (*sector_valid).iter_mut()
    {
        *v = false;
    }
    let cam_slots = std::ptr::addr_of_mut!(CAM_SLOTS);
    for cam in (*cam_slots).iter_mut()
    {
        cam.prev_valid = false;
        cam.curr_valid = false;
    }
    let pspr_slots = std::ptr::addr_of_mut!(PSPR_SLOTS);
    for ps in (*pspr_slots).iter_mut()
    {
        ps.prev_valid = false;
        ps.curr_valid = false;
    }
    MOBJ_OVERFLOW_LOGGED = false;
    BOARD_ACTIVE = false;
    LAST_CAPTURE_LEVELTIME = 0;
}

/// Address used as the mobj-thinker identity when filtering the thinker list.
/// Comparing `acp1` addresses is how dsda tells mobj thinkers apart from
/// floor/ceiling/plat thinkers in the same list.
fn mobj_thinker_addr() -> usize
{
    crate::doom::p_mobj::P_MobjThinker
        as unsafe extern "C" fn(*mut crate::doom::p_telept::mobj_t) as usize
}

/// Walk the thinker list once and refresh the curr generation for every mobj.
unsafe fn capture_mobjs()
{
    // Re-index the EXISTING slots (the prev/curr pair state lives in the slot
    // array and must survive the rebuild; only the pointer->slot index is
    // regenerated each capture).
    MOBJ_HASH = [MOBJ_HASH_EMPTY; MOBJ_HASH_SIZE];
    for i in 0..MOBJ_LEN
    {
        mobj_hash_insert(MOBJ_SLOTS[i].ptr, i);
    }

    let cap = &raw mut thinkercap;
    let want = mobj_thinker_addr();
    let mut node = (*cap).next;

    // An uninitialised list (boot tics before the first P_InitThinkers) reads
    // as an empty board — engine state must never panic the render side.
    if node.is_null()
    {
        return;
    }

    while node != cap
    {
        let is_mobj = (*node)
            .function
            .acp1
            .is_some_and(|f| f as usize == want);
        if is_mobj
        {
            if MOBJ_LEN >= BOARD_MOBJ_SLOTS
            {
                if !MOBJ_OVERFLOW_LOGGED
                {
                    MOBJ_OVERFLOW_LOGGED = true;
                    log::warn!(
                        "r_interp: mobj board full ({BOARD_MOBJ_SLOTS} slots); \
                         further mobjs sample curr-only this level"
                    );
                }
                break;
            }

            let mo = node as *mut c_ffi::mobj_t;
            let slot = mobj_find_or_insert(mo);
            let s = &mut MOBJ_SLOTS[slot];
            s.curr = PosSample { x: (*mo).x, y: (*mo).y, z: (*mo).z, angle: (*mo).angle };
            s.curr_valid = true;
            s.age = s.age.wrapping_add(1);
        }
        node = (*node).next;
    }
}

/// Place one ptr -> slot index entry into the (already cleared) table.
unsafe fn mobj_hash_insert(mo: *mut c_ffi::mobj_t, idx: usize)
{
    let mut h = mobj_hash(mo);
    while MOBJ_HASH[h] != MOBJ_HASH_EMPTY
    {
        h = (h + 1) % MOBJ_HASH_SIZE;
    }
    MOBJ_HASH[h] = idx as u16;
}

/// Open-addressing lookup (linear probe). Returns the slot index, appending a
/// fresh slot (spawned into the board this tic) when the pointer was never
/// seen. Owns the MOBJ_LEN counter.
unsafe fn mobj_find_or_insert(mo: *mut c_ffi::mobj_t) -> usize
{
    let mut h = mobj_hash(mo);
    loop
    {
        let entry = MOBJ_HASH[h];
        if entry == MOBJ_HASH_EMPTY
        {
            // Truly new slot: no prev generation yet.
            let idx = MOBJ_LEN;
            MOBJ_LEN += 1;
            MOBJ_SLOTS[idx] = MobjSlot {
                ptr: mo,
                prev: PosSample { x: 0, y: 0, z: 0, angle: 0 },
                curr: PosSample { x: 0, y: 0, z: 0, angle: 0 },
                prev_valid: false,
                curr_valid: false,
                age: 0,
            };
            MOBJ_HASH[h] = idx as u16;
            return idx;
        }
        if MOBJ_SLOTS[entry as usize].ptr == mo
        {
            return entry as usize;
        }
        h = (h + 1) % MOBJ_HASH_SIZE;
    }
}

/// Lookup only (no insert) for the render path.
pub(super) unsafe fn mobj_lookup(mo: *mut c_ffi::mobj_t) -> Option<usize>
{
    let mut h = mobj_hash(mo);
    loop
    {
        let entry = MOBJ_HASH[h];
        if entry == MOBJ_HASH_EMPTY
        {
            return None;
        }
        if MOBJ_SLOTS[entry as usize].ptr == mo
        {
            return Some(entry as usize);
        }
        h = (h + 1) % MOBJ_HASH_SIZE;
    }
}

/// Pointer hash (multiplicative, address granularity 16).
fn mobj_hash(mo: *mut c_ffi::mobj_t) -> usize
{
    (mo as usize >> 4).wrapping_mul(0x9E37_79B9) % MOBJ_HASH_SIZE
}

/// Capture every sector's floor/ceiling pair, keyed by sector index. Sector
/// heights that did not move produce an identity pair, which lerps to
/// itself — no mover predicate needed at sample time.
unsafe fn capture_sectors()
{
    let count = std::cmp::min(*std::ptr::addr_of!(numsectors) as usize, BOARD_SECTOR_SLOTS);
    let base = *std::ptr::addr_of!(sectors) as *const c_ffi::sector_t;
    SECTOR_LEN = count;
    for i in 0..count
    {
        let sec = base.add(i);
        SECTOR_CURR_FLOOR[i] = (*sec).floorheight;
        SECTOR_CURR_CEIL[i] = (*sec).ceilingheight;
    }
}

/// Capture the per-player camera quad (mobj x/y/angle + `player.viewz`).
unsafe fn capture_cameras()
{
    for i in 0..MAXPLAYERS
    {
        let mo = players[i].mo;
        if mo.is_null()
        {
            CAM_SLOTS[i].curr_valid = false;
            continue;
        }
        let mo = mo as *mut c_ffi::mobj_t;
        CAM_SLOTS[i].curr = PosSample {
            x: (*mo).x,
            y: (*mo).y,
            z: players[i].viewz,
            angle: (*mo).angle,
        };
        CAM_SLOTS[i].curr_valid = true;
    }
}

/// Capture the weapon-sprite (slot 0) screen position per player.
unsafe fn capture_psprites()
{
    for i in 0..MAXPLAYERS
    {
        let psp = std::ptr::addr_of!(players[i].psprites[0]);
        let ps = &mut PSPR_SLOTS[i];
        ps.curr_sx = (*psp).sx;
        ps.curr_sy = (*psp).sy;
        ps.curr_state = (*psp).state as *mut c_void;
        ps.curr_valid = true;
    }
}

/// True when the prev/curr pair jumped further than the teleport snap
/// threshold (2 x MAXMOVE on either axis). A legitimate per-tic move is
/// momentum-clamped to MAXMOVE per axis, so only teleports (or pointer
/// reuse) trip this.
pub(super) fn teleported(a: PosSample, b: PosSample) -> bool
{
    let limit = 2 * MAXMOVE;
    let dx = a.x.wrapping_sub(b.x);
    let dy = a.y.wrapping_sub(b.y);
    // Comparison form (not .abs()): safe for the full i32 range.
    dx > limit || dx < -limit || dy > limit || dy < -limit
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests
{
    use std::ffi::{c_int, c_void};
    use std::sync::Mutex;

    use super::super::math::set_fraction;
    use crate::doom::c_ffi;
    use crate::doom::d_player::{players, NUMPSPRITES, PlayerT, PspdefT};
    use crate::doom::info::MF_MISSILE;
    use crate::doom::m_fixed::{fixed_t, FRACUNIT};
    use crate::doom::p_setup::{numsectors, sectors};
    use crate::doom::p_tick::{leveltime, thinker_t, thinkercap};
    use crate::doom::r_interp::{
        fraction, sample_camera, sample_mobj, sample_psp, sector_ceiling, sector_floor,
    };

    use super::*;

    /// One lock for every test that touches engine globals (thinkercap,
    /// sectors, players, leveltime, viewplayer): cargo runs tests in parallel
    /// threads and the engine state is process-global.
    static LOCK: Mutex<()> = Mutex::new(());

    /// Half tic — the midpoint fraction.
    const HALF: fixed_t = FRACUNIT / 2;
    /// Vanilla eye height, used as the camera z fixture start.
    const EYE_Z: c_int = 41 * FRACUNIT as c_int;

    #[test]
    fn teleport_threshold_is_two_maxmove()
    {
        let a = PosSample { x: 0, y: 0, z: 0, angle: 0 };
        // A per-tic move up to MAXMOVE per axis is legitimate.
        assert!(!teleported(a, PosSample { x: MAXMOVE, y: MAXMOVE, z: 0, angle: 0 }));
        // Beyond 2 x MAXMOVE on either axis snaps.
        assert!(teleported(a, PosSample { x: 2 * MAXMOVE + 1, y: 0, z: 0, angle: 0 }));
        assert!(teleported(a, PosSample { x: 0, y: 2 * MAXMOVE + 1, z: 0, angle: 0 }));
    }

    // -- board + guards ------------------------------------------------------
    //
    // The census/guard tests fabricate a mini engine state (a mobj thinker
    // list, a sector array, one player) around the global state, run
    // begin_tic / end_tic_and_capture pairs, and assert the board contract:
    // every census entry has a snapshot pair and interpolates to the expected
    // midpoint. This table IS the module's mover-census contract:
    // mob position, mob teleport snap, mob spawn, missile first pair, paused,
    // level-change reset, sector floor mover, sector ceiling mover, static
    // sector, player camera (+ first-tic guard), weapon sprite (+ state
    // change snap), legacy Tick path (board inactive).

    /// Fabricated mobj living in a Box (stable address), linked into the
    /// global thinker list so the capture walker finds it.
    struct FakeMobj;

    unsafe fn spawn_fake_mobj(flags: c_int) -> *mut c_ffi::mobj_t
    {
        let mut raw = Box::new(std::mem::zeroed::<c_ffi::mobj_t>());
        raw.flags = flags;
        // The walker filters the thinker list by the acp1 address; write the
        // function's address into the raw pointer field (same bits).
        raw.thinker_fn = crate::doom::p_mobj::P_MobjThinker
            as unsafe extern "C" fn(*mut crate::doom::p_telept::mobj_t)
            as usize as *mut c_void;
        let ptr = Box::into_raw(raw);
        let cap = &raw mut thinkercap;
        (*ptr).thinker_prev = (*cap).prev as *mut c_void;
        (*ptr).thinker_next = cap as *mut c_void;
        (*(*cap).prev).next = ptr as *mut thinker_t;
        (*cap).prev = ptr as *mut thinker_t;
        ptr
    }

    unsafe fn despawn_fake_mobj(ptr: *mut c_ffi::mobj_t)
    {
        let prev = (*ptr).thinker_prev as *mut thinker_t;
        let next = (*ptr).thinker_next as *mut thinker_t;
        (*prev).next = next;
        (*next).prev = prev;
        drop(Box::from_raw(ptr));
    }

    /// RAII save/restore of every global the board tests touch.
    struct GlobalsGuard
    {
        saved_cap: thinker_t,
        saved_sectors: *mut c_ffi::sector_t,
        saved_numsectors: c_int,
        saved_leveltime: c_int,
        saved_viewplayer: *mut PlayerT,
        saved_p0_mo: *mut crate::doom::d_player::mobj_t,
        saved_p0_viewz: c_int,
        saved_p0_psprites: [PspdefT; NUMPSPRITES],
        saved_frac: fixed_t,
        saved_active: bool,
        saved_enabled: c_int,
    }

    impl GlobalsGuard
    {
        unsafe fn new() -> Self
        {
            GlobalsGuard
            {
                saved_cap: std::ptr::read(&raw const thinkercap),
                saved_sectors: *std::ptr::addr_of!(sectors),
                saved_numsectors: *std::ptr::addr_of!(numsectors),
                saved_leveltime: leveltime,
                saved_viewplayer: crate::doom::r_main::viewplayer,
                saved_p0_mo: players[0].mo,
                saved_p0_viewz: players[0].viewz,
                saved_p0_psprites: players[0].psprites,
                saved_frac: fraction(),
                saved_active: board_active(),
                saved_enabled: r_interp_enabled,
            }
        }
    }

    impl Drop for GlobalsGuard
    {
        fn drop(&mut self)
        {
            unsafe
            {
                std::ptr::write(&raw mut thinkercap, self.saved_cap);
                std::ptr::write(std::ptr::addr_of_mut!(sectors), self.saved_sectors);
                std::ptr::write(std::ptr::addr_of_mut!(numsectors), self.saved_numsectors);
                std::ptr::write(std::ptr::addr_of_mut!(leveltime), self.saved_leveltime);
                crate::doom::r_main::viewplayer = self.saved_viewplayer;
                players[0].mo = self.saved_p0_mo;                players[0].viewz = self.saved_p0_viewz;
                players[0].psprites = self.saved_p0_psprites;
                set_fraction(self.saved_frac);
                r_interp_enabled = self.saved_enabled;
                BOARD_ACTIVE = self.saved_active;
                OLD_LEVELTIME = 0;
                LAST_CAPTURE_LEVELTIME = 0;
            }
        }
    }

    /// Fabricated sector array with stable addresses.
    unsafe fn install_fake_sectors(count: usize) -> *mut c_ffi::sector_t
    {
        let layout = std::alloc::Layout::array::<c_ffi::sector_t>(count).unwrap();
        let base = std::alloc::alloc_zeroed(layout) as *mut c_ffi::sector_t;
        std::ptr::write(std::ptr::addr_of_mut!(sectors), base);
        std::ptr::write(std::ptr::addr_of_mut!(numsectors), count as c_int);
        base
    }

    unsafe fn drop_fake_sectors(base: *mut c_ffi::sector_t, count: usize)
    {
        let layout = std::alloc::Layout::array::<c_ffi::sector_t>(count).unwrap();
        std::alloc::dealloc(base as *mut u8, layout);
    }

    /// One full simulated tic: age the board, mirror P_Ticker's leveltime
    /// increment (which happens mid-tic in the engine), let the caller mutate
    /// the fabricated world, then capture. The paused-guard test blocks below
    /// pin the frozen-leveltime behaviour by hand instead.
    macro_rules! tic
    {
        ($setup:expr, $move:expr) =>
        {{
            begin_tic();
            $setup;
            leveltime += 1;
            $move;
            end_tic_and_capture();
        }};
    }

    #[test]
    fn census_every_mover_class_gets_a_pair_and_midpoint()
    {
        let _lock = LOCK.lock().unwrap();
        unsafe
        {
            let _g = GlobalsGuard::new();
            // The lib test process never booted the engine, so thinkercap is
            // zeroed; initialise it to the empty-list state (P_InitThinkers).
            thinkercap.prev = &raw mut thinkercap;
            thinkercap.next = &raw mut thinkercap;
            reset_board();
            set_enabled(true);
            // Mark the frame active (as doomgeneric_frame does); the fraction
            // passed at the shell boundary is overridden with the exact test
            // fraction right after.
            begin_frame(0);
            set_fraction(HALF);
            leveltime = 100;

            // -- world: 4 sectors, 5 mobjs, one player --------------------------
            const NSEC: usize = 4;
            let secs = install_fake_sectors(NSEC);

            // Census entry: sector floor mover (platform/lift/floor class).
            let floor0 = 48 * FRACUNIT as c_int;
            let floor1 = 56 * FRACUNIT as c_int;
            (*secs.add(1)).floorheight = floor0;

            // Census entry: sector ceiling mover (door class).
            let ceil0 = 128 * FRACUNIT as c_int;
            let ceil1 = 120 * FRACUNIT as c_int;
            (*secs.add(2)).ceilingheight = ceil0;

            // Census entry: static sector (identity pair lerps to itself).
            (*secs.add(3)).floorheight = 16 * FRACUNIT as c_int;
            (*secs.add(3)).ceilingheight = 160 * FRACUNIT as c_int;

            let mob = spawn_fake_mobj(0);
            let teleporter = spawn_fake_mobj(0);
            let spawnee = spawn_fake_mobj(0);
            let missile = spawn_fake_mobj(MF_MISSILE);

            const X0: c_int = 100 * FRACUNIT as c_int;
            const X1: c_int = 110 * FRACUNIT as c_int;
            const MID: c_int = 105 * FRACUNIT as c_int;
            const TELE: c_int = 600 * FRACUNIT as c_int;
            (*mob).x = X0;
            (*teleporter).x = X0;
            (*spawnee).x = X0;
            (*missile).x = X0;

            // Census entry: player camera.
            let cam_mo = spawn_fake_mobj(0);
            (*cam_mo).x = X0;
            players[0].mo = cam_mo as *mut crate::doom::d_player::mobj_t;
            players[0].viewz = EYE_Z;
            crate::doom::r_main::viewplayer = &raw mut players[0];

            // Census entry: weapon sprite (bob moves sx).
            players[0].psprites[0].state = 0x10 as *mut _;
            players[0].psprites[0].sx = 0;
            players[0].psprites[0].sy = 32 * FRACUNIT as c_int;

            // -- capture pair: v0 then v1 ---------------------------------------
            tic!(
                {},
                {
                    (*mob).x = X0;
                    (*teleporter).x = X0;
                    (*spawnee).x = X0;
                    (*missile).x = X0;
                    (*cam_mo).x = X0;
                    players[0].viewz = EYE_Z;
                    players[0].psprites[0].sx = 0;
                    (*secs.add(1)).floorheight = floor0;
                    (*secs.add(2)).ceilingheight = ceil0;
                    (*secs.add(0)).floorheight = floor0;
                    (*secs.add(0)).ceilingheight = ceil0;
                }
            );
            tic!(
                {},
                {
                    (*mob).x = X1;
                    (*teleporter).x = X0 + TELE;
                    (*spawnee).x = X1;
                    (*missile).x = X1;
                    (*cam_mo).x = X1;
                    players[0].viewz = EYE_Z + 4 * FRACUNIT as c_int;
                    players[0].psprites[0].sx = 4 * FRACUNIT as c_int;
                    (*secs.add(1)).floorheight = floor1;
                    (*secs.add(2)).ceilingheight = ceil1;
                    (*secs.add(0)).floorheight = floor0;
                    (*secs.add(0)).ceilingheight = ceil0;
                }
            );

            // -- census assertions ------------------------------------------------
            // mob: pair + midpoint.
            let got = sample_mobj(mob);
            assert_eq!(got.x, MID, "census mob: must lerp to midpoint");

            // camera: pair + midpoint (x and viewz).
            let got = sample_camera(&raw mut players[0], cam_mo);
            assert_eq!(got.x, MID, "census camera: x midpoint");
            assert_eq!(
                got.z,
                EYE_Z + 2 * FRACUNIT as c_int,
                "census camera: viewz midpoint"
            );

            // weapon sprite: pair + midpoint; flash slot mirrors weapon slot.
            let (sx, _sy) = sample_psp(&raw mut players[0].psprites[0]);
            assert_eq!(sx, 2 * FRACUNIT as c_int, "census psprite: sx midpoint");
            let (sx_flash, _) = sample_psp(&raw mut players[0].psprites[1]);
            assert_eq!(sx_flash, sx, "census psprite: flash glued to weapon");

            // sector floor mover: pair + midpoint.
            let got = sector_floor(secs.add(1));
            assert_eq!(got, 52 * FRACUNIT as c_int, "census sector floor: midpoint");

            // sector ceiling mover: pair + midpoint.
            let got = sector_ceiling(secs.add(2));
            assert_eq!(got, 124 * FRACUNIT as c_int, "census sector ceiling: midpoint");

            // static sector: identity.
            let got = sector_floor(secs.add(3));
            assert_eq!(got, 16 * FRACUNIT as c_int, "census static sector: identity");
            let got = sector_ceiling(secs.add(3));
            assert_eq!(got, 160 * FRACUNIT as c_int, "census static sector: identity");

            // -- guard census ------------------------------------------------------
            // Teleport: prev/curr displacement > 2 x MAXMOVE snaps to curr.
            let got = sample_mobj(teleporter);
            assert_eq!(got.x, X0 + TELE, "guard teleport: must snap to curr");

            // Spawn: spawnee existed only since capture 2 -> no prev -> curr.
            // Capture it once (age 1), then sample.
            let fresh = spawn_fake_mobj(0);
            (*fresh).x = X1;
            tic!(
                {},
                {
                    (*fresh).x = X1;
                }
            );
            let got = sample_mobj(fresh);
            assert_eq!(got.x, X1, "guard spawn: curr only, no lerp");
            despawn_fake_mobj(fresh);

            // Missile first pair: snaps (Woof! p_mobj.c:752-756).
            let got = sample_mobj(missile);
            assert_eq!(got.x, X1, "guard missile: first pair must snap");

            // Psprite state change: a capture pair whose state pointer changed
            // snaps (Woof! p_pspr.c:1221) — the weapon never slides between
            // two different frames. Without the guard this pair would lerp
            // 4 -> 8 units to a 6-unit midpoint instead of snapping to 8.
            tic!(
                {},
                {
                    players[0].psprites[0].state = 0x20 as *mut _;
                    players[0].psprites[0].sx = 8 * FRACUNIT as c_int;
                }
            );
            let (sx, _) = sample_psp(&raw mut players[0].psprites[0]);
            assert_eq!(
                sx,
                8 * FRACUNIT as c_int,
                "guard psprite state change: must snap to curr"
            );

            // Paused: leveltime latched equal -> no interpolation.
            OLD_LEVELTIME = leveltime;
            let got = sample_mobj(mob);
            assert_eq!(got.x, X1, "guard paused: no interpolation");
            let got = sample_camera(&raw mut players[0], cam_mo);
            assert_eq!(got.x, X1, "guard paused: camera frozen");
            OLD_LEVELTIME = 0;

            // Disabled gate: today's output bit-for-bit.
            set_enabled(false);
            let got = sample_mobj(mob);
            assert_eq!(got.x, X1, "gate off: live value");
            set_enabled(true);

            // Legacy Tick path: board inactive -> live value.
            BOARD_ACTIVE = false;
            let got = sample_mobj(mob);
            assert_eq!(got.x, X1, "Tick path: board inactive -> live value");
            BOARD_ACTIVE = true;

            // First level tic: camera guard leveltime > 1 (r_main.c:709-713).
            // Restore forward to the last captured tic afterwards so the
            // monotonic level-change detector does not fire.
            leveltime = 1;
            let got = sample_camera(&raw mut players[0], cam_mo);
            assert_eq!(got.x, X1, "guard first tic: camera snaps");
            leveltime = LAST_CAPTURE_LEVELTIME;

            // -- missile after the first pair lerps ---------------------------------
            const X2: c_int = 114 * FRACUNIT as c_int;
            tic!(
                {},
                {
                    (*missile).x = X2;
                }
            );
            let got = sample_mobj(missile);
            assert_eq!(
                got.x,
                (X1 + X2) / 2,
                "missile lerps once past its first pair"
            );

            // -- level change: board resets, no stale lerp --------------------------
            leveltime = 500;
            tic!(
                {},
                {
                    (*mob).x = X0;
                }
            );
            leveltime = 1; // new level loads mid-tic; leveltime restarts
            tic!(
                {},
                {
                    (*mob).x = X1; // zone reuse may alias old addresses
                }
            );
            let got = sample_mobj(mob);
            assert_eq!(got.x, X1, "level change: stale prev must not lerp");

            // -- cleanup ------------------------------------------------------------
            despawn_fake_mobj(mob);
            despawn_fake_mobj(teleporter);
            despawn_fake_mobj(spawnee);
            despawn_fake_mobj(missile);
            despawn_fake_mobj(cam_mo);
            drop_fake_sectors(secs, NSEC);
        }
    }
}
