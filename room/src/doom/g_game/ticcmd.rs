//! Ticcmd assembly: the local player's per-tic command builder
//! (`build_ticcmd`, upstream `G_BuildTiccmd`), its input marshalling
//! helpers (mouse/joystick button latches, weapon cycling) and the
//! input-latch statics shared with `responder` and `actions`.
//!
//! `build_ticcmd`'s output bytes ARE the recorded demo stream, so this
//! file is demo-synchronization surface wholesale (F10 wave C3
//! adjudication): the consistency byte, the two-stage turn tables, the
//! `BT_CHANGE` weapon encoding and the lowres-turn rounding (extracted
//! to `dtmc::lowres_turn_round`) are all on-stream. The bodies moved
//! verbatim from pre-split `g_game.rs` (only the mechanical helper
//! renames below and the sanctioned lowres extraction differ); see the
//! module root for the mapping table.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_uint};

use crate::doom::d_loop::ticdup;
use crate::doom::d_mode::{doom, shareware};
use crate::doom::d_player::TiccmdT;
use crate::doom::doomstat::{gamemode, gamemission};
use crate::doom::hu_stuff::HU_dequeueChatChar;
use crate::doom::m_controls::{
    dclick_use, joybfire, joybnextweapon, joybprevweapon, joybspeed, joybstrafe, joybstrafeleft,
    joybstraferight, joybuse, key_down, key_fire, key_left, key_right, key_speed, key_strafe,
    key_strafeleft, key_straferight, key_up, key_use, key_weapon1, key_weapon2, key_weapon3,
    key_weapon4, key_weapon5, key_weapon6, key_weapon7, key_weapon8, mousebbackward, mousebfire,
    mousebforward, mousebnextweapon, mousebprevweapon, mousebstrafe, mousebstrafeleft,
    mousebstraferight, mousebuse,
};

use super::consts::{
    boolean, pw_strength, wp_bfg, wp_chainsaw, wp_fist, wp_nochange, wp_pistol, wp_plasma,
    wp_supershotgun, BT_ATTACK, BT_CHANGE, BT_SPECIAL, BT_WEAPONSHIFT, BT_USE, BTS_PAUSE,
    BTS_SAVESHIFT, BTS_SAVEGAME, GS_LEVEL, MAX_JOY_BUTTONS, MAX_MOUSE_BUTTONS, NUMKEYS,
};
use super::state::{
    angleturn, consistancy, consoleplayer, forwardmove, gamestate, lowres_turn, players, sendpause,
    sendsave, sidemove, testcontrols_mousespeed, SAVEGAMESLOT,
};
use super::dtmc::lowres_turn_round;

// ---------------------------------------------------------------------------
// Input latches (module-private; the reset in `G_DoLoadLevel` and the
// event writes in `G_Responder` reach these across subfiles, so the
// multi-writer set is `pub(super)`)
// ---------------------------------------------------------------------------

/// Per-key "is held down" flags, indexed by Doom key code.
pub(super) static mut GAMEKEYDOWN: [boolean; NUMKEYS] = [0; NUMKEYS];

/// Mouse button held-down flags (slot 0 unused to allow `[-1]` indexing).
pub(super) static mut MOUSEARRAY: [boolean; MAX_MOUSE_BUTTONS + 1] = [0; MAX_MOUSE_BUTTONS + 1];
// mousebuttons is &mousearray[1] in C — negative indexing; access via MOUSEARRAY[1+n]
/// Most recent raw mouse delta on the X axis (cleared each tic by `G_BuildTiccmd`).
pub(super) static mut MOUSEX: c_int = 0;
/// Most recent raw mouse delta on the Y axis (cleared each tic by `G_BuildTiccmd`).
pub(super) static mut MOUSEY: c_int = 0;

/// Most recent joystick X (turn / strafe) axis value.
pub(super) static mut JOYXMOVE: c_int = 0;
/// Most recent joystick Y (forward / back) axis value.
pub(super) static mut JOYYMOVE: c_int = 0;
/// Most recent joystick strafe axis value.
pub(super) static mut JOYSTRAFEMOVE: c_int = 0;
/// Joystick button held-down flags (slot 0 unused to allow `[-1]` indexing).
pub(super) static mut JOYARRAY: [boolean; MAX_JOY_BUTTONS + 1] = [0; MAX_JOY_BUTTONS + 1];
// joybuttons is &joyarray[1] in C — negative indexing; access via JOYARRAY[1+n]

/// Pending weapon-cycle direction: `-1` previous, `+1` next, `0` none.
pub(super) static mut NEXT_WEAPON: c_int = 0;

/// Tics that turn input has been held; drives two-stage accelerative turning.
static mut TURNHELD: c_int = 0;

/// Tics since the last forward-mouse click for double-click-as-use detection.
static mut DCLICKTIME: c_int = 0;
/// Last sampled state of the mousebforward button for double-click detection.
static mut DCLICKSTATE: boolean = 0;
/// Click count toward a forward-mouse double-click (2 triggers `BT_USE`).
static mut DCLICKS: c_int = 0;
/// Tics since the last strafe-button click for double-click detection.
static mut DCLICKTIME2: c_int = 0;
/// Last sampled state of the strafe button for double-click detection.
static mut DCLICKSTATE2: boolean = 0;
/// Click count toward a strafe-button double-click (2 triggers `BT_USE`).
static mut DCLICKS2: c_int = 0;

/// Carry for low-resolution turn rounding (static local in `G_BuildTiccmd`).
static mut LOWRES_TURN_CARRY: i16 = 0;

// ---------------------------------------------------------------------------
// Weapon ordering table (for prev/next weapon cycling)
// ---------------------------------------------------------------------------

/// One entry of the prev/next weapon cycling table.
///
/// `weapon` is the concrete weapon checked for availability; `weapon_num` is
/// the slot index ultimately emitted in the ticcmd (e.g. both fist and
/// chainsaw cycle to slot 1, both shotgun and supershotgun to slot 3).
struct WeaponOrder
{
    /// Concrete weapon identifier (`wp_*`) used for selectability tests.
    weapon: c_int,
    /// Slot number (1-8) encoded into the `BT_CHANGE` ticcmd field.
    weapon_num: c_int,
}

/// Cyclic ordering of weapons used by next/previous-weapon hotkeys.
///
/// Mirrors `weapon_order_table[]` in `g_game.c`. The order determines the
/// scan direction: indices 0..=8 are walked clockwise (forward direction)
/// or counter-clockwise (back), skipping unavailable weapons.
static WEAPON_ORDER_TABLE: [WeaponOrder; 9] = [
    WeaponOrder {
        weapon: wp_fist,
        weapon_num: wp_fist,
    },
    WeaponOrder {
        weapon: wp_chainsaw,
        weapon_num: wp_fist,
    },
    WeaponOrder {
        weapon: wp_pistol,
        weapon_num: wp_pistol,
    },
    WeaponOrder {
        weapon: 3,
        /* shotgun */ weapon_num: 3,
    },
    WeaponOrder {
        weapon: 8,
        /* supershotgun */ weapon_num: 3,
    },
    WeaponOrder {
        weapon: 4,
        /* chaingun */ weapon_num: 4,
    },
    WeaponOrder {
        weapon: wp_plasma - 1,
        /* missile */ weapon_num: wp_plasma - 1,
    },
    WeaponOrder {
        weapon: wp_plasma,
        weapon_num: wp_plasma,
    },
    WeaponOrder {
        weapon: wp_bfg,
        weapon_num: wp_bfg,
    },
];

// ---------------------------------------------------------------------------
// logical_gamemission helper (mirrors doomstat.h macro)
// ---------------------------------------------------------------------------

/// Mirror of the `logical_gamemission` macro from `doomstat.h`.
///
/// Collapses the TNT/Plutonia mission types onto plain `doom2`, since they
/// share the Doom II ruleset; called by `weapon_selectable` to avoid
/// branching on every Doom II variant individually.
///
/// # Safety
/// Reads the `gamemission` global; safe as long as the caller respects the
/// single-threaded engine convention.
#[inline]
unsafe fn logical_gamemission() -> c_int
{
    use crate::doom::d_mode::{doom2, pack_plut, pack_tnt};
    if gamemission == pack_tnt || gamemission == pack_plut
    {
        doom2
    }
    else
    {
        gamemission
    }
}

// ---------------------------------------------------------------------------
// Mouse/joystick button accessors
// (In C: mousebuttons = &mousearray[1], joybuttons = &joyarray[1],
//  allowing negative index -1 meaning "no button".)
// ---------------------------------------------------------------------------

/// Read a mouse button state by C-style "may be `-1`" index.
///
/// Vanilla Doom stores mouse buttons in `mousearray[MAX_MOUSE_BUTTONS+1]`
/// with `mousebuttons = &mousearray[1]`, so `mousebuttons[-1]` is a legal
/// "no button bound" sentinel that always reads false. This helper restores
/// the same behaviour without aliasing (docs/vanilla-workarounds.md,
/// "not a bug emulation" disposition).
///
/// Returns `0` for `n < 0` or `n >= MAX_MOUSE_BUTTONS`, otherwise the
/// currently latched state of mouse button `n`.
///
/// # Safety
/// Reads the `MOUSEARRAY` global; safe under the single-threaded contract.
#[doc(alias = "mousebutton")]
#[inline]
unsafe fn mouse_button(n: c_int) -> boolean
{
    if n < 0 || n >= MAX_MOUSE_BUTTONS as c_int
    {
        return 0;
    }
    MOUSEARRAY[(n + 1) as usize]
}

/// Joystick equivalent of [`mouse_button`]; same `-1`-as-unbound semantics.
///
/// # Safety
/// Reads the `JOYARRAY` global; safe under the single-threaded contract.
#[doc(alias = "joybutton")]
#[inline]
unsafe fn joy_button(n: c_int) -> boolean
{
    if n < 0 || n >= MAX_JOY_BUTTONS as c_int
    {
        return 0;
    }
    JOYARRAY[(n + 1) as usize]
}

// ---------------------------------------------------------------------------
// WeaponSelectable (static)
// ---------------------------------------------------------------------------

/// Decide whether `weapon` is eligible for selection by the prev/next
/// weapon cycler.
///
/// Mirrors `WeaponSelectable` in `g_game.c`. Returns `false` when the
/// weapon is unavailable in the current `gamemission` / `gamemode` (e.g.
/// supershotgun in Doom 1, plasma/BFG in shareware), when the console
/// player does not own it, or when it is the fist while the chainsaw is
/// owned without an active berserk power.
///
/// The decision feeds the `BT_CHANGE` payload bytes the cycler emits, so
/// it is demo-synchronization surface (F10 wave C3 adjudication).
///
/// # Safety
/// Reads several engine globals (`gamemission`, `gamemode`, `players`,
/// `consoleplayer`); safe under the single-threaded engine convention.
#[doc(alias = "WeaponSelectable")]
unsafe fn weapon_selectable(weapon: c_int) -> bool
{
    // Can't select supershotgun in Doom 1.
    if weapon == wp_supershotgun && logical_gamemission() == doom
    {
        return false;
    }
    // Plasma and BFG unavailable in shareware.
    if (weapon == wp_plasma || weapon == wp_bfg) && gamemission == doom && gamemode == shareware
    {
        return false;
    }
    let cp = consoleplayer as usize;
    if players[cp].weaponowned[weapon as usize] == 0
    {
        return false;
    }
    // Can't select fist if we have chainsaw, unless we also have berserk.
    if weapon == wp_fist
        && players[cp].weaponowned[wp_chainsaw as usize] != 0
        && players[cp].powers[pw_strength] == 0
    {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// G_NextWeapon (static)
// ---------------------------------------------------------------------------

/// Walk [`WEAPON_ORDER_TABLE`] from the player's current weapon and return
/// the slot number to switch to.
///
/// `direction` is `+1` for "next weapon" or `-1` for "previous weapon". The
/// search wraps and stops on the first selectable entry; if none are
/// selectable it returns the slot for the player's current weapon (the
/// `i == start_i` guard prevents an infinite loop).
///
/// Mirrors `G_NextWeapon` in `g_game.c`; the returned slot is encoded into
/// the ticcmd's `BT_CHANGE` field, so this is demo-sync surface.
///
/// # Safety
/// Reads `players[consoleplayer]` and `WEAPON_ORDER_TABLE`; safe under the
/// single-threaded engine convention.
#[doc(alias = "G_NextWeapon")]
unsafe fn next_weapon_slot(direction: c_int) -> c_int
{
    let cp = consoleplayer as usize;
    let weapon = if players[cp].pendingweapon == wp_nochange
    {
        players[cp].readyweapon
    }
    else
    {
        players[cp].pendingweapon
    };

    let n = WEAPON_ORDER_TABLE.len() as c_int;
    let mut i = 0;
    while i < n
    {
        if WEAPON_ORDER_TABLE[i as usize].weapon == weapon
        {
            break;
        }
        i += 1;
    }

    let start_i = i;
    loop
    {
        i += direction;
        i = (i + n) % n;
        if i == start_i || weapon_selectable(WEAPON_ORDER_TABLE[i as usize].weapon)
        {
            break;
        }
    }
    WEAPON_ORDER_TABLE[i as usize].weapon_num
}

// ---------------------------------------------------------------------------
// G_BuildTiccmd
// ---------------------------------------------------------------------------

/// Assemble one tic's `TiccmdT` for the local console player from the latest
/// input snapshot, and apply low-resolution turn rounding when recording a
/// vanilla demo.
///
/// `maketic` is the tic number being built; it indexes the consistency-check
/// ring buffer for the local player. Behaviour mirrors `G_BuildTiccmd` in
/// `g_game.c` exactly:
///
/// * Forward / strafe / turn input from keys, joystick and mouse is summed
///   with two-stage accelerative turning (first 6 tics use the "slow" turn
///   table, beyond that the regular table or the speed-button "fast" table).
/// * Mouse movement is added to `forward` directly and to either `side`
///   (when strafing) or `angleturn` (otherwise) scaled by `mouseSensitivity`.
/// * Weapon-cycle hotkeys are encoded into the `BT_CHANGE` field.
/// * Mouse forward/strafe double-clicks synthesise `BT_USE` when
///   `dclick_use` is on.
/// * Pause and savegame requests are encoded into the `BT_SPECIAL` field.
/// * When `lowres_turn` is set, `angleturn` is rounded to a 256-BAM boundary
///   and the residual carried into the next tic (so successive small turns
///   accumulate accurately in 1-byte-per-tic demos) -- the arithmetic lives
///   in [`super::dtmc::lowres_turn_round`].
///
/// # Safety
/// `cmd` must point to a writable `TiccmdT`; the entire struct is zeroed
/// before assembly. The C symbol is pinned (`G_BuildTiccmd`): the d_net /
/// d_loop extern blocks link it by symbol, and `d_net`'s
/// `DOOM_LOOP_INTERFACE` binds it as a function pointer.
#[doc(alias = "G_BuildTiccmd")]
#[export_name = "G_BuildTiccmd"]
pub unsafe extern "C" fn build_ticcmd(cmd: *mut TiccmdT, maketic: c_int)
{
    use crate::doom::c_ffi::BACKUPTICS;

    let cmd = &mut *cmd;
    *cmd = std::mem::zeroed();

    cmd.consistancy = consistancy[consoleplayer as usize][(maketic as usize) % BACKUPTICS];

    let strafe = (GAMEKEYDOWN[key_strafe as usize] != 0)
        || (mouse_button(mousebstrafe) != 0)
        || (joy_button(joybstrafe) != 0);

    // "joyb_speed = 31" autorun hack: key_speed >= NUMKEYS means always running
    let speed = (key_speed >= NUMKEYS as c_int)
        || (joybspeed >= MAX_JOY_BUTTONS as c_int)
        || (GAMEKEYDOWN[key_speed as usize] != 0)
        || (joy_button(joybspeed) != 0);
    let speed = speed as usize;

    let mut forward: c_int = 0;
    let mut side: c_int = 0;

    // Two-stage accelerative turning
    if JOYXMOVE != 0 || GAMEKEYDOWN[key_right as usize] != 0 || GAMEKEYDOWN[key_left as usize] != 0
    {
        TURNHELD += ticdup;
    }
    else
    {
        TURNHELD = 0;
    }

    let tspeed = if TURNHELD < 6 { 2usize } else { speed };

    if strafe
    {
        if GAMEKEYDOWN[key_right as usize] != 0
        {
            side += sidemove[speed];
        }
        if GAMEKEYDOWN[key_left as usize] != 0
        {
            side -= sidemove[speed];
        }
        if JOYXMOVE > 0
        {
            side += sidemove[speed];
        }
        if JOYXMOVE < 0
        {
            side -= sidemove[speed];
        }
    }
    else
    {
        if GAMEKEYDOWN[key_right as usize] != 0
        {
            cmd.angleturn -= angleturn[tspeed] as i16;
        }
        if GAMEKEYDOWN[key_left as usize] != 0
        {
            cmd.angleturn += angleturn[tspeed] as i16;
        }
        if JOYXMOVE > 0
        {
            cmd.angleturn -= angleturn[tspeed] as i16;
        }
        if JOYXMOVE < 0
        {
            cmd.angleturn += angleturn[tspeed] as i16;
        }
    }

    if GAMEKEYDOWN[key_up as usize] != 0
    {
        forward += forwardmove[speed];
    }
    if GAMEKEYDOWN[key_down as usize] != 0
    {
        forward -= forwardmove[speed];
    }
    if JOYYMOVE < 0
    {
        forward += forwardmove[speed];
    }
    if JOYYMOVE > 0
    {
        forward -= forwardmove[speed];
    }

    if GAMEKEYDOWN[key_strafeleft as usize] != 0
        || joy_button(joybstrafeleft) != 0
        || mouse_button(mousebstrafeleft) != 0
        || JOYSTRAFEMOVE < 0
    {
        side -= sidemove[speed];
    }
    if GAMEKEYDOWN[key_straferight as usize] != 0
        || joy_button(joybstraferight) != 0
        || mouse_button(mousebstraferight) != 0
        || JOYSTRAFEMOVE > 0
    {
        side += sidemove[speed];
    }

    // Buttons
    cmd.chatchar = HU_dequeueChatChar() as u8;

    if GAMEKEYDOWN[key_fire as usize] != 0
        || mouse_button(mousebfire) != 0
        || joy_button(joybfire) != 0
    {
        cmd.buttons |= BT_ATTACK;
    }

    if GAMEKEYDOWN[key_use as usize] != 0 || joy_button(joybuse) != 0 || mouse_button(mousebuse) != 0
    {
        cmd.buttons |= BT_USE;
        DCLICKS = 0;
    }

    // Weapon cycling
    if gamestate == GS_LEVEL && NEXT_WEAPON != 0
    {
        let i = next_weapon_slot(NEXT_WEAPON);
        cmd.buttons |= BT_CHANGE;
        cmd.buttons |= (i as u8) << BT_WEAPONSHIFT;
    }
    else
    {
        let weapon_keys_vals = [
            key_weapon1,
            key_weapon2,
            key_weapon3,
            key_weapon4,
            key_weapon5,
            key_weapon6,
            key_weapon7,
            key_weapon8,
        ];
        for (i, &key) in weapon_keys_vals.iter().enumerate()
        {
            if GAMEKEYDOWN[key as usize] != 0
            {
                cmd.buttons |= BT_CHANGE;
                cmd.buttons |= (i as u8) << BT_WEAPONSHIFT;
                break;
            }
        }
    }
    NEXT_WEAPON = 0;

    // Mouse forward/backward
    if mouse_button(mousebforward) != 0
    {
        forward += forwardmove[speed];
    }
    if mouse_button(mousebbackward) != 0
    {
        forward -= forwardmove[speed];
    }

    // Double-click use
    if dclick_use != 0
    {
        if mouse_button(mousebforward) != DCLICKSTATE && DCLICKTIME > 1
        {
            DCLICKSTATE = mouse_button(mousebforward);
            if DCLICKSTATE != 0
            {
                DCLICKS += 1;
            }
            if DCLICKS == 2
            {
                cmd.buttons |= BT_USE;
                DCLICKS = 0;
            }
            else
            {
                DCLICKTIME = 0;
            }
        }
        else
        {
            DCLICKTIME += ticdup;
            if DCLICKTIME > 20
            {
                DCLICKS = 0;
                DCLICKSTATE = 0;
            }
        }

        let bstrafe = (mouse_button(mousebstrafe) != 0 || joy_button(joybstrafe) != 0) as boolean;
        if bstrafe != DCLICKSTATE2 && DCLICKTIME2 > 1
        {
            DCLICKSTATE2 = bstrafe;
            if DCLICKSTATE2 != 0
            {
                DCLICKS2 += 1;
            }
            if DCLICKS2 == 2
            {
                cmd.buttons |= BT_USE;
                DCLICKS2 = 0;
            }
            else
            {
                DCLICKTIME2 = 0;
            }
        }
        else
        {
            DCLICKTIME2 += ticdup;
            if DCLICKTIME2 > 20
            {
                DCLICKS2 = 0;
                DCLICKSTATE2 = 0;
            }
        }
    }

    forward += MOUSEY;
    if strafe
    {
        side += MOUSEX * 2;
    }
    else
    {
        cmd.angleturn -= (MOUSEX * 0x8) as i16;
    }

    if MOUSEX == 0
    {
        testcontrols_mousespeed = 0;
    }
    MOUSEX = 0;
    MOUSEY = 0;

    // Clamp movement
    let maxplmove = forwardmove[1];
    if forward > maxplmove
    {
        forward = maxplmove;
    }
    else if forward < -maxplmove
    {
        forward = -maxplmove;
    }
    if side > maxplmove
    {
        side = maxplmove;
    }
    else if side < -maxplmove
    {
        side = -maxplmove;
    }

    cmd.forwardmove = (cmd.forwardmove as c_int + forward) as i8;
    cmd.sidemove = (cmd.sidemove as c_int + side) as i8;

    // Special buttons
    if sendpause != 0
    {
        sendpause = 0;
        cmd.buttons = BT_SPECIAL | BTS_PAUSE;
    }
    if sendsave != 0
    {
        sendsave = 0;
        cmd.buttons = BT_SPECIAL | BTS_SAVEGAME | (SAVEGAMESLOT as u8) << BTS_SAVESHIFT;
    }

    // Low-resolution turning
    if lowres_turn != 0
    {
        let (rounded, carry) = lowres_turn_round(cmd.angleturn, LOWRES_TURN_CARRY);
        cmd.angleturn = rounded;
        LOWRES_TURN_CARRY = carry;
    }
}

// ---------------------------------------------------------------------------
// SetJoyButtons / SetMouseButtons (static helpers)
// ---------------------------------------------------------------------------

/// Update the joystick button latch from a packed bitmask and detect
/// rising edges of the prev/next weapon buttons, scheduling a weapon cycle
/// in `NEXT_WEAPON` for the next `G_BuildTiccmd`.
///
/// # Safety
/// Mutates `JOYARRAY` and `NEXT_WEAPON`; safe under the single-threaded
/// engine convention.
#[doc(alias = "SetJoyButtons")]
pub(super) unsafe fn set_joy_buttons(buttons_mask: c_uint)
{
    for i in 0..MAX_JOY_BUTTONS
    {
        let button_on = ((buttons_mask >> i) & 1) != 0;
        if JOYARRAY[i + 1] == 0 && button_on
        {
            if i as c_int == joybprevweapon
            {
                NEXT_WEAPON = -1;
            }
            else if i as c_int == joybnextweapon
            {
                NEXT_WEAPON = 1;
            }
        }
        JOYARRAY[i + 1] = button_on as boolean;
    }
}

/// Mouse-button equivalent of [`set_joy_buttons`]: latches per-button state
/// and arms a weapon-cycle on the rising edge of the bound mouse buttons.
///
/// # Safety
/// Mutates `MOUSEARRAY` and `NEXT_WEAPON`; safe under the single-threaded
/// engine convention.
#[doc(alias = "SetMouseButtons")]
pub(super) unsafe fn set_mouse_buttons(buttons_mask: c_uint)
{
    for i in 0..MAX_MOUSE_BUTTONS
    {
        let button_on = ((buttons_mask >> i) & 1) != 0;
        if MOUSEARRAY[i + 1] == 0 && button_on
        {
            if i as c_int == mousebprevweapon
            {
                NEXT_WEAPON = -1;
            }
            else if i as c_int == mousebnextweapon
            {
                NEXT_WEAPON = 1;
            }
        }
        MOUSEARRAY[i + 1] = button_on as boolean;
    }
}

#[cfg(test)]
mod tests
{
    use super::{GAMEKEYDOWN, LOWRES_TURN_CARRY, MOUSEX, MOUSEY, NEXT_WEAPON};
    use super::super::consts::NUMKEYS;
    use super::super::dtmc::lowres_turn_round;
    use crate::doom::d_player::TiccmdT;
    use crate::doom::g_game::{lowres_turn, G_BuildTiccmd};
    use crate::doom::m_controls::key_right;

    /// Helper returning a freshly zeroed [`TiccmdT`] for test assembly.
    fn zeroed_cmd() -> TiccmdT
    {
        unsafe { std::mem::zeroed() }
    }

    /// The live `build_ticcmd` lowres path produces exactly the pure
    /// vectors pinned in `dtmc::lowres_turn_round_baseline_vectors` --
    /// post-split evidence that the extraction is wired into the real
    /// drive (the pre-split twin of this test ran against the inline
    /// body; commit `1c92fdd`). The pre-rounding `angleturn` entering
    /// the block is driven two ways: zero (no input; the carry alone
    /// reaches the report's `desired` values 100/200) and one held
    /// `key_right` tic (`-angleturn[2] = -320` at `TURNHELD < 6` with
    /// `ticdup == 0`).
    #[test]
    fn lowres_turn_live_build_ticcmd_matches_vectors()
    {
        //* Serialises mutation of the shared input-latch statics.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let saved_lowres = lowres_turn;
            let saved_carry = LOWRES_TURN_CARRY;
            lowres_turn = 1;
            // No input at all: keys, mouse deltas and latches zeroed so
            // the only angleturn contributor is the rounding block.
            GAMEKEYDOWN = [0; NUMKEYS];
            MOUSEX = 0;
            MOUSEY = 0;
            NEXT_WEAPON = 0;

            // Anchor vectors: with angleturn entering at 0, the carry
            // alone reproduces the report's (100, 0) / (200, 0) sums.
            // (The carry is copied into a local before asserting:
            // `assert_eq!` on the static directly would create a shared
            // reference to a mutable static.)
            LOWRES_TURN_CARRY = 0;
            let mut cmd = zeroed_cmd();
            G_BuildTiccmd(&mut cmd, 0);
            let carry = LOWRES_TURN_CARRY;
            assert_eq!(cmd.angleturn, 0);
            assert_eq!(carry, 0);

            LOWRES_TURN_CARRY = 100;
            let mut cmd = zeroed_cmd();
            G_BuildTiccmd(&mut cmd, 0);
            let carry = LOWRES_TURN_CARRY;
            assert_eq!(cmd.angleturn, 0);
            assert_eq!(carry, 100);

            LOWRES_TURN_CARRY = 200;
            let mut cmd = zeroed_cmd();
            G_BuildTiccmd(&mut cmd, 0);
            let carry = LOWRES_TURN_CARRY;
            assert_eq!(cmd.angleturn, 256);
            assert_eq!(carry, -56);

            // Wrap edge through the live body, oracle-checked.
            LOWRES_TURN_CARRY = 0x7FFF;
            let mut cmd = zeroed_cmd();
            G_BuildTiccmd(&mut cmd, 0);
            let carry = LOWRES_TURN_CARRY;
            assert_eq!((cmd.angleturn, carry), lowres_turn_round(0, 0x7FFF));

            // One held key_right tic: pre-rounding angleturn is
            // -angleturn[2] = -320 (tspeed 2 while TURNHELD < 6), and
            // the live post-rounding result must equal the pure core
            // applied to that input.
            GAMEKEYDOWN[key_right as usize] = 1;
            LOWRES_TURN_CARRY = 0;
            let mut cmd = zeroed_cmd();
            G_BuildTiccmd(&mut cmd, 0);
            let carry = LOWRES_TURN_CARRY;
            assert_eq!((cmd.angleturn, carry), lowres_turn_round(-320, 0));

            GAMEKEYDOWN = [0; NUMKEYS];
            lowres_turn = saved_lowres;
            LOWRES_TURN_CARRY = saved_carry;
        }
    }
}
