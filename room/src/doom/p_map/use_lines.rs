//! The player's Use action: the `usething` static and the traversal that
//! activates the first special linedef within reach -- bit-exact with the
//! use-line half of `vendor/doomgeneric/p_map.c`.

use std::ffi::{c_int, c_uint, c_void};
use std::ptr;

use crate::doom::c_ffi::{intercept_t, mobj_t};
use crate::doom::m_fixed::FRACBITS;
use crate::doom::p_maputl::{openrange, P_LineOpening, P_PathTraverse, P_PointOnLineSide, PT_ADDLINES};
use crate::doom::p_switch::P_UseSpecialLine;
use crate::doom::s_sound::S_StartSound;
use crate::doom::sounds::Sfx;
use crate::doom::tables::{finecosine, finesine, ANGLETOFINESHIFT};

use super::consts::USERANGE;

/// The map object currently attempting a Use action.
///
/// Set by `use_lines` before calling `P_PathTraverse`; read by
/// `ptr_use_traverse` to determine the initiator's position and side.
#[no_mangle]
pub static mut usething: *mut mobj_t = ptr::null_mut();

/// Path-traversal callback for the player's Use action.
///
/// For non-special lines with a closed opening, plays the "oof" sound and
/// stops traversal. For special lines, determines which side the player is
/// on and calls `P_UseSpecialLine`, then stops (only one special per Use
/// press). Passable non-special lines allow traversal to continue.
///
/// Returns 1 to continue traversal, 0 to stop.
///
/// ## Technical Details
///
/// The Noway-sound-vs-activate decision reads `openrange` from the
/// opening quartet, and the side computation (`P_PointOnLineSide == 1`)
/// decides which `P_UseSpecialLine` side index fires -- both are
/// order-sensitive: the special activation must stop the traversal so
/// exactly one special answers a single Use press.
///
/// ## On Calling
///
/// C ABI callback passed to `P_PathTraverse` by `use_lines` (retained via
/// the `PTR_UseTraverse` export pin); never call it directly. `in_` must
/// be a valid, non-null pointer to an initialised `intercept_t` whose
/// `d.line` is valid; `usething` must be set.
///
/// # Safety
///
/// Raw `intercept_t` dereferences and the `p_lights::line_t` cast below;
/// see On Calling.
#[doc(alias = "PTR_UseTraverse")]
#[export_name = "PTR_UseTraverse"]
pub unsafe extern "C" fn ptr_use_traverse(in_: *mut intercept_t) -> c_uint
{
    let in_ = &*in_;
    if (*in_.d.line).special == 0
    {
        P_LineOpening(in_.d.line);
        if openrange <= 0
        {
            S_StartSound(usething as *mut c_void, Sfx::Noway as c_int);
            return 0;
        }
        return 1;
    }
    let mut side = 0;
    if P_PointOnLineSide((*usething).x, (*usething).y, in_.d.line) == 1 { side = 1; }
    P_UseSpecialLine(
        usething as *mut c_void,
        //? Crosses into the not-yet-graduated p_lights mirror types (the
        //? freeze-zone quirk carries verbatim; see the module mapping
        //? table's dependency note).
        in_.d.line as *mut crate::doom::p_lights::line_t,
        side,
    );
    0
}

/// Activate special linedefs in front of `player` along their view direction.
///
/// Traces a ray of length `USERANGE` (64 map units) from the player's position using
/// `ptr_use_traverse`. The first special line within reach and in the
/// correct orientation is activated.
///
/// ## Technical Details
///
/// The ray endpoint math reuses the `(USERANGE >> FRACBITS) *
/// finecosine/finesine` pattern of the attack entries; the exact reach
/// and the `PT_ADDLINES`-only flag (no thing intercepts) decide which
/// specials are reachable -- both are pinned values.
///
/// ## On Calling
///
/// `player` must be a valid, non-null pointer to a
/// `crate::doom::d_player::PlayerT` whose `mo` field points to a live
/// `mobj_t`; map data must be initialised. The `c_void` parameter shape
/// is the C ABI surface (see the export pin) -- callers pass the player
/// pointer opaquely. Single-threaded sim only.
///
/// # Safety
///
/// Raw `PlayerT` / `mobj_t` dereferences; see On Calling.
#[doc(alias = "P_UseLines")]
#[export_name = "P_UseLines"]
pub unsafe extern "C" fn use_lines(player: *mut c_void)
{
    let player = player as *mut crate::doom::d_player::PlayerT;
    let mo = (*player).mo as *mut mobj_t;
    usething = mo;
    let angle = ((*mo).angle >> ANGLETOFINESHIFT) as usize;
    let x1 = (*mo).x;
    let y1 = (*mo).y;
    let x2 = x1 + ((USERANGE >> FRACBITS) as c_int) * *finecosine.0.add(angle);
    let y2 = y1 + ((USERANGE >> FRACBITS) as c_int) * finesine[angle];
    P_PathTraverse(x1, y1, x2, y2, PT_ADDLINES, Some(ptr_use_traverse));
}
