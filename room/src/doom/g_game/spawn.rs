//! Player spawning and respawning: `init_player`, `player_finish_level`,
//! `player_reborn`, `check_spot` (with the teleport-fog emulation),
//! `deathmatch_spawn_player` and `do_reborn`.
//!
//! All dtmc whole-body except the parity wrapper `init_player` (glue,
//! currently uncalled in-tree -- kept + pinned per the symbol-surface
//! rule): `player_reborn` seeds exactly the hashed `players[]` fields,
//! `deathmatch_spawn_player` draws from `P_Random` (draw count and order
//! are the demo surface), and `check_spot` carries the
//! TeleportFogAngleOverrun emulation plus the bounded bodyque corpse
//! flush. The bodies moved verbatim from pre-split `g_game.rs`; see the
//! module root for the mapping table.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_void};
use std::ptr;

use crate::doom::d_player::MAXPLAYERS;
use crate::doom::i_system::I_Error;
use crate::doom::m_random::P_Random;
use crate::doom::p_inter::maxammo;
use crate::doom::p_map::P_CheckPosition;
use crate::doom::p_mobj::{P_RemoveMobj, P_SpawnMobj, P_SpawnPlayer};
use crate::doom::p_setup::{
    deathmatch_p, deathmatchstarts, playerstarts, mapthing_t as SetupMapThing,
};
use crate::doom::p_telept::{mapthing_t, mobj_t};
use crate::doom::r_main::R_PointInSubsector;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::consts::{
    boolean, fixed_t, ga_loadlevel, am_clip, wp_fist, wp_pistol, DEH_INITIAL_BULLETS,
    DEH_INITIAL_HEALTH, MT_TFOG, PST_LIVE,
};
use super::dtmc::teleport_fog_offset;
use super::state::{bodyque, bodyqueslot, consoleplayer, deathmatch, gameaction, netgame, players};

// ---------------------------------------------------------------------------
// Intra-module aliases: same-file callers keep the upstream names the
// pre-split bodies used.
// ---------------------------------------------------------------------------

use self::{
    check_spot as G_CheckSpot, deathmatch_spawn_player as G_DeathMatchSpawnPlayer,
    player_reborn as G_PlayerReborn,
};

// ---------------------------------------------------------------------------
// G_InitPlayer
// ---------------------------------------------------------------------------

/// Initialise a player slot to its default state at game start.
///
/// Currently a thin wrapper around [`player_reborn`] (which clears the
/// slot and seeds it with starting health, weapons and ammo); kept as a
/// distinct entry point because vanilla C code invokes it at startup time.
/// Glue (F10 wave C3 adjudication); uncalled in-tree -- kept + pinned per
/// the symbol-surface rule.
///
/// # Safety
/// `player` must be a valid index into `players[]` (0..`MAXPLAYERS`).
/// The C symbol is pinned (`G_InitPlayer`) so the wasm export set stays
/// byte-identical.
#[doc(alias = "G_InitPlayer")]
#[export_name = "G_InitPlayer"]
pub unsafe extern "C" fn init_player(player: c_int)
{
    G_PlayerReborn(player);
}

// ---------------------------------------------------------------------------
// G_PlayerFinishLevel
// ---------------------------------------------------------------------------

/// Strip transient powerups, key cards and HUD effects from `player` when
/// a level completes; keeps weapons, ammo and frags intact.
///
/// Clears `powers[]`, `cards[]`, the invisibility (`MF_SHADOW`) flag on the
/// player's mobj, extra-light, fixed colormap, damage-flash and bonus-flash
/// counters.
///
/// # Safety
/// `player` must index a valid slot whose `mo` pointer is non-null (the
/// caller is `G_DoCompleted`, which guarantees this). The C symbol is
/// pinned (`G_PlayerFinishLevel`) so the wasm export set stays
/// byte-identical.
#[doc(alias = "G_PlayerFinishLevel")]
#[export_name = "G_PlayerFinishLevel"]
pub unsafe extern "C" fn player_finish_level(player: c_int)
{
    let p = &mut players[player as usize];
    p.powers = [0; 6];
    p.cards = [0; 6];
    (*(p.mo as *mut mobj_t)).flags &= !crate::doom::info::MF_SHADOW;
    p.extralight = 0;
    p.fixedcolormap = 0;
    p.damagecount = 0;
    p.bonuscount = 0;
}

// ---------------------------------------------------------------------------
// G_PlayerReborn
// ---------------------------------------------------------------------------

/// Reset a player slot after death, preserving frags and kill/item/secret
/// counters; everything else is zeroed and re-seeded with the vanilla
/// starting inventory (fist + pistol, 50 bullets, `DEH_INITIAL_HEALTH`).
///
/// The `usedown`/`attackdown` latches are set so the player cannot
/// immediately fire or activate switches on the first tic after respawn.
///
/// # Safety
/// `player` must be a valid index into `players[]`. The C symbol is
/// pinned (`G_PlayerReborn`): p_mobj's mapthing spawn path calls it.
#[doc(alias = "G_PlayerReborn")]
#[export_name = "G_PlayerReborn"]
pub unsafe extern "C" fn player_reborn(player: c_int)
{
    let idx = player as usize;
    let frags = players[idx].frags;
    let killcount = players[idx].killcount;
    let itemcount = players[idx].itemcount;
    let secretcount = players[idx].secretcount;

    players[idx] = std::mem::zeroed();

    players[idx].frags = frags;
    players[idx].killcount = killcount;
    players[idx].itemcount = itemcount;
    players[idx].secretcount = secretcount;

    players[idx].usedown = 1;
    players[idx].attackdown = 1;
    players[idx].playerstate = PST_LIVE;
    players[idx].health = DEH_INITIAL_HEALTH;
    players[idx].readyweapon = wp_pistol;
    players[idx].pendingweapon = wp_pistol;
    players[idx].weaponowned[wp_fist as usize] = 1;
    players[idx].weaponowned[wp_pistol as usize] = 1;
    players[idx].ammo[am_clip] = DEH_INITIAL_BULLETS;

    for i in 0..4
    {
        // NUMAMMO = 4
        players[idx].maxammo[i] = maxammo[i];
    }
}

// ---------------------------------------------------------------------------
// G_CheckSpot
// ---------------------------------------------------------------------------

/// Test whether `playernum` can respawn at `mthing` (a `mapthing_t` spawn
/// spot) and, if so, evict the oldest corpse and spawn a teleport-fog mobj.
///
/// Returns non-zero when the spot is usable. On the very first spawn of a
/// level (before any player has a mobj) this only checks against earlier
/// player spawn positions. Otherwise it calls `P_CheckPosition` to verify
/// the spot is clear of monsters / players.
///
/// The teleport-fog placement mirrors the vanilla Doom bug carried in PrBoom+
/// where the `an` angle index overflows into `finetangent[]` for spawns
/// facing west, southwest, south, or southeast (angles 180..315); the
/// special-case `an` values (4096, 5120, 6144, 7168, and 8192 for 360
/// degrees) reproduce those table lookups exactly to keep demos compatible
/// (docs/vanilla-workarounds.md #6, census: `TeleportFogAngleOverrun`).
/// See [`teleport_fog_offset`] for the offset arithmetic.
///
/// # Safety
/// Dereferences `mthing`; reads and mutates the players / bodyque / corpse
/// queue globals. The C symbol is pinned (`G_CheckSpot`) so the wasm
/// export set stays byte-identical.
#[doc(alias = "G_CheckSpot")]
#[export_name = "G_CheckSpot"]
pub unsafe extern "C" fn check_spot(playernum: c_int, mthing: *mut mapthing_t) -> boolean
{
    if players[playernum as usize].mo.is_null()
    {
        // First spawn of level, before corpses
        for i in 0..playernum as usize
        {
            let mo = players[i].mo as *mut mobj_t;
            if (*mo).x == ((*mthing).x as fixed_t) << 16
                && (*mo).y == ((*mthing).y as fixed_t) << 16
            {
                return 0;
            }
        }
        return 1;
    }

    let x = ((*mthing).x as fixed_t) << 16;
    let y = ((*mthing).y as fixed_t) << 16;

    if P_CheckPosition(
        players[playernum as usize].mo as *mut crate::doom::c_ffi::mobj_t,
        x,
        y,
    ) == 0
    {
        return 0;
    }

    // Flush old corpse
    if bodyqueslot >= 32
    {
        P_RemoveMobj(bodyque[(bodyqueslot % 32) as usize]);
    }
    bodyque[(bodyqueslot % 32) as usize] = players[playernum as usize].mo as *mut mobj_t;
    bodyqueslot += 1;

    // Spawn teleport fog
    let ss = R_PointInSubsector(x, y);

    // Replicate vanilla signed-angle overflow (from PrBoom+)
    let (xa, ya) = match teleport_fog_offset((*mthing).angle as c_int)
    {
        Some(offset) => offset,
        None => I_Error(c"G_CheckSpot: unexpected angle %d\n".as_ptr()),
    };

    let floorheight = (*(*ss).sector).floorheight;
    let mo = P_SpawnMobj(x + 20 * xa, y + 20 * ya, floorheight, MT_TFOG);

    if players[consoleplayer as usize].viewz != 1
    {
        S_StartSound(mo as *mut c_void, Sfx::Telept as c_int);
    }
    1
}

// ---------------------------------------------------------------------------
// G_DeathMatchSpawnPlayer
// ---------------------------------------------------------------------------

/// Spawn `playernum` at a random deathmatch start; falls back to the
/// player's normal start spot after 20 failed attempts.
///
/// Requires at least 4 deathmatch starts on the map (vanilla limit); raises
/// `I_Error` otherwise. The chosen start has its `type` temporarily
/// rewritten to `playernum + 1` so `P_SpawnPlayer` treats it as the player's
/// own spawn.
///
/// # Safety
/// `playernum` must be in `0..MAXPLAYERS` and the level's deathmatch start
/// table must already be populated by `P_SetupLevel`. The C symbol is
/// pinned (`G_DeathMatchSpawnPlayer`): p_setup's level loader calls it.
#[doc(alias = "G_DeathMatchSpawnPlayer")]
#[export_name = "G_DeathMatchSpawnPlayer"]
pub unsafe extern "C" fn deathmatch_spawn_player(playernum: c_int)
{
    let selections = deathmatch_p.offset_from(std::ptr::addr_of!(deathmatchstarts[0])) as c_int;
    if selections < 4
    {
        I_Error(c"Only %i deathmatch spots, 4 required".as_ptr());
    }

    for _ in 0..20
    {
        let i = (P_Random() % selections as c_int) as usize;
        if G_CheckSpot(
            playernum,
            &mut deathmatchstarts[i] as *mut SetupMapThing as *mut mapthing_t,
        ) != 0
        {
            deathmatchstarts[i].r#type = (playernum + 1) as i16;
            P_SpawnPlayer(&mut deathmatchstarts[i] as *mut SetupMapThing as *mut mapthing_t);
            return;
        }
    }
    P_SpawnPlayer(&mut playerstarts[playernum as usize] as *mut SetupMapThing as *mut mapthing_t);
}

// ---------------------------------------------------------------------------
// G_DoReborn
// ---------------------------------------------------------------------------

/// Respawn `playernum` either by reloading the level (single-player) or
/// by selecting a fresh spawn point (netgame).
///
/// In netgames the player's existing corpse is detached (`mo->player =
/// NULL`), then deathmatch routes through `G_DeathMatchSpawnPlayer` and
/// co-op tries the player's own start first before falling through to other
/// players' starts (temporarily faking the `type` field so `P_SpawnPlayer`
/// accepts them).
///
/// # Safety
/// `playernum` must be a valid player index. The C symbol is pinned
/// (`G_DoReborn`) so the wasm export set stays byte-identical.
#[doc(alias = "G_DoReborn")]
#[export_name = "G_DoReborn"]
pub unsafe extern "C" fn do_reborn(playernum: c_int)
{
    if netgame == 0
    {
        gameaction = ga_loadlevel;
    }
    else
    {
        let mo = players[playernum as usize].mo as *mut mobj_t;
        if !mo.is_null()
        {
            (*mo).player = ptr::null_mut();
        }

        if deathmatch != 0
        {
            G_DeathMatchSpawnPlayer(playernum);
            return;
        }

        if G_CheckSpot(
            playernum,
            &mut playerstarts[playernum as usize] as *mut SetupMapThing as *mut mapthing_t,
        ) != 0
        {
            P_SpawnPlayer(
                &mut playerstarts[playernum as usize] as *mut SetupMapThing as *mut mapthing_t,
            );
            return;
        }

        for i in 0..MAXPLAYERS as c_int
        {
            if G_CheckSpot(
                playernum,
                &mut playerstarts[i as usize] as *mut SetupMapThing as *mut mapthing_t,
            ) != 0
            {
                playerstarts[i as usize].r#type = (playernum + 1) as i16;
                P_SpawnPlayer(
                    &mut playerstarts[i as usize] as *mut SetupMapThing as *mut mapthing_t,
                );
                playerstarts[i as usize].r#type = (i + 1) as i16;
                return;
            }
        }
        P_SpawnPlayer(
            &mut playerstarts[playernum as usize] as *mut SetupMapThing as *mut mapthing_t,
        );
    }
}
