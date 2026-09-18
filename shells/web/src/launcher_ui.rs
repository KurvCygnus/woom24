//! DOM config UI for the minimal entry (D5 / AGENTS.md launcher mode).
//!
//! Built entirely from raw web-sys DOM APIs -- no framework, no CDN. File
//! reads go through FileReader callbacks (the closures own the reader),
//! avoiding a new wasm-bindgen-futures dependency; the bytes read are
//! registered straight into the VFS. The profile is only assembled and handed
//! to init_pipeline when the user clicks Start -- the game never self-starts.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::init_pipeline;
use crate::profile::BootProfile;
use crate::wasm_vfs;

thread_local! {
    /// PWAD names read in and registered during change events (order =
    /// FileList order).
    static PWAD_NAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static SF2_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
    /// File reads started but not yet finished (fix round 1). Start refuses to
    /// boot while this is non-zero, so in-flight PWADs can never be silently
    /// missing from the boot profile.
    static PENDING_READS: Cell<u32> = const { Cell::new(0) };
}

/// Mounts the config panel on the body: PWAD multi-select + SF2 picker +
/// Start button + error banner.
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
        //? dyn_into's error side is the original type (Element), not a
        //? JsValue -- convert to JsValue to reuse js_err.
        .map_err(|e| js_err(e.into()))?;
    panel.set_id("woom24-launcher");

    let title = doc.create_element("p").map_err(js_err)?;
    title.set_text_content(Some(&format!("woom24 — IWAD: {iwad_name}")));
    let _ = panel.append_child(&title);

    // PWAD multi-select (the order is the -file load order).
    let pwad_label = doc.create_element("label").map_err(js_err)?;
    pwad_label.set_text_content(Some("PWADs（可多选，按选择顺序加载）"));
    let pwads = file_input(&doc, true, ".wad").map_err(js_err)?;
    let _ = panel.append_child(&pwad_label);
    let _ = panel.append_child(&pwads);

    // SF2 picker (optional; without one, music stays silent).
    let sf2_label = doc.create_element("label").map_err(js_err)?;
    sf2_label.set_text_content(Some("SoundFont（可选；不选则音乐静音）"));
    let sf2 = file_input(&doc, false, ".sf2").map_err(js_err)?;
    let _ = panel.append_child(&sf2_label);
    let _ = panel.append_child(&sf2);

    // Error banner: a missing IWAD / failed engine init shows up here, never
    // a raw panic into the console.
    let banner = doc.create_element("p").map_err(js_err)?;
    banner.set_id("woom24-banner");

    let start = doc
        .create_element("button")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlButtonElement>()
        //? Same as above: dyn_into's error side is Element.
        .map_err(|e| js_err(e.into()))?;
    start.set_text_content(Some("Start"));

    let iwad = iwad_name.to_string();
    let doc_for_cb = doc.clone();
    let banner_for_cb = banner.clone();
    let on_click = Closure::<dyn FnMut()>::new(move || {
        banner_for_cb.set_text_content(None);
        // Fix round 1: file reads land in the VFS asynchronously (onload). While
        // any read is still in flight, refuse to boot -- otherwise a quick Start
        // click would silently drop the not-yet-registered PWADs from the profile.
        let pending = PENDING_READS.with(|c| c.get());
        if pending > 0 {
            banner_for_cb.set_text_content(Some(&format!(
                "文件仍在读取中（{pending} 个未完成），请稍后再点 Start"
            )));
            return;
        }
        // Bytes were registered into the VFS by each change→onload chain;
        // Start only assembles the name lists into the boot profile.
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
            // Boot succeeded: tear down the config panel (the presentation
            // canvas is provided by the loader).
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

/// Builds a file picker; the change event reads each file's bytes and
/// registers them into the VFS.
fn file_input(
    doc: &web_sys::Document,
    multiple: bool,
    accept: &str,
) -> Result<web_sys::HtmlInputElement, JsValue> {
    let input = doc
        .create_element("input")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlInputElement>()
        //? Same as above: dyn_into's error side is Element.
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

/// change event: builds a FileReader per file; onload grabs the bytes,
/// registers them into the VFS and records the name.
fn on_files_picked(ev: &web_sys::Event, is_sf2: bool) -> Result<(), String> {
    let input = ev
        .target()
        .ok_or("no target")?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|e| format!("cast: {e:?}"))?;
    let files = input.files().ok_or("no files")?;
    // Fix round 1 (replace-append semantics): one picker session's FileList is
    // the whole PWAD set -- clear the previous pick so re-picking cannot
    // accumulate stale names. (SF2_NAME is single-valued and already replaces.)
    if !is_sf2 {
        PWAD_NAMES.with_borrow_mut(|n| n.clear());
    }
    for i in 0..files.length() {
        let f = files.get(i).ok_or("file vanished")?;
        let name = f.name();
        let reader = web_sys::FileReader::new().map_err(js_err)?;
        // The closure owns reader/fname/is_sf2; result is ready by onload time.
        //? The reader itself is still needed for read_as_array_buffer below,
        //? so the closures get clones (two handles to the same JS object).
        let reader_for_cb = reader.clone();
        let fname = name.clone();
        let on_load = Closure::<dyn FnMut()>::new(move || {
            // This read is finished (success path): release the pending slot.
            PENDING_READS.with(|c| c.set(c.get().saturating_sub(1)));
            match reader_for_cb.result() {
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
            }
        });
        reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
        on_load.forget();
        // Fix round 1: a failed read never fires onload -- decrement (and log)
        // here too, so the pending counter cannot leak and block Start forever.
        let on_error = Closure::<dyn FnMut()>::new(move || {
            PENDING_READS.with(|c| c.set(c.get().saturating_sub(1)));
            web_sys::console::error_1(&JsValue::from_str("file read failed"));
        });
        reader.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();
        // Bookkeeping order: count the read as pending only once it actually
        // starts; a synchronous start failure must undo the increment, since
        // neither onload nor onerror will fire for it.
        PENDING_READS.with(|c| c.set(c.get() + 1));
        if let Err(e) = reader.read_as_array_buffer(&f) {
            PENDING_READS.with(|c| c.set(c.get().saturating_sub(1)));
            return Err(format!("read {name}: {e:?}"));
        }
    }
    Ok(())
}

fn js_err(e: JsValue) -> String {
    format!("DOM: {e:?}")
}
