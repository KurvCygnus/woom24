//! 六个 DG_* 回调的 wasm 实现 (D2) -- 与 shells/native/src/platform.rs 同形.
//!
//! | 回调 | wasm 行为 |
//! |---|---|
//! | DG_Init | no-op (画布/呈现器由 woom24_attach_canvas 先行装好) |
//! | DG_DrawFrame | 从 DG_ScreenBuffer 读 BGRA 帧 → Presenter |
//! | DG_SleepMs | no-op (rAF 节奏主导; 引擎节流走自身 tick 时钟) |
//! | DG_GetTicksMs | clock::now_ms() |
//! | DG_GetKey | 弹出 JS 经 woom24_push_key 压入的队列 |
//! | DG_SetWindowTitle | 写 document.title |
//!
//! ## 全局状态 (与 shells/native/src/platform.rs 同一契约)
//!
//! 共享可变状态放在 [`thread_local!`] + [`RefCell`] 里: 这些函数经 C ABI 被
//! doomgeneric 调用, 调用方没有 Rust 所有权概念. 这样做是安全的, 因为**所有**
//! 调用都源自主线程 (JS 事件循环驱动的 DG_*/woom24_* 导出); wasm32 上主线程
//! 事实上单线程, 不存在并发进入, 故无需锁.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::CStr;

use crate::clock;
use crate::present_c2d::{choose_presenter, Canvas2dPresenter, Presenter, PresenterKind};
use crate::present_gl2::WebGl2Presenter;

thread_local! {
    /// 装好的呈现器 (Task 5 用 Canvas2D; Task 6 升级为运行时选择,
    /// 静态类型不变, 始终是 trait 对象).
    static PRESENTER: RefCell<Option<Box<dyn Presenter>>> = const { RefCell::new(None) };
    /// JS 压入的按键队列, 条目 = (pressed, doom_key).
    static KEY_QUEUE: RefCell<VecDeque<(bool, u8)>> = const { RefCell::new(VecDeque::new()) };
}

/// woom24_attach_canvas 的落点.
pub fn attach_canvas(canvas: &web_sys::HtmlCanvasElement) -> Result<(), String> {
    // D3: WebGL2 默认, 失败落回 Canvas2D (两者恒编译在内).
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

/// woom24_push_key 的落点.
pub fn push_key(pressed: bool, doom_key: u8) {
    KEY_QUEUE.with_borrow_mut(|q| q.push_back((pressed, doom_key)));
}

#[no_mangle]
pub extern "C" fn DG_Init() {
    log::debug!("DG_Init: 平台已由 attach_canvas 备好");
}

/// # Safety
/// DG_ScreenBuffer 由 doomgeneric_Create 分配, 容量 = DOOMGENERIC_PIXELS * 4.
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
    // no-op: rAF 驱动节奏 (D2).
}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    clock::now_ms()
}

/// # Safety
/// pressed/doom_key 必须是有效可写指针 (doomgeneric 的调用约定保证).
#[no_mangle]
pub unsafe extern "C" fn DG_GetKey(pressed: *mut i32, doom_key: *mut u8) -> i32 {
    KEY_QUEUE.with_borrow_mut(|q| {
        if let Some((is_pressed, key)) = q.pop_front() {
            // SAFETY: 调用方保证指针有效.
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
/// title 必须是 NUL 结尾 C 字符串.
#[no_mangle]
pub unsafe extern "C" fn DG_SetWindowTitle(title: *const std::ffi::c_char) {
    if title.is_null() {
        return;
    }
    // SAFETY: 调用方保证 NUL 结尾.
    let s = unsafe { CStr::from_ptr(title) }
        .to_string_lossy()
        .into_owned();
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        doc.set_title(&s);
    }
}
