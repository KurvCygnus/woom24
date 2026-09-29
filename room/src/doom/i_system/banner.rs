//! The startup-banner printers, the stdout probe that gates the
//! fatal-error GUI popup, and the tactile-feedback no-op.

use std::ffi::{c_char, c_int};

use crate::doom::crt::c_printf;

/// Print a NUL-terminated banner string centred in a 70-character
/// column (35 leading spaces minus half the message length), followed
/// by a newline. Mirrors `I_PrintBanner` from `i_system.c`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// `msg` must point to a valid NUL-terminated C string.
#[doc(alias = "I_PrintBanner")]
#[export_name = "I_PrintBanner"]
pub extern "C" fn print_banner(msg: *mut c_char)
{
    unsafe
    {
        let len = libc::strlen(msg) as c_int;
        let spaces = 35 - len / 2;
        for _ in 0..spaces
        {
            libc::putchar(b' ' as c_int);
        }
        libc::puts(msg);
    }
}

/// Print a 75-character horizontal divider made of `=` followed by a
/// newline. Mirrors `I_PrintDivider` from `i_system.c`. Used by the
/// startup banner and a few menu screens.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_PrintDivider")]
#[export_name = "I_PrintDivider"]
pub extern "C" fn print_divider()
{
    unsafe
    {
        for _ in 0..75
        {
            libc::putchar(b'=' as c_int);
        }
        libc::putchar(b'\n' as c_int);
    }
}

/// Print the startup banner: a divider, the game description centred
/// between dividers, and the GPL copyright notice. Mirrors
/// `I_PrintStartupBanner` from `i_system.c`, but substitutes the
/// project name (the C source pulls it from `PACKAGE_NAME`).
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// `gamedescription` must point to a valid NUL-terminated C string.
#[doc(alias = "I_PrintStartupBanner")]
#[export_name = "I_PrintStartupBanner"]
pub extern "C" fn print_startup_banner(gamedescription: *mut c_char)
{
    unsafe
    {
        print_divider();
        print_banner(gamedescription);
        print_divider();
        c_printf(
            c" Room, like Doom Generic, is free software, covered by the GNU General Public\n License.  There is NO warranty; not even for MERCHANTABILITY or FITNESS\n FOR A PARTICULAR PURPOSE. You are welcome to change and distribute\n copies under certain conditions. See the source for more information.\n".as_ptr(),
        );
        print_divider();
    }
}

/// Return non-zero if stdout is a real interactive console.
///
/// The port always returns 0, matching the non-`ORIGCODE` branch in
/// `i_system.c`: without that flag the C source also returns 0. The
/// result is used by `fatal_error` to decide whether to pop up a GUI
/// dialog when no console is available to display the message.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `r_data/init.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_ConsoleStdout")]
#[export_name = "I_ConsoleStdout"]
pub extern "C" fn console_stdout() -> c_int { 0 }

/// Tactile feedback hook. Originally targeted the Logitech Cyberman in
/// the DOS Doom source; a no-op stub here, as in the C reference.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `p_inter/damage.rs` imports the upstream name through the root shim.
///
/// # Safety
///
/// Trivially safe.
#[doc(alias = "I_Tactile")]
#[export_name = "I_Tactile"]
pub extern "C" fn tactile(_on: c_int, _off: c_int, _total: c_int) {}
