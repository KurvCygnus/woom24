//! The `-file` command-line scanner.

#![allow(non_snake_case)]

use std::ffi::c_char;

use crate::doom::crt::c_printf1;

use crate::types::Boolean;

use crate::doom::d_iwad::D_TryFindWADByName;
use crate::doom::m_argv::{myargc, myargv, M_CheckParmWithArgs};
use crate::doom::w_wad::W_AddFile;

/// Parse `-file <wad>...` from the command line and append each WAD to the
/// lump directory.
///
/// Walks `myargv` starting after the `-file` parameter and feeds each
/// non-flag argument through `D_TryFindWADByName` and `W_AddFile`. Logs
/// `" adding <filename>\n"` via `printf` for each WAD loaded.
///
/// Returns `Boolean::TRUE` if at least one WAD was added (the original
/// "homebrew levels" / `modifiedgame` flag, used downstream to disable
/// network play and demo recording), otherwise `Boolean::FALSE`.
///
/// Called from `D_DoomMain` during startup. Unlike the C original this
/// port does not implement the `FEATURE_WAD_MERGE` parameters
/// (`-merge`, `-nwtmerge`, `-af`, `-as`, `-aa`) because doomgeneric
/// `#undef`s that feature.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "W_ParseCommandLine")]
#[export_name = "W_ParseCommandLine"]
pub extern "C" fn parse_command_line_wads() -> Boolean
{
    let mut modifiedgame: Boolean = Boolean::FALSE;

    unsafe {
        let p = M_CheckParmWithArgs(c"-file".as_ptr(), 1);
        if p != 0 {
            let mut idx = p + 1;
            modifiedgame = Boolean::TRUE;
            while idx < myargc && **myargv.offset(idx as isize) != b'-' as c_char {
                let filename = D_TryFindWADByName(*myargv.offset(idx as isize));
                c_printf1(c" adding %s\n".as_ptr(), filename);
                W_AddFile(filename);
                idx += 1;
            }
        }
    }

    modifiedgame
}
