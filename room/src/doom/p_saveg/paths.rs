//! Save-file path construction: the filename formatting helper, the two
//! lazily-allocated filename buffers, and the two C-entry path helpers.
//! The final filename is rebuilt from the current `slot` argument on every
//! call (matching C); only the allocation is reused.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int};
use std::ptr;

// `savegamedir` is p_saveg's extern-by-symbol declaration of d_main's
// static (mod.rs wiring block).
use super::savegamedir;

/// Formats a save-game filename into `buf` as `"{dir}{SAVEGAMENAME}{slot}.dsg\0"`.
///
/// Writes at most `buf.len() - 1` bytes plus a NUL terminator. Returns the
/// number of non-NUL bytes written. The caller must ensure `buf` is at least
/// `dir.len() + 32` bytes to avoid truncation.
fn fill_save_filename(buf: &mut [u8], dir: &str, slot: c_int) -> usize
{
    assert!(
        !buf.is_empty(),
        "fill_save_filename: buffer must have at least 1 byte"
    );
    let full = format!("{}{}{}.dsg", dir, SAVEGAMENAME, slot);
    let bytes = full.as_bytes();
    let len = std::cmp::min(bytes.len(), buf.len() - 1);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = 0;
    len
}

/// Lazily allocated buffer holding the path to the temporary save file.
/// Initialized once by `P_TempSaveGameFile`; never freed (process lifetime).
static mut TEMP_SAVE_FILENAME: *mut c_char = std::ptr::null_mut();

/// Lazily allocated buffer holding the path to the current slot's save file.
/// Allocated once by `P_SaveGameFile`, then reused for every slot.
static mut SAVE_FILENAME: *mut c_char = std::ptr::null_mut();

/// Base name prefix for save-game files; slot number and `.dsg` extension are
/// appended. Matches the `SAVEGAMENAME` define in the C source.
const SAVEGAMENAME: &str = "doomsav";

/// Returns the path to the temporary save-game file used during a save
/// operation.
///
/// The file is written first, then atomically renamed to the final slot
/// filename on success. The returned pointer is valid for the lifetime of the
/// process; it is allocated once and cached in `TEMP_SAVE_FILENAME`.
///
/// Called by `g_game.c` (`G_DoSaveGame`).
///
/// # Safety
///
/// `savegamedir` must be a valid, NUL-terminated C string for the lifetime of
/// this call. The returned pointer is valid until process exit; the caller must
/// not free or mutate it.
#[doc(alias = "P_TempSaveGameFile")]
#[export_name = "P_TempSaveGameFile"]
pub extern "C" fn temp_save_game_file() -> *mut c_char
{
    unsafe
    {
        if TEMP_SAVE_FILENAME.is_null()
        {
            let dir = std::ffi::CStr::from_ptr(savegamedir).to_string_lossy();
            let full = format!("{}temp.dsg\0", dir);
            let bytes = full.into_bytes();
            let ptr = std::alloc::alloc(std::alloc::Layout::from_size_align(bytes.len(), 1).unwrap())
                as *mut c_char;
            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
            TEMP_SAVE_FILENAME = ptr;
        }
        TEMP_SAVE_FILENAME
    }
}

/// Returns the path to the save-game file for the given `slot` (0-7).
///
/// The buffer is allocated once and reused; the filename is rebuilt on every
/// call so the same buffer can serve different slot numbers. The returned
/// pointer is valid until the next call or process exit.
///
/// Called by `g_game.c` to open the final save file for reading or renaming.
///
/// # Safety
///
/// `savegamedir` must be a valid, NUL-terminated C string for the lifetime of
/// this call. `slot` must be in the range `0..=7`. The returned pointer is
/// valid until the next call to this function; the caller must not free it.
#[doc(alias = "P_SaveGameFile")]
#[export_name = "P_SaveGameFile"]
pub extern "C" fn save_game_file(slot: c_int) -> *mut c_char
{
    unsafe
    {
        let dir_len = std::ffi::CStr::from_ptr(savegamedir).to_bytes().len();
        let alloc_size = dir_len + 32;

        if SAVE_FILENAME.is_null()
        {
            let layout = std::alloc::Layout::from_size_align(alloc_size, 1).unwrap();
            let ptr = std::alloc::alloc(layout);
            if ptr.is_null()
            {
                std::alloc::handle_alloc_error(layout);
            }
            SAVE_FILENAME = ptr as *mut c_char;
        }

        let dir_str = std::ffi::CStr::from_ptr(savegamedir).to_str().unwrap();
        let buf = std::slice::from_raw_parts_mut(SAVE_FILENAME as *mut u8, alloc_size);
        fill_save_filename(buf, dir_str, slot);

        SAVE_FILENAME
    }
}

#[cfg(test)]
mod tests
{
    // Test moved with its subject (F10 wave B4b); pure formatting check,
    // no shared state.

    use super::{fill_save_filename, SAVEGAMENAME};

    #[test]
    fn save_game_path_not_truncated_for_long_directory()
    {
        let long_dir = "/very/long/savegame/directory/path/"; // 35 chars — forces path > 31 chars
        let slot = 3i32;
        let alloc_size = long_dir.len() + 32;
        let mut buf = vec![0u8; alloc_size];
        fill_save_filename(&mut buf, long_dir, slot);
        let result = std::ffi::CStr::from_bytes_until_nul(&buf)
            .unwrap()
            .to_str()
            .unwrap();
        let expected = format!("{}{}{}.dsg", long_dir, SAVEGAMENAME, slot);
        assert_eq!(
            result, expected,
            "save path was truncated for long directory"
        );
    }
}
