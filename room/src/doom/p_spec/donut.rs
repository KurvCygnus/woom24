//! The donut effect and its vanilla NULL-backsector overrun emulation
//! (catalog entry 3): `do_donut` with the private `donut_overrun`
//! default-value hook -- bit-exact with the `EV_DoDonut` /
//! `DonutOverrun` pair of `vendor/doomgeneric/p_spec.c`.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_int, c_void};
use std::ptr;

use crate::doom::c_ffi::{line_t, sector_t, FLOORSPEED};
use crate::doom::m_argv::{myargv, M_CheckParmWithArgs};
use crate::doom::m_misc::M_StrToInt;
use crate::doom::p_floor::{floormove_t, T_MoveFloor};
use crate::doom::p_setup::sectors;
use crate::doom::p_tick::P_AddThinker;
use crate::doom::r_data::numflats;
use crate::doom::violations::{self, VanillaViolation};
use crate::doom::z_zone::{Z_Malloc, PU_LEVSPEC};

use super::consts::{donutRaise, lowerFloor};
use super::geometry::{get_next_sector, sector_from_line_tag};

/// Fills `*s3_floorheight` and `*s3_floorpic` with the values that Vanilla Doom
/// would have read from address `0000:0000` when the donut s3 sector is null.
///
/// Mirrors the C-static-local shape of `DonutOverrun` in `p_spec.c`
/// (catalog entry 3; see `docs/vanilla-workarounds.md`). Private before and
/// after the graduation -- never a C symbol, doc alias only.
///
/// This replicates a real memory-access bug: in Vanilla Doom on DOS, reading
/// `s3->floorheight` with a null `s3` reads whatever happened to be at the
/// start of the DOS data segment.  The Chocolate Doom approach (which this port
/// follows) is to use configurable default values (`0` and `0x16`) that match
/// the Windows 98 memory layout, overridable with `-donut <height> <pic>`.
///
/// The function initialises its state only on the first call (guarded by the
/// `first` static flag), parsing `-donut` command-line arguments at that point.
/// Subsequent calls return the same cached values.
///
/// - `s3_floorheight`: output - the substitute floor height (fixed-point 16.16).
/// - `s3_floorpic`: output - the substitute floor picture lump index.
/// - `_line`: the triggering linedef (used only for diagnostic output in C;
///   unused in this Rust port).
/// - `_pillar_sector`: the inner donut sector (used only for diagnostics in C;
///   unused in this Rust port).
///
/// # Safety
///
/// `s3_floorheight` and `s3_floorpic` must be valid, non-null, writable
/// pointers. They are written unconditionally on every call.
#[doc(alias = "DonutOverrun")]
unsafe fn donut_overrun(
    s3_floorheight: *mut c_int,
    s3_floorpic: *mut i16,
    _line: *mut line_t,
    _pillar_sector: *mut sector_t,
)
{
    static mut first: c_int = 1;
    static mut tmp_s3_floorheight: c_int = 0;
    static mut tmp_s3_floorpic: c_int = 0;

    if first != 0
    {
        first = 0;
        tmp_s3_floorheight = 0;
        tmp_s3_floorpic = 0x16;

        let p = M_CheckParmWithArgs(c"-donut".as_ptr(), 2);
        if p > 0
        {
            M_StrToInt(
                *myargv.offset((p + 1) as isize),
                &raw mut tmp_s3_floorheight,
            );
            M_StrToInt(*myargv.offset((p + 2) as isize), &raw mut tmp_s3_floorpic);
            if tmp_s3_floorpic >= numflats
            {
                eprintln!(
                    "DonutOverrun: The second parameter for \"-donut\" switch should be greater than 0 and less than number of flats ({}). Using default value ({}) instead. ",
                    numflats as c_int, 0x16
                );
                tmp_s3_floorpic = 0x16;
            }
        }
    }

    *s3_floorheight = tmp_s3_floorheight;
    *s3_floorpic = tmp_s3_floorpic as i16;
}

/// Executes the "donut" special effect for all sectors tagged to `line`.
///
/// Mirrors `EV_DoDonut` in `p_spec.c`; called from p_switch's use-line
/// traversal.
///
/// ## Technical Details
///
/// The donut effect involves three concentric sectors:
/// - **s1** (inner, pillar): the sector directly tagged by the line.  Its floor
///   lowers to match s3's floor height (`lowerFloor` thinker).
/// - **s2** (ring): the sector adjacent to s1 via s1's first line.  Its floor
///   rises to s3's height and adopts s3's floor texture (`donutRaise` thinker).
/// - **s3** (outer): the sector adjacent to s2 that is not s1; provides the
///   target height and texture.
///
/// The thinker wiring order (rising-slime thinker first, then the lowering
/// donut-hole thinker, `specialdata` set before the fields) is the demo
/// surface. Edge cases (matching Chocolate Doom behaviour):
/// - If s1 already has a `specialdata` thinker running, the sector is skipped.
/// - If s2 is null (s1's first line is one-sided), a warning is printed and the
///   loop breaks early without spawning thinkers.
/// - If s3 is null (s2's bounding line has no back sector), the private
///   `donut_overrun` hook is
///   called to supply substitute height and texture values, emulating the
///   vanilla memory overrun (catalog entry 3; the census record + warning
///   fire at that hook).
///
/// ## On Calling
///
/// Returns 1 if at least one sector was acted upon, 0 otherwise. Both
/// thinkers use `T_MoveFloor` and are allocated with `Z_Malloc(PU_LEVSPEC)`.
/// The transmute of `T_MoveFloor` is required because the thinker function
/// pointer is typed as `unsafe extern "C" fn(*mut c_void)` at the FFI boundary.
///
/// # Safety
///
/// `line` must be a valid, non-null linedef pointer into loaded map data.
#[doc(alias = "EV_DoDonut")]
#[export_name = "EV_DoDonut"]
pub unsafe extern "C" fn do_donut(line: *mut line_t) -> c_int
{
    let mut secnum = -1;
    let mut rtn = 0;

    while
    {
        secnum = sector_from_line_tag(line, secnum);
        secnum
    } >= 0
    {
        let s1 = sectors.offset(secnum as isize);
        if !(*s1).specialdata.is_null() { continue; }

        rtn = 1;
        let s2 = get_next_sector(*(*s1).lines.offset(0) as *mut line_t, s1);

        if s2.is_null()
        {
            eprintln!("EV_DoDonut: linedef had no second sidedef! Unexpected behavior may occur in Vanilla Doom. ");
            break;
        }

        for i in 0..(*s2).linecount
        {
            let s3 =
                (*((*(*s2).lines.offset(i as isize)) as *mut line_t)).backsector as *mut sector_t;

            if s3 == s1 { continue; }

            let (s3_floorheight, s3_floorpic) = if s3.is_null()
            {
                eprintln!("EV_DoDonut: WARNING: emulating buffer overrun due to NULL back sector. Unexpected behavior may occur in Vanilla Doom.");
                violations::record(VanillaViolation::DonutOverrun);
                let mut fh = 0;
                let mut fp = 0i16;
                donut_overrun(&mut fh, &mut fp, line, s1);
                (fh, fp)
            }
            else { ((*s3).floorheight, (*s3).floorpic) };

            // Spawn rising slime
            let floor = Z_Malloc(
                std::mem::size_of::<floormove_t>() as c_int,
                PU_LEVSPEC,
                ptr::null_mut(),
            ) as *mut floormove_t;
            P_AddThinker(&mut (*floor).thinker);
            (*s2).specialdata = floor as *mut c_void;
            (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
                unsafe extern "C" fn(*mut floormove_t),
                unsafe extern "C" fn(*mut c_void),
            >(T_MoveFloor));
            (*floor).r#type = donutRaise;
            (*floor).crush = 0;
            (*floor).direction = 1;
            (*floor).sector = s2 as *mut _;
            (*floor).speed = FLOORSPEED / 2;
            (*floor).texture = s3_floorpic;
            (*floor).newspecial = 0;
            (*floor).floordestheight = s3_floorheight;

            // Spawn lowering donut-hole
            let floor = Z_Malloc(
                std::mem::size_of::<floormove_t>() as c_int,
                PU_LEVSPEC,
                ptr::null_mut(),
            ) as *mut floormove_t;
            P_AddThinker(&mut (*floor).thinker);
            (*s1).specialdata = floor as *mut c_void;
            (*floor).thinker.function.acp1 = Some(core::mem::transmute::<
                unsafe extern "C" fn(*mut floormove_t),
                unsafe extern "C" fn(*mut c_void),
            >(T_MoveFloor));
            (*floor).r#type = lowerFloor;
            (*floor).crush = 0;
            (*floor).direction = -1;
            (*floor).sector = s1 as *mut _;
            (*floor).speed = FLOORSPEED / 2;
            (*floor).floordestheight = s3_floorheight;

            break;
        }
    }

    rtn
}
