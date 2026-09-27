//! The pickup dispatcher: `P_TouchSpecialThing`, the sprite-match over
//! every gettable thing that routes to the `P_Give*` helpers, sets the
//! HUD message, removes the special mobj, and plays the pickup sound --
//! bit-exact with the corresponding function of
//! `vendor/doomgeneric/p_inter.c`.

#![allow(non_snake_case)]

use std::os::raw::c_int;

use crate::doom::d_player::{consoleplayer, players, PlayerT};
use crate::doom::doomstat::gamemode;
use crate::doom::g_game::netgame;
use crate::i_error;
use crate::doom::info::{
    MF_COUNTITEM, MF_DROPPED, SPR_AMMO, SPR_ARM1, SPR_ARM2, SPR_BFUG, SPR_BON1, SPR_BON2,
    SPR_BPAK, SPR_BROK, SPR_BSKU, SPR_BKEY, SPR_CELL, SPR_CELP, SPR_CLIP, SPR_CSAW, SPR_LAUN,
    SPR_MEDI, SPR_MEGA, SPR_MGUN, SPR_PINS, SPR_PINV, SPR_PLAS, SPR_PMAP, SPR_PSTR, SPR_PVIS,
    SPR_RKEY, SPR_RSKU, SPR_ROCK, SPR_SBOX, SPR_SGN2, SPR_SHEL, SPR_SHOT, SPR_SOUL, SPR_STIM,
    SPR_SUIT, SPR_YKEY, SPR_YSKU,
};
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::p_mobj::P_RemoveMobj;
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::consts::{
    am_cell, am_clip, am_misl, am_shell, commercial, it_blueskull, it_bluecard, it_redcard,
    it_redskull, it_yellowskull, it_yellowcard, pw_allmap, pw_invisibility, pw_infrared,
    pw_invulnerability, pw_ironfeet, pw_strength, wp_bfg, wp_chaingun, wp_chainsaw, wp_fist,
    wp_missile, wp_plasma, wp_shotgun, wp_supershotgun, BONUSADD, NUMAMMO,
};
use super::consts::{
    DEH_String, GOTARMOR, GOTARMBONUS, GOTBACKPACK, GOTBERSERK, GOTBFG9000, GOTBLUECARD,
    GOTBLUESKUL, GOTCELL, GOTCELLBOX, GOTCHAINSAW, GOTCHAINGUN, GOTCLIP, GOTCLIPBOX,
    GOTHTHBONUS, GOTINVIS, GOTINVUL, GOTLAUNCHER, GOTMAP, GOTMEDIKIT, GOTMEDINEED, GOTMEGA,
    GOTMSPHERE, GOTPLASMA, GOTREDCARD, GOTREDSKULL, GOTROCKBOX, GOTROCKET, GOTSHELLBOX,
    GOTSHELLS, GOTSHOTGUN, GOTSHOTGUN2, GOTSTIM, GOTSUPER, GOTSUIT, GOTVISOR, GOTYELWCARD,
    GOTYELWSKUL,
};
use super::give::{
    p_give_ammo, p_give_armor, p_give_body, p_give_card, p_give_power, p_give_weapon,
};
use super::state::{
    deh_blue_armor_class, deh_green_armor_class, deh_max_armor, deh_max_health,
    deh_max_soulsphere, deh_megasphere_health, deh_soulsphere_health,
};

/// Handle a player touching a special (pickup) thing.
///
/// Called by the collision-detection code when `toucher` overlaps `special`
/// and `special` has the special-thing flag set.  Dispatches on the sprite
/// number of `special` to call the appropriate `P_Give*` helper, sets the
/// HUD pickup message, plays a sound, removes the special mobj, and
/// increments `itemcount` for items with `MF_COUNTITEM`.
///
/// The function returns early (no pickup) if the vertical gap between
/// `special` and `toucher` is greater than the toucher's height or less
/// than -8 map units, preventing pickups from platforms above or pits below.
///
/// # Safety
///
/// Both `special` and `toucher` must be valid, non-null pointers to live
/// map objects.  `toucher.player` must be a valid, non-null pointer to the
/// owning `PlayerT`.  Global game-state statics (`players`, `consoleplayer`,
/// `netgame`, `gamemode`, `gameskill`) must only be accessed from the
/// game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_TouchSpecialThing(special: *mut mobj_t, toucher: *mut mobj_t)
{
    // Read all primitives from `special` / `toucher` up front via raw
    // pointers so we never hold a `&mut` to either of them across the
    // FFI helper calls or `P_RemoveMobj` below; those reborrow the
    // raw pointers and would alias.
    let delta = (*special).z - (*toucher).z;
    if delta > (*toucher).height || delta < -8 * FRACUNIT
    {
        return;
    }

    // Dead thing touching.
    // Can happen with a sliding player corpse.
    if (*toucher).health <= 0
    {
        return;
    }

    let mut sound: c_int = Sfx::Itemup as c_int;
    let player_ptr = (*toucher).player as *mut PlayerT;
    let sprite = (*special).sprite;
    let special_flags = (*special).flags;
    // Snapshot the DEH-tunable `static mut` values into locals so each
    // unsafe read happens here, not inline inside the match arms.
    let green_armor_class = deh_green_armor_class;
    let blue_armor_class = deh_blue_armor_class;
    let max_health = deh_max_health;
    let max_armor = deh_max_armor;
    let max_soulsphere = deh_max_soulsphere;
    let soulsphere_health = deh_soulsphere_health;
    let megasphere_health = deh_megasphere_health;
    // `player` is the long-lived borrow. Every helper called below
    // takes `&mut PlayerT` (the `p_give_*` inner fns), so no second
    // `&mut PlayerT` is ever derived from `player_ptr`.
    let player = &mut *player_ptr;

    // Identify by sprite.
    match sprite
    {
        // armor
        SPR_ARM1 =>
        {
            if p_give_armor(player, green_armor_class) == 0
            {
                return;
            }
            player.message = DEH_String(GOTARMOR);
        }
        SPR_ARM2 =>
        {
            if p_give_armor(player, blue_armor_class) == 0
            {
                return;
            }
            player.message = DEH_String(GOTMEGA);
        }
        // bonus items
        SPR_BON1 =>
        {
            player.health += 1;
            if player.health > max_health
            {
                player.health = max_health;
            }
            let mo = &mut *(player.mo as *mut mobj_t);
            mo.health = player.health;
            player.message = DEH_String(GOTHTHBONUS);
        }
        SPR_BON2 =>
        {
            player.armorpoints += 1;
            if player.armorpoints > max_armor
            {
                player.armorpoints = max_armor;
            }
            // `deh_green_armor_class` only applies to the green armor
            // shirt; for the armor helmets, armortype 1 is always used.
            if player.armortype == 0
            {
                player.armortype = 1;
            }
            player.message = DEH_String(GOTARMBONUS);
        }
        SPR_SOUL =>
        {
            player.health += soulsphere_health;
            if player.health > max_soulsphere
            {
                player.health = max_soulsphere;
            }
            let mo = &mut *(player.mo as *mut mobj_t);
            mo.health = player.health;
            player.message = DEH_String(GOTSUPER);
            sound = Sfx::Getpow as c_int;
        }
        SPR_MEGA =>
        {
            if gamemode != commercial
            {
                return;
            }
            player.health = megasphere_health;
            {
                let mo = &mut *(player.mo as *mut mobj_t);
                mo.health = player.health;
            }
            // We always give armor type 2 for the megasphere; DEHacked
            // only affects the standalone MegaArmor pickup
            // (`SPR_ARM2`), not this one.
            p_give_armor(player, 2);
            player.message = DEH_String(GOTMSPHERE);
            sound = Sfx::Getpow as c_int;
        }
        // cards
        SPR_BKEY =>
        {
            if player.cards[it_bluecard as usize] == 0
            {
                player.message = DEH_String(GOTBLUECARD);
            }
            p_give_card(player, it_bluecard);
            if netgame != 0
            {
                return;
            }
        }
        SPR_YKEY =>
        {
            if player.cards[it_yellowcard as usize] == 0
            {
                player.message = DEH_String(GOTYELWCARD);
            }
            p_give_card(player, it_yellowcard);
            if netgame != 0
            {
                return;
            }
        }
        SPR_RKEY =>
        {
            if player.cards[it_redcard as usize] == 0
            {
                player.message = DEH_String(GOTREDCARD);
            }
            p_give_card(player, it_redcard);
            if netgame != 0
            {
                return;
            }
        }
        SPR_BSKU =>
        {
            if player.cards[it_blueskull as usize] == 0
            {
                player.message = DEH_String(GOTBLUESKUL);
            }
            p_give_card(player, it_blueskull);
            if netgame != 0
            {
                return;
            }
        }
        SPR_YSKU =>
        {
            if player.cards[it_yellowskull as usize] == 0
            {
                player.message = DEH_String(GOTYELWSKUL);
            }
            p_give_card(player, it_yellowskull);
            if netgame != 0
            {
                return;
            }
        }
        SPR_RSKU =>
        {
            if player.cards[it_redskull as usize] == 0
            {
                player.message = DEH_String(GOTREDSKULL);
            }
            p_give_card(player, it_redskull);
            if netgame != 0
            {
                return;
            }
        }
        // medikits, heals
        SPR_STIM =>
        {
            if p_give_body(player, 10) == 0
            {
                return;
            }
            player.message = DEH_String(GOTSTIM);
        }
        SPR_MEDI =>
        {
            if p_give_body(player, 25) == 0
            {
                return;
            }
            if player.health < 25
            {
                player.message = DEH_String(GOTMEDINEED);
            }
            else
            {
                player.message = DEH_String(GOTMEDIKIT);
            }
        }
        // power ups
        SPR_PINV =>
        {
            if p_give_power(player, pw_invulnerability as c_int) == 0
            {
                return;
            }
            player.message = DEH_String(GOTINVUL);
            sound = Sfx::Getpow as c_int;
        }
        SPR_PSTR =>
        {
            if p_give_power(player, pw_strength as c_int) == 0
            {
                return;
            }
            player.message = DEH_String(GOTBERSERK);
            if player.readyweapon != wp_fist
            {
                player.pendingweapon = wp_fist;
            }
            sound = Sfx::Getpow as c_int;
        }
        SPR_PINS =>
        {
            if p_give_power(player, pw_invisibility as c_int) == 0
            {
                return;
            }
            player.message = DEH_String(GOTINVIS);
            sound = Sfx::Getpow as c_int;
        }
        SPR_SUIT =>
        {
            if p_give_power(player, pw_ironfeet as c_int) == 0
            {
                return;
            }
            player.message = DEH_String(GOTSUIT);
            sound = Sfx::Getpow as c_int;
        }
        SPR_PMAP =>
        {
            if p_give_power(player, pw_allmap as c_int) == 0
            {
                return;
            }
            player.message = DEH_String(GOTMAP);
            sound = Sfx::Getpow as c_int;
        }
        SPR_PVIS =>
        {
            if p_give_power(player, pw_infrared as c_int) == 0
            {
                return;
            }
            player.message = DEH_String(GOTVISOR);
            sound = Sfx::Getpow as c_int;
        }
        // ammo
        SPR_CLIP =>
        {
            let amount = if special_flags & MF_DROPPED != 0
            {
                0
            }
            else
            {
                1
            };
            if p_give_ammo(player, am_clip, amount) == 0
            {
                return;
            }
            player.message = DEH_String(GOTCLIP);
        }
        SPR_AMMO =>
        {
            if p_give_ammo(player, am_clip, 5) == 0
            {
                return;
            }
            player.message = DEH_String(GOTCLIPBOX);
        }
        SPR_ROCK =>
        {
            if p_give_ammo(player, am_misl, 1) == 0
            {
                return;
            }
            player.message = DEH_String(GOTROCKET);
        }
        SPR_BROK =>
        {
            if p_give_ammo(player, am_misl, 5) == 0
            {
                return;
            }
            player.message = DEH_String(GOTROCKBOX);
        }
        SPR_CELL =>
        {
            if p_give_ammo(player, am_cell, 1) == 0
            {
                return;
            }
            player.message = DEH_String(GOTCELL);
        }
        SPR_CELP =>
        {
            if p_give_ammo(player, am_cell, 5) == 0
            {
                return;
            }
            player.message = DEH_String(GOTCELLBOX);
        }
        SPR_SHEL =>
        {
            if p_give_ammo(player, am_shell, 1) == 0
            {
                return;
            }
            player.message = DEH_String(GOTSHELLS);
        }
        SPR_SBOX =>
        {
            if p_give_ammo(player, am_shell, 5) == 0
            {
                return;
            }
            player.message = DEH_String(GOTSHELLBOX);
        }
        SPR_BPAK =>
        {
            if player.backpack == 0
            {
                for i in 0..NUMAMMO
                {
                    player.maxammo[i] *= 2;
                }
                player.backpack = 1;
            }
            for i in 0..NUMAMMO
            {
                p_give_ammo(player, i as c_int, 1);
            }
            player.message = DEH_String(GOTBACKPACK);
        }
        // weapons
        SPR_BFUG =>
        {
            if p_give_weapon(player, wp_bfg, 0) == 0
            {
                return;
            }
            player.message = DEH_String(GOTBFG9000);
            sound = Sfx::Wpnup as c_int;
        }
        SPR_MGUN =>
        {
            if p_give_weapon(
                player,
                wp_chaingun,
                ((special_flags & MF_DROPPED) != 0) as c_int,
            ) == 0
            {
                return;
            }
            player.message = DEH_String(GOTCHAINGUN);
            sound = Sfx::Wpnup as c_int;
        }
        SPR_CSAW =>
        {
            if p_give_weapon(player, wp_chainsaw, 0) == 0
            {
                return;
            }
            player.message = DEH_String(GOTCHAINSAW);
            sound = Sfx::Wpnup as c_int;
        }
        SPR_LAUN =>
        {
            if p_give_weapon(player, wp_missile, 0) == 0
            {
                return;
            }
            player.message = DEH_String(GOTLAUNCHER);
            sound = Sfx::Wpnup as c_int;
        }
        SPR_PLAS =>
        {
            if p_give_weapon(player, wp_plasma, 0) == 0
            {
                return;
            }
            player.message = DEH_String(GOTPLASMA);
            sound = Sfx::Wpnup as c_int;
        }
        SPR_SHOT =>
        {
            if p_give_weapon(
                player,
                wp_shotgun,
                ((special_flags & MF_DROPPED) != 0) as c_int,
            ) == 0
            {
                return;
            }
            player.message = DEH_String(GOTSHOTGUN);
            sound = Sfx::Wpnup as c_int;
        }
        SPR_SGN2 =>
        {
            if p_give_weapon(
                player,
                wp_supershotgun,
                ((special_flags & MF_DROPPED) != 0) as c_int,
            ) == 0
            {
                return;
            }
            player.message = DEH_String(GOTSHOTGUN2);
            sound = Sfx::Wpnup as c_int;
        }
        _ =>
        {
            i_error!("P_SpecialThing: Unknown gettable thing");
        }
    }

    if special_flags & MF_COUNTITEM != 0
    {
        player.itemcount += 1;
    }
    P_RemoveMobj(special);
    player.bonuscount += BONUSADD;
    if std::ptr::eq(
        player_ptr,
        std::ptr::addr_of_mut!(players[0]).add(consoleplayer as usize),
    )
    {
        S_StartSound(std::ptr::null_mut(), sound);
    }
}
