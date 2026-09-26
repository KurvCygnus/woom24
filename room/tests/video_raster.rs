//! F1 M2 — raster invariants and reconfiguration smoke.
//!
//! Host-testable contract from the F1 spec (Testing item 3): for a set of
//! raster sizes (320x200, 640x400, 1366x768) the column/span/fuzz renderers
//! write only within the raster bounds after a live `VideoConfig::apply`
//! reconfiguration (the mid-game resolution-switch smoke item).
//!
//* The regression anchor that used to share this file now lives in its own
//* binary (`video_anchor.rs`): one engine per process is the only shape where
//* its golden is a function of the tree alone, and this test's process-global
//* renderer churn is exactly what the anchor must not inherit.

#![allow(non_snake_case, non_upper_case_globals)]

// This binary only drives `boot` from the shared harness; the drive/compare
// half belongs to the frame/pump-split binaries, so its items are unused here
// by design.
#[allow(dead_code)]
mod frame_split_common;

use frame_split_common::boot;

/// Canaries past the raster end: any write beyond `w*h` bytes shows up here.
const CANARY_BYTES: usize = 4096;
const CANARY_BYTE: u8 = 0x5A;

/// For each raster size: apply the config mid-game, re-execute the view-size
/// path, sweep the column/span/fuzz renderers across the full raster, and
/// assert no write left the raster bounds. One test function on purpose: the
/// reconfiguration state is process-global, so the sizes must run serially
/// and in a fixed order.
#[test]
fn raster_invariants_and_reconfiguration_smoke()
{
    boot();

    for (w, h) in [(320u32, 200u32), (640u32, 400u32), (1366u32, 768u32)]
    {
        apply_and_exercise(w, h);
    }

    // Back to the default as the last reconfiguration: the switch must be
    // repeatable in both directions (up and down).
    apply_and_exercise(320, 200);
}

fn apply_and_exercise(w_in: u32, h_in: u32)
{
    use room::doom::i_video::I_VideoBuffer;
    use room::doom::r_draw::{
        columnofs, dc_colormap, dc_source, dc_x, dc_yl, dc_yh, ds_colormap, ds_source, ds_x1,
        ds_x2, ds_xfrac, ds_xstep, ds_y, ds_yfrac, ds_ystep, viewheight, viewwidth, ylookup,
    };

    // -- apply the config mid-game (the resolution-switch smoke item) --------
    let cfg = room::doom::video_cfg::VideoConfig::new(
        w_in,
        h_in,
        room::doom::video_cfg::AspectMode::VanillaStretch,
    );
    room::doom::video_cfg::apply(cfg).expect("reconfiguration must succeed for a valid config");

    // Widths round up to a multiple of 4 (crispy i_video.c:1769-1772), so
    // e.g. 1366 applies as 1368; assert the live raster against the
    // validated config.
    let applied = cfg.validated().expect("already validated in apply");
    let (w, h) = (applied.width, applied.height);
    assert_eq!(room::doom::video_cfg::screen_width() as u32, w);
    assert_eq!(room::doom::video_cfg::screen_height() as u32, h);

    // Fix-round 1 (Critical 2): the automap's latched video state must be
    // re-seated after a reconfiguration — its framebuffer pointer must
    // follow the new primary buffer and the window extent must track the
    // new raster (minus the 32-row status-bar reservation). AM_Drawer runs
    // exactly this re-seat when the map is open (its change-detection
    // compares fb/finit_* per map frame); the test invokes the re-seat
    // directly because opening the automap here could run
    // AM_findMinMaxBoundaries over a not-yet-loaded level.
    unsafe {
        room::doom::am_map::AM_reseatVideoState();
    }
    assert_eq!(
        room::doom::am_map::am_framebuffer(),
        unsafe { I_VideoBuffer },
        "automap framebuffer must be re-seated on reconfiguration"
    );
    assert_eq!(
        room::doom::am_map::am_window_dims(),
        (w as i32, h as i32 - 32),
        "automap window must track the reconfigured raster"
    );

    // Present scale: the 320x200 default keeps today's 2x doubling to
    // 640x400; any raster >= 640x400 presents pass-through.
    let (dg_w, dg_h) = room::doom::doomgeneric::dg_res();
    let expected_fb: u32 = if (w, h) == (320, 200) { 2 } else { 1 };
    assert_eq!(
        (dg_w, dg_h),
        ((w * expected_fb) as usize, (h * expected_fb) as usize),
        "doomgeneric present dims must follow the applied config"
    );

    // -- swap in a sentinel-padded buffer so out-of-bounds writes show -------
    let raster_len = (w * h) as usize;
    let mut buf = vec![0x33u8; raster_len + CANARY_BYTES];
    for b in buf[raster_len..].iter_mut()
    {
        *b = CANARY_BYTE;
    }
    let orig_video = unsafe { I_VideoBuffer };
    unsafe
    {
        I_VideoBuffer = buf.as_mut_ptr();
    }

    // Full-screen view at the new raster; rebuilds ylookup/columnofs,
    // xtoviewangle, psprite scales, yslope/distscale, scalelight.
    unsafe
    {
        room::doom::r_main::R_SetViewSize(11, 0);
        room::doom::r_main::R_ExecuteSetViewSize();
    }

    assert_eq!(unsafe { viewwidth } as u32, w, "viewwidth must track the raster");
    assert_eq!(unsafe { viewheight } as u32, h, "viewheight must track the raster");

    // LUT sanity: a full-screen view has identity column offsets and a
    // raster-stride row table.
    for x in 0..w as usize
    {
        let ofs =
            unsafe { *(std::ptr::addr_of!(columnofs) as *const i32).add(x) } as usize;
        assert_eq!(ofs, x, "columnofs[{x}]");
    }
    for y in 0..h as usize
    {
        let row = unsafe { *(std::ptr::addr_of!(ylookup) as *const *mut u8).add(y) };
        assert_eq!(
            row,
            unsafe { I_VideoBuffer.add(y * w as usize) },
            "ylookup[{y}]"
        );
    }

    // Fake texture/colormap inputs; every sampled index stays masked to a
    // 128-byte (column) or 64x64 (span) tile.
    let mut texture = vec![0u8; 128];
    for (i, b) in texture.iter_mut().enumerate()
    {
        *b = i as u8;
    }
    let mut flat = vec![0u8; 64 * 64];
    for (i, b) in flat.iter_mut().enumerate()
    {
        *b = (i % 251) as u8;
    }
    let mut colormap = vec![0u8; 256];
    for (i, b) in colormap.iter_mut().enumerate()
    {
        *b = (i ^ 0x55) as u8;
    }
    let mut translation = vec![0u8; 256];
    for (i, b) in translation.iter_mut().enumerate()
    {
        *b = i as u8;
    }

    unsafe
    {
        dc_colormap = colormap.as_mut_ptr();
        dc_source = texture.as_mut_ptr();
        ds_colormap = colormap.as_mut_ptr();
        ds_source = flat.as_mut_ptr();
        room::doom::r_draw::dc_translation = translation.as_mut_ptr();

        // Sweep every column edge-to-edge at both raster extremes: the
        // column renderers must clamp/step within [0, w) x [0, h).
        dc_x = 0;
        dc_yl = 0;
        dc_yh = h as i32 - 1;
        room::doom::r_draw::R_DrawColumn();
        dc_x = w as i32 - 1;
        room::doom::r_draw::R_DrawColumn();

        // Translated column at the right edge.
        dc_x = w as i32 - 1;
        room::doom::r_draw::R_DrawTranslatedColumn();

        // Fuzz column along the top and bottom edges (clamping path).
        dc_x = w as i32 - 1;
        dc_yl = 0;
        dc_yh = h as i32 - 1;
        room::doom::r_draw::R_DrawFuzzColumn();

        // Span across a full row at the first and last raster row.
        ds_x1 = 0;
        ds_x2 = w as i32 - 1;
        ds_xfrac = 0;
        ds_yfrac = 0;
        ds_xstep = 0x10000;
        ds_ystep = 0;
        ds_y = 0;
        room::doom::r_draw::R_DrawSpan();
        ds_y = h as i32 - 1;
        room::doom::r_draw::R_DrawSpan();
    }

    // -- Low-detail family (review fix, Minor 5): the dc_x << 1 index-math
    // variants must stay in bounds too. In low detail the view is half as
    // wide and every draw doubles its column; all three raster sizes here
    // are even, so 2*(viewwidth-1)+1 == w-1 exactly.
    unsafe
    {
        room::doom::r_main::R_SetViewSize(11, 1);
        room::doom::r_main::R_ExecuteSetViewSize();
    }
    assert_eq!(
        unsafe { viewwidth } as u32,
        w / 2,
        "low-detail viewwidth must halve the raster"
    );

    unsafe
    {
        // Column variants at both column extremes (dc_x = viewwidth-1 writes
        // columns w-2 and w-1).
        dc_x = 0;
        dc_yl = 0;
        dc_yh = h as i32 - 1;
        room::doom::r_draw::R_DrawColumnLow();
        dc_x = w as i32 / 2 - 1;
        room::doom::r_draw::R_DrawColumnLow();

        // Translated + fuzz variants at the right edge (fuzz exercises the
        // viewheight-2 clamp path).
        dc_x = w as i32 / 2 - 1;
        room::doom::r_draw::R_DrawTranslatedColumnLow();
        dc_yl = 0;
        room::doom::r_draw::R_DrawFuzzColumnLow();

        // Span variant across a full logical row at the first and last row.
        ds_x1 = 0;
        ds_x2 = w as i32 / 2 - 1;
        ds_xfrac = 0;
        ds_yfrac = 0;
        ds_xstep = 0x10000;
        ds_ystep = 0;
        ds_y = 0;
        room::doom::r_draw::R_DrawSpanLow();
        ds_y = h as i32 - 1;
        room::doom::r_draw::R_DrawSpanLow();
    }

    // -- Windowed-view background exercise (review fix, Critical 1): drop to
    // a sub-screen view so R_FillBackScreen/R_DrawViewBorder own the
    // background buffer at THIS raster; the next loop iteration's up-switch
    // then re-fills it at a larger size, which is exactly the OOB trigger
    // under review.
    unsafe
    {
        room::doom::r_main::R_SetViewSize(9, 0);
        room::doom::r_main::R_ExecuteSetViewSize();
        room::doom::r_draw::R_FillBackScreen();
        room::doom::r_draw::R_DrawViewBorder();
    }
    assert_eq!(
        room::doom::r_draw::background_buffer_bytes(),
        (w as i32) * (h as i32 - 32),
        "background buffer must be re-sized for the live raster"
    );

    // The canary zone past the raster must be untouched.
    for (i, &b) in buf[raster_len..].iter().enumerate()
    {
        assert_eq!(
            b, CANARY_BYTE,
            "write past raster end at canary byte {i} for {w}x{h}"
        );
    }

    unsafe
    {
        I_VideoBuffer = orig_video;
    }

    // Restore the engine's own view size/detail so the churn above cannot
    // leak past this function.
    unsafe
    {
        room::doom::r_main::R_SetViewSize(
            room::doom::m_menu::screenblocks,
            room::doom::m_menu::detailLevel,
        );
        room::doom::r_main::R_ExecuteSetViewSize();
    }

    park_render_pointers();
}

//* The sweep Vecs above are function-local, but the global render state they
//* were loaded into (`dc_colormap`/`dc_source`/`dc_translation`/`ds_colormap`
//* /`ds_source`) outlives this function. Leaving those pointers dangling into
//* freed heap is UB, so park them on process-lifetime buffers before
//* returning.
fn park_render_pointers()
{
    use std::sync::OnceLock;

    /// Colormap, texture column, flat and translation tables, in park order.
    type ParkBuffers = (Box<[u8; 256]>, Box<[u8; 128]>, Box<[u8; 64 * 64]>, Box<[u8; 256]>);

    static PARK: OnceLock<ParkBuffers> = OnceLock::new();
    let (colormap, texture, flat, translation) = PARK.get_or_init(|| {
        (
            Box::new([0u8; 256]),
            Box::new([0u8; 128]),
            Box::new([0u8; 64 * 64]),
            Box::new([0u8; 256]),
        )
    });

    use room::doom::r_draw::{dc_colormap, dc_source, dc_translation, ds_colormap, ds_source};
    unsafe
    {
        dc_colormap = colormap.as_ptr() as *mut u8;
        dc_source = texture.as_ptr() as *mut u8;
        dc_translation = translation.as_ptr() as *mut u8;
        ds_colormap = colormap.as_ptr() as *mut u8;
        ds_source = flat.as_ptr() as *mut u8;
    }
}
