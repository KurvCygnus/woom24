//! The ENDOOM no-op stub.

/// Display the text-mode ENDOOM screen.
///
/// In chocolate-doom this initialises the textgraphics subsystem, copies the
/// 80x25 character/attribute cells from `endoom_data` into the text screen
/// buffer and waits for a keypress. This port has no text-mode backend, so
/// the call is a no-op and `endoom_data` is ignored.
///
/// Called from `D_DoomMain` during shutdown when the user has not requested
/// `-noendoom`.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `d_main/boot.rs` imports the upstream name through the root shim.
#[doc(alias = "I_Endoom")]
#[export_name = "I_Endoom"]
pub extern "C" fn show_endoom_screen(_endoom_data: *mut u8) {}
