//! The animated background: the init/update/draw trio driving the
//! episode-map overlay animations. The init and update halves are the
//! module's RNG-ledger surface -- their `M_Random` draws advance
//! `rndindex` (state-hash word 2) -- so the pure delay routes through
//! [`super::dtmc::anim_delay`] while the draws themselves stay whole
//! and in config order.

use super::dtmc;
use super::state::{bcnt, state, wbs};
use super::tables::{anim_config, anim_state_ptr, NUMANIMS};
use super::types::{animenum_t, stateenum_t};
use crate::doom::d_mode;
use crate::doom::doomstat::gamemode;
use crate::doom::m_random::M_Random;
use crate::doom::v_video::V_DrawPatch;

/// Reset all background animation states for the current episode at the start of an intermission.
///
/// No-op in commercial mode and for episode 3 (no animated background exists).
/// Schedules the first frame of each animation by computing `nexttic` from
/// `bcnt` plus a random offset.
///
/// # Safety
///
/// Dereferences the global `wbs` pointer and mutates the per-episode
/// animation-state tables via `anim_state_ptr`. Caller must ensure `wbs`
/// has been initialised (typically by `lifecycle::init_variables`) and that
/// no other code is concurrently reading the animation tables.
///
/// The `M_Random` call sites are the RNG-ledger surface: exactly one draw
/// per ANIM_ALWAYS/ANIM_RANDOM anim, first-to-last in config order -- the
/// delay routes through `dtmc::anim_delay` without touching the draw
/// order (baseline vectors commit feb6318, pre-move).
#[doc(alias = "WI_initAnimatedBack")]
pub(super) unsafe fn init_animated_back() {
    if gamemode == d_mode::commercial {
        return;
    }
    if (*wbs).epsd > 2 {
        return;
    }

    let epsd = (*wbs).epsd as usize;
    let count = NUMANIMS[epsd] as usize;

    for i in 0..count {
        let cfg = anim_config(epsd, i);
        let st = anim_state_ptr(epsd, i);
        (*st).ctr = -1;
        if cfg.type_ == animenum_t::ANIM_ALWAYS {
            (*st).nexttic = bcnt + 1 + dtmc::anim_delay(M_Random(), cfg.period);
        } else if cfg.type_ == animenum_t::ANIM_RANDOM {
            (*st).nexttic = bcnt + 1 + cfg.data2 + dtmc::anim_delay(M_Random(), cfg.data1);
        } else if cfg.type_ == animenum_t::ANIM_LEVEL {
            (*st).nexttic = bcnt + 1;
        }
    }
}

/// Advance all background animation states by one tic.
///
/// No-op in commercial mode and for episode 3. For each animation whose
/// `nexttic` matches the current `bcnt`, increments `ctr` and schedules the
/// next frame according to the animation mode.
///
/// # Safety
///
/// Dereferences the global `wbs` pointer and mutates the per-episode
/// animation-state tables via `anim_state_ptr`. Caller must ensure `wbs`
/// is valid and the animation state was initialised by `init_animated_back`.
///
/// The ANIM_RANDOM retrigger draw is the update-half RNG-ledger surface
/// (`bcnt + data2 + delay` -- NO `+ 1` lead-in, unlike the init site).
/// The ANIM_LEVEL "gawd-awful hack" (`state == StatCount && i == 7`)
/// moves verbatim.
#[doc(alias = "WI_updateAnimatedBack")]
pub(super) unsafe fn update_animated_back() {
    if gamemode == d_mode::commercial {
        return;
    }
    if (*wbs).epsd > 2 {
        return;
    }

    let epsd = (*wbs).epsd as usize;
    let count = NUMANIMS[epsd] as usize;

    for i in 0..count {
        let cfg = anim_config(epsd, i);
        let st = anim_state_ptr(epsd, i);
        if bcnt == (*st).nexttic {
            match cfg.type_ {
                animenum_t::ANIM_ALWAYS => {
                    (*st).ctr += 1;
                    if (*st).ctr >= cfg.nanims {
                        (*st).ctr = 0;
                    }
                    (*st).nexttic = bcnt + cfg.period;
                }
                animenum_t::ANIM_RANDOM => {
                    (*st).ctr += 1;
                    if (*st).ctr == cfg.nanims {
                        (*st).ctr = -1;
                        (*st).nexttic = bcnt + cfg.data2 + dtmc::anim_delay(M_Random(), cfg.data1);
                    } else {
                        (*st).nexttic = bcnt + cfg.period;
                    }
                }
                animenum_t::ANIM_LEVEL => {
                    // gawd-awful hack for level anims
                    if !(state == stateenum_t::StatCount && i == 7)
                        && (*wbs).next == cfg.data1
                    {
                        (*st).ctr += 1;
                        if (*st).ctr == cfg.nanims {
                            (*st).ctr -= 1;
                        }
                        (*st).nexttic = bcnt + cfg.period;
                    }
                }
            }
        }
    }
}

/// Draw the current frame of each active background animation over the already-slammed background.
///
/// Skips animations whose `ctr` is negative (not yet started).
/// No-op in commercial mode and for episode 3.
///
/// # Safety
///
/// Dereferences the global `wbs` pointer and reads the per-episode
/// animation-state tables via `anim_state_ptr` plus the patch arrays they
/// reference. Caller must ensure `wbs` is valid and animations have been
/// initialised by `init_animated_back`.
#[doc(alias = "WI_drawAnimatedBack")]
pub(super) unsafe fn draw_animated_back() {
    if gamemode == d_mode::commercial {
        return;
    }
    if (*wbs).epsd > 2 {
        return;
    }

    let epsd = (*wbs).epsd as usize;
    let count = NUMANIMS[epsd] as usize;

    for i in 0..count {
        let cfg = anim_config(epsd, i);
        let st = anim_state_ptr(epsd, i);
        if (*st).ctr >= 0 {
            V_DrawPatch(cfg.loc.x, cfg.loc.y, (*st).p[(*st).ctr as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_int;

    use super::super::state::{bcnt, wbs};
    use super::super::tables::{anim_state_ptr, EPSD0_NANIM, EPSD1_NANIM, EPSD2_NANIM};
    use super::super::types::wbstartstruct_t;
    use crate::doom::d_mode;
    use crate::doom::doomstat::gamemode;
    use crate::doom::m_random::{M_ClearRandom, RNDTABLE, prndindex, rndindex};
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// Live ledger vector against the REAL `init_animated_back` body:
    /// with the cursors cleared, one init pass consumes EXACTLY one
    /// `M_Random` draw per ANIM_ALWAYS anim (10 for episode 0, 0 for
    /// the all-ANIM_LEVEL episode 1, 6 for episode 2) and each slot's
    /// `nexttic` is the hand-computed `bcnt + 1 + (draw % period)` with
    /// the Nth anim consuming the Nth draw. Any extraction-era
    /// reordering that adds, drops, or moves a draw shifts `rndindex`
    /// (state-hash word 2) and breaks every frozen state digest.
    #[test]
    fn wi_init_animated_back_draw_ledger() {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            use super::init_animated_back;

            let saved_wbs = wbs;
            let saved_bcnt = bcnt;
            let saved_gamemode = gamemode;
            let saved_rnd = rndindex;
            let saved_prnd = prndindex;
            let save0: [super::super::types::anim_state_t; EPSD0_NANIM] =
                std::ptr::read(anim_state_ptr(0, 0) as *const [_; EPSD0_NANIM]);
            let save2: [super::super::types::anim_state_t; EPSD2_NANIM] =
                std::ptr::read(anim_state_ptr(2, 0) as *const [_; EPSD2_NANIM]);

            let mut start: wbstartstruct_t = std::mem::zeroed();
            wbs = &mut start;
            gamemode = d_mode::doom; // non-commercial: the animated back runs

            // Episode 0: ten ANIM_ALWAYS anims, period TICRATE/3 = 11,
            // bcnt 0. Draws 1..=10 in order (M_Random pre-increments).
            M_ClearRandom();
            bcnt = 0;
            start.epsd = 0;
            init_animated_back();
            assert_eq!(rndindex, EPSD0_NANIM as c_int, "episode 0 draw count");
            let ep0_nexttic = [9, 11, 1, 3, 11, 7, 9, 10, 7, 2];
            for i in 0..EPSD0_NANIM {
                let st = anim_state_ptr(0, i);
                assert_eq!((*st).ctr, -1);
                assert_eq!((*st).nexttic, ep0_nexttic[i], "ep0 anim {i}");
                assert_eq!(
                    (*st).nexttic,
                    0 + 1 + super::super::dtmc::anim_delay(
                        RNDTABLE[i + 1] as c_int,
                        crate::doom::i_timer::TICRATE / 3
                    ),
                    "ep0 anim {i} composition"
                );
            }

            // Episode 1: nine ANIM_LEVEL anims, ZERO draws, each slot
            // lands exactly on bcnt + 1.
            bcnt = 40;
            start.epsd = 1;
            init_animated_back();
            assert_eq!(rndindex, EPSD0_NANIM as c_int, "episode 1 draws nothing");
            for i in 0..EPSD1_NANIM {
                let st = anim_state_ptr(1, i);
                assert_eq!((*st).ctr, -1);
                assert_eq!((*st).nexttic, 41, "ep1 anim {i}");
            }

            // Episode 2: six ANIM_ALWAYS anims, periods 11 x5 + 8,
            // bcnt 77. Draws 11..=16 in order.
            bcnt = 77;
            start.epsd = 2;
            init_animated_back();
            assert_eq!(rndindex, (EPSD0_NANIM + EPSD2_NANIM) as c_int, "episode 2 draw count");
            let ep2_nexttic = [86, 83, 78, 86, 88, 81];
            let ep2_period = [
                crate::doom::i_timer::TICRATE / 3,
                crate::doom::i_timer::TICRATE / 3,
                crate::doom::i_timer::TICRATE / 3,
                crate::doom::i_timer::TICRATE / 3,
                crate::doom::i_timer::TICRATE / 3,
                crate::doom::i_timer::TICRATE / 4,
            ];
            for i in 0..EPSD2_NANIM {
                let st = anim_state_ptr(2, i);
                assert_eq!((*st).ctr, -1);
                assert_eq!((*st).nexttic, ep2_nexttic[i], "ep2 anim {i}");
                assert_eq!(
                    (*st).nexttic,
                    77 + 1 + super::super::dtmc::anim_delay(
                        RNDTABLE[EPSD0_NANIM + 1 + i] as c_int,
                        ep2_period[i]
                    ),
                    "ep2 anim {i} composition"
                );
            }

            // Commercial mode draws nothing at all.
            gamemode = d_mode::commercial;
            init_animated_back();
            assert_eq!(rndindex, (EPSD0_NANIM + EPSD2_NANIM) as c_int, "commercial draws nothing");

            wbs = saved_wbs;
            bcnt = saved_bcnt;
            gamemode = saved_gamemode;
            rndindex = saved_rnd;
            prndindex = saved_prnd;
            std::ptr::write(anim_state_ptr(0, 0) as *mut [_; EPSD0_NANIM], save0);
            std::ptr::write(anim_state_ptr(2, 0) as *mut [_; EPSD2_NANIM], save2);
        }
    }
}
