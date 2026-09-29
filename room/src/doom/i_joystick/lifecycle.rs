//! The no-op joystick lifecycle stubs.

/// Initialise the joystick subsystem.
///
/// No-op stub. The original opened the SDL joystick, validated the
/// configured axes and registered an at-exit hook to shut it down. This
/// port has no joystick backend.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_InitJoystick")]
#[export_name = "I_InitJoystick"]
pub extern "C" fn init_joystick() {}

/// Shut down the joystick subsystem. No-op stub.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_ShutdownJoystick")]
#[export_name = "I_ShutdownJoystick"]
pub extern "C" fn shutdown_joystick() {}

/// Sample the joystick state and post an `ev_joystick` event into the
/// input queue.
///
/// No-op stub. The original read button mask plus three axes and called
/// `D_PostEvent`. With no joystick backend the input queue simply never
/// sees joystick events.
///
/// Dead-but-exported (zero callers in the tree): kept for symbol-set
/// byte-identity, retires with the freeze zone. The pre-move export symbol
/// is kept with `#[export_name]` below.
#[doc(alias = "I_UpdateJoystick")]
#[export_name = "I_UpdateJoystick"]
pub extern "C" fn update_joystick() {}
