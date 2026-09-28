//! The per-tic command packet (`ticcmd_t`, `d_ticcmd.h`): the inputs
//! sampled from one player for one game tic. The field ORDER is the
//! save format (`p_saveg/records.rs` serializes it field-exact) -- never
//! reorder or drop the padding.

use std::os::raw::c_int;

/// Per-tic command packet: the inputs sampled from one player for one game tic.
///
/// Corresponds to `ticcmd_t` in `d_ticcmd.h`.  One `TiccmdT` is built each
/// tic by `G_BuildTiccmd` and stored in `PlayerT::cmd`.  In a network game,
/// these packets are also transmitted to peers so every machine runs the same
/// simulation.
///
/// Field semantics (from `d_ticcmd.h`):
/// - `forwardmove`: signed forward/back movement, scaled by 2048 inside the sim.
/// - `sidemove`: signed strafe movement, scaled by 2048.
/// - `angleturn`: signed yaw delta, shifted left by 16 when applied.
/// - `chatchar`: character typed for chat, or 0.
/// - `buttons`: bitfield of `BT_*` action flags (fire, use, weapon change).
/// - `consistancy`: checksum byte used in network games to detect desync.
/// - `buttons2`: Strife-specific secondary button bitfield (`BT2_*`).
/// - `inventory`: Strife-specific inventory item index.
/// - `lookfly`: Heretic/Hexen look-up/down/center.
/// - `arti`: Heretic/Hexen artifact type to use.
///
/// The two padding bytes after `arti` are not present in the C struct but are
/// required here to reach the ABI size on x86-64 Linux.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TiccmdT
{
    /// Signed forward/backward movement unit; multiply by 2048 to get fixed-point speed.
    pub forwardmove: i8,
    /// Signed strafe movement unit; multiply by 2048 to get fixed-point speed.
    pub sidemove: i8,
    /// Signed yaw turn delta; shift left 16 to get a `angle_t` increment.
    pub angleturn: i16,
    /// Chat character typed this tic, or 0 if none.
    pub chatchar: u8,
    /// `BT_*` button bitfield for fire, use, weapon-change, and special actions.
    pub buttons: u8,
    /// Network consistency check byte; compared across peers to detect desyncs.
    pub consistancy: u8,
    /// Strife-specific `BT2_*` secondary button bitfield (look, jump, inventory).
    pub buttons2: u8,
    /// Strife-specific inventory item index to use this tic.
    pub inventory: c_int,
    /// Heretic/Hexen look-up/down/center command byte.
    pub lookfly: u8,
    /// Heretic/Hexen artifact (`artitype_t`) to activate this tic.
    pub arti: u8,
    /// Explicit padding to match C ABI struct size on x86-64 Linux.
    _pad: [u8; 2],
}
