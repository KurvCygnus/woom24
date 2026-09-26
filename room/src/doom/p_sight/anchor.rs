//! Link anchor for the sight module: a single C function referencing
//! the module's exported entry so the linker keeps its symbol alive.

use super::P_CheckSight;

/// Anchor function to ensure exports survive linker dead-code elimination.
#[no_mangle]
pub extern "C" fn P_Sight_Link_Anchor() {
    let _ = P_CheckSight as *const () as usize;
}
