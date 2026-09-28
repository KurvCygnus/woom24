//! The event-queue transport into the responder chain:
//! `process_events` (upstream `D_ProcessEvents`), called by
//! `d_loop`/`d_net`'s tic loop through the pinned C symbol. The
//! deterministic part of input handling lives downstream in
//! `G_Responder` (already `g_game`'s surface); the queue's contents are
//! async host timing.

use crate::doom::d_event::D_PopEvent;
use crate::doom::g_game::G_Responder;
use crate::doom::m_menu::M_Responder;

use super::state::storedemo;

/// Send all buffered events down the responder chain.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_loop`/`d_net` import the upstream name through the root shim
/// (their `extern "C"` blocks and the `Some(D_ProcessEvents)` fn
/// pointer in `DOOM_LOOP_INTERFACE` link it by symbol).
#[doc(alias = "D_ProcessEvents")]
#[export_name = "D_ProcessEvents"]
pub extern "C" fn process_events() {
    unsafe {
        // IF STORE DEMO, DO NOT ACCEPT INPUT
        if storedemo != 0 { return; }

        while {
            let ev = D_PopEvent();
            if ev.is_null() { false } else {
                if M_Responder(ev).is_false() { G_Responder(ev); }
                true
            }
        } {}
    }
}
