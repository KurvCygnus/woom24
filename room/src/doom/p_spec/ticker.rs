//! The per-tic special-effect ticker: the scrolling-wall registry and
//! deathmatch timer statics, and `update_specials` (level timer,
//! texture/flat animation phase advance, wall scroll, button countdown)
//! -- bit-exact with the `P_UpdateSpecials` half of
//! `vendor/doomgeneric/p_spec.c`.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_short, c_void};
use std::ptr;

use crate::doom::c_ffi::line_t;
use crate::doom::g_game::G_ExitLevel;
use crate::doom::m_fixed::FRACUNIT;
use crate::doom::p_setup::sides;
use crate::doom::p_switch::{buttonlist, MAXBUTTONS};
use crate::doom::p_tick::leveltime;
use crate::doom::r_data::{flattranslation, texturetranslation};
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::anims::{anims, lastanim};
use super::consts::MAXLINEANIMS;
use super::dtmc::anim_frame_pic;

/// Number of scrolling-wall linedefs registered in `linespeciallist`.
///
/// Counts lines with special 48 (first-column texture scroll).  Reset to zero
/// by `P_SpawnSpecials` at map start.  Exported with C linkage; referenced by
/// `P_UpdateSpecials` and p_spec.c.
#[no_mangle]
pub static mut numlinespecials: c_short = 0;

/// List of pointers to linedefs that carry the scrolling-wall special (48).
///
/// Populated by `P_SpawnSpecials`; iterated by `P_UpdateSpecials` each tic to
/// advance `textureoffset` by one `FRACUNIT`.  Capped at `MAXLINEANIMS` (64)
/// entries.  Exported with C linkage.
#[no_mangle]
pub static mut linespeciallist: [*mut line_t; MAXLINEANIMS] = [ptr::null_mut(); MAXLINEANIMS];

/// Non-zero when a deathmatch level timer is active.
///
/// Set to 1 by `P_SpawnSpecials` when `timelimit > 0` and `deathmatch != 0`.
/// Checked each tic by `P_UpdateSpecials`.  Stored as `c_int` boolean to
/// match the C declaration `boolean levelTimer`.  Exported with C linkage.
#[no_mangle]
pub static mut levelTimer: c_int = 0; // boolean

/// Remaining tics before the deathmatch time limit expires.
///
/// Initialised by `P_SpawnSpecials` to `timelimit * 60 * TICRATE`.
/// Decremented each tic by `P_UpdateSpecials`; reaching zero calls
/// `G_ExitLevel`.  Exported with C linkage.
#[no_mangle]
pub static mut levelTimeCount: c_int = 0;

/// Advances all time-based special effects by one tic.
///
/// Mirrors `P_UpdateSpecials` in `p_spec.c`; called once per tic from
/// p_tick's `P_Ticker` (whose `P_UpdateSpecials -> P_RespawnSpecials`
/// ordering is pinned by `p_tick/mod.rs`).
///
/// ## Technical Details
///
/// Four per-tic stages in fixed order, each demo-visible:
/// 1. the deathmatch level timer (`levelTimer`/`levelTimeCount`), exiting
///    the level when the count hits zero;
/// 2. texture/flat animation: every active `anims` entry rewrites
///    `texturetranslation`/`flattranslation` for each frame using the pure
///    phase formula [`super::dtmc::anim_frame_pic`] (the
///    `(leveltime / speed + i) % numpics` round-robin -- integer
///    division/modulo order is load-bearing for every animated surface in
///    demos);
/// 3. scrolling walls: one `FRACUNIT` of `textureoffset` per registered
///    special-48 line;
/// 4. timed buttons: decrement, restore the original switch texture
///    (top/mid/bottom by `where_`), play the switch sound, zero the slot.
///
/// ## On Calling
///
/// Requires `init_pic_anims` to have run (the `anim < lastanim` walk).
/// No RNG draws.
///
/// # Safety
///
/// Reads/writes the module statics and the global translation tables; must
/// run on the game tick thread (single-threaded tick assumption as upstream).
#[doc(alias = "P_UpdateSpecials")]
#[export_name = "P_UpdateSpecials"]
pub unsafe extern "C" fn update_specials()
{
    if levelTimer != 0
    {
        levelTimeCount -= 1;
        if levelTimeCount == 0 { G_ExitLevel(); }
    }

    let mut anim = std::ptr::addr_of_mut!(anims[0]);
    while anim < lastanim
    {
        let base = (*anim).basepic;
        let numpics = (*anim).numpics;
        for i in base..base + numpics
        {
            let pic = anim_frame_pic(base, numpics, (*anim).speed, leveltime, i);
            if(*anim).istexture != 0 { *texturetranslation.offset(i as isize) = pic; }
            else { *flattranslation.offset(i as isize) = pic; }
        }
        anim = anim.offset(1);
    }

    for i in 0..numlinespecials as usize
    {
        let line = linespeciallist[i];
        if (*line).special as c_int == 48
        {
            let sidenum = (*line).sidenum[0] as isize;
            (*sides.offset(sidenum)).textureoffset += FRACUNIT;
        }
    }

    for i in 0..MAXBUTTONS
    {
        if buttonlist[i].btimer != 0
        {
            buttonlist[i].btimer -= 1;
            if buttonlist[i].btimer == 0
            {
                let sidenum = (*buttonlist[i].line).sidenum[0] as isize;
                match buttonlist[i].where_
                {
                    0 => { (*sides.offset(sidenum)).toptexture = buttonlist[i].btexture as i16; }
                    1 => { (*sides.offset(sidenum)).midtexture = buttonlist[i].btexture as i16; }
                    2 => { (*sides.offset(sidenum)).bottomtexture = buttonlist[i].btexture as i16; }
                    _ => {}
                }
                S_StartSound(
                    &mut buttonlist[i].soundorg as *mut _ as *mut c_void,
                    Sfx::Swtchn as c_int,
                );
                buttonlist[i] = std::mem::zeroed();
            }
        }
    }
}

#[cfg(test)]
mod tests
{
    use std::sync::Mutex;

    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    use super::{lastanim, levelTimeCount, levelTimer, linespeciallist, numlinespecials, anims};

    static LOCK: Mutex<()> = Mutex::new(());

    /// Carried verbatim from the pre-split module (`p_spec.rs`
    /// `globals_are_zero_initialized`): every ticker/animation static
    /// starts zeroed/null before any level loads. The five
    /// shared-reference-to-mutable-static clippy findings are the
    /// pre-existing moved-verbatim test lints (base had the same five
    /// findings at `p_spec.rs:1528-1535`).
    #[test]
    fn globals_are_zero_initialized()
    {
        let _engine = ENGINE_STATICS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            assert_eq!(levelTimer, 0);
            assert_eq!(levelTimeCount, 0);
            assert_eq!(numlinespecials, 0);
            assert!(lastanim.is_null());
            for (i, &v) in linespeciallist.iter().enumerate() { assert!(v.is_null(), "linespeciallist[{i}] should be null"); }
            for (i, a) in anims.iter().enumerate()
            {
                assert_eq!(a.istexture, 0, "anims[{i}].istexture should be 0");
                assert_eq!(a.picnum, 0, "anims[{i}].picnum should be 0");
            }
        }
    }
}
