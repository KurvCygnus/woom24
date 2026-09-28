//! The stub definitions: the two constant-zero networking booleans
//! and the empty Timidity-config initializer, moved wholesale from
//! the pre-graduation `dummy.rs` (F10 wave A3).

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_uint;

/// Whether the client is connected to a network game session.
///
/// Corresponds to `boolean net_client_connected` in `dummy.c`.
/// Initialised to `false` (0).  The networking subsystem sets this to a
/// non-zero value when a connection is established; because the real network
/// module is not yet ported, this stub keeps it permanently `false`.
/// Referenced from C as `net_client_connected`.
/// 
/// `boolean` in doomtype.h is `typedef unsigned int boolean;` — use c_uint.
#[no_mangle]
pub static mut net_client_connected: c_uint = 0; // false

/// Whether this instance is running as a "drone" (observer) client.
///
/// Corresponds to `boolean drone` in `dummy.c`.  A drone connects to a
/// network game but does not control a player; it is used for recording or
/// observation purposes.  Initialised to `false` (0) and never set while the
/// networking subsystem is stubbed out.  Referenced from C as `drone`.
#[no_mangle]
pub static mut drone: c_uint = 0; // false

/// No-op stub for Timidity MIDI configuration initialisation.
///
/// Corresponds to the `#ifndef FEATURE_SOUND` block in `dummy.c`.  When the
/// sound system is disabled or not yet ported, this function is compiled in
/// place of the real implementation to satisfy link-time references from
/// `i_sound.c` and related modules.  The real implementation would locate and
/// load a Timidity configuration file for software MIDI synthesis.
///
/// # Note
/// The original C stub is also empty; no `// FIXME` is needed here.
#[no_mangle]
pub extern "C" fn I_InitTimidityConfig() {}

#[cfg(test)]
mod tests
{
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn globals_default_to_zero()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            assert_eq!(net_client_connected, 0);
            assert_eq!(drone, 0);
        }
    }

    #[test]
    fn globals_store_unsigned_values()
    {
        let _g = LOCK.lock().unwrap();
        unsafe
        {
            net_client_connected = c_uint::MAX;
            drone = c_uint::MAX;
            assert_eq!(net_client_connected, c_uint::MAX);
            assert_eq!(drone, c_uint::MAX);
            net_client_connected = 0;
            drone = 0;
        }
    }

    #[test]
    fn timidity_stub_does_not_panic() { I_InitTimidityConfig(); }
}
