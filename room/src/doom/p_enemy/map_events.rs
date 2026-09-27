//! Map-event actions keyed to monster deaths and explosions: the
//! Commander Keen secret-level opener, the boss-death special-line
//! dispatcher (with its private pre/post-ultimate episode/map matrix
//! `check_boss_end`), and the barrel/rocket radius blast -- bit-exact
//! with `A_KeenDie` / `CheckBossEnd` / `A_BossDeath` / `A_Explode` of
//! `vendor/doomgeneric/p_enemy.c`.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::os::raw::c_int;
use std::os::raw::c_uint;

use crate::doom::c_ffi::{line_t, vertex_t};
use crate::doom::doomstat::{gamemode, gameversion};
use crate::doom::d_player::{players, MAXPLAYERS};
use crate::doom::g_game::{gameepisode, gamemap, playeringame, G_ExitLevel};
use crate::doom::info::{MT_BABY, MT_BRUISER, MT_CYBORG, MT_FATSO, MT_SPIDER};
use crate::doom::p_doors::EV_DoDoor;
use crate::doom::p_floor::EV_DoFloor;
use crate::doom::p_map::P_RadiusAttack;
use crate::doom::p_mobj::P_MobjThinker;
use crate::doom::p_telept::mobj_t;
use crate::doom::p_tick::{thinkercap, thinker_t};
use crate::types::Boolean;

use super::consts::{
    c_short, exe_ultimate, mobjtype_t, commercial, lowerFloorToLowest, raiseToTexture,
    vld_blazeOpen, vld_open,
};

/// Type alias used when casting a `line_t` pointer for `EV_DoDoor`/`EV_DoFloor` calls that
/// require the `p_lights` module's `line_t` definition; all `line_t` variants share an
/// identical `#[repr(C)]` layout so the cast is safe.
type PLineThing = crate::doom::p_lights::line_t;

/// Shared thinker-identity helper: the `acp1 == P_MobjThinker` function
/// pointer compare that upstream repeats verbatim at four scan sites
/// (`A_KeenDie`, `A_PainShootSkull`, `A_BossDeath`, `A_BrainAwake`),
/// single-sourced here at graduation. Pointer identity, not demo
/// arithmetic -- deliberately not `dtmc` surface (report 3.4 item 3).
/// Never a C symbol; `pub(super)` for the three consuming subfiles.
pub(super) unsafe fn is_mobj_thinker(thinker: *mut thinker_t) -> bool
{
    (*thinker).function.acp1
        == core::mem::transmute::<
            Option<unsafe extern "C" fn(*mut mobj_t) -> ()>,
            Option<unsafe extern "C" fn(*mut c_void) -> ()>,
        >(Some(
            P_MobjThinker as unsafe extern "C" fn(*mut mobj_t) -> (),
        ))
}

/// Action function for Commander Keen's death (Doom II map 32 special).
///
/// Calls `A_Fall` to make the corpse non-solid, then scans all thinkers; if any other
/// Keen of the same type is still alive the function returns early. When the last Keen
/// dies it synthesises a `line_t` with `tag = 666` and calls `EV_DoDoor` with
/// `vld_open` to open the tagged door, allowing exit from the secret level.
///
/// # Safety
///
/// `mo` must be a non-null, valid `mobj_t`. The thinker list (`thinkercap`) must be
/// consistent. Called from C via the state-machine action pointer.
#[doc(alias = "A_KeenDie")]
#[export_name = "A_KeenDie"]
pub unsafe extern "C" fn action_keen_die(mo: *mut mobj_t)
{
    let mut th: *mut thinker_t;

    let mut mo2: *mut mobj_t;

    let mut junk: line_t = line_t {
        v1: std::ptr::null_mut::<vertex_t>(),
        v2: std::ptr::null_mut::<vertex_t>(),
        dx: 0,
        dy: 0,
        flags: 0,
        special: 0,
        tag: 0,
        sidenum: [0; 2],
        bbox: [0; 4],
        slopetype: 0,
        frontsector: std::ptr::null_mut::<c_void>(),
        backsector: std::ptr::null_mut::<c_void>(),
        validcount: 0,
        specialdata: std::ptr::null_mut::<c_void>(),
    };
    super::death::action_fall(mo);
    th = thinkercap.next;
    while !std::ptr::eq(th, &raw const thinkercap)
    {
        if is_mobj_thinker(th)
        {
            mo2 = th as *mut mobj_t;
            if mo2 != mo
                && (*mo2).mobjtype as c_uint == (*mo).mobjtype as c_uint
                && (*mo2).health > 0 as c_int
            {
                return;
            }
        }
        th = (*th).next;
    }
    junk.tag = 666 as c_short;
    EV_DoDoor(&mut junk as *mut line_t as *mut PLineThing, vld_open);
}

/// Determines whether the death of a monster of type `motype` should trigger episode-end effects.
///
/// Pre-v1.9 (before `exe_ultimate`): only triggers on map 8; Barons on episodes 2+ are ignored.
/// `exe_ultimate` and later: episode-specific logic introduced with Ultimate Doom episode 4 support:
/// - Ep 1 map 8: Barons.
/// - Ep 2 map 8: Cyberdemon.
/// - Ep 3 map 8: Spider Mastermind.
/// - Ep 4 map 6: Cyberdemon; map 8: Spider Mastermind.
/// - All other episodes: map 8 unconditionally.
///
/// # Safety
///
/// Reads `gameversion`, `gameepisode`, and `gamemap` globals which must be initialised.
#[doc(alias = "CheckBossEnd")]
pub(super) unsafe extern "C" fn check_boss_end(motype: mobjtype_t) -> Boolean
{
    if (gameversion as c_uint) < exe_ultimate as c_int as c_uint
    {
        if gamemap != 8 as c_int
        {
            return Boolean::FALSE;
        }
        if motype == MT_BRUISER && gameepisode != 1 as c_int
        {
            return Boolean::FALSE;
        }
        Boolean::TRUE
    }
    else
    {
        match gameepisode
        {
            1 => Boolean::from(gamemap == 8 as c_int && motype == MT_BRUISER),
            2 => Boolean::from(gamemap == 8 as c_int && motype == MT_CYBORG),
            3 => Boolean::from(gamemap == 8 as c_int && motype == MT_SPIDER),
            4 => Boolean::from(
                gamemap == 6 as c_int && motype == MT_CYBORG
                    || gamemap == 8 as c_int && motype == MT_SPIDER,
            ),
            _ => Boolean::from(gamemap == 8 as c_int),
        }
    }
}

/// Triggers map-special effects when a boss monster dies (if all bosses of the same type are dead).
///
/// Dispatch logic:
/// - Doom II (`commercial`), map 7: Mancubus death lowers floor tag 666;
///   Arachnotron death raises floor tag 667.
/// - Doom I episodes: calls `CheckBossEnd`; on success triggers `EV_DoFloor`/`EV_DoDoor`
///   with a synthetic `line_t` (tag 666) or falls through to `G_ExitLevel`.
///
/// Returns early if any player is dead, or if another live boss of the same type exists.
///
/// # Safety
///
/// `mo` must be non-null with a valid `mobjtype`. The thinker list and game-state globals
/// must be consistent. Called from C via state-machine action pointer.
#[doc(alias = "A_BossDeath")]
#[export_name = "A_BossDeath"]
pub unsafe extern "C" fn action_boss_death(mo: *mut mobj_t)
{
    let mut th: *mut thinker_t;

    let mut mo2: *mut mobj_t;

    let mut junk: line_t = line_t {
        v1: std::ptr::null_mut::<vertex_t>(),
        v2: std::ptr::null_mut::<vertex_t>(),
        dx: 0,
        dy: 0,
        flags: 0,
        special: 0,
        tag: 0,
        sidenum: [0; 2],
        bbox: [0; 4],
        slopetype: 0,
        frontsector: std::ptr::null_mut::<c_void>(),
        backsector: std::ptr::null_mut::<c_void>(),
        validcount: 0,
        specialdata: std::ptr::null_mut::<c_void>(),
    };
    let _i: c_int = 0;
    if gamemode as c_uint == commercial as c_int as c_uint
    {
        if gamemap != 7 as c_int
        {
            return;
        }
        if (*mo).mobjtype as c_uint != MT_FATSO as c_int as c_uint
            && (*mo).mobjtype as c_uint != MT_BABY as c_int as c_uint
        {
            return;
        }
    }
    else if check_boss_end((*mo).mobjtype).is_false()
    {
        return;
    }
    let mut i: usize = 0;
    while i < MAXPLAYERS
    {
        if playeringame[i] != 0 && players[i].health > 0 as c_int
        {
            break;
        }
        i += 1;
    }
    if i == MAXPLAYERS
    {
        return;
    }
    th = thinkercap.next;
    while !std::ptr::eq(th, &raw const thinkercap)
    {
        if is_mobj_thinker(th)
        {
            mo2 = th as *mut mobj_t;
            if mo2 != mo
                && (*mo2).mobjtype as c_uint == (*mo).mobjtype as c_uint
                && (*mo2).health > 0 as c_int
            {
                return;
            }
        }
        th = (*th).next;
    }
    if gamemode as c_uint == commercial as c_int as c_uint
    {
        if gamemap == 7 as c_int
        {
            if (*mo).mobjtype as c_uint == MT_FATSO as c_int as c_uint
            {
                junk.tag = 666 as c_short;
                EV_DoFloor(
                    &mut junk as *mut line_t as *mut PLineThing,
                    lowerFloorToLowest,
                );
                return;
            }
            if (*mo).mobjtype as c_uint == MT_BABY as c_int as c_uint
            {
                junk.tag = 667 as c_short;
                EV_DoFloor(&mut junk as *mut line_t as *mut PLineThing, raiseToTexture);
                return;
            }
        }
    }
    else
    {
        match gameepisode
        {
            1 =>
            {
                junk.tag = 666 as c_short;
                EV_DoFloor(
                    &mut junk as *mut line_t as *mut PLineThing,
                    lowerFloorToLowest,
                );
                return;
            }
            4 =>
            {
                match gamemap
                {
                    6 =>
                    {
                        junk.tag = 666 as c_short;
                        EV_DoDoor(&mut junk as *mut line_t as *mut PLineThing, vld_blazeOpen);
                        return;
                    }
                    8 =>
                    {
                        junk.tag = 666 as c_short;
                        EV_DoFloor(
                            &mut junk as *mut line_t as *mut PLineThing,
                            lowerFloorToLowest,
                        );
                        return;
                    }
                    _ =>
                    {}
                }
            }
            _ =>
            {}
        }
    }
    G_ExitLevel();
}

/// Triggers a 128-unit radius blast centred on `thingy`, sourced from `thingy->target`.
///
/// Used by rockets and barrel explosions. Calls `P_RadiusAttack` which damages all
/// shootable things within range proportional to distance.
///
/// # Safety
///
/// `actor` (`thingy`) must be non-null; `thingy->target` may be null (passed directly
/// to `P_RadiusAttack`). Called from C.
#[doc(alias = "A_Explode")]
#[export_name = "A_Explode"]
pub unsafe extern "C" fn action_explode(thingy: *mut mobj_t)
{
    P_RadiusAttack(
        thingy as *mut super::consts::CffiMobj,
        (*thingy).target as *mut super::consts::CffiMobj,
        128 as c_int,
    );
}

#[cfg(test)]
mod tests
{
    use std::ffi::c_void;
    use std::os::raw::c_int;
    use std::sync::Mutex;

    use crate::doom::doomstat::gameversion;
    use crate::doom::g_game::{gameepisode, gamemap};
    use crate::doom::info::{MT_BABY, MT_BRUISER, MT_CYBORG, MT_FATSO, MT_SPIDER};
    use crate::doom::p_telept::mobj_t;
    use crate::doom::p_tick::thinker_t;

    // --- F10 wave B3c baseline vectors, retargeted post-move (F10 §2.3) ---

    /// The (version arm, episode, map, boss type) cross-product the matrix
    /// test sweeps: both `gameversion` arms of `check_boss_end`
    /// (pre-`exe_ultimate` 1.9 and `exe_ultimate`; the `exe_final2` default
    /// rides the same `>=` arm, pinned separately), episodes 0..=5 (0 =
    /// "no episode" floor, 5 = one past Ultimate's 4), maps 1/6/7/8/9
    /// (6/8 are the boss-map arms, 1/9 are non-boss controls), and the
    /// five boss types named by the matrix (Baron, Cyberdemon, Spider,
    /// Mancubus, Arachnotron).
    const MATRIX_VERSIONS: [c_int; 2] = [4, 6]; // exe_doom_1_9, exe_ultimate
    const MATRIX_EPISODES: [c_int; 6] = [0, 1, 2, 3, 4, 5];
    const MATRIX_MAPS: [c_int; 5] = [1, 6, 7, 8, 9];
    const MATRIX_TYPES: [c_int; 5] = [MT_BRUISER, MT_CYBORG, MT_SPIDER, MT_FATSO, MT_BABY];

    /// Baseline transcription of the `check_boss_end` episode/map matrix,
    /// verbatim from the pre-move private fn `CheckBossEnd`
    /// (`p_enemy.rs:1847-1868`, read from its doc comment, which restates
    /// the shipped branches). The matrix test asserts the fn against this
    /// table over the full cross-product.
    fn expected_boss_end(version: c_int, episode: c_int, map: c_int, motype: c_int) -> bool
    {
        if version < super::exe_ultimate
        {
            // Pre-ultimate: map 8 only; Barons ignored outside episode 1.
            if map != 8
            {
                return false;
            }
            if motype == MT_BRUISER && episode != 1
            {
                return false;
            }
            true
        }
        else
        {
            // Ultimate and later: per-episode boss assignment.
            match episode
            {
                1 => map == 8 && motype == MT_BRUISER,
                2 => map == 8 && motype == MT_CYBORG,
                3 => map == 8 && motype == MT_SPIDER,
                4 => map == 6 && motype == MT_CYBORG || map == 8 && motype == MT_SPIDER,
                _ => map == 8,
            }
        }
    }

    /// Baseline vectors (F10 §2.3, wave B3c): the full
    /// 2 x (episode x map x boss-type) boss-death matrix of the private
    /// `check_boss_end`, now retargeted onto `map_events::check_boss_end`
    /// (pre-move commit `7d60051` ran the same vectors against
    /// `CheckBossEnd` in place). The globals are saved and restored
    /// around the sweep; the fn reads no other state.
    #[test]
    fn baseline_check_boss_end_matrix()
    {
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe
        {
            let saved = (gameversion, gameepisode, gamemap);
            for &version in &MATRIX_VERSIONS
            {
                for &episode in &MATRIX_EPISODES
                {
                    for &map in &MATRIX_MAPS
                    {
                        for &motype in &MATRIX_TYPES
                        {
                            gameversion = version;
                            gameepisode = episode;
                            gamemap = map;
                            let got = super::check_boss_end(motype).is_truthy();
                            assert_eq!(
                                got,
                                expected_boss_end(version, episode, map, motype),
                                "version={version} episode={episode} map={map} type={motype}"
                            );
                        }
                    }
                }
            }
            // The >= pin: the shipped default (exe_final2) rides the same
            // ultimate arm, not a third matrix.
            gameversion = 8; // exe_final2
            gameepisode = 4;
            gamemap = 6;
            assert_eq!(
                super::check_boss_end(MT_CYBORG).is_truthy(),
                expected_boss_end(8, 4, 6, MT_CYBORG)
            );
            (gameversion, gameepisode, gamemap) = saved;
        }
    }

    /// Baseline vectors: a thinker whose `acp1` holds the
    /// `P_MobjThinker` function item compares equal (the live-mobj
    /// discriminator all four scan sites rely on); a null `acp1` and a
    /// foreign `acp1` (a non-mobj action) both miss. Retargeted onto the
    /// shared `map_events::is_mobj_thinker` helper (pre-move commit
    /// `7d60051` ran the same vectors against the in-file transcription).
    /// The sentinel-vs-live removal pattern itself is covered by the
    /// p_tick dtmc tests.
    #[test]
    fn baseline_is_mobj_thinker_discriminates()
    {
        let mut live = thinker_t {
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            function: crate::doom::p_tick::actionf_t { acp1: None },
        };
        // Foreign action: a real (non-mobj) single-argument action -- any
        // address other than P_MobjThinker's must miss.
        let foreign_ptr = unsafe {
            core::mem::transmute::<
                unsafe extern "C" fn(*mut mobj_t) -> (),
                unsafe extern "C" fn(*mut c_void),
            >(crate::doom::p_enemy::death::action_fall)
        };
        let mut foreign = thinker_t {
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            function: crate::doom::p_tick::actionf_t { acp1: Some(foreign_ptr) },
        };
        live.function.acp1 = Some(unsafe {
            core::mem::transmute::<
                unsafe extern "C" fn(*mut mobj_t) -> (),
                unsafe extern "C" fn(*mut c_void),
            >(crate::doom::p_mobj::P_MobjThinker)
        });

        assert!(unsafe { super::is_mobj_thinker(&mut live) });
        assert!(!unsafe { super::is_mobj_thinker(&mut foreign) });
        let mut empty = thinker_t {
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            function: crate::doom::p_tick::actionf_t { acp1: None },
        };
        assert!(!unsafe { super::is_mobj_thinker(&mut empty) });
    }
}
