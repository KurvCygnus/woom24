//! 最小入口的 DOM 配置 UI (D5 / AGENTS.md launcher mode).
//!
//! //! 全部用 web-sys 裸 DOM API, 无框架无 CDN.
//! //! 文件读取用 FileReader 回调 (闭包持有 reader 所有权),
//! //! 避开 wasm-bindgen-futures 新依赖; 读到的字节直接注册进 VFS.
//! //! 用户点"开始"才构造档案并进入 init_pipeline -- 游戏绝不自启.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::init_pipeline;
use crate::profile::BootProfile;
use crate::wasm_vfs;

thread_local! {
    /// change 事件里已读入并注册的 PWAD 名 (顺序 = FileList 顺序).
    static PWAD_NAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static SF2_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 在 body 上挂出配置面板: PWAD 多选 + SF2 单选 + 开始按钮 + 错误横幅.
pub fn show(max_render_res: u32, iwad_name: &str) -> Result<(), String> {
    let doc = web_sys::window()
        .ok_or("no window")?
        .document()
        .ok_or("no document")?;
    let body = doc.body().ok_or("no body")?;

    let panel = doc
        .create_element("div")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlDivElement>()
        //? dyn_into 的错误侧是原类型 (Element), 不是 JsValue -- 转 JsValue 后复用 js_err.
        .map_err(|e| js_err(e.into()))?;
    panel.set_id("woom24-launcher");

    let title = doc.create_element("p").map_err(js_err)?;
    title.set_text_content(Some(&format!("woom24 — IWAD: {iwad_name}")));
    let _ = panel.append_child(&title);

    // PWAD 多选 (顺序即 -file 加载顺序).
    let pwad_label = doc.create_element("label").map_err(js_err)?;
    pwad_label.set_text_content(Some("PWADs（可多选，按选择顺序加载）"));
    let pwads = file_input(&doc, true, ".wad").map_err(js_err)?;
    let _ = panel.append_child(&pwad_label);
    let _ = panel.append_child(&pwads);

    // SF2 单选 (可选; 不选则音乐静音).
    let sf2_label = doc.create_element("label").map_err(js_err)?;
    sf2_label.set_text_content(Some("SoundFont（可选；不选则音乐静音）"));
    let sf2 = file_input(&doc, false, ".sf2").map_err(js_err)?;
    let _ = panel.append_child(&sf2_label);
    let _ = panel.append_child(&sf2);

    // 错误横幅: 缺 IWAD / 引擎初始化失败显示于此, 绝不裸 panic 进控制台.
    let banner = doc.create_element("p").map_err(js_err)?;
    banner.set_id("woom24-banner");

    let start = doc
        .create_element("button")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlButtonElement>()
        //? 同上: dyn_into 的错误侧是 Element.
        .map_err(|e| js_err(e.into()))?;
    start.set_text_content(Some("Start"));

    let iwad = iwad_name.to_string();
    let doc_for_cb = doc.clone();
    let banner_for_cb = banner.clone();
    let on_click = Closure::<dyn FnMut()>::new(move || {
        banner_for_cb.set_text_content(None);
        // 字节已在各自 change→onload 链里注册进 VFS;
        // start 只需要把名字列表拼进档案.
        let pwad_names = PWAD_NAMES.with_borrow(|n| n.clone());
        let sf2_name = SF2_NAME.with_borrow(|n| n.clone());
        let profile = BootProfile {
            iwad: iwad.clone(),
            pwads: pwad_names,
            sf2: sf2_name,
            max_render_res: Some(max_render_res),
            engine_args: Vec::new(),
        };
        if let Err(e) = init_pipeline::run(&profile) {
            banner_for_cb.set_text_content(Some(&format!("启动失败：{e}")));
        } else {
            // 启动成功: 拆掉配置面板 (呈现画布由 loader 提供).
            let _ = doc_for_cb
                .get_element_by_id("woom24-launcher")
                .map(|n| n.remove());
        }
    });
    start.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget();
    let _ = panel.append_child(&start);
    let _ = panel.append_child(&banner);
    let _ = body.append_child(&panel);
    Ok(())
}

/// 构造文件选择框; change 事件里逐文件读字节并注册 VFS.
fn file_input(
    doc: &web_sys::Document,
    multiple: bool,
    accept: &str,
) -> Result<web_sys::HtmlInputElement, JsValue> {
    let input = doc
        .create_element("input")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlInputElement>()
        //? 同上: dyn_into 的错误侧是 Element.
        .map_err(|e| js_err(e.into()))?;
    input.set_type("file");
    input.set_accept(accept);
    input.set_multiple(multiple);
    let is_sf2 = accept == ".sf2";
    let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        if let Err(e) = on_files_picked(&ev, is_sf2) {
            web_sys::console::error_1(&JsValue::from_str(&e));
        }
    });
    input
        .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
        .map_err(js_err)?;
    on_change.forget();
    Ok(input)
}

/// change 事件: 逐文件建 FileReader, onload 里取字节并注册 VFS + 记名.
fn on_files_picked(ev: &web_sys::Event, is_sf2: bool) -> Result<(), String> {
    let input = ev
        .target()
        .ok_or("no target")?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|e| format!("cast: {e:?}"))?;
    let files = input.files().ok_or("no files")?;
    for i in 0..files.length() {
        let f = files.get(i).ok_or("file vanished")?;
        let name = f.name();
        let reader = web_sys::FileReader::new().map_err(js_err)?;
        // 闭包持有 reader/fname/is_sf2 所有权; onload 时 result 已就绪.
        //? reader 本体还要 read_as_array_buffer, 闭包里用克隆 (同一 JS 对象的两个句柄).
        let reader_for_cb = reader.clone();
        let fname = name.clone();
        let on_load = Closure::<dyn FnMut()>::new(move || match reader_for_cb.result() {
            Ok(v) if !v.is_null() => {
                let bytes = js_sys::Uint8Array::new(&v).to_vec();
                wasm_vfs::vfs_register(&fname, bytes);
                if is_sf2 {
                    SF2_NAME.with_borrow_mut(|n| *n = Some(fname.clone()));
                } else {
                    PWAD_NAMES.with_borrow_mut(|n| n.push(fname.clone()));
                }
            }
            _ => web_sys::console::error_1(&JsValue::from_str(&format!("读取 {fname} 失败"))),
        });
        reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
        on_load.forget();
        reader
            .read_as_array_buffer(&f)
            .map_err(|e| format!("read {name}: {e:?}"))?;
    }
    Ok(())
}

fn js_err(e: JsValue) -> String {
    format!("DOM: {e:?}")
}
