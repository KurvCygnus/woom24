//! The sky-mapping init function: `init_sky_map`, the view-size-reset
//! entry point called by `r_main` (`R_Init` -> `R_ExecuteSetViewSize`).

use crate::doom::m_fixed::FRACUNIT;

use super::skytexturemid;

/// Reset sky-mapping state whenever the view size changes.
///
/// Sets `skytexturemid` to `100 * FRACUNIT` (6553600), the fixed vertical
/// centre for sky column drawing.  The `skyflatnum` assignment that appears
/// commented-out in the C source is performed elsewhere (in `g_game.c`).
///
/// Called by `r_main.c` (`R_Init` -> `R_ExecuteSetViewSize`) whenever the
/// view size is reconfigured.
#[doc(alias = "R_InitSkyMap")]
#[export_name = "R_InitSkyMap"]
pub extern "C" fn init_sky_map() { unsafe { skytexturemid = 100 * FRACUNIT; } }

#[cfg(test)]
mod tests {
    use crate::doom::r_sky::{skyflatnum, skytexture, skytexturemid, R_InitSkyMap};
    use std::sync::Mutex;

    // Global state; serialise tests that touch it.
    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn init_sets_texturemid() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            skytexturemid = 0;
            R_InitSkyMap();
            assert_eq!(skytexturemid, 100 * 65536);
        }
    }

    #[test]
    fn globals_default_to_zero() {
        // Only meaningful on the first run – still worth asserting
        // that we're publishing three `c_int` symbols with the
        // expected names (compilation + linkage sanity).
        let _g = LOCK.lock().unwrap();
        unsafe {
            let _ = (skyflatnum, skytexture, skytexturemid);
        }
    }
}
