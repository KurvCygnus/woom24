//! Runtime wiring of the self-referential `S_sfx` link entries.

use super::enums::Sfx;
use super::sfx_table::{S_sfx, SfxInfo};

/// Wire up the runtime cross-links in `S_sfx`.
///
/// Currently only `sfx_chgun -> sfx_pistol` (sharing the pistol sample). Must
/// be called before any `S_StartSound`. Called from `S_Init`. C origin:
/// `S_InitSfxLinks` in `sounds.c` (done there by the `SOUND_LINK` table
/// initializer; the Rust port needs runtime init because `link` is
/// self-referential).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `s_sound.rs` and `tables.rs` import the upstream name through the root
/// shim.
#[doc(alias = "S_InitSfxLinks")]
#[export_name = "S_InitSfxLinks"]
pub extern "C" fn init_sfx_links() {
    unsafe {
        S_sfx[Sfx::Chgun as usize].link = &mut S_sfx[Sfx::Pistol as usize] as *mut SfxInfo;
    }
}

#[cfg(test)]
mod tests {
    use crate::doom::sounds::{S_InitSfxLinks, S_sfx, Sfx, SfxInfo};

    /// sfx_chgun (entry 86) must link to sfx_pistol after S_InitSfxLinks.
    #[test]
    fn sfx_chgun_links_to_pistol_after_init() {
        unsafe {
            S_InitSfxLinks();
            let pistol_ptr = &S_sfx[Sfx::Pistol as usize] as *const SfxInfo;
            let chgun_link = S_sfx[Sfx::Chgun as usize].link as *const SfxInfo;
            assert_eq!(
                chgun_link, pistol_ptr,
                "sfx_chgun.link must point to sfx_pistol"
            );
        }
    }
}
