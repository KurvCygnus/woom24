//! WebGL2 呈现器 (D3 默认路径): 整帧一张纹理 + 全屏三角 blit.
//!
//! 引擎帧是 BGRA 字节序, WebGL2 核心不吃 BGRA --
//! 复用 present_c2d::bgra_to_rgba, 单一换序代码路径.

use wasm_bindgen::JsCast;

use crate::present_c2d::{bgra_to_rgba, Presenter};

const VS_SRC: &str = r#"
#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = vec2((a_pos.x + 1.0) * 0.5, 1.0 - (a_pos.y + 1.0) * 0.5);
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

const FS_SRC: &str = r#"
#version 300 es
precision mediump float;
in vec2 v_uv;
uniform sampler2D u_frame;
out vec4 out_color;
void main() {
    out_color = texture(u_frame, v_uv);
}
"#;

pub struct WebGl2Presenter {
    gl: web_sys::WebGl2RenderingContext,
    texture: web_sys::WebGlTexture,
    rgba: Vec<u8>,
}

impl WebGl2Presenter {
    /// 画布被设为引擎分辨率 (640×400); 失败返回 Err 由上层降级.
    pub fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, String> {
        canvas.set_width(room::doom::doomgeneric::DOOMGENERIC_RESX as u32);
        canvas.set_height(room::doom::doomgeneric::DOOMGENERIC_RESY as u32);
        let gl = canvas
            .get_context("webgl2")
            .map_err(|e| format!("webgl2: {e:?}"))?
            .ok_or("webgl2 unavailable")?
            .dyn_into::<web_sys::WebGl2RenderingContext>()
            .map_err(|_| "webgl2 cast failed")?;
        let program = compile_program(&gl)?;
        gl.use_program(Some(&program));
        // 全屏三角: 一条大三角形覆盖 clip 空间.
        let verts: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
        let vbo = gl.create_buffer().ok_or("create_buffer failed")?;
        gl.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
        unsafe {
            // SAFETY: 静态数组, 长度精确.
            let slice = js_sys::Float32Array::view(&verts);
            gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &slice,
                web_sys::WebGl2RenderingContext::STATIC_DRAW,
            );
        }
        //? 计划印的是 get_attrib_location(...).ok_or(...)? as u32; 0.3.98 的同义形状
        //? 直接返回 i32 (失败为 -1, gen_WebGl2RenderingContext.rs:8553), 语义不变.
        let loc = gl.get_attrib_location(&program, "a_pos");
        if loc < 0 {
            return Err("a_pos location".into());
        }
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
        // 像素对齐 + 最近邻 (整数放大交给 CSS, 保持像素风).
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
        let tex_loc = gl
            .get_uniform_location(&program, "u_frame")
            .ok_or("u_frame loc")?;
        gl.uniform1i(Some(&tex_loc), 0);
        let len = room::doom::doomgeneric::DOOMGENERIC_PIXELS * 4;
        Ok(Self {
            gl,
            texture,
            rgba: vec![0u8; len],
        })
    }
}

fn compile_shader(
    gl: &web_sys::WebGl2RenderingContext,
    kind: u32,
    src: &str,
) -> Result<web_sys::WebGlShader, String> {
    let sh = gl.create_shader(kind).ok_or("create_shader failed")?;
    gl.shader_source(&sh, src);
    gl.compile_shader(&sh);
    let ok = gl
        .get_shader_parameter(&sh, web_sys::WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false);
    if ok {
        Ok(sh)
    } else {
        //? 计划印的是 get_shader_info_log(&sh) 直接内插; 0.3.98 返回 Option<String>
        //? (gen_WebGl2RenderingContext.rs:8696), unwrap_or_default 同义.
        Err(format!(
            "shader compile: {}",
            gl.get_shader_info_log(&sh).unwrap_or_default()
        ))
    }
}

fn compile_program(gl: &web_sys::WebGl2RenderingContext) -> Result<web_sys::WebGlProgram, String> {
    let vs = compile_shader(gl, web_sys::WebGl2RenderingContext::VERTEX_SHADER, VS_SRC)?;
    let fs = compile_shader(gl, web_sys::WebGl2RenderingContext::FRAGMENT_SHADER, FS_SRC)?;
    let p = gl.create_program().ok_or("create_program failed")?;
    gl.attach_shader(&p, &vs);
    gl.attach_shader(&p, &fs);
    gl.link_program(&p);
    let ok = gl
        .get_program_parameter(&p, web_sys::WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false);
    if ok {
        Ok(p)
    } else {
        //? 同 get_shader_info_log: 0.3.98 的 get_program_info_log 返回
        //? Option<String> (gen_WebGl2RenderingContext.rs:8650).
        Err(format!(
            "program link: {}",
            gl.get_program_info_log(&p).unwrap_or_default()
        ))
    }
}

impl Presenter for WebGl2Presenter {
    fn draw_frame(&mut self, bgra: &[u8]) -> Result<(), String> {
        bgra_to_rgba(bgra, &mut self.rgba);
        let gl = &self.gl;
        gl.bind_texture(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            Some(&self.texture),
        );
        //? 计划印的 tex_image_2d_with_u32_and_u32_and_html_image_element_or_canvas_or_video
        //? 与其指名的备选 ..._u8_array_and_opt_u32 在 0.3.98 均不存在 (E0599); 同义形状是
        //? tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array
        //? (gen_WebGl2RenderingContext.rs:3196) -- 实参 = 原列表去掉元素槽, 字节缓冲由
        //? wasm-bindgen 内部包成 Uint8Array 视图, 语义不变, 也不再需要 unsafe 视图.
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            0,
            web_sys::WebGl2RenderingContext::RGBA as i32,
            room::doom::doomgeneric::DOOMGENERIC_RESX as i32,
            room::doom::doomgeneric::DOOMGENERIC_RESY as i32,
            0,
            web_sys::WebGl2RenderingContext::RGBA,
            web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
            Some(&self.rgba),
        )
        .map_err(|e| format!("texImage2D: {e:?}"))?;
        gl.draw_arrays(web_sys::WebGl2RenderingContext::TRIANGLES, 0, 3);
        Ok(())
    }
}
