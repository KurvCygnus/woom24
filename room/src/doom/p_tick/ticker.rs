//! The per-tic game heartbeat: the `leveltime` clock static and its
//! only writer `P_Ticker`, which advances every player, the thinker
//! list, and the specials in the demo-pinned order, then increments
//! the clock, plus its test module.

#![allow(non_upper_case_globals, non_snake_case)]

use std::os::raw::c_int;

use super::thinkers::P_RunThinkers;
use crate::doom::d_player::{consoleplayer, players, MAXPLAYERS};
use crate::doom::g_game::{demoplayback, netgame, paused, playeringame};
use crate::doom::m_menu::menuactive;
use crate::doom::p_mobj::P_RespawnSpecials;
use crate::doom::p_spec::P_UpdateSpecials;
use crate::doom::p_user::P_PlayerThink;

/// Current level time in tics, incremented once per [`P_Ticker`] call.
///
/// Exported as `leveltime` for C linkage.  Used throughout the engine to
/// throttle periodic events (e.g. crushing-ceiling sound plays every 8 tics).
#[no_mangle]
pub static mut leveltime: c_int = 0;

/// Run one game tic: advance all players, thinkers, specials, and the level
/// timer.
///
/// Returns early if the game is paused, or if in single-player mode with the
/// menu open and the view already initialised (i.e. at least one tic has run).
/// The `viewz == 1` sentinel is the initial value set before any tic; once the
/// player has been processed once the check is false and menus pause the game.
///
/// Corresponds to `P_Ticker` in `p_tick.c`.
#[no_mangle]
pub extern "C" fn P_Ticker()
{
    unsafe
    {
        if paused != 0 { return; }

        if netgame == 0 && menuactive != 0 && demoplayback == 0 && (*std::ptr::addr_of!(players[0]).offset(consoleplayer as isize)).viewz != 1 { return; }

        for i in 0..MAXPLAYERS { if playeringame[i] != 0 { P_PlayerThink(&mut players[i]); } }

        P_RunThinkers();
        P_UpdateSpecials();
        P_RespawnSpecials();

        leveltime += 1;
    }
}

#[cfg(test)]
mod tests
{
    use crate::doom::p_tick::leveltime;
    use crate::doom::violations::ENGINE_STATICS_TEST_LOCK;

    /// `leveltime` starts at zero: the savegame archive path and the F9
    /// harness pins assume the default-initialized clock. Serialized on
    /// the shared engine-statics lock (this test exact-value-asserts a
    /// process-global static).
    #[test]
    fn globals_default()
    {
        let _g = ENGINE_STATICS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { assert_eq!(std::ptr::addr_of!(leveltime).read(), 0); }
    }
}
