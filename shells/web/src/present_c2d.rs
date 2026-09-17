//! Canvas2D 呈现器 (D3 基线): ImageData + putImageData.

/// 引擎帧缓冲是 BGRA 字节序 (native gpu.rs 同源);
/// Canvas ImageData 是 RGBA 字节序, 逐像素换位.
pub fn bgra_to_rgba(src: &[u8], dst: &mut [u8]) {
    assert_eq!(src.len(), dst.len(), "缓冲区必须等长");
    //? 计划印的是 chunks_exact(4).zip(chunks_exact_mut(4)); clippy
    //? (chunks_exact_to_as_chunks) 建议的 as_chunks 同义且更短, 逐块语义不变.
    for (s, d) in src
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dst.as_chunks_mut::<4>().0.iter_mut())
    {
        d[0] = s[2]; // R
        d[1] = s[1]; // G
        d[2] = s[0]; // B
        d[3] = s[3]; // A
    }
}

use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;

/// Canvas2D 呈现器: 常驻复用的 RGBA 缓冲, 每帧包一次 ImageData.
pub struct Canvas2dPresenter {
    ctx: web_sys::CanvasRenderingContext2d,
    rgba: Vec<u8>,
}

impl Canvas2dPresenter {
    /// 画布被设为引擎分辨率 (640×400); 失败返回 Err 由上层降级.
    pub fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String> {
        canvas.set_width(room::doom::doomgeneric::DOOMGENERIC_RESX as u32);
        canvas.set_height(room::doom::doomgeneric::DOOMGENERIC_RESY as u32);
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("2d context: {e:?}"))?
            .ok_or("2d context unavailable")?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| "2d context cast failed")?;
        let len = room::doom::doomgeneric::DOOMGENERIC_PIXELS * 4;
        Ok(Self {
            ctx,
            rgba: vec![0u8; len],
        })
    }
}

/// Presenter 统一形状 (Task 6 的 WebGL2 实现同名同签名).
pub trait Presenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String>;
}

impl Presenter for Canvas2dPresenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String> {
        bgra_to_rgba(bgra, &mut self.rgba);
        // ImageData 按 (宽, 高) 像素计; 字节缓冲用 Clamped 包装成视图.
        //? 计划印的是 new_with_u8_clamped_array_and_width_and_height, 该名属于
        //? 其他 web-sys 世代; 0.3.98 的同义构造是 _and_sh (WebIDL 的 sw/sh =
        //? 宽/高, gen_ImageData.rs:73), 实参形状完全一致.
        let img = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&self.rgba),
            room::doom::doomgeneric::DOOMGENERIC_RESX as u32,
            room::doom::doomgeneric::DOOMGENERIC_RESY as u32,
        )
        .map_err(|e| format!("ImageData: {e:?}"))?;
        self.ctx
            .put_image_data(&img, 0.0, 0.0)
            .map_err(|e| format!("putImageData: {e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_blue_and_red_keeps_green_alpha() {
        let src = [1u8, 2, 3, 4];
        let mut dst = [0u8; 4];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [3, 2, 1, 4]);
    }

    #[test]
    fn full_frame_conversion_is_reversible_in_shape() {
        let src = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let mut dst = vec![0u8; 8];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [30, 20, 10, 255, 60, 50, 40, 255]);
    }
}
