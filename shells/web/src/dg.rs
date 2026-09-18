//! wasm implementations of the six DG_* callbacks (D2) -- same shape as
//! shells/native/src/platform.rs.
//!
//! | Callback | wasm behavior |
//! |---|---|
//! | DG_Init | no-op (canvas/presenter are installed beforehand by woom24_attach_canvas) |
//! | DG_DrawFrame | reads a BGRA frame from DG_ScreenBuffer → Presenter |
//! | DG_SleepMs | no-op (rAF drives the cadence; engine throttling uses its own tick clock) |
//! | DG_GetTicksMs | clock::now_ms() |
//! | DG_GetKey | pops the queue fed by JS via woom24_push_key |
//! | DG_SetWindowTitle | writes document.title |
//!
//! ## Global state (same contract as shells/native/src/platform.rs)
//!
//! Shared mutable state lives in [`thread_local!`] + [`RefCell`]: these
//! functions are called through the C ABI by doomgeneric, and the caller has
//! no notion of Rust ownership. This is safe because **all** calls originate
//! from the main thread (the JS event loop drives the DG_*/woom24_* exports);
//! on wasm32 the main thread is de facto single-threaded, so no concurrent
//! entry exists and no lock is needed.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::CStr;

use crate::clock;
use crate::present_c2d::{choose_presenter, Canvas2dPresenter, Presenter, PresenterKind};
use crate::present_gl2::WebGl2Presenter;

thread_local! {
    /// Installed presenter. The static type never changes: it is always a
    /// trait object; the backend (WebGL2 preferred, Canvas2D fallback) is
    /// picked at runtime in `attach_canvas`.
    static PRESENTER: RefCell<Option<Box<dyn Presenter>>> = const { RefCell::new(None) };
    /// Key queue pushed from JS; entries are (pressed, doom_key).
    static KEY_QUEUE: RefCell<VecDeque<(bool, u8)>> = const { RefCell::new(VecDeque::new()) };
}

/// Landing point of woom24_attach_canvas.
pub fn attach_canvas(canvas: &web_sys::HtmlCanvasElement) -> Result<(), String> {
    // D3: WebGL2 default, falling back to Canvas2D on failure (both always compiled in).
    let kind = choose_presenter(
        canvas
            .get_context("webgl2")
            .map(|c| c.is_some())
            .unwrap_or(false),
    );
    let presenter: Box<dyn Presenter> = match kind {
        PresenterKind::WebGL2 => WebGl2Presenter::new(canvas)
            .map(|p| Box::new(p) as Box<dyn Presenter>)
            .or_else(|e| {
                log::warn!("WebGL2 初始化失败, 回退 Canvas2D: {e}");
                Canvas2dPresenter::new(canvas).map(|p| Box::new(p) as Box<dyn Presenter>)
            })?,
        PresenterKind::Canvas2D => Box::new(Canvas2dPresenter::new(canvas)?),
    };
    PRESENTER.with_borrow_mut(|s| *s = Some(presenter));
    clock::init_start_time();
    Ok(())
}

/// Landing point of woom24_push_key.
pub fn push_key(pressed: bool, doom_key: u8) {
    KEY_QUEUE.with_borrow_mut(|q| q.push_back((pressed, doom_key)));
}

#[no_mangle]
pub extern "C" fn DG_Init() {
    log::debug!("DG_Init: 平台已由 attach_canvas 备好");
}

/// # Safety
/// DG_ScreenBuffer is allocated by doomgeneric_Create with capacity =
/// DOOMGENERIC_PIXELS * 4.
#[no_mangle]
pub unsafe extern "C" fn DG_DrawFrame() {
    let pixel_bytes = {
        let ptr = room::doom::doomgeneric::DG_ScreenBuffer as *const u8;
        if ptr.is_null() {
            log::warn!("DG_DrawFrame: DG_ScreenBuffer 为空，跳过本帧");
            return;
        }
        std::slice::from_raw_parts(ptr, room::doom::doomgeneric::DOOMGENERIC_PIXELS * 4)
    };
    PRESENTER.with_borrow_mut(|s| match s.as_mut() {
        Some(p) => {
            if let Err(e) = p.draw_frame(pixel_bytes) {
                log::warn!("DG_DrawFrame: {e}");
            }
        }
        None => log::warn!("DG_DrawFrame: 呈现器未就绪"),
    });
}

#[no_mangle]
pub extern "C" fn DG_SleepMs(_ms: u32) {
    // no-op: rAF drives the cadence (D2).
}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    clock::now_ms()
}

/// # Safety
/// pressed/doom_key must be valid writable pointers (guaranteed by
/// doomgeneric's calling convention).
#[no_mangle]
pub unsafe extern "C" fn DG_GetKey(pressed: *mut i32, doom_key: *mut u8) -> i32 {
    KEY_QUEUE.with_borrow_mut(|q| {
        if let Some((is_pressed, key)) = q.pop_front() {
            // SAFETY: the caller guarantees valid pointers.
            unsafe {
                *pressed = i32::from(is_pressed);
                *doom_key = key;
            }
            1
        } else {
            0
        }
    })
}

/// # Safety
/// title must be a NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn DG_SetWindowTitle(title: *const std::ffi::c_char) {
    if title.is_null() {
        return;
    }
    // SAFETY: the caller guarantees NUL termination.
    let s = unsafe { CStr::from_ptr(title) }
        .to_string_lossy()
        .into_owned();
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        doc.set_title(&s);
    }
}
