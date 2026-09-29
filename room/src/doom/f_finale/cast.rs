//! The Doom II cast-of-characters roll: start/tick/respond/draw over
//! `CASTORDER`, with the three load-bearing `caststate` state-address
//! compares (the attack-stop hack observes `info::states` item
//! identities directly -- `info::states` is data and never graduates,
//! so the compares survive iff both sides keep referencing the same
//! items).

use std::ffi::{c_char, c_int, c_short};
use std::ptr;

use super::text_tables::CASTORDER;
use super::{
    castattacking, castdeath, castframes, castnum, castonmelee, caststate, casttics, FinaleStage,
    FINALE_STAGE,
};
use crate::doom::d_event::event_t;
use crate::doom::hu_stuff::{hu_font, HU_FONTSIZE, HU_FONTSTART};
use crate::doom::info::*;
use crate::doom::r_data::firstspritelump;
use crate::doom::r_things::sprites;
use crate::doom::s_sound::{S_ChangeMusic, S_StartSound};
use crate::doom::sounds::{Mus, Sfx};
use crate::doom::v_video::{patch_t, V_DrawPatch, V_DrawPatchFlipped};
use crate::doom::w_wad::{W_CacheLumpNum, W_CacheLumpName};
use crate::doom::z_zone::PU_CACHE;

/// `event_t.type_` value for key-down events.
///
/// C origin: `ev_keydown` in `d_event.h`.
const ev_keydown: c_int = 0;

/// Mask applied to a sprite-frame index to strip the full-bright flag.
///
/// C origin: `FF_FRAMEMASK` in f_finale.c.
const FF_FRAMEMASK: c_int = 0x7fff;

/// Mirrors the C `spriteframe_t` structure from `r_things.h`.
///
/// `rotate` is non-zero if the sprite has rotations.  `lump` gives the WAD
/// lump number for each of the 8 rotation angles; `flip` indicates whether
/// each angle should be drawn mirrored.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct spriteframe_t {
    pub rotate: c_int,
    pub lump: [c_short; 8],
    pub flip: [u8; 8],
}

/// Mirrors the C `spritedef_t` structure from `r_things.h`.
///
/// Describes all frames for a single sprite.  `spriteframes` points to an
/// array of `numframes` [`spriteframe_t`] entries.
#[repr(C)]
pub(super) struct spritedef_t {
    pub numframes: c_int,
    pub spriteframes: *mut spriteframe_t,
}

/// Start the Doom II cast-of-characters roll.
///
/// Triggers a wipe (`wipegamestate = -1`), resets the cast index to 0,
/// initialises the first monster's see-state animation, and starts the
/// "evil" music track.
///
/// Called by [`super::lifecycle::ticker`] when the player presses fire on
/// map 30.  C origin: `F_StartCast` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle` reaches the upstream name through the root shim.
#[doc(alias = "F_StartCast")]
#[export_name = "F_StartCast"]
pub extern "C" fn start_cast() {
    unsafe {
        crate::doom::d_main::wipegamestate = -1;
        castnum = 0;
        let st = &(*std::ptr::addr_of!(crate::doom::info::mobjinfo[0])
            .add(CASTORDER[0].type_ as usize))
        .seestate;
        caststate = &mut crate::doom::info::states[*st as usize];
        casttics = (*caststate).tics;
        castdeath = 0;
        FINALE_STAGE = FinaleStage::Cast;
        castframes = 0;
        castonmelee = 0;
        castattacking = 0;
        S_ChangeMusic(Mus::Evil as c_int, 1);
    }
}

/// Advance the cast-roll animation by one game tick.
///
/// Decrements `casttics`; when it reaches zero either advances to the next
/// animation state (playing attack-sound effects at specific states) or, when
/// the current enemy's death animation has finished, moves on to the next
/// entry in `CASTORDER`.  At frame 12 an attack sequence (melee or missile,
/// alternating) is triggered; at frame 24 or when the enemy returns to its
/// see-state the attack is cancelled via the `stopattack` logic (refactored
/// into [`stop_attack`]).
///
/// Called by [`super::lifecycle::ticker`] each game tick while in the Cast
/// stage.  C origin: `F_CastTicker` in f_finale.c.
///
/// The three `caststate == &mut info::states[..]` compares are carried
/// VERBATIM from the pre-move file: they are pointer-identity checks
/// against `info::states` items (data tier, never renamed), the
/// demo-relevant surface of the cast attack-stop hack.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle` reaches the upstream name through the root shim.
#[doc(alias = "F_CastTicker")]
#[export_name = "F_CastTicker"]
pub extern "C" fn cast_ticker() {
    unsafe {
        casttics -= 1;
        if casttics > 0 { return; }

        if (*caststate).tics == -1 || (*caststate).nextstate == S_NULL {
            // switch from deathstate to next monster
            castnum += 1;
            castdeath = 0;
            if CASTORDER[castnum as usize].name.is_null() { castnum = 0; }
            let info = &*std::ptr::addr_of!(crate::doom::info::mobjinfo[0])
                .add(CASTORDER[castnum as usize].type_ as usize);
            if info.seesound != Sfx::None { S_StartSound(ptr::null_mut(), info.seesound as c_int); }
            let st = info.seestate;
            caststate = &mut crate::doom::info::states[st as usize];
            castframes = 0;
        } else {
            // just advance to next state in animation
            if caststate == &mut crate::doom::info::states[S_PLAY_ATK1 as usize] {
                // Oh, gross hack! Jump to stopattack logic below
                stop_attack();
                return;
            }
            let st = (*caststate).nextstate;
            caststate = &mut crate::doom::info::states[st as usize];
            castframes += 1;

            let sfx = match st {
                S_PLAY_ATK1 => Sfx::Dshtgn as c_int,
                S_POSS_ATK2 => Sfx::Pistol as c_int,
                S_SPOS_ATK2 => Sfx::Shotgn as c_int,
                S_VILE_ATK2 => Sfx::Vilatk as c_int,
                S_SKEL_FIST2 => Sfx::Skeswg as c_int,
                S_SKEL_FIST4 => Sfx::Skepch as c_int,
                S_SKEL_MISS2 => Sfx::Skeatk as c_int,
                S_FATT_ATK8 | S_FATT_ATK5 | S_FATT_ATK2 => Sfx::Firsht as c_int,
                S_CPOS_ATK2 | S_CPOS_ATK3 | S_CPOS_ATK4 => Sfx::Shotgn as c_int,
                S_TROO_ATK3 => Sfx::Claw as c_int,
                S_SARG_ATK2 => Sfx::Sgtatk as c_int,
                S_BOSS_ATK2 | S_BOS2_ATK2 | S_HEAD_ATK2 => Sfx::Firsht as c_int,
                S_SKULL_ATK2 => Sfx::Sklatk as c_int,
                S_SPID_ATK2 | S_SPID_ATK3 => Sfx::Shotgn as c_int,
                S_BSPI_ATK2 => Sfx::Plasma as c_int,
                S_CYBER_ATK2 | S_CYBER_ATK4 | S_CYBER_ATK6 => Sfx::Rlaunc as c_int,
                S_PAIN_ATK3 => Sfx::Sklatk as c_int,
                _ => 0,
            };

            if sfx != 0 { S_StartSound(ptr::null_mut(), sfx); }
        }

        if castframes == 12 {
            castattacking = 1;
            let info = &*std::ptr::addr_of!(crate::doom::info::mobjinfo[0])
                .add(CASTORDER[castnum as usize].type_ as usize);
            let st = if castonmelee != 0 {
                info.meleestate
            } else {
                info.missilestate
            };
            castonmelee ^= 1;
            caststate = &mut crate::doom::info::states[st as usize];
            if caststate == &mut crate::doom::info::states[S_NULL as usize] {
                let st2 = if castonmelee != 0 {
                    info.meleestate
                } else {
                    info.missilestate
                };
                caststate = &mut crate::doom::info::states[st2 as usize];
            }
        }

        if castattacking != 0
            && (castframes == 24
                || caststate
                    == &mut crate::doom::info::states[(*std::ptr::addr_of!(
                        crate::doom::info::mobjinfo[0]
                    )
                    .add(CASTORDER[castnum as usize].type_ as usize))
                    .seestate as usize])
        {
            castattacking = 0;
            castframes = 0;
            let st = (*std::ptr::addr_of!(crate::doom::info::mobjinfo[0])
                .add(CASTORDER[castnum as usize].type_ as usize))
            .seestate;
            caststate = &mut crate::doom::info::states[st as usize];
        }

        casttics = (*caststate).tics;
        if casttics == -1 { casttics = 15; }
    }
}

/// Cancel the current cast attack and return to the see-state animation.
///
/// Extracted from a `goto stopattack` label in the original C source.
/// Resets `castattacking` and `castframes`, jumps `caststate` back to the
/// current enemy's `seestate`, and reloads `casttics`.  If `tics == -1` (a
/// looping state with no fixed duration), defaults to 15 ticks.
///
/// C origin: the `stopattack:` label inside `F_CastTicker` in f_finale.c.
///
/// # Safety
///
/// All cast globals (`caststate`, `castnum`, etc.) must be in a consistent
/// state, as established by [`start_cast`].
#[doc(alias = "goto_stopattack")]
unsafe fn stop_attack() {
    castattacking = 0;
    castframes = 0;
    let st = (*std::ptr::addr_of!(crate::doom::info::mobjinfo[0])
        .add(CASTORDER[castnum as usize].type_ as usize))
    .seestate;
    caststate = &mut crate::doom::info::states[st as usize];
    casttics = (*caststate).tics;
    if casttics == -1 { casttics = 15; }
}

/// Handle a key-down event during the cast roll.
///
/// On the first key press, triggers the current enemy's death animation and
/// plays its death sound.  Subsequent key presses while `castdeath != 0` are
/// consumed but ignored.  Returns 1 if the event was consumed, 0 otherwise.
///
/// Called by [`super::lifecycle::responder`] while in the Cast stage.
/// C origin: `F_CastResponder` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle` reaches the upstream name through the root shim.
#[doc(alias = "F_CastResponder")]
#[export_name = "F_CastResponder"]
pub extern "C" fn cast_responder(ev: *mut event_t) -> c_int {
    unsafe {
        let event = &*ev;
        if event.type_ != ev_keydown { return 0; }
        if castdeath != 0 { return 1; }
        castdeath = 1;
        let info = &*std::ptr::addr_of!(crate::doom::info::mobjinfo[0])
            .add(CASTORDER[castnum as usize].type_ as usize);
        caststate = &mut crate::doom::info::states[info.deathstate as usize];
        casttics = (*caststate).tics;
        castframes = 0;
        castattacking = 0;
        if info.deathsound != Sfx::None {
            S_StartSound(ptr::null_mut(), info.deathsound as c_int);
        }
        1
    }
}

/// Compute the pixel width of `text` using the HUD font, then draw it
/// horizontally centred at y=180.
///
/// Characters not present in the HUD font advance the cursor by 4 pixels.
/// The text is drawn with `V_DrawPatch` at the calculated x-offset.
///
/// Called by [`cast_drawer`].  C origin: `F_CastPrint` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// the cast draw path reaches the upstream name through the root
/// shim.
#[doc(alias = "F_CastPrint")]
#[export_name = "F_CastPrint"]
pub extern "C" fn cast_print(text: *mut c_char) {
    unsafe {
        let mut ch = text;
        let mut width = 0;

        // find width
        loop {
            let c = *ch;
            if c == 0 {
                break;
            }
            ch = ch.add(1);
            let cidx = super::toupper(c as c_int) - HU_FONTSTART as c_int;
            if cidx < 0 || cidx >= HU_FONTSIZE as c_int {
                width += 4;
                continue;
            }
            let font = hu_font[cidx as usize];
            if font.is_null() {
                width += 4;
                continue;
            }
            width += (*font).width as c_int;
        }

        // draw it
        let mut cx = 160 - width / 2;
        ch = text;
        loop {
            let c = *ch;
            if c == 0 {
                break;
            }
            ch = ch.add(1);
            let cidx = super::toupper(c as c_int) - HU_FONTSTART as c_int;
            if cidx < 0 || cidx >= HU_FONTSIZE as c_int {
                cx += 4;
                continue;
            }
            let font = hu_font[cidx as usize];
            if font.is_null() {
                cx += 4;
                continue;
            }
            let w = (*font).width as c_int;
            V_DrawPatch(cx, 180, font);
            cx += w;
        }
    }
}

/// Draw the current cast-roll frame: BOSSBACK background, centred enemy sprite,
/// and the enemy name at the bottom.
///
/// The sprite frame is selected from `caststate.sprite` and
/// `caststate.frame & FF_FRAMEMASK`, using rotation 0.  If the frame's flip
/// flag is set, `V_DrawPatchFlipped` is used.
///
/// Called from [`super::lifecycle::drawer`] during the Cast stage.
/// C origin: `F_CastDrawer` in f_finale.c.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `lifecycle` reaches the upstream name through the root shim.
#[doc(alias = "F_CastDrawer")]
#[export_name = "F_CastDrawer"]
pub extern "C" fn cast_drawer() {
    unsafe {
        V_DrawPatch(
            0,
            0,
            W_CacheLumpName(super::DEH_String(c"BOSSBACK".as_ptr().cast_mut()), PU_CACHE)
                as *mut patch_t,
        );
        cast_print(super::DEH_String(CASTORDER[castnum as usize].name));

        let sprdef = &*(sprites as *mut spritedef_t).add((*caststate).sprite as usize);
        let sprframe = &*sprdef
            .spriteframes
            .add(((*caststate).frame & FF_FRAMEMASK) as usize);
        let lump = sprframe.lump[0] as c_int;
        let flip = sprframe.flip[0] as c_int;

        let patch = W_CacheLumpNum(lump + firstspritelump, PU_CACHE) as *mut patch_t;
        if flip != 0 {
            V_DrawPatchFlipped(160, 170, patch);
        } else {
            V_DrawPatch(160, 170, patch);
        }
    }
}
