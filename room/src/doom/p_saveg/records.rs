//! Struct record serialization for the save format: the per-struct
//! read/write pairs (mapthing, thinker header, mobj, ticcmd, pspdef,
//! player), plus the `make_actionf_p1!` builder family that turns the
//! ROOT-SHIMMED `T_*` thinker items into the type-erased `actionf_t`
//! values stored into `acp1` on specials reload.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

use crate::doom::d_player::{
    PlayerT, PspdefT, TiccmdT, MAXPLAYERS, NUMAMMO, NUMCARDS, NUMPOWERS, NUMPSPRITES, NUMWEAPONS,
};
use crate::doom::info::State;
use crate::doom::p_ceilng::{ceiling_t, T_MoveCeiling};
use crate::doom::p_doors::{vldoor_t, T_VerticalDoor};
use crate::doom::p_floor::{floormove_t, T_MoveFloor};
use crate::doom::p_lights::{glow_t, lightflash_t, strobe_t, T_Glow, T_LightFlash, T_StrobeFlash};
use crate::doom::p_plats::{plat_t, T_PlatRaise};
use crate::doom::p_tick::{actionf_t, thinker_t};

use super::stream::{read_byte, read_enum32, read_le16, read_le32, write_byte, write_enum32,
                    write_le16, write_le32};
use super::swizzle::{
    read_player_index, read_state_index, write_player_index, write_state_index,
};

// ---------------------------------------------------------------------------
// Thinker function pointer reassignment helpers
// ---------------------------------------------------------------------------

/// Generates a helper function that constructs an `actionf_t` holding a
/// type-erased pointer to a specific thinker function.
///
/// Each generated function (e.g. `actionf_of_move_ceiling`) transmutes the
/// concrete thinker function pointer (`T_MoveCeiling`, etc.) to the generic
/// `unsafe extern "C" fn(*mut c_void)` required by `actionf_t::acp1`. This
/// is the Rust equivalent of the C cast
/// `(actionf_p1)T_MoveCeiling`.
macro_rules! make_actionf_p1 {
    ($fn_name:ident, $arg_ty:ty, $c_fn:expr) => {
        pub(super) fn $fn_name() -> actionf_t
        {
            actionf_t
            {
                acp1: Some(unsafe {
                    core::mem::transmute::<
                        unsafe extern "C" fn(*mut $arg_ty),
                        unsafe extern "C" fn(*mut c_void),
                    >($c_fn)
                }),
            }
        }
    };
}

// Returns an `actionf_t` for `T_MoveCeiling` (ceiling special thinker).
// Each $c_fn argument is the ROOT-SHIMMED item imported through the
// graduated module root above -- never a local wrapper (the reload
// compares in `specials` must see the same address).
make_actionf_p1!(actionf_of_move_ceiling, ceiling_t, T_MoveCeiling);
// Returns an `actionf_t` for `T_VerticalDoor` (door special thinker).
make_actionf_p1!(actionf_of_vertical_door, vldoor_t, T_VerticalDoor);
// Returns an `actionf_t` for `T_MoveFloor` (floor special thinker).
make_actionf_p1!(actionf_of_move_floor, floormove_t, T_MoveFloor);
// Returns an `actionf_t` for `T_PlatRaise` (platform special thinker).
make_actionf_p1!(actionf_of_plat_raise, plat_t, T_PlatRaise);
// Returns an `actionf_t` for `T_LightFlash` (random light-flash thinker).
make_actionf_p1!(actionf_of_light_flash, lightflash_t, T_LightFlash);
// Returns an `actionf_t` for `T_StrobeFlash` (strobe light thinker).
make_actionf_p1!(actionf_of_strobe_flash, strobe_t, T_StrobeFlash);
// Returns an `actionf_t` for `T_Glow` (glow light thinker).
make_actionf_p1!(actionf_of_glow, glow_t, T_Glow);

// ---------------------------------------------------------------------------
// Struct serialization
// ---------------------------------------------------------------------------

//
// mapthing_t
//

/// Reads a `mapthing_t` from the save stream into `*mt`.
///
/// Reads five `i16` fields in order: x, y, angle, type, options.
/// C origin: `saveg_read_mapthing_t`.
#[doc(alias = "saveg_read_mapthing_t")]
pub(super) unsafe fn read_mapthing(mt: *mut crate::doom::c_ffi::mapthing_t)
{
    let s = &mut *mt;
    s.x = read_le16() as i16;
    s.y = read_le16() as i16;
    s.angle = read_le16() as i16;
    s.r#type = read_le16() as i16;
    s.options = read_le16() as i16;
}

/// Writes a `mapthing_t` to the save stream from `*mt`.
///
/// Writes five `i16` fields in order: x, y, angle, type, options.
/// C origin: `saveg_write_mapthing_t`.
#[doc(alias = "saveg_write_mapthing_t")]
pub(super) unsafe fn write_mapthing(mt: *const crate::doom::c_ffi::mapthing_t)
{
    let s = &*mt;
    write_le16(s.x as u16);
    write_le16(s.y as u16);
    write_le16(s.angle as u16);
    write_le16(s.r#type as u16);
    write_le16(s.options as u16);
}

//
// thinker_t
//

/// Reads a `thinker_t` header (prev, next, function) from the save stream.
///
/// The `prev` and `next` pointer values stored in the stream are raw addresses
/// from the saving session and are not meaningful in the loading session; they
/// will be overwritten when the thinker is re-linked. A stream value of `0`
/// for the function pointer becomes `None` rather than `Some(NULL)` to avoid
/// calling a null function pointer (which would be UB).
/// C origin: `saveg_read_thinker_t`.
#[doc(alias = "saveg_read_thinker_t")]
pub(super) unsafe fn read_thinker_header(th: *mut thinker_t)
{
    let s = &mut *th;
    // Read prev/next as raw pointer indices (will be rebuilt)
    s.prev = read_le32() as usize as *mut thinker_t;
    s.next = read_le32() as usize as *mut thinker_t;
    // 0 in the save stream means no function; transmuting 0 to fn is UB.
    let raw = read_le32() as usize;
    s.function.acp1 = if raw == 0
    {
        None
    }
    else
    {
        Some(core::mem::transmute::<
            usize,
            unsafe extern "C" fn(*mut c_void),
        >(raw))
    };
}

/// Writes a `thinker_t` header (prev, next, function) to the save stream.
///
/// Pointer fields are written as raw 32-bit addresses; the function pointer is
/// written as `0` when absent (`None`). C origin: `saveg_write_thinker_t`.
#[doc(alias = "saveg_write_thinker_t")]
pub(super) unsafe fn write_thinker_header(th: *const thinker_t)
{
    let s = &*th;
    write_le32(s.prev as u32);
    write_le32(s.next as u32);
    write_le32(s.function.acp1.map(|f| f as usize as u32).unwrap_or(0));
}

//
// mobj_t — serialized as C struct via FFI
//

/// Reads a full `mobj_t` record from the save stream into the memory at `mobj`.
///
/// The `mobj` pointer must point to a valid `mobj_t`-sized allocation.
/// After reading:
/// - `prev`/`next` in the embedded thinker are raw addresses (stale from the
///   saving session) and will be rebuilt by `P_AddThinker`.
/// - `snext`, `sprev`, `bnext`, `bprev`, `subsector`, `target`, `tracer` are
///   raw saved addresses that callers must NULL out or re-resolve.
/// - `state` is resolved from a stream index via `saveg_read_state_ptr`.
/// - `player` is a 1-based index; re-resolved and the back-pointer
///   `player->mo` is updated in place.
/// - `info` is a raw saved pointer (will be overwritten by caller with
///   `&mobjinfo[type]`).
///
/// C origin: `saveg_read_mobj_t`.
#[doc(alias = "saveg_read_mobj_t")]
pub(super) unsafe fn read_mobj_record(mobj: *mut c_void)
{
    let mo: *mut crate::doom::c_ffi::mobj_t = mobj as *mut crate::doom::c_ffi::mobj_t;

    // thinker_t
    {
        let thinker_ptr = std::ptr::addr_of_mut!((*mo).thinker_prev) as *mut thinker_t;
        (*thinker_ptr).prev = read_le32() as usize as *mut thinker_t;
        (*thinker_ptr).next = read_le32() as usize as *mut thinker_t;
        let raw = read_le32() as usize;
        (*thinker_ptr).function.acp1 = if raw == 0
        {
            None
        }
        else
        {
            Some(core::mem::transmute::<
                usize,
                unsafe extern "C" fn(*mut c_void),
            >(raw))
        };
    }

    // x, y, z
    (*mo).x = read_le32() as c_int;
    (*mo).y = read_le32() as c_int;
    (*mo).z = read_le32() as c_int;

    // snext, sprev
    (*mo).snext = read_le32() as *mut c_void;
    (*mo).sprev = read_le32() as *mut c_void;

    // angle, sprite, frame
    (*mo).angle = read_le32();
    (*mo).sprite = read_enum32() as c_int;
    (*mo).frame = read_le32() as c_int;

    // bnext, bprev
    (*mo).bnext = read_le32() as *mut c_void;
    (*mo).bprev = read_le32() as *mut c_void;

    // subsector
    (*mo).subsector = read_le32() as *mut c_void;

    // floorz, ceilingz, radius, height
    (*mo).floorz = read_le32() as c_int;
    (*mo).ceilingz = read_le32() as c_int;
    (*mo).radius = read_le32() as c_int;
    (*mo).height = read_le32() as c_int;

    // momx, momy, momz
    (*mo).momx = read_le32() as c_int;
    (*mo).momy = read_le32() as c_int;
    (*mo).momz = read_le32() as c_int;

    // validcount
    (*mo).validcount = read_le32() as c_int;

    // type
    (*mo).type_ = read_enum32() as c_int;

    // info (raw pointer)
    (*mo).info = read_le32() as *mut crate::doom::c_ffi::mobjinfo_t;

    // tics
    (*mo).tics = read_le32() as c_int;

    // state (index into states array)
    let state_idx = read_le32();
    (*mo).state = read_state_index(state_idx) as *mut crate::doom::c_ffi::state_t;

    // flags
    (*mo).flags = read_le32() as c_int;

    // health
    (*mo).health = read_le32() as c_int;

    // movedir, movecount
    (*mo).movedir = read_le32() as c_int;
    (*mo).movecount = read_le32() as c_int;

    // target (raw pointer)
    (*mo).target = read_le32() as *mut c_void;

    // reactiontime, threshold
    (*mo).reactiontime = read_le32() as c_int;
    (*mo).threshold = read_le32() as c_int;

    // player (index + 1)
    let pl = read_le32();
    if pl > 0
    {
        (*mo).player = read_player_index(pl) as *mut c_void;
        let player = (*mo).player as *mut PlayerT;
        if !player.is_null() { (*player).mo = mobj as *mut crate::doom::d_player::mobj_t; }
    }
    else { (*mo).player = std::ptr::null_mut(); }

    // lastlook
    (*mo).lastlook = read_le32() as c_int;

    // spawnpoint
    read_mapthing(std::ptr::addr_of_mut!((*mo).spawnpoint));

    // tracer (raw pointer)
    (*mo).tracer = read_le32() as *mut c_void;
}

/// Writes a full `mobj_t` record to the save stream from `mobj`.
///
/// Pointers (snext, sprev, bnext, bprev, subsector, target, tracer, info) are
/// written as raw 32-bit addresses; they are not portable across sessions but
/// are reconstructed on load. `state` is written as an index into `states`.
/// `player` is written as a 1-based player index (0 for NULL).
/// C origin: `saveg_write_mobj_t`.
#[doc(alias = "saveg_write_mobj_t")]
pub(super) unsafe fn write_mobj_record(mobj: *const c_void)
{
    let mo: *const crate::doom::c_ffi::mobj_t = mobj as *const crate::doom::c_ffi::mobj_t;

    // thinker_t
    {
        let thinker_ptr = std::ptr::addr_of!((*mo).thinker_prev) as *const thinker_t;
        write_le32((*thinker_ptr).prev as u32);
        write_le32((*thinker_ptr).next as u32);
        write_le32(
            (*thinker_ptr)
                .function
                .acp1
                .map(|f| f as usize as u32)
                .unwrap_or(0),
        );
    }

    // x, y, z
    write_le32((*mo).x as u32);
    write_le32((*mo).y as u32);
    write_le32((*mo).z as u32);

    // snext, sprev
    write_le32((*mo).snext as u32);
    write_le32((*mo).sprev as u32);

    // angle, sprite, frame
    write_le32((*mo).angle);
    write_enum32((*mo).sprite as u32);
    write_le32((*mo).frame as u32);

    // bnext, bprev
    write_le32((*mo).bnext as u32);
    write_le32((*mo).bprev as u32);

    // subsector
    write_le32((*mo).subsector as u32);

    // floorz, ceilingz, radius, height
    write_le32((*mo).floorz as u32);
    write_le32((*mo).ceilingz as u32);
    write_le32((*mo).radius as u32);
    write_le32((*mo).height as u32);

    // momx, momy, momz
    write_le32((*mo).momx as u32);
    write_le32((*mo).momy as u32);
    write_le32((*mo).momz as u32);

    // validcount
    write_le32((*mo).validcount as u32);

    // type
    write_enum32((*mo).type_ as u32);

    // info
    write_le32((*mo).info as u32);

    // tics
    write_le32((*mo).tics as u32);

    // state (index)
    let state = (*mo).state as *const State;
    if state.is_null() { write_le32(0); }
    else { write_le32(write_state_index(state)); }

    // flags
    write_le32((*mo).flags as u32);

    // health
    write_le32((*mo).health as u32);

    // movedir, movecount
    write_le32((*mo).movedir as u32);
    write_le32((*mo).movecount as u32);

    // target
    write_le32((*mo).target as u32);

    // reactiontime, threshold
    write_le32((*mo).reactiontime as u32);
    write_le32((*mo).threshold as u32);

    // player
    let player = (*mo).player as *const PlayerT;
    if player.is_null() { write_le32(0); }
    else { write_le32(write_player_index(player)); }

    // lastlook
    write_le32((*mo).lastlook as u32);

    // spawnpoint
    write_mapthing(std::ptr::addr_of!((*mo).spawnpoint));

    // tracer
    write_le32((*mo).tracer as u32);
}

//
// ticcmd_t
//
// Only serialize the 6 fields the C version writes. TiccmdT has extra fields.
//

/// Reads a `ticcmd_t` (player input command snapshot) from the save stream.
///
/// Only the 6 fields serialized by the C version are read:
/// `forwardmove`, `sidemove`, `angleturn`, `consistancy`, `chatchar`,
/// `buttons`. Extra fields present in `TiccmdT` are not touched.
/// C origin: `saveg_read_ticcmd_t`.
#[doc(alias = "saveg_read_ticcmd_t")]
pub(super) unsafe fn read_ticcmd(cmd: *mut TiccmdT)
{
    let s = &mut *cmd;
    s.forwardmove = read_byte() as i8;
    s.sidemove = read_byte() as i8;
    s.angleturn = read_le16() as i16;
    s.consistancy = read_le16() as u8;
    s.chatchar = read_byte();
    s.buttons = read_byte();
}

/// Writes the 6 canonical `ticcmd_t` fields to the save stream.
///
/// C origin: `saveg_write_ticcmd_t`.
#[doc(alias = "saveg_write_ticcmd_t")]
pub(super) unsafe fn write_ticcmd(cmd: *const TiccmdT)
{
    let s = &*cmd;
    write_byte(s.forwardmove as u8);
    write_byte(s.sidemove as u8);
    write_le16(s.angleturn as u16);
    write_le16(s.consistancy as u16);
    write_byte(s.chatchar);
    write_byte(s.buttons);
}

//
// pspdef_t
//

/// Reads a `pspdef_t` (player sprite definition) from the save stream.
///
/// The `state` field uses a 1-based convention for psprites: index `0` maps
/// to NULL (weapon not active), while any positive index maps to
/// `states[index]`. This differs from `mobj_t` state handling where index `0`
/// refers to `states[0]`.
/// C origin: `saveg_read_pspdef_t`.
#[doc(alias = "saveg_read_pspdef_t")]
pub(super) unsafe fn read_pspdef(psp: *mut PspdefT)
{
    let s = &mut *psp;
    let state_idx = read_le32();
    // C guards state == 0 → NULL for pspdef (unlike mobj which always indexes states[]).
    s.state = if state_idx == 0
    {
        std::ptr::null_mut()
    }
    else { read_state_index(state_idx) as *mut crate::doom::d_player::state_t };
    s.tics = read_le32() as c_int;
    s.sx = read_le32() as c_int;
    s.sy = read_le32() as c_int;
}

/// Writes a `pspdef_t` to the save stream.
///
/// Writes the state index (`state - states`) or `0` for NULL, followed by
/// `tics`, `sx` (fixed-point screen x offset), and `sy` (fixed-point screen y
/// offset). C origin: `saveg_write_pspdef_t`.
#[doc(alias = "saveg_write_pspdef_t")]
pub(super) unsafe fn write_pspdef(psp: *const PspdefT)
{
    let s = &*psp;
    if s.state.is_null() { write_le32(0); }
    else { write_le32(write_state_index(s.state as *const State)); }
    write_le32(s.tics as u32);
    write_le32(s.sx as u32);
    write_le32(s.sy as u32);
}

//
// player_t
//

/// Reads a full `player_t` record from the save stream into `*pl`.
///
/// Fields are read in the same order as the C version. Notable points:
/// - `mo` is read as a raw pointer (will be set to NULL by `P_UnArchivePlayers`
///   and rebuilt by `P_UnArchiveThinkers`).
/// - `playerstate` and enum fields (`readyweapon`, `pendingweapon`) are read
///   as 32-bit values.
/// - Power timers, key cards, frag counts, weapon ownership, ammo, and max
///   ammo are each 32-bit entries.
/// - `message` and `attacker` are raw pointers; callers zero them after read.
/// - Each of `NUMPSPRITES` player-sprite slots is read via
///   `saveg_read_pspdef_t`.
///
/// C origin: `saveg_read_player_t`.
#[doc(alias = "saveg_read_player_t")]
pub(super) unsafe fn read_player_record(pl: *mut PlayerT)
{
    let s = &mut *pl;

    // mo (raw pointer, will be NULL after)
    s.mo = read_le32() as *mut crate::doom::d_player::mobj_t;

    // playerstate
    s.playerstate = read_enum32() as c_int;

    // cmd
    read_ticcmd(&mut s.cmd);

    // viewz, viewheight, deltaviewheight, bob
    s.viewz = read_le32() as c_int;
    s.viewheight = read_le32() as c_int;
    s.deltaviewheight = read_le32() as c_int;
    s.bob = read_le32() as c_int;

    // health, armorpoints, armortype
    s.health = read_le32() as c_int;
    s.armorpoints = read_le32() as c_int;
    s.armortype = read_le32() as c_int;

    // powers[NUMPOWERS]
    for i in 0..NUMPOWERS { s.powers[i] = read_le32() as c_int; }

    // cards[NUMCARDS]
    for i in 0..NUMCARDS { s.cards[i] = read_le32() as c_int; }

    // backpack
    s.backpack = read_le32() as c_int;

    // frags[MAXPLAYERS]
    for i in 0..MAXPLAYERS { s.frags[i] = read_le32() as c_int; }

    // readyweapon, pendingweapon
    s.readyweapon = read_enum32() as c_int;
    s.pendingweapon = read_enum32() as c_int;

    // weaponowned[NUMWEAPONS]
    for i in 0..NUMWEAPONS { s.weaponowned[i] = read_le32() as c_int; }

    // ammo[NUMAMMO]
    for i in 0..NUMAMMO { s.ammo[i] = read_le32() as c_int; }

    // maxammo[NUMAMMO]
    for i in 0..NUMAMMO { s.maxammo[i] = read_le32() as c_int; }

    // attackdown, usedown
    s.attackdown = read_le32() as c_int;
    s.usedown = read_le32() as c_int;

    // cheats, refire
    s.cheats = read_le32() as c_int;
    s.refire = read_le32() as c_int;

    // killcount, itemcount, secretcount
    s.killcount = read_le32() as c_int;
    s.itemcount = read_le32() as c_int;
    s.secretcount = read_le32() as c_int;

    // message (raw pointer)
    s.message = read_le32() as *mut c_char;

    // damagecount, bonuscount
    s.damagecount = read_le32() as c_int;
    s.bonuscount = read_le32() as c_int;

    // attacker (raw pointer)
    s.attacker = read_le32() as *mut crate::doom::d_player::mobj_t;

    // extralight, fixedcolormap, colormap
    s.extralight = read_le32() as c_int;
    s.fixedcolormap = read_le32() as c_int;
    s.colormap = read_le32() as c_int;

    // psprites[NUMPSPRITES]
    for i in 0..NUMPSPRITES { read_pspdef(&mut s.psprites[i]); }

    // didsecret
    s.didsecret = read_le32() as c_int;
}

/// Writes a full `player_t` record to the save stream from `*pl`.
///
/// Raw pointers (`mo`, `message`, `attacker`) are written as 32-bit addresses;
/// they are not valid across sessions but are zeroed on unarchive. All other
/// fields mirror `saveg_read_player_t` in order.
/// C origin: `saveg_write_player_t`.
#[doc(alias = "saveg_write_player_t")]
pub(super) unsafe fn write_player_record(pl: *const PlayerT)
{
    let s = &*pl;

    write_le32(s.mo as u32);
    write_enum32(s.playerstate as u32);
    write_ticcmd(&s.cmd);
    write_le32(s.viewz as u32);
    write_le32(s.viewheight as u32);
    write_le32(s.deltaviewheight as u32);
    write_le32(s.bob as u32);
    write_le32(s.health as u32);
    write_le32(s.armorpoints as u32);
    write_le32(s.armortype as u32);
    for i in 0..NUMPOWERS { write_le32(s.powers[i] as u32); }
    for i in 0..NUMCARDS { write_le32(s.cards[i] as u32); }
    write_le32(s.backpack as u32);
    for i in 0..MAXPLAYERS { write_le32(s.frags[i] as u32); }
    write_enum32(s.readyweapon as u32);
    write_enum32(s.pendingweapon as u32);
    for i in 0..NUMWEAPONS { write_le32(s.weaponowned[i] as u32); }
    for i in 0..NUMAMMO { write_le32(s.ammo[i] as u32); }
    for i in 0..NUMAMMO { write_le32(s.maxammo[i] as u32); }
    write_le32(s.attackdown as u32);
    write_le32(s.usedown as u32);
    write_le32(s.cheats as u32);
    write_le32(s.refire as u32);
    write_le32(s.killcount as u32);
    write_le32(s.itemcount as u32);
    write_le32(s.secretcount as u32);
    write_le32(s.message as u32);
    write_le32(s.damagecount as u32);
    write_le32(s.bonuscount as u32);
    write_le32(s.attacker as u32);
    write_le32(s.extralight as u32);
    write_le32(s.fixedcolormap as u32);
    write_le32(s.colormap as u32);
    for i in 0..NUMPSPRITES { write_pspdef(&s.psprites[i]); }
    write_le32(s.didsecret as u32);
}

#[cfg(test)]
mod tests
{
    // The fmemopen tests (and everything they touch) are Unix-only; the
    // imports are gated with them so Windows builds carry no unused-import
    // warnings.
    #[cfg(unix)]
    use crate::doom::p_saveg::stream::{save_stream, savegamelength, savegame_error};
    #[cfg(unix)]
    use crate::doom::p_tick::{actionf_t, thinker_t};
    #[cfg(unix)]
    use std::ptr;
    #[cfg(unix)]
    use std::sync::Mutex;

    #[cfg(unix)]
    static LOCK: Mutex<()> = Mutex::new(());

    /// Runs `f` with `save_stream` pointed at an in-memory buffer backed by
    /// `data`, then restores the original stream and error globals on exit.
    // glibc `fmemopen` has no MSVC equivalent; Unix-only until the savegame
    // stream source is abstracted.
    #[cfg(unix)]
    unsafe fn with_mem_stream<F: FnOnce()>(data: &mut [u8], f: F)
    {
        let old_stream = save_stream;
        let old_error = savegame_error;
        let old_len = savegamelength;
        save_stream = libc::fmemopen(
            data.as_mut_ptr() as *mut libc::c_void,
            data.len(),
            c"r".as_ptr() as *const libc::c_char,
        );
        savegame_error = 0;
        savegamelength = 0;
        f();
        libc::fclose(save_stream);
        save_stream = old_stream;
        savegame_error = old_error;
        savegamelength = old_len;
    }

    // glibc `fmemopen` (see `with_mem_stream`) is unavailable on Windows.
    #[cfg(unix)]
    #[test]
    fn read_thinker_header_zero_fn_ptr_becomes_none()
    {
        let _g = LOCK.lock().unwrap();
        // 12 bytes: prev(4) + next(4) + fn_ptr(4), all zero
        let mut data = [0u8; 12];
        unsafe
        {
            let mut th = thinker_t
            {
                prev: ptr::null_mut(),
                next: ptr::null_mut(),
                function: actionf_t
                {
                    acp1: None
                },
            };
            with_mem_stream(&mut data, || {
                super::read_thinker_header(&raw mut th as *mut thinker_t);
            });
            assert!(
                th.function.acp1.is_none(),
                "zero function pointer must deserialize as None, not Some(NULL)"
            );
        }
    }

    // glibc `fmemopen` (see `with_mem_stream`) is unavailable on Windows.
    #[cfg(unix)]
    #[test]
    fn read_mobj_record_zero_fn_ptr_becomes_none()
    {
        use crate::doom::c_ffi::mobj_t;
        let _g = LOCK.lock().unwrap();
        // mobj_t is large; we need enough bytes for the full struct read.
        // The thinker (prev+next+fn) is the first 12 bytes.
        // read_mobj_record reads many fields — pad to 256 zeros.
        let mut data = [0u8; 256];
        unsafe
        {
            let mut mo = std::mem::MaybeUninit::<mobj_t>::zeroed().assume_init();
            with_mem_stream(&mut data, || {
                super::read_mobj_record(&raw mut mo as *mut mobj_t as *mut libc::c_void);
            });
            let thinker_ptr = std::ptr::addr_of!(mo.thinker_prev) as *const thinker_t;
            assert!(
                (*thinker_ptr).function.acp1.is_none(),
                "zero function pointer in mobj thinker must deserialize as None, not Some(NULL)"
            );
        }
    }
}
