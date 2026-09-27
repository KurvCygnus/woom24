//! State-machine transitions for map objects: `set_mobj_state` (the
//! zero-tic chain walker and action dispatcher) and `explode_missile`
//! (the missile detonation sequence) -- bit-exact with the
//! state-transition half of `vendor/doomgeneric/p_mobj.c`.

use std::ffi::c_void;
use std::os::raw::c_int;

use crate::doom::info::{self, *};
use crate::doom::m_random::P_Random;
use crate::doom::p_telept::mobj_t;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;

use super::dtmc::tics_jitter_clamp;
use super::lifecycle::remove_mobj;

/// Transition `mobj` to state `state`, running zero-tic chain states
/// immediately.  Returns `1` if the mobj is still alive after the transition,
/// `0` if it removed itself (`S_NULL`).
///
/// Mirrors `P_SetMobjState` in `p_mobj.c`.
///
/// ## Technical Details
///
/// The zero-tic chain is the demo surface: a state with `tics == 0`
/// advances again inside the same call, so a whole initial state chain
/// runs in statement order before the caller resumes. The action
/// function (the transmuted `acp1` pointer from `info::states`) fires
/// mid-chain and may remove the mobj via the `S_NULL` arm -- which
/// order the action sees relative to the chain walk is pinned by demo
/// playback.
///
/// ## On Calling
///
/// Raw pointer in/out: `mobj` must be a valid, non-null pointer to an
/// initialised `mobj_t`; `state` must be a valid state-table index or
/// `S_NULL`. The returned `c_int` is `1` (alive) / `0` (removed
/// itself), not a boolean success flag. Single-threaded game tick
/// assumption as upstream.
///
/// # Safety
///
/// `mobj` must be a valid, non-null pointer to an initialised `mobj_t`.
/// `state` must be a valid state-table index or `S_NULL`.
#[doc(alias = "P_SetMobjState")]
#[export_name = "P_SetMobjState"]
pub unsafe extern "C" fn set_mobj_state(mobj: *mut mobj_t, state: c_int) -> c_int
{
    let mobj = &mut *mobj;
    let mut state = state;
    loop
    {
        if state == S_NULL
        {
            mobj.state = S_NULL as *mut State as *mut crate::doom::p_telept::state_t;
            remove_mobj(mobj as *mut mobj_t);
            return 0;
        }
        let st = &mut info::states[state as usize] as *mut State;
        mobj.state = st as *mut crate::doom::p_telept::state_t;
        mobj.tics = (*st).tics;
        mobj.sprite = (*st).sprite;
        mobj.frame = (*st).frame;
        if let Some(action) = (*st).action
        {
            let action: unsafe extern "C" fn(*mut c_void) = core::mem::transmute(action);
            action(mobj as *mut mobj_t as *mut c_void);
        }
        state = (*st).nextstate;
        if mobj.tics != 0
        {
            break;
        }
    }
    1
}

/// Detonate a missile: zero its momentum, transition to its death state,
/// randomise the initial tic count slightly, clear `MF_MISSILE`, and play the
/// death sound.
///
/// ## Technical Details
///
/// The statement order is the demo surface: the death-state transition
/// runs BEFORE the tic jitter draw (`P_Random() & 3` through
/// [`super::dtmc::tics_jitter_clamp`]), so a death-state action that
/// itself draws from `P_Random` consumes its byte first; `MF_MISSILE`
/// clears and the death sound plays after the transition. Draw count
/// and order are pinned by the F9 goldens.
///
/// ## On Calling
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` with
/// `MF_MISSILE` set (the movement paths call this when a missile
/// blocks or hits the sky). The mobj may end up removed inside the
/// death-state chain; callers must not touch `mo` afterwards without
/// the sentinel check.
///
/// # Safety
///
/// `mo` must be a valid, non-null pointer to an `mobj_t` with `MF_MISSILE` set.
#[doc(alias = "P_ExplodeMissile")]
#[export_name = "P_ExplodeMissile"]
pub unsafe extern "C" fn explode_missile(mo: *mut mobj_t)
{
    let mo = &mut *mo;
    let info = mo.info as *mut MobjInfo;
    mo.momx = 0;
    mo.momy = 0;
    mo.momz = 0;
    set_mobj_state(mo as *mut mobj_t, (*info).deathstate);
    mo.tics = tics_jitter_clamp(mo.tics, P_Random());
    mo.flags &= !MF_MISSILE;
    if (*info).deathsound != Sfx::None
    {
        S_StartSound(
            mo as *mut mobj_t as *mut c_void,
            (*info).deathsound as c_int,
        );
    }
}
