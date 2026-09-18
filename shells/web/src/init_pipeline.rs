//! 两入口共享的初始化管线 (D5 / AGENTS.md 入口契约).
//!
//! //! 档案 → argv (Rust 侧构造, 无 JS argv) → 工厂安装 → doomgeneric_Create.
//! //! 引擎会永久保存 myargv, CString 与指针数组必须活满整个进程:
//! //! 存进 thread_local, 与 native main.rs 的 App.args 同一手法.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CString};

use room::audio::AudioBackend;

use crate::profile::BootProfile;
use crate::wasm_vfs;
use crate::web_audio;

thread_local! {
    /// argv 的所有权锚点 (引擎保存 myargv 指针, 绝不可释放).
    static ARG_STORAGE: RefCell<Vec<CString>> = const { RefCell::new(Vec::new()) };
}

/// 由档案构造引擎 argv.
/// 约定: argv[0] = "woom24"; -iwad <名>; PWAD 逐个 -file <名>;
/// SF2 不进 argv (i_sound 的 exists() 在 wasm 上恒 false, 见 Task 4 说明).
fn build_argv(profile: &BootProfile) -> Vec<CString> {
    let mut v = vec![CString::new("woom24").unwrap()];
    v.push(CString::new("-iwad").unwrap());
    v.push(CString::new(profile.iwad.clone()).unwrap());
    for pwad in &profile.pwads {
        v.push(CString::new("-file").unwrap());
        v.push(CString::new(pwad.clone()).unwrap());
    }
    for a in &profile.engine_args {
        // 引擎参数原样透传 (宿主档案是信任边界内的输入).
        v.push(CString::new(a.clone()).unwrap());
    }
    v
}

/// 管线主体: 两入口在此汇合, 导出面之外没有任何分支.
pub fn run(profile: &BootProfile) -> Result<(), String> {
    // 1. IWAD 必须已在 VFS (minimal 入口刚注册; standard 入口由宿主预注册).
    if wasm_vfs::vfs_get(&profile.iwad).is_none() {
        return Err(format!(
            "IWAD '{}' 未注册：先经 woom24_register_file 注册",
            profile.iwad
        ));
    }
    // 2. 帧缓冲上限 (D5): 引擎侧 cap 属于后续自定义分辨率工作;
    //    本 spec 先记录到呈现器画布尺寸钳制 (Task 5/6 已按 640×400 落地).
    if let Some(res) = profile.max_render_res {
        log::info!("宿主最大渲染分辨率: {res}px（引擎侧 cap 待自定义分辨率工作）");
    }
    // 3. 音频: 记住 SF2 名 (后端构造时从 VFS 预载), 装工厂.
    web_audio::set_pending_sf2(profile.sf2.clone());
    room::audio::set_backend_factory(|| {
        web_audio::WebAudioBackend::new().map(|b| Box::new(b) as Box<dyn AudioBackend>)
    });
    // 4. argv → doomgeneric_Create (D_DoomMain 由此返回, tick 交给 woom24_tick).
    let args = build_argv(profile);
    let mut argv: Vec<*mut c_char> = args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut()); // C 约定 argv[argc] = NULL
    let argc = (argv.len() - 1) as c_int;
    ARG_STORAGE.with_borrow_mut(|s| *s = args);
    // SAFETY: argv 指向 ARG_STORAGE 持有的 NUL 结尾串, 进程级存活;
    // doomgeneric_Create 在本进程至多调用一次 (入口契约保证).
    unsafe {
        room::doom::doomgeneric::doomgeneric_Create(argc, argv.as_mut_ptr());
    }
    Ok(())
}
