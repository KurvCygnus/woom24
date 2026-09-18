//! Canvas2D presenter (D3 baseline): ImageData + putImageData.

/// The engine framebuffer is BGRA byte order (same as native gpu.rs); Canvas
/// ImageData is RGBA, so bytes swap per pixel.
//* The engine never writes the alpha byte (DG_ScreenBuffer is zero-initialized
//* and cmap_to_fb only writes r/g/b), while putImageData composites alpha onto
//* the page -- passing 0 through would mean a fully transparent frame every
//* time. So alpha is always set to 0xFF; the function stays pure and does not
//* lean on the {alpha:false} context option.
pub fn bgra_to_rgba(src: &[u8], dst: &mut [u8]) {
    assert_eq!(src.len(), dst.len(), "缓冲区必须等长");
    //? The plan printed chunks_exact(4).zip(chunks_exact_mut(4)); the
    //? as_chunks suggested by clippy (chunks_exact_to_as_chunks) is equivalent
    //? and shorter, per-chunk semantics unchanged.
    for (s, d) in src
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dst.as_chunks_mut::<4>().0.iter_mut())
    {
        d[0] = s[2]; // R
        d[1] = s[1]; // G
        d[2] = s[0]; // B
        d[3] = 0xFF; // A: always 0 on the engine side (see the function header); passing it through would composite as a transparent frame
    }
}

use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;

/// Canvas2D presenter: a persistent reused RGBA buffer, wrapped into an
/// ImageData once per frame.
pub struct Canvas2dPresenter {
    ctx: web_sys::CanvasRenderingContext2d,
    rgba: Vec<u8>,
}

impl Canvas2dPresenter {
    /// The canvas is set to engine resolution (640×400); failure returns Err
    /// for the caller to degrade.
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

/// The unified Presenter shape (the Task 6 WebGL2 implementation shares the
/// name and signature).
pub trait Presenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String>;
}

/// Presentation backend kinds (D3: WebGL2 default, Canvas2D always the
/// fallback).
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PresenterKind {
    Canvas2D,
    WebGL2,
}

/// Runtime choice: WebGL2 when available, Canvas2D otherwise.
/// WebGPU is out of this spec (non-goal).
pub fn choose_presenter(has_webgl2: bool) -> PresenterKind {
    if has_webgl2 {
        PresenterKind::WebGL2
    } else {
        PresenterKind::Canvas2D
    }
}

#[cfg(test)]
mod presenter_kind_tests {
    use super::*;

    #[test]
    fn webgl2_preferred_when_available() {
        assert_eq!(choose_presenter(true), PresenterKind::WebGL2);
        assert_eq!(choose_presenter(false), PresenterKind::Canvas2D);
    }
}

impl Presenter for Canvas2dPresenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String> {
        bgra_to_rgba(bgra, &mut self.rgba);
        // ImageData is measured in (width, height) pixels; the byte buffer is
        // wrapped into a Clamped view.
        //? The plan printed new_with_u8_clamped_array_and_width_and_height, a
        //? name from another web-sys generation; the 0.3.98 equivalent is
        //? _and_sh (WebIDL sw/sh = width/height, gen_ImageData.rs:73), the
        //? argument shape is identical.
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
    fn swaps_blue_and_red_keeps_green_forces_opaque_alpha() {
        let src = [1u8, 2, 3, 4];
        let mut dst = [0u8; 4];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [3, 2, 1, 255]);
    }

    #[test]
    fn full_frame_conversion_swaps_rgb_alpha_forced_opaque() {
        // Source alpha 0 = the engine's true value (nobody writes byte 3
        // after DG_ScreenBuffer's zero init).
        let src = vec![10u8, 20, 30, 0, 40, 50, 60, 0];
        let mut dst = vec![0u8; 8];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [30, 20, 10, 255, 60, 50, 40, 255]);
    }
}
