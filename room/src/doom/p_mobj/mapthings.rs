//! THINGS-lump entry: `spawn_map_thing` (the map-load dispatcher --
//! deathmatch starts, player starts, skill/network filtering, and the
//! doomednum lookup) and `spawn_player` (the player mobj, including
//! the type-0 `playeringame[-1]` overrun guard, catalog entry 10) --
//! bit-exact with the mapthing half of `vendor/doomgeneric/p_mobj.c`.

use std::os::raw::c_int;

use crate::doom::d_main::nomonsters;
use crate::doom::d_player::PlayerT;
use crate::doom::g_game::{
    consoleplayer, deathmatch, gameskill, netgame, playeringame, players, totalitems, totalkills,
    G_PlayerReborn,
};
use crate::doom::hu_stuff::HU_Start;
use crate::doom::info::{self, *};
use crate::doom::m_fixed::FRACBITS;
use crate::doom::m_random::P_Random;
use crate::doom::p_pspr::P_SetupPsprites;
use crate::doom::p_setup::{deathmatch_p, deathmatchstarts, playerstarts};
use crate::doom::p_telept::mapthing_t;
use crate::doom::st_stuff::ST_Start;
use crate::doom::violations::{self, VanillaViolation};

use crate::i_error;

/// Type alias for the `mapthing_t` definition from `p_setup`, used when
/// writing directly to the deathmatch-starts array.
type SetupMapThing = crate::doom::p_setup::mapthing_t;

use super::consts::{MTF_AMBUSH, ONCEILINGZ, ONFLOORZ, PST_LIVE, PST_REBORN, VIEWHEIGHT};
use super::dtmc::{mapthing_angle_quantize, spawn_skill_bit};
use super::spawn::spawn_mobj;

/// Spawn the player mobj for the player indicated by `mthing->type` (1-4).
///
/// Reborns the player if necessary, initialises all HUD state, and sets up
/// weapon psprites.  Skips the slot if the player is not in the current game.
///
/// A type-0 mapthing (an empty `playerstarts` slot reached through the
/// respawn/fallback paths) models vanilla's `playeringame[-1]` overrun: it
/// records `VanillaViolation::PlayeringameOverrun` and spawns nothing
/// (`docs/vanilla-workarounds.md`, entry 10).
///
/// ## Technical Details
///
/// The type-0 prologue guard sits at the same control-flow point dsda
/// chose (`PlayeringameOverrun`): the aliased
/// `players[MAXPLAYERS-1].didsecret` byte is diagnosed, never branched
/// on, and the skip is total. The player-init order afterwards is the
/// demo surface: reborn handling, the mobj spawn, the color
/// translation shift, the quantized facing, then the HUD/psprite
/// resets and (for the console player) `ST_Start`/`HU_Start`.
///
/// ## On Calling
///
/// `mthing` must be a valid, non-null pointer to a `mapthing_t` whose
/// `type` field is in `0..=4`; must be called only during level load
/// with zone memory active (or from the respawn/fallback paths in
/// `g_game`). No RNG draws on any path.
///
/// # Safety
///
/// `mthing` must be a valid, non-null pointer to a `mapthing_t` whose `type`
/// field is in `0..=4`.  Must be called only during level load with zone
/// memory active.
#[doc(alias = "P_SpawnPlayer")]
#[export_name = "P_SpawnPlayer"]
pub unsafe extern "C" fn spawn_player(mthing: *mut mapthing_t)
{
    let mthing = &mut *mthing;
    if mthing.r#type as c_int == 0
    {
        // Vanilla read `playeringame[mthing->type - 1]` at this exact point,
        // so a type-0 mapthing read `playeringame[-1]` -- which aliases
        // `players[3].didsecret` in the DOS `.bss` (last byte of `players[]`
        // sitting directly before `playeringame[]`). When that byte was set
        // (a co-op partner found a secret in an earlier level of the
        // vex6d "running body" demo family), vanilla spawned a player
        // through `p = &players[-1]` (per the e6y/dsda account; we did not
        // independently disassemble doom2.exe); reproducing that
        // write-through of a
        // fabricated player slot is not modelable without emulating the
        // whole `.bss` trample. dsda-doom bounds the defect at this same
        // control-flow point (`PlayeringameOverrun`,
        // reference/dsda-doom/prboom2/src/g_overflow.c:203-216, call site
        // `p_mobj.c:2074`): surface the aliased byte as a diagnostic, then
        // return without spawning. The condition shape is `type == 0`
        // alone -- the aliased byte is diagnosed, never branched on -- so
        // the skip is total: a type-0 arrival spawns nothing whether the
        // byte is set or clear, exactly like the model. Census counts every
        // arrival.
        violations::record(VanillaViolation::PlayeringameOverrun);
        return;
    }
    if playeringame[(mthing.r#type - 1) as usize] == 0
    {
        return;
    }

    let p = &mut players[(mthing.r#type - 1) as usize] as *mut PlayerT;

    if (*p).playerstate == PST_REBORN
    {
        G_PlayerReborn(mthing.r#type as c_int - 1);
    }

    let x = (mthing.x as c_int) << FRACBITS;
    let y = (mthing.y as c_int) << FRACBITS;
    let z = ONFLOORZ;
    let mobj = spawn_mobj(x, y, z, MT_PLAYER);

    if mthing.r#type > 1
    {
        (*mobj).flags |= ((mthing.r#type - 1) as c_int) << MF_TRANSSHIFT;
    }

    (*mobj).angle = mapthing_angle_quantize(mthing.angle as u32);
    (*mobj).player = p as *mut crate::doom::p_telept::player_s;
    (*mobj).health = (*p).health;

    (*p).mo = mobj as *mut crate::doom::d_player::mobj_t;
    (*p).playerstate = PST_LIVE;
    (*p).refire = 0;
    (*p).message = std::ptr::null_mut();
    (*p).damagecount = 0;
    (*p).bonuscount = 0;
    (*p).extralight = 0;
    (*p).fixedcolormap = 0;
    (*p).viewheight = VIEWHEIGHT;

    P_SetupPsprites(p);

    if deathmatch != 0
    {
        for i in 0..crate::doom::d_player::NUMCARDS
        {
            (*p).cards[i] = 1;
        }
    }

    if mthing.r#type as c_int - 1 == consoleplayer
    {
        ST_Start();
        HU_Start();
    }
}

/// Spawn one thing from the map's THINGS lump.
///
/// Handles deathmatch start positions (type 11), player starts (types 1-4),
/// skill-level and network-mode filtering, and the `-nomonsters` flag.
/// Calls `P_SpawnMobj` for all other thing types after looking up the
/// `mobjinfo` entry by `doomednum`.
///
/// ## Technical Details
///
/// The dispatch order is the demo surface: deathmatch starts (type 11)
/// are stored into `deathmatchstarts` under the bounded
/// `deathmatch_p < base + 10` guard (the vanilla trample beyond 10 is
/// a deathmatch-only F2 concern, catalog G1), the `type <= 0` skip
/// matches dsda's `case 0: return NULL` (entry 10's level-load stop),
/// types 1-4 store into `playerstarts` and (non-deathmatch) spawn
/// immediately, then the multiplayer-thing filter, the skill bit
/// ([`super::dtmc::spawn_skill_bit`]), the doomednum walk, the
/// `MF_NOTDMATCH`/`-nomonsters` filters, and finally the spawn with
/// the initial-tic draw `1 + P_Random() % tics` (ONE draw per spawned
/// thing with tics > 0), the kill/item counters, and the quantized
/// facing + ambush flag.
///
/// ## On Calling
///
/// `mthing` must be a valid, non-null pointer to a `mapthing_t` with
/// host byte order fields; must be called during level load (or from
/// the deathmatch respawn fallback, which only reaches
/// [`spawn_player`]). Draw count: exactly one `P_Random` draw per
/// surviving thing whose spawn state has `tics > 0`.
///
/// # Safety
///
/// `mthing` must be a valid, non-null pointer to a `mapthing_t` with host
/// byte order fields.  Must be called during level load.
#[doc(alias = "P_SpawnMapThing")]
#[export_name = "P_SpawnMapThing"]
pub unsafe extern "C" fn spawn_map_thing(mthing: *mut mapthing_t)
{
    let mthing = &mut *mthing;
    let mut i: usize;

    if mthing.r#type as c_int == 11
    {
        if deathmatch_p < std::ptr::addr_of_mut!(deathmatchstarts[0]).add(10)
        {
            *deathmatch_p = *(mthing as *mut _ as *mut SetupMapThing);
            deathmatch_p = deathmatch_p.add(1);
        }
        return;
    }

    if mthing.r#type as c_int <= 0
    {
        return;
    }

    if mthing.r#type as c_int <= 4
    {
        let ps = std::ptr::addr_of_mut!(playerstarts[0]) as *mut mapthing_t;
        *ps.add((mthing.r#type - 1) as usize) = *mthing;
        if deathmatch == 0
        {
            spawn_player(mthing as *mut mapthing_t);
        }
        return;
    }

    if netgame == 0 && mthing.options & 16i16 != 0
    {
        return;
    }

    let bit = spawn_skill_bit(gameskill);

    if mthing.options & bit as i16 == 0
    {
        return;
    }

    i = 0;
    while i < NUMMOBJTYPES
    {
        if mthing.r#type as c_int == info::mobjinfo[i].doomednum
        {
            break;
        }
        i += 1;
    }

    if i == NUMMOBJTYPES
    {
        i_error!(
            "P_SpawnMapThing: Unknown type {} at ({}, {})",
            mthing.r#type as c_int,
            mthing.x as c_int,
            mthing.y as c_int
        );
    }

    if deathmatch != 0 && info::mobjinfo[i].flags & MF_NOTDMATCH != 0
    {
        return;
    }

    if nomonsters != 0 && (i == MT_SKULL as usize || info::mobjinfo[i].flags & MF_COUNTKILL != 0)
    {
        return;
    }

    let x = (mthing.x as c_int) << FRACBITS;
    let y = (mthing.y as c_int) << FRACBITS;

    let z = if info::mobjinfo[i].flags & MF_SPAWNCEILING != 0
    {
        ONCEILINGZ
    }
    else
    {
        ONFLOORZ
    };

    let mobj = spawn_mobj(x, y, z, i as c_int);
    (*mobj).spawnpoint = *mthing;

    if (*mobj).tics > 0
    {
        (*mobj).tics = 1 + (P_Random() % (*mobj).tics);
    }
    if (*mobj).flags & MF_COUNTKILL != 0
    {
        totalkills += 1;
    }
    if (*mobj).flags & MF_COUNTITEM != 0
    {
        totalitems += 1;
    }

    (*mobj).angle = mapthing_angle_quantize(mthing.angle as u32);
    if mthing.options & MTF_AMBUSH as i16 != 0
    {
        (*mobj).flags |= MF_AMBUSH;
    }
}

#[cfg(test)]
mod tests
{
    use std::sync::Mutex;

    use crate::doom::d_player::{players, MAXPLAYERS};
    use crate::doom::p_telept::mapthing_t;
    use crate::doom::p_mobj::P_SpawnPlayer;
    use crate::doom::violations::{self, VanillaViolation};

    /// A type-0 mapthing reaching `P_SpawnPlayer` is vanilla's
    /// `playeringame[-1]` read (`players[3].didsecret` alias in the DOS
    /// `.bss`). dsda's `PlayeringameOverrun` model
    /// (`reference/dsda-doom/prboom2/src/g_overflow.c:203-216`, call site
    /// `p_mobj.c:2074`) surfaces the aliased byte as a diagnostic and
    /// returns without spawning, regardless of its value: every arrival
    /// records a census hit and no player mobj may appear. Safe to drive
    /// without a booted engine precisely because the modeled path returns
    /// before any spawn machinery.
    #[test]
    fn type0_mapthing_records_playeringame_overrun_and_spawns_nothing()
    {
        // `players[]` and the violation counters are process-global statics;
        // serialize against sibling tests like the p_map.rs convention. The
        // crate-wide census lock additionally guards the census windows
        // below against sibling census tests' reset_all() (see violations.rs).
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _census = violations::CENSUS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // The players[i].mo.is_null() asserts below race harness_hash's
        // state tests, which transiently set players[0].mo non-null.
        let _engine = violations::ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        unsafe
        {
            let before = violations::hits(VanillaViolation::PlayeringameOverrun);

            // Preset the aliased byte the way the vex6d co-op family does
            // (player 4 found a secret in an earlier level, so the byte
            // persists into the next level load).
            players[MAXPLAYERS - 1].didsecret = 1;

            let mut mt = mapthing_t {
                x: 1234,
                y: 567,
                angle: 90,
                r#type: 0,
                options: 7,
            };
            P_SpawnPlayer(&mut mt);

            // dsda's condition shape is `mthing->type == 0` alone; the
            // aliased byte is diagnosed, never branched on.
            assert_eq!(
                violations::hits(VanillaViolation::PlayeringameOverrun),
                before + 1,
                "flag-set arrival must record PlayeringameOverrun"
            );
            for i in 0..MAXPLAYERS
            {
                assert!(
                    players[i].mo.is_null(),
                    "no player mobj may spawn for player {i}"
                );
            }

            // Companion: aliased byte clear - same condition shape, same
            // early return, same census (current skip preserved).
            players[MAXPLAYERS - 1].didsecret = 0;
            P_SpawnPlayer(&mut mt);
            assert_eq!(
                violations::hits(VanillaViolation::PlayeringameOverrun),
                before + 2,
                "flag-clear arrival must record PlayeringameOverrun too"
            );
            for i in 0..MAXPLAYERS
            {
                assert!(
                    players[i].mo.is_null(),
                    "no player mobj may spawn for player {i}"
                );
            }
        }
    }
}
