//! WebGL2 presenter (D3 default path): the whole frame as one texture +
//! fullscreen-triangle blit.
//!
//! The engine frame is BGRA byte order, which WebGL2 core does not take --
//! reuse present_c2d::bgra_to_rgba; one single byte-swap code path.

use wasm_bindgen::JsCast;

use crate::present_c2d::{bgra_to_rgba, Presenter};

//* GLSL ES requires the #version directive at byte 0 of the source: the raw
//* string must open directly onto the directive -- any leading newline puts it
//* on line 2 and every shader compile fails (which also poisons the canvas for
//* the Canvas2D fallback; see dg::attach_canvas).
const VS_SRC: &str =
    //language=WGSL
    r#"#version 300 es
    in vec2 a_pos;
    out vec2 v_uv;
    void main()
    {
        v_uv = vec2((a_pos.x + 1.0) * 0.5, 1.0 - (a_pos.y + 1.0) * 0.5);
        gl_Position = vec4(a_pos, 0.0, 1.0);
    }
    "#;

const FS_SRC: &str =
    //language=WGSL
    r#"#version 300 es
    precision mediump float;
    in vec2 v_uv;
    uniform sampler2D u_frame;
    out vec4 out_color;
    void main() { out_color = texture(u_frame, v_uv); }
    "#;

pub struct WebGl2Presenter
{
    gl: web_sys::WebGl2RenderingContext,
    canvas: web_sys::HtmlCanvasElement,
    texture: web_sys::WebGlTexture,
    rgba: Vec<u8>,
}

impl WebGl2Presenter
{
    /// The canvas is set to engine resolution (the boot present buffer,
    /// 640x400); failure returns Err for the caller to degrade.
    pub fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String>
    {
        let (w, h) = room::doom::doomgeneric::dg_res();
        canvas.set_width(w as u32);
        canvas.set_height(h as u32);
        let gl = canvas.
            get_context("webgl2").
            map_err(|e| format!("webgl2: {e:?}"))?.
            ok_or("webgl2 unavailable")?.
            dyn_into::<web_sys::WebGl2RenderingContext>().
            map_err(|_| "webgl2 cast failed")?;
        let program = compile_program(&gl)?;
        gl.use_program(Some(&program));
        // Fullscreen triangle: one big triangle covering clip space.
        let verts: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
        let vbo = gl.create_buffer().ok_or("create_buffer failed")?;
        gl.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
        unsafe
        {
            // SAFETY: static array, exact length.
            let slice = js_sys::Float32Array::view(&verts);
            gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &slice,
                web_sys::WebGl2RenderingContext::STATIC_DRAW,
            );
        }
        //? The plan printed get_attrib_location(...).ok_or(...)? as u32; the
        //? 0.3.98 shape returns i32 directly (-1 on failure,
        //? gen_WebGl2RenderingContext.rs:8553), semantics unchanged.
        let loc = gl.get_attrib_location(&program, "a_pos");
        if loc < 0 { return Err("a_pos location".into()); }
        let loc = loc as u32;
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(
            loc,
            2,
            web_sys::WebGl2RenderingContext::FLOAT,
            false,
            0,
            0,
        );
        let texture = gl.create_texture().ok_or("create_texture failed")?;
        gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        // Pixel alignment + nearest filtering (integer scaling is left to CSS,
        // keeping the pixel look).
        gl.tex_parameteri(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            web_sys::WebGl2RenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            web_sys::WebGl2RenderingContext::NEAREST as i32,
        );
        let tex_loc = gl.
            get_uniform_location(&program, "u_frame").
            ok_or("u_frame loc")?;
        gl.uniform1i(Some(&tex_loc), 0);
        let len = room::doom::doomgeneric::dg_pixels() * 4;
        Ok(
            Self
            {
                gl,
                canvas: canvas.clone(),
                texture,
                rgba: vec![0u8; len],
            }
        )
    }
}

fn compile_shader(
    gl: &web_sys::WebGl2RenderingContext,
    kind: u32,
    src: &str,
) -> Result<web_sys::WebGlShader, String>
{
    let sh = gl.create_shader(kind).ok_or("create_shader failed")?;
    gl.shader_source(&sh, src);
    gl.compile_shader(&sh);
    let ok = gl.
        get_shader_parameter(&sh, web_sys::WebGl2RenderingContext::COMPILE_STATUS).
        as_bool().
        unwrap_or(false);
    if ok { Ok(sh) } else
    {
        //? The plan interpolated get_shader_info_log(&sh) directly; 0.3.98
        //? returns Option<String> (gen_WebGl2RenderingContext.rs:8696),
        //? unwrap_or_default is equivalent.
        Err(
            format!(
                "shader compile: {}",
                gl.get_shader_info_log(&sh).unwrap_or_default()
            )
        )
    }
}

fn compile_program(gl: &web_sys::WebGl2RenderingContext) -> Result<web_sys::WebGlProgram, String>
{
    let vs = compile_shader(gl, web_sys::WebGl2RenderingContext::VERTEX_SHADER, VS_SRC)?;
    let fs = compile_shader(gl, web_sys::WebGl2RenderingContext::FRAGMENT_SHADER, FS_SRC)?;
    let p = gl.create_program().ok_or("create_program failed")?;
    gl.attach_shader(&p, &vs);
    gl.attach_shader(&p, &fs);
    gl.link_program(&p);
    let ok = gl.
        get_program_parameter(&p, web_sys::WebGl2RenderingContext::LINK_STATUS).
        as_bool().
        unwrap_or(false);
    if ok { Ok(p) } else
    {
        //? Same as get_shader_info_log: 0.3.98's get_program_info_log returns
        //? Option<String> (gen_WebGl2RenderingContext.rs:8650).
        Err(
            format!(
                "program link: {}",
                gl.get_program_info_log(&p).unwrap_or_default()
            )
        )
    }
}

impl Presenter for WebGl2Presenter
{
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String>
    {
        // Track the live present dims (a video_cfg reconfiguration changes
        // them mid-game); resize canvas and scratch buffer on change. The
        // texture below is fully re-specified per frame, so a dim change is
        // picked up by texImage2D itself.
        let (w, h) = room::doom::doomgeneric::dg_res();
        let need = w * h * 4;
        if self.rgba.len() != need
        {
            self.canvas.set_width(w as u32);
            self.canvas.set_height(h as u32);
            self.rgba.resize(need, 0);
        }

        bgra_to_rgba(bgra, &mut self.rgba);
        let gl = &self.gl;
        gl.bind_texture(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            Some(&self.texture),
        );
        //? The plan's tex_image_2d_with_u32_and_u32_and_html_image_element_or_canvas_or_video
        //? and its named alternative ..._u8_array_and_opt_u32 both do not exist
        //? in 0.3.98 (E0599); the equivalent shape is
        //? tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array
        //? (gen_WebGl2RenderingContext.rs:3196) -- arguments = the original list
        //? minus the element slot; wasm-bindgen wraps the byte buffer internally
        //? as a Uint8Array view, semantics unchanged, and the unsafe view is no
        //? longer needed.
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            0,
            web_sys::WebGl2RenderingContext::RGBA as i32,
            w as i32,
            h as i32,
            0,
            web_sys::WebGl2RenderingContext::RGBA,
            web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
            Some(&self.rgba),
        ).map_err(|e| format!("texImage2D: {e:?}"))?;

        gl.draw_arrays(web_sys::WebGl2RenderingContext::TRIANGLES, 0, 3);
        Ok(())
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn shader_sources_start_with_the_version_directive_at_byte_zero()
    {
        // GLSL ES: the #version directive must be the very first bytes of the
        // source; a leading newline puts it on line 2 and the shader refuses
        // to compile (Firefox field report: webgl2 init always failed).
        for (name, src) in [("VS_SRC", VS_SRC), ("FS_SRC", FS_SRC)]
        {
            assert!(
                src.starts_with("#version 300 es\n"),
                "{name} must start at byte 0 with the #version directive, got: {:?}",
                src.chars().take(24).collect::<String>()
            );
        }
    }
}
