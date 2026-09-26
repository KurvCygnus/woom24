//! Switch mechanics: the setup-time build of the switch-pair table
//! (`P_InitSwitchList`), timed button registration (`P_StartButton`),
//! and the switch texture flip / button queue
//! (`P_ChangeSwitchTexture`) -- bit-exact with the table half of
//! `vendor/doomgeneric/p_switch.c`.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_void};
use std::os::raw::c_short;

use super::state::{
    bottom, middle, top, ALPH_SWITCH_LIST, buttonlist, numswitches, switchlist, BUTTONTIME,
    MAXBUTTONS,
};
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::p_lights::line_t;
use crate::doom::p_setup::sides;
use crate::doom::r_data::R_TextureNumForName;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::i_error;

/// Build the runtime switch-pair table from the built-in `ALPH_SWITCH_LIST`.
///
/// Should be called once during level initialization.  The episode number
/// is derived from `gamemode`: shareware uses episode 1, registered/retail
/// use episode 2, and commercial uses episode 3.  Only pairs whose
/// `episode` field is <= the current episode are included.
///
/// Populates `switchlist` with alternating texture-number pairs and sets
/// `numswitches`.  A sentinel value of `-1` is written after the last valid
/// pair.
///
/// # FIXME
///
/// The C source (`p_switch.c` line 138-139) wraps each texture name with
/// `DEH_String()` before passing it to `R_TextureNumForName`, allowing
/// DEHacked patches to rename switch textures.  This port omits that
/// wrapper because `FEATURE_DEHACKED` is disabled.
#[no_mangle]
pub extern "C" fn P_InitSwitchList()
{
    unsafe {
        let episode: i16 = match gamemode
        {
            m if m == d_mode::registered || m == d_mode::retail => 2,
            m if m == d_mode::commercial => 3,
            _ => 1,
        };

        let mut index = 0usize;
        for sw in &ALPH_SWITCH_LIST
        {
            if sw.episode == 0
            {
                numswitches = (index / 2) as c_int;
                switchlist[index] = -1;
                break;
            }
            if sw.episode <= episode
            {
                switchlist[index] = R_TextureNumForName(sw.name1.as_ptr() as *mut c_char);
                index += 1;
                switchlist[index] = R_TextureNumForName(sw.name2.as_ptr() as *mut c_char);
                index += 1;
            }
        }
    }
}

/// Register a timed button that will reset after `time` tics.
///
/// Scans `buttonlist` for an existing active slot for `line`; if one is
/// found the call is a no-op (the button is already pressed).  Otherwise
/// the first free slot (`btimer == 0`) is filled.  If no slot is available
/// `I_Error` is called.
///
/// - `w` — which sidedef texture the button occupies (`top`, `middle`, or
///   `bottom`).
/// - `texture` — original texture number to restore on expiry.
/// - `time` — countdown in tics; typically `BUTTONTIME` (35).
///
/// # Safety
///
/// `line` must be a valid, non-null pointer to a live `line_t`.
/// `buttonlist` must only be accessed from the game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_StartButton(line: *mut line_t, w: c_int, texture: c_int, time: c_int)
{
    // See if button is already pressed
    for i in 0..MAXBUTTONS
    {
        if buttonlist[i].btimer != 0 && buttonlist[i].line == line
        {
            return;
        }
    }

    for i in 0..MAXBUTTONS
    {
        if buttonlist[i].btimer == 0
        {
            buttonlist[i].line = line;
            buttonlist[i].where_ = w;
            buttonlist[i].btexture = texture;
            buttonlist[i].btimer = time;
            buttonlist[i].soundorg =
                &(*(*line).frontsector).soundorg as *const [u8; 40] as *mut c_void;
            return;
        }
    }

    i_error!("P_StartButton: no button slots left!");
}

/// Toggle the switch texture on the front side of `line` and, if `useAgain`
/// is non-zero, queue a timed button to restore it after `BUTTONTIME` tics.
///
/// Reads the top, middle, and bottom textures of the front sidedef and
/// searches `switchlist` for a match.  When a match is found the texture is
/// flipped to its partner (`switchlist[i ^ 1]`), a switch sound is played
/// from `buttonlist[0].soundorg`, and the function returns.
///
/// If `useAgain == 0` the line's special is cleared to make the switch
/// one-shot.  Exit switches (special 11) play `sfx_swtchx` instead of the
/// normal `sfx_swtchn`.
///
/// # Safety
///
/// `line` must be a valid, non-null pointer to a live `line_t`.  The global
/// `switchlist`, `numswitches`, `buttonlist`, and `sides` arrays must only
/// be accessed from the game-logic thread.
#[no_mangle]
pub unsafe extern "C" fn P_ChangeSwitchTexture(line: *mut line_t, useAgain: c_int)
{
    if useAgain == 0
    {
        (*line).special = 0;
    }

    let sidenum = (*line).sidenum[0] as isize;
    let texTop = (*sides.offset(sidenum)).toptexture;
    let texMid = (*sides.offset(sidenum)).midtexture;
    let texBot = (*sides.offset(sidenum)).bottomtexture;

    let mut sound = Sfx::Swtchn as c_int;

    // EXIT SWITCH?
    if (*line).special == 11
    {
        sound = Sfx::Swtchx as c_int;
    }

    for i in 0..(numswitches * 2) as usize
    {
        if switchlist[i] == texTop as c_int
        {
            S_StartSound(buttonlist[0].soundorg, sound);
            (*sides.offset(sidenum)).toptexture = switchlist[i ^ 1] as c_short;
            if useAgain != 0
            {
                P_StartButton(line, top, switchlist[i], BUTTONTIME);
            }
            return;
        }
        else if switchlist[i] == texMid as c_int
        {
            S_StartSound(buttonlist[0].soundorg, sound);
            (*sides.offset(sidenum)).midtexture = switchlist[i ^ 1] as c_short;
            if useAgain != 0
            {
                P_StartButton(line, middle, switchlist[i], BUTTONTIME);
            }
            return;
        }
        else if switchlist[i] == texBot as c_int
        {
            S_StartSound(buttonlist[0].soundorg, sound);
            (*sides.offset(sidenum)).bottomtexture = switchlist[i ^ 1] as c_short;
            if useAgain != 0
            {
                P_StartButton(line, bottom, switchlist[i], BUTTONTIME);
            }
            return;
        }
    }
}
