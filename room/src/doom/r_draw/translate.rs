//! The player-color translation tables: the 3 × 256 byte ramp remapper
//! built at map load.

use std::ffi::{c_int, c_void};
use std::ptr;

use super::state::translationtables;

extern "C" {
    /// Allocates `size` bytes from the zone heap with the given tag; returns a pointer to the block.
    fn Z_Malloc(size: c_int, tag: c_int, user: *mut c_void) -> *mut c_void;
}

/// Allocates and initialises the three color-translation tables used for player colors.
///
/// Builds a 3 × 256 byte block (gray / brown / red) in the zone heap (tag `PU_STATIC`).
/// The green palette ramp (`0x70`-`0x7f`) is remapped to the gray ramp (`0x60`),
/// brown ramp (`0x40`), and red ramp (`0x20`). All other palette entries are identity-mapped.
///
/// Sets the `translationtables` global to the allocated block.
///
/// Exported as `#[no_mangle]` for C callers.
///
/// # Safety
///
/// Requires an initialized zone allocator (`Z_Init`); writes
/// [`translationtables`], which every `draw_translated_column` call reads.
#[doc(alias = "R_InitTranslationTables")]
#[export_name = "R_InitTranslationTables"]
pub extern "C" fn init_translation_tables() {
    unsafe {
        translationtables = Z_Malloc(256 * 3, 1, ptr::null_mut()) as *mut u8; // PU_STATIC = 1

        let tt = translationtables;
        for i in 0..256 {
            if (0x70..=0x7f).contains(&i) {
                // map green ramp to gray, brown, red
                *tt.add(i) = (0x60 + (i & 0xf)) as u8;
                *tt.add(i + 256) = (0x40 + (i & 0xf)) as u8;
                *tt.add(i + 512) = (0x20 + (i & 0xf)) as u8;
            } else {
                // Keep all other colors as is.
                *tt.add(i) = i as u8;
                *tt.add(i + 256) = i as u8;
                *tt.add(i + 512) = i as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::Mutex;

    use crate::doom::r_draw::{translationtables, R_InitTranslationTables};

    /// Serialises all tests that touch the shared mutable renderer globals.
    static LOCK: Mutex<()> = Mutex::new(());

    /// Verifies that `R_InitTranslationTables` maps the green ramp to gray/brown/red and
    /// leaves all other palette entries as identity mappings.
    #[test]
    fn init_translation_tables() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            // Initialize the zone allocator so Z_Malloc works.
            // It's fine to re-initialize in a unit test.
            crate::doom::z_zone::Z_Init();

            R_InitTranslationTables();

            let tt = translationtables;
            for i in 0..256usize {
                if i >= 0x70 && i <= 0x7f {
                    assert_eq!(
                        *tt.add(i),
                        (0x60 + (i & 0xf)) as u8,
                        "tt[{i}] gray mismatch"
                    );
                    assert_eq!(
                        *tt.add(i + 256),
                        (0x40 + (i & 0xf)) as u8,
                        "tt[{i}+256] brown mismatch"
                    );
                    assert_eq!(
                        *tt.add(i + 512),
                        (0x20 + (i & 0xf)) as u8,
                        "tt[{i}+512] red mismatch"
                    );
                } else {
                    assert_eq!(*tt.add(i), i as u8, "tt[{i}] identity mismatch");
                    assert_eq!(*tt.add(i + 256), i as u8, "tt[{i}+256] identity mismatch");
                    assert_eq!(*tt.add(i + 512), i as u8, "tt[{i}+512] identity mismatch");
                }
            }

            // Clean up
            crate::doom::z_zone::Z_Free(translationtables as *mut c_void);
            translationtables = ptr::null_mut();
        }
    }
}
