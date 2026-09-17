//! performance.now 时钟 (D2): shell 起点之后的毫秒数.
//!
//! 只进 DG_GetTicksMs (引擎节拍), 绝不进模拟状态 (D6 确定性护栏).

use std::cell::Cell;

thread_local! {
    static START_MS: Cell<f64> = const { Cell::new(0.0) };
}

/// 在 doomgeneric_Create 之前调用一次.
pub fn init_start_time() {
    START_MS.with(|t| t.set(performance_now()));
}

/// 引擎节拍时钟: 起点后毫秒数 (u32 截断 = 原生 Instant 行为一致).
pub fn now_ms() -> u32 {
    START_MS.with(|t| (performance_now() - t.get()) as u32)
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}
