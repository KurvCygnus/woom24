//! The audio-CD stub surface: the `cd_Error` flag and nine no-op entry
//! points, byte-identical in behavior to the doomgeneric C stubs.

#![allow(non_upper_case_globals)]

use std::ffi::c_int;

/// Last CD-ROM error code, mirroring the C `cd_Error` global. Set to a
/// non-zero value by the original SDL backend on failure; in the
/// doomgeneric stub it is only ever cleared to 0.
#[no_mangle]
pub static mut cd_Error: c_int = 0;

/// Initialise the CD-ROM music subsystem.
///
/// Stub that simply clears `cd_Error` and returns 0 (success). The original
/// implementation opened an `SDL_CD` handle and reported drive presence.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusInit")]
#[export_name = "I_CDMusInit"]
pub extern "C" fn cd_mus_init() -> c_int
{
    unsafe { cd_Error = 0; }
    0
}

/// Print deferred CD startup status messages.
///
/// No-op stub. The original printed the drive name and any startup error
/// captured in `I_CDMusInit`.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusPrintStartup")]
#[export_name = "I_CDMusPrintStartup"]
pub extern "C" fn cd_mus_print_startup() {}

/// Begin playback of the given audio track (1-indexed). Stub returns 0.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusPlay")]
#[export_name = "I_CDMusPlay"]
pub extern "C" fn cd_mus_play(_track: c_int) -> c_int { 0 }

/// Stop the currently playing track. Stub returns 0.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusStop")]
#[export_name = "I_CDMusStop"]
pub extern "C" fn cd_mus_stop() -> c_int { 0 }

/// Resume paused playback. Stub returns 0.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusResume")]
#[export_name = "I_CDMusResume"]
pub extern "C" fn cd_mus_resume() -> c_int { 0 }

/// Set the CD-ROM playback volume. Stub clears `cd_Error` and returns 0,
/// matching the C version which was never implemented even outside
/// `ORIGCODE` ("Not supported yet").
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusSetVolume")]
#[export_name = "I_CDMusSetVolume"]
pub extern "C" fn cd_mus_set_volume(_volume: c_int) -> c_int
{
    unsafe { cd_Error = 0; }
    0
}

/// Return the first audio track number (1-indexed) on the inserted CD.
/// Stub returns 0; the original scanned the SDL track table for the first
/// `SDL_AUDIO_TRACK` entry.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusFirstTrack")]
#[export_name = "I_CDMusFirstTrack"]
pub extern "C" fn cd_mus_first_track() -> c_int { 0 }

/// Return the index of the last track on the inserted CD. Stub returns 0.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusLastTrack")]
#[export_name = "I_CDMusLastTrack"]
pub extern "C" fn cd_mus_last_track() -> c_int { 0 }

/// Return the length of the given track, rounded up to the next second.
/// Stub returns 0; the original computed `(track->length + CD_FPS - 1) /
/// CD_FPS` from the SDL track table.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_CDMusTrackLength")]
#[export_name = "I_CDMusTrackLength"]
pub extern "C" fn cd_mus_track_length(_track_num: c_int) -> c_int { 0 }
