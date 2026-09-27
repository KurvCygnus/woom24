//! The level entry points: `setup_level` (the whole-map load orchestrator
//! called by `G_DoLoadLevel`) and `init_map_system` (the once-at-startup
//! initializer). The loader call ORDER inside `setup_level` is the
//! behavior -- it is dtmc as a whole body and must not be reordered.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;
use std::ptr;

use crate::doom::c_ffi::MapLump;
use crate::doom::doomstat::gamemode;
use crate::doom::d_mode;
use crate::doom::d_player::{consoleplayer, players, MAXPLAYERS};
use crate::doom::g_game::{
    bodyqueslot, deathmatch, playeringame, precache, totalitems, totalkills, totalsecret, wminfo,
    G_DeathMatchSpawnPlayer,
};
use crate::doom::info::sprnames;
use crate::doom::p_mobj::{iquehead, iquetail};
use crate::doom::p_spec::{P_InitPicAnims, P_SpawnSpecials};
use crate::doom::p_switch::P_InitSwitchList;
use crate::doom::p_tick::{leveltime, P_InitThinkers};
use crate::doom::r_data::R_PrecacheLevel;
use crate::doom::r_things::R_InitSprites;
use crate::doom::s_sound::S_Start;
use crate::doom::w_wad::W_GetNumForName;
use crate::doom::z_zone::{PU_LEVEL, Z_FreeTags};

use super::globals::{deathmatch_p, deathmatchstarts};
use super::loaders::{
    load_blockmap, load_linedefs, load_nodes, load_sectors, load_segs, load_sidedefs,
    load_subsectors, load_things, load_vertexes,
};
use super::reject::load_reject;
use super::structs::PU_PURGELEVEL;
use super::grouplines::group_lines;

/// Initialize all game state for the given episode/map and load its geometry.
///
/// This is the main map-load entry point, called by `G_DoLoadLevel` in
/// `g_game.c`. It performs the following steps in order (order matters):
///
/// 1. Reset kill/item/secret counters and intermission stats.
/// 2. Set the console player's `viewz` to 1 (will be corrected by player think).
/// 3. Stop all sounds (`S_Start`) before freeing zone memory.
/// 4. Free all `PU_LEVEL` through `PU_PURGELEVEL - 1` zone blocks.
/// 5. Re-initialize the thinker list (`P_InitThinkers`).
/// 6. Construct the WAD lump name: `"MAPxx"` for commercial, `"ExMy"` for
///    episodic. The `episode` and `map` parameters are 1-based.
/// 7. Load map lumps in this specific order: BLOCKMAP, VERTEXES, SECTORS,
///    SIDEDEFS, LINEDEFS, SSECTORS, NODES, SEGS.
/// 8. Post-process: `P_GroupLines`, then load REJECT.
/// 9. Reset `bodyqueslot` and `deathmatch_p`, then load THINGS
///    (`P_LoadThings` spawns all map objects).
/// 10. If deathmatch mode, randomly respawn active players.
/// 11. Reset the item-queue indices (`iquehead`, `iquetail`).
/// 12. Spawn special sector effects (`P_SpawnSpecials`).
/// 13. If `precache` is set, preload all level graphics (`R_PrecacheLevel`).
///
/// `_playermask` and `_skill` parameters are accepted for C ABI compatibility
/// but are unused; skill filtering is handled by `P_SpawnMapThing`.
///
/// C callers: `G_DoLoadLevel` in `g_game.c`.
// FIXME: C uses DEH_snprintf for lump name construction, allowing DeHackEd
// patches to rename map lumps. The Rust port uses format! and does not apply
// DEH patches to the lump name, which may break modded WADs that rely on this.
// (Known port deviation, recorded per the F10 no-behavior-change rule; do not
// "fix" it as part of a refactor.)
#[doc(alias = "P_SetupLevel")]
#[export_name = "P_SetupLevel"]
pub extern "C" fn setup_level(episode: c_int, map: c_int, _playermask: c_int, _skill: c_int)
{
    unsafe
    {
        totalkills = 0;
        totalitems = 0;
        totalsecret = 0;
        wminfo.maxfrags = 0;
        wminfo.partime = 180;

        for i in 0..MAXPLAYERS
        {
            players[i].killcount = 0;
            players[i].secretcount = 0;
            players[i].itemcount = 0;
        }

        players[consoleplayer as usize].viewz = 1;

        S_Start();
        Z_FreeTags(PU_LEVEL, PU_PURGELEVEL - 1);

        P_InitThinkers();

        // Find map name.
        let mut lumpname = [0i8; 9];
        if gamemode == d_mode::commercial
        {
            if map < 10
            {
                let s = format!("map0{}", map);
                ptr::copy_nonoverlapping(s.as_ptr(), lumpname.as_mut_ptr() as *mut u8, s.len());
            }
            else
            {
                let s = format!("map{}", map);
                ptr::copy_nonoverlapping(s.as_ptr(), lumpname.as_mut_ptr() as *mut u8, s.len());
            }
        }
        else
        {
            lumpname[0] = b'E' as i8;
            lumpname[1] = (b'0' + episode as u8) as i8;
            lumpname[2] = b'M' as i8;
            lumpname[3] = (b'0' + map as u8) as i8;
            lumpname[4] = 0;
        }

        let lumpnum = W_GetNumForName(lumpname.as_mut_ptr());

        leveltime = 0;

        // Note: most of this ordering is important.
        load_blockmap(lumpnum + MapLump::BLOCKMAP);
        load_vertexes(lumpnum + MapLump::VERTEXES);
        load_sectors(lumpnum + MapLump::SECTORS);
        load_sidedefs(lumpnum + MapLump::SIDEDEFS);

        load_linedefs(lumpnum + MapLump::LINEDEFS);
        load_subsectors(lumpnum + MapLump::SSECTORS);
        load_nodes(lumpnum + MapLump::NODES);
        load_segs(lumpnum + MapLump::SEGS);

        group_lines();
        load_reject(lumpnum + MapLump::REJECT);

        bodyqueslot = 0;
        deathmatch_p = std::ptr::addr_of_mut!(deathmatchstarts[0]);
        load_things(lumpnum + MapLump::THINGS);

        if deathmatch != 0
        {
            for i in 0..MAXPLAYERS
            {
                if playeringame[i] != 0
                {
                    players[i].mo = ptr::null_mut();
                    G_DeathMatchSpawnPlayer(i as c_int);
                }
            }
        }

        iquehead = 0;
        iquetail = 0;

        P_SpawnSpecials();

        if precache != 0
        {
            R_PrecacheLevel();
        }
    }
}

/// Initialize the map/physics subsystem at engine startup.
///
/// Called once from `D_DoomMain` before the game loop starts. Sets up:
/// - `P_InitSwitchList`: builds the switch animation list from the texture
///   names defined in `p_switch.c`.
/// - `P_InitPicAnims`: builds the animated flat/texture list from the lump
///   sequence tables in `p_spec.c`.
/// - `R_InitSprites`: builds the sprite frame lookup table from the WAD's
///   sprite lump names, starting at `sprnames[0]`.
///
/// C callers: `D_DoomMain` in `d_main.c`.
#[doc(alias = "P_Init")]
#[export_name = "P_Init"]
pub extern "C" fn init_map_system()
{
    unsafe
    {
        P_InitSwitchList();
        P_InitPicAnims();
        R_InitSprites(std::ptr::addr_of_mut!(sprnames[0]));
    }
}
