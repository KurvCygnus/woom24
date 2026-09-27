//! The module link anchor.

use super::{
    P_ArchivePlayers, P_ArchiveSpecials, P_ArchiveThinkers, P_ArchiveWorld, P_ReadSaveGameEOF,
    P_ReadSaveGameHeader, P_SaveGameFile, P_TempSaveGameFile, P_UnArchivePlayers,
    P_UnArchiveSpecials, P_UnArchiveThinkers, P_UnArchiveWorld, P_WriteSaveGameEOF,
    P_WriteSaveGameHeader,
};

//* Dead-but-exported: zero callers exist (`doomgeneric.rs`'s anchor list has
//* no p_saveg entry). Kept so the symbol set stays byte-identical to the
//* pre-split module, retiring with the freeze zone (p_spec precedent).
/// Forces all public `extern "C"` functions in this module to be included in
/// the final binary by taking their addresses.
///
/// Without this anchor the linker may dead-strip functions that are only called
/// from C translation units, since Rust does not see those call sites.
#[doc(alias = "P_Saveg_Link_Anchor")]
#[export_name = "P_Saveg_Link_Anchor"]
pub extern "C" fn saveg_link_anchor()
{
    let _ = P_TempSaveGameFile as *const () as usize;
    let _ = P_SaveGameFile as *const () as usize;
    let _ = P_WriteSaveGameHeader as *const () as usize;
    let _ = P_ReadSaveGameHeader as *const () as usize;
    let _ = P_ReadSaveGameEOF as *const () as usize;
    let _ = P_WriteSaveGameEOF as *const () as usize;
    let _ = P_ArchivePlayers as *const () as usize;
    let _ = P_UnArchivePlayers as *const () as usize;
    let _ = P_ArchiveWorld as *const () as usize;
    let _ = P_UnArchiveWorld as *const () as usize;
    let _ = P_ArchiveThinkers as *const () as usize;
    let _ = P_UnArchiveThinkers as *const () as usize;
    let _ = P_ArchiveSpecials as *const () as usize;
    let _ = P_UnArchiveSpecials as *const () as usize;
}
