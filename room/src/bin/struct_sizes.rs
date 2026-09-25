//! Diagnostic binary that prints the size and field offsets of the most
//! ABI-sensitive map-data structs.
//!
//! The Doom map format and the C engine both rely on `#[repr(C)]` layouts
//! matching exactly between the runtime types and the compiled-in tables (and
//! across the FFI boundary with any residual C code).  Running this binary
//! gives a quick visual diff against the expected layout when chasing
//! save-game corruption, render glitches, or BSP-traversal bugs introduced by
//! struct edits.
//!
//! Build and run with:
//!
//! ```sh
//! cargo run --release --bin struct_sizes
//! ```
//!
//! No options, no output parsing: it just prints to stdout.

/// No-op stand-ins for the six doomgeneric platform callbacks.
///
/// This diagnostic binary links the engine library without the winit/wgpu
/// platform layer (`shells/native`, the shell crate) that provides the
/// `DG_*` callbacks; the stubs satisfy those link-time references. The
/// engine is never run here.
#[no_mangle]
extern "C" fn DG_Init() {}
#[no_mangle]
extern "C" fn DG_DrawFrame() {}
#[no_mangle]
extern "C" fn DG_SetWindowTitle(_title: *const std::ffi::c_char) {}
#[no_mangle]
extern "C" fn DG_GetKey(_pressed: *mut i32, _doom_key: *mut u8) -> i32 { 0 }
#[no_mangle]
extern "C" fn DG_GetTicksMs() -> u32 { 0 }
#[no_mangle]
extern "C" fn DG_SleepMs(_ms: u32) {}

/// Program entry point: dump struct sizes and field offsets to stdout.
///
/// Each block of `println!` calls follows the same pattern: one heading line,
/// then one line per field, formatted to line up visually in a fixed-width
/// terminal.  Add new types here as they become layout-sensitive.
fn main()
{
    use room::doom::c_ffi::*;
    use std::mem::{offset_of, size_of};
    
    println!(
        r#"
        Rust struct sizes:
          vertex_t:     {}
          seg_t:        {}
          subsector_t:  {}
          sector_t:     {}
          line_t:       {}
          side_t:       {}
          node_t:       {}
        "#,
        size_of::<vertex_t>(),
        size_of::<seg_t>(),
        size_of::<subsector_t>(),
        size_of::<sector_t>(),
        size_of::<line_t>(),
        size_of::<side_t>(),
        size_of::<node_t>()
    );
    
    println!(
        r#"
        
        seg_t field offsets:
        
          v1:           {}
          v2:           {}
          angle:        {}
          offset:       {}
          linedef:      {}
          sidedef:      {}
          frontsector:  {}
          backsector:   {}
        "#,
        offset_of!(seg_t, v1),
        offset_of!(seg_t, v2),
        offset_of!(seg_t, angle),
        offset_of!(seg_t, offset),
        offset_of!(seg_t, linedef),
        offset_of!(seg_t, sidedef),
        offset_of!(seg_t, frontsector),
        offset_of!(seg_t, backsector)
    );
    
    println!(
        r#"
        
        subsector_t field offsets:
        
          sector:       {}
          numlines:     {}
          firstline:    {}
        "#,
        offset_of!(subsector_t, sector),
        offset_of!(subsector_t, numlines),
        offset_of!(subsector_t, firstline)
    );
    
    println!(
        r#"
        
        sector_t field offsets:
        
          floorheight:    {}
          ceilingheight:  {}
          floorpic:       {}
          ceilingpic:     {}
          lightlevel:     {}
          special:        {}
          tag:            {}
          soundtraversed: {}
          soundtarget:    {}
          blockbox:       {}
          soundorg:       {}
          validcount:     {}
          thinglist:      {}
          specialdata:    {}
          linecount:      {}
          lines:          {}
        "#,
        offset_of!(sector_t, floorheight),
        offset_of!(sector_t, ceilingheight),
        offset_of!(sector_t, floorpic),
        offset_of!(sector_t, ceilingpic),
        offset_of!(sector_t, lightlevel),
        offset_of!(sector_t, special),
        offset_of!(sector_t, tag),
        offset_of!(sector_t, soundtraversed),
        offset_of!(sector_t, soundtarget),
        offset_of!(sector_t, blockbox),
        offset_of!(sector_t, soundorg),
        offset_of!(sector_t, validcount),
        offset_of!(sector_t, thinglist),
        offset_of!(sector_t, specialdata),
        offset_of!(sector_t, linecount),
        offset_of!(sector_t, lines)
    );
    
    println!(
        r#"
        
        side_t field offsets:
        
          textureoffset: {}
          rowoffset:     {}
          toptexture:    {}
          bottomtexture: {}
          midtexture:    {}
          sector:        {}
        "#,
        offset_of!(side_t, textureoffset),
        offset_of!(side_t, rowoffset),
        offset_of!(side_t, toptexture),
        offset_of!(side_t, bottomtexture),
        offset_of!(side_t, midtexture),
        offset_of!(side_t, sector)
    );

    println!(
        r#"
        
        line_t field offsets:
        
          v1:           {}
          v2:           {}
          dx:           {}
          dy:           {}
          flags:        {}
          special:      {}
          tag:          {}
          sidenum:      {}
          bbox:         {}
          slopetype:    {}
          frontsector:  {}
          backsector:   {}
          validcount:   {}
          specialdata:  {}
        "#,
        offset_of!(line_t, v1),
        offset_of!(line_t, v2),
        offset_of!(line_t, dx),
        offset_of!(line_t, dy),
        offset_of!(line_t, flags),
        offset_of!(line_t, special),
        offset_of!(line_t, tag),
        offset_of!(line_t, sidenum),
        offset_of!(line_t, bbox),
        offset_of!(line_t, slopetype),
        offset_of!(line_t, frontsector),
        offset_of!(line_t, backsector),
        offset_of!(line_t, validcount),
        offset_of!(line_t, specialdata)
    );
}
