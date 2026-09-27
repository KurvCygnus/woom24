//! The `P_Give*` pickup helpers: ammo, weapon, body (health), armor,
//! card, and power grants over the shared `PlayerT` surfaces -- bit-
//! exact with the corresponding half of `vendor/doomgeneric/p_inter.c`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::os::raw::c_int;

use crate::doom::d_items::weaponinfo;
use crate::doom::d_player::{consoleplayer, players, PlayerT};
use crate::doom::g_game::{deathmatch, gameskill, netgame};
use crate::i_error;
use crate::doom::info::MF_SHADOW;
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::consts::{
    am_cell, am_clip, am_misl, am_noammo, am_shell, pw_infrared, pw_invulnerability,
    pw_invisibility, pw_ironfeet,
    pw_strength, sk_baby, sk_nightmare, wp_chaingun, wp_fist, wp_missile, wp_pistol, wp_plasma,
    wp_shotgun, BONUSADD, INFRATICS, INVULNTICS, INVISTICS, IRONTICS, MAXHEALTH, NUMAMMO,
};
use super::state::clipammo;

// ---------------------------------------------------------------------------
// P_GiveAmmo
// ---------------------------------------------------------------------------

/// Attempt to give the player `num` clip-loads of ammo type `ammo`.
///
/// `num` is a multiplier applied to `clipammo[ammo]`.  A value of `0`
/// gives half a clip (used when picking up a dropped weapon).  On skill
/// levels `sk_baby` and `sk_nightmare` the final count is doubled.
///
/// Returns `1` if any ammo was actually added; `0` if the player was
/// already at maximum or the ammo type is `am_noammo`.  As a side-effect,
/// if the player had zero ammo of this type before the pickup, a more
/// appropriate weapon may be queued as `pendingweapon`.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a live `PlayerT`.
/// Global mutable statics `maxammo`, `clipammo`, `gameskill` must only be
/// accessed from the game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_GiveAmmo(player: *mut PlayerT, ammo: c_int, num: c_int) -> c_int
{
    p_give_ammo(&mut *player, ammo, num)
}

/// Rust-side body of [`P_GiveAmmo`]. Takes `&mut PlayerT` so callers can
/// pass an existing borrow without re-deriving a second `&mut` from the
/// same raw pointer (which would violate aliasing rules).
pub(super) unsafe fn p_give_ammo(player: &mut PlayerT, ammo: c_int, mut num: c_int) -> c_int
{
    if ammo == am_noammo
    {
        return 0;
    }
    if ammo > NUMAMMO as c_int
    {
        i_error!("P_GiveAmmo: bad type");
    }
    let idx = ammo as usize;
    if player.ammo[idx] == player.maxammo[idx]
    {
        return 0;
    }
    if num != 0
    {
        num *= clipammo[idx];
    }
    else
    {
        num = clipammo[idx] / 2;
    }
    if gameskill == sk_baby || gameskill == sk_nightmare
    {
        num <<= 1;
    }
    let oldammo = player.ammo[idx];
    player.ammo[idx] += num;
    if player.ammo[idx] > player.maxammo[idx]
    {
        player.ammo[idx] = player.maxammo[idx];
    }
    if oldammo != 0
    {
        return 1;
    }
    match ammo
    {
        am_clip if player.readyweapon == wp_fist =>
        {
            if player.weaponowned[wp_chaingun as usize] != 0
            {
                player.pendingweapon = wp_chaingun;
            }
            else
            {
                player.pendingweapon = wp_pistol;
            }
        }
        am_shell
            if (player.readyweapon == wp_fist || player.readyweapon == wp_pistol)
                && player.weaponowned[wp_shotgun as usize] != 0 =>
        {
            player.pendingweapon = wp_shotgun;
        }
        am_cell
            if (player.readyweapon == wp_fist || player.readyweapon == wp_pistol)
                && player.weaponowned[wp_plasma as usize] != 0 =>
        {
            player.pendingweapon = wp_plasma;
        }
        am_misl
            if player.readyweapon == wp_fist && player.weaponowned[wp_missile as usize] != 0 =>
        {
            player.pendingweapon = wp_missile;
        }
        _ => {}
    }
    1
}

// ---------------------------------------------------------------------------
// P_GiveWeapon
// ---------------------------------------------------------------------------

/// Attempt to give the player weapon `weapon`.
///
/// `dropped` is non-zero when the weapon was dropped by a dying monster
/// (half the normal ammo is given).  In a net-game without deathmatch-2,
/// weapons stay in the level and only ammo is given.
///
/// Returns `1` if either the weapon or its ammo was successfully added;
/// `0` otherwise.  The weapon is queued as `pendingweapon` when granted.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a live `PlayerT`.
/// Global mutable statics `netgame`, `deathmatch`, `consoleplayer`,
/// `players`, and `weaponinfo` must only be accessed from the game-logic
/// thread.
#[no_mangle]
pub unsafe extern "C" fn P_GiveWeapon(
    player: *mut PlayerT,
    weapon: c_int,
    dropped: c_int,
) -> c_int
{
    p_give_weapon(&mut *player, weapon, dropped)
}

/// Rust-side body of [`P_GiveWeapon`]. Takes `&mut PlayerT` so callers can
/// pass an existing borrow; avoids re-deriving a second `&mut` from the
/// same raw pointer.
pub(super) unsafe fn p_give_weapon(player: &mut PlayerT, weapon: c_int, dropped: c_int) -> c_int
{
    let widx = weapon as usize;
    let ammo_kind = weaponinfo[widx].ammo;
    if netgame != 0 && deathmatch != 2 && dropped == 0
    {
        if player.weaponowned[widx] != 0
        {
            return 0;
        }
        player.bonuscount += BONUSADD;
        player.weaponowned[widx] = 1;
        if deathmatch != 0
        {
            p_give_ammo(player, ammo_kind, 5);
        }
        else
        {
            p_give_ammo(player, ammo_kind, 2);
        }
        player.pendingweapon = weapon;
        let player_ptr = player as *mut PlayerT;
        if std::ptr::eq(
            player_ptr,
            std::ptr::addr_of_mut!(players[0]).add(consoleplayer as usize),
        )
        {
            S_StartSound(std::ptr::null_mut(), Sfx::Wpnup as c_int);
        }
        return 0;
    }
    let gaveammo: c_int = if ammo_kind != am_noammo
    {
        if dropped != 0
        {
            p_give_ammo(player, ammo_kind, 1)
        }
        else
        {
            p_give_ammo(player, ammo_kind, 2)
        }
    }
    else
    {
        0
    };
    let gaveweapon: c_int;
    if player.weaponowned[widx] != 0
    {
        gaveweapon = 0;
    }
    else
    {
        gaveweapon = 1;
        player.weaponowned[widx] = 1;
        player.pendingweapon = weapon;
    }
    (gaveweapon != 0 || gaveammo != 0) as c_int
}

// ---------------------------------------------------------------------------
// P_GiveBody
// ---------------------------------------------------------------------------

/// Attempt to add `num` health points to the player, capped at `MAXHEALTH`
/// (100).
///
/// Does nothing and returns `0` if the player is already at or above the
/// cap.  Also syncs `player.mo.health` to match.  Returns `1` on success.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a live `PlayerT`, and
/// `player.mo` must be a valid, non-null pointer to the player's map object.
#[no_mangle]
pub unsafe extern "C" fn P_GiveBody(player: *mut PlayerT, num: c_int) -> c_int
{
    p_give_body(&mut *player, num)
}

/// Rust-side body of [`P_GiveBody`]. See [`p_give_ammo`] for rationale.
pub(super) unsafe fn p_give_body(player: &mut PlayerT, num: c_int) -> c_int
{
    if player.health >= MAXHEALTH
    {
        return 0;
    }
    player.health += num;
    if player.health > MAXHEALTH
    {
        player.health = MAXHEALTH;
    }
    let mo = &mut *(player.mo as *mut mobj_t);
    mo.health = player.health;
    1
}

// ---------------------------------------------------------------------------
// P_GiveArmor
// ---------------------------------------------------------------------------

/// Attempt to give the player armor of `armortype` (1 = green, 2 = blue).
///
/// The effective armor-point value is `armortype * 100`.  Returns `0` if the
/// player already has at least that many armor points (i.e. the pick-up
/// would not help).  Otherwise sets `armortype` and `armorpoints` and
/// returns `1`.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a live `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_GiveArmor(player: *mut PlayerT, armortype: c_int) -> c_int
{
    p_give_armor(&mut *player, armortype)
}

/// Rust-side body of [`P_GiveArmor`]. See [`p_give_ammo`] for rationale.
pub(super) fn p_give_armor(player: &mut PlayerT, armortype: c_int) -> c_int
{
    let hits = armortype * 100;
    if player.armorpoints >= hits
    {
        return 0;
    }
    player.armortype = armortype;
    player.armorpoints = hits;
    1
}

// ---------------------------------------------------------------------------
// P_GiveCard
// ---------------------------------------------------------------------------

/// Give the player key `card` if they do not already have it.
///
/// Also adds `BONUSADD` to `bonuscount` to flash the HUD gold.  If the
/// player already owns the card the function returns immediately without
/// side-effects.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a live `PlayerT`.
#[no_mangle]
pub unsafe extern "C" fn P_GiveCard(player: *mut PlayerT, card: c_int)
{
    p_give_card(&mut *player, card)
}

/// Rust-side body of [`P_GiveCard`]. See [`p_give_ammo`] for rationale.
pub(super) fn p_give_card(player: &mut PlayerT, card: c_int)
{
    let idx = card as usize;
    if player.cards[idx] != 0
    {
        return;
    }
    player.bonuscount = BONUSADD;
    player.cards[idx] = 1;
}

// ---------------------------------------------------------------------------
// P_GivePower
// ---------------------------------------------------------------------------

/// Attempt to activate power-up `power` for the player.
///
/// Sets the appropriate `powers[]` timer for timed power-ups.  The
/// invisibility power additionally sets `MF_SHADOW` on the player's mobj.
/// The strength (berserk) power calls `P_GiveBody` to restore health to
/// 100.  Power-ups that are already active return `0`.
///
/// Returns `1` if the power was granted, `0` if it was already active.
///
/// # Safety
///
/// `player` must be a valid, non-null pointer to a live `PlayerT`, and for
/// `pw_invisibility`, `player.mo` must also be valid.
#[no_mangle]
pub unsafe extern "C" fn P_GivePower(player: *mut PlayerT, power: c_int) -> c_int
{
    p_give_power(&mut *player, power)
}

/// Rust-side body of [`P_GivePower`]. See [`p_give_ammo`] for rationale.
pub(super) unsafe fn p_give_power(player: &mut PlayerT, power: c_int) -> c_int
{
    let power = power as usize;
    if power == pw_invulnerability
    {
        player.powers[power] = INVULNTICS;
        return 1;
    }
    if power == pw_invisibility
    {
        player.powers[power] = INVISTICS;
        let mo = &mut *(player.mo as *mut mobj_t);
        mo.flags |= MF_SHADOW;
        return 1;
    }
    if power == pw_infrared
    {
        player.powers[power] = INFRATICS;
        return 1;
    }
    if power == pw_ironfeet
    {
        player.powers[power] = IRONTICS;
        return 1;
    }
    if power == pw_strength
    {
        p_give_body(player, 100);
        player.powers[power] = 1;
        return 1;
    }
    if player.powers[power] != 0
    {
        return 0;
    }
    player.powers[power] = 1;
    1
}
