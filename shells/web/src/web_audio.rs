//! WebAudioBackend：引擎 AudioBackend 控制面在浏览器上的实现（spec ② D4）。
//!
//! //! 要点：
//! - SF2 在构造时从 VFS 预载（i_sound 的 exists() 在 wasm 上恒 false，
//!   I_InitMusic 不会调 load_sound_font —— 见计划 Task 4 设计说明）。
//! - 音乐走"分块拉取"：woom24_tick 每帧调 pump_music()，提前 0.2s 调度。
//! - SFX 每逻辑声道一对 source+gain+panner；end_time 时刻簿记清槽 = is_playing。

use std::cell::{Cell, RefCell};

use room::audio::{decode_doom_sfx, AudioBackend, SynthEngine, SynthFont, BLOCK_SIZE};

use crate::wasm_vfs;

/// Doom 音量(0..=127) → 线性增益。
// 供 trait 方法与测试使用；Task 7 工厂接线前宿主 cdylib 报 dead_code。
#[allow(dead_code)]
fn vol_gain(vol: i32) -> f32 {
    vol.clamp(0, 127) as f32 / 127.0
}

/// sep(0..=255, 128 居中) → StereoPannerNode.pan ∈ [-1, 1]。
/// 复用引擎的 gains_from（左/右增益），折算成单一 pan 值。
/// 注意：引擎 NORM_SEP = 128 居中（0..=255，见 s_sound.rs），
/// 而 gains_from 以 127 居中（0..=254，lib 自身约定）——
/// 曲线同为线性，中心平移 1 后两者逐点重合，故 (sep - 1) 喂入。
#[allow(dead_code)]
fn sep_to_pan(sep: i32) -> f32 {
    let (l, r) = room::audio::gains_from(127, (sep - 1).clamp(0, 254));
    let sum = l + r;
    if sum <= f32::EPSILON {
        0.0
    } else {
        (r - l) / sum
    }
}

thread_local! {
    /// 构造时要预载的 SF2 名（由 init_pipeline 在创建后端之前设置）。
    static PENDING_SF2: RefCell<Option<String>> = const { RefCell::new(None) };
    /// shell 侧保留的后端句柄：引擎把工厂产物存进 lib 私有的 AUDIO 单元，
    /// shell 拿不到 —— 工厂每次构造成功都在这里留一份 Rc 引用，
    /// 专供 woom24_tick 的音乐泵使用（Task 7）。
    static LAST_BUILT: RefCell<Option<std::rc::Rc<RefCell<WebAudioBackend>>>> =
        const { RefCell::new(None) };
}

/// init_pipeline 在 doomgeneric_Create 之前调用：记住 SF2 名。
// Task 7 init_pipeline 接线后移除此 allow。
#[allow(dead_code)]
pub fn set_pending_sf2(name: Option<String>) {
    PENDING_SF2.with_borrow_mut(|s| *s = name);
}

/// 供 woom24_tick 每帧调用的音乐泵（engine 的 AUDIO 单元 shell 摸不到，
/// 所以泵走这里保留的句柄；两个持有者各司其职，互不共享可变状态）。
// Task 7 woom24_tick 接线后移除此 allow。
#[allow(dead_code)]
pub fn pump_current_backend() {
    LAST_BUILT.with_borrow(|slot| {
        if let Some(b) = slot.as_ref() {
            b.borrow_mut().pump_music();
        }
    });
}

/// 单个 SFX 逻辑声道的活动节点组。
/// `end_time` = 声源结束的 AudioContext 绝对时刻（AudioBufferSourceNode
/// 没有 ended 轮询属性；用时间簿记代替 onended 回调，无全局句柄）。
#[allow(dead_code)]
struct SfxChannel {
    source: web_sys::AudioBufferSourceNode,
    gain: web_sys::GainNode,
    panner: web_sys::StereoPannerNode,
    end_time: f64,
}

//? 计划 Task 4 Step 3 的 new() 用 Rc::try_unwrap 同时交出"本体 + LAST_BUILT
//? 泵句柄"，但 Rust 所有权下同一个值不能既被 Box 独占又被 Rc 别名
//? （try_unwrap 因强引用计数为 2 恒失败，且其 Ok 侧是 RefCell 而非本体，
//? 原样照抄无法编译）。最小修复：本体状态下沉到 BackendCore，
//? WebAudioBackend 退化为廉价的 Rc 句柄（Clone），引擎 Box 与 shell 泵
//? 句柄共享同一 core —— 签名与 LAST_BUILT 类型保持计划原样。
/// 后端本体：真正持有 AudioContext 与全部节点/引擎状态。
// 状态仅经 WebAudioBackend 句柄触达；Task 7 接线前 dead_code 允许。
#[allow(dead_code)]
struct BackendCore {
    ctx: web_sys::AudioContext,
    /// 8 个逻辑声道（与引擎 I_UpdateSoundParams 的 0..8 对应）。
    channels: [Option<SfxChannel>; 8],
    music_gain: web_sys::GainNode,
    /// 已预载字体（构造时从 VFS 读取；Clone = Arc 克隆，随取随用）。
    font: Option<SynthFont>,
    /// 当前音乐引擎；None = 无字体/未播放/已结束。
    music: Option<SynthEngine>,
    /// 下一个音乐块应调度的绝对时间（AudioContext 时钟，秒）。
    music_next_time: f64,
    /// 已调度但未播完的源节点（stop_music 时统一 stop）。
    scheduled: Vec<web_sys::AudioBufferSourceNode>,
    /// 音乐是否被显式暂停。trait 的 pause/resume 是 &self，
    /// 用 Cell 内部可变（修复计划代码对 &self 赋值的 E0594）。
    paused: Cell<bool>,
}

/// 引擎 AudioBackend 的 wasm 实现：共享 core 的 Rc 句柄（Clone = 计数 +1）。
#[allow(dead_code)]
#[derive(Clone)]
pub struct WebAudioBackend {
    core: std::rc::Rc<RefCell<BackendCore>>,
}

impl BackendCore {
    // deprecated：web-sys 0.3.98 把 AudioBufferSourceNode::stop 全系标为
    // #[deprecated]（WebIDL 重载生成的历史包袱），无替代 API —— 显式允许。
    #![allow(dead_code, deprecated)]
    /// 从 VFS 预载 SF2（名字由 init_pipeline 经 set_pending_sf2 提供）。
    /// 失败只降级为"音乐静音"，绝不 panic（AGENTS 约束 2）。
    fn preload_sound_font(&mut self) {
        let Some(name) = PENDING_SF2.with_borrow(|s| s.clone()) else {
            return;
        };
        let Some(bytes) = wasm_vfs::vfs_get(&name) else {
            log::warn!("SF2 '{name}' 未注册，音乐将静音");
            return;
        };
        match SynthFont::parse(&bytes) {
            Some(f) => self.font = Some(f),
            None => log::warn!("SF2 '{name}' 解析失败，音乐将静音"),
        }
    }

    /// pump：SFX 槽清理 + 音乐块调度。由 woom24_tick 每帧调用（D4 pull 模型）。
    fn pump_music(&mut self) {
        if self.paused.get() {
            return;
        }
        let now = self.ctx.current_time();
        // SFX 槽清理：声源到达 end_time 即视为空闲（对齐原生 !player.empty()）。
        for slot in self.channels.iter_mut() {
            if let Some(ch) = slot {
                if now >= ch.end_time {
                    *slot = None;
                }
            }
        }
        // 音乐调度：时间线上保持 0.2s 领先。
        while self.music.is_some() && self.music_next_time < now + 0.2 {
            let Some(buf) = self.render_block_buffer() else {
                // 序列排空: 会话归位 (fix round 2)。若不置 None, engine 会话
                // 永不结束 -- scheduled 每块累积一个已播完的节点包装
                // (长曲数万个), 且 is_music_playing 恒 true, 卡死引擎经
                // i_sound 的会话轮询。
                self.music = None;
                break;
            };
            let Ok(src) = self.ctx.create_buffer_source() else {
                break;
            };
            src.set_buffer(Some(&buf));
            let _ = src.connect_with_audio_node(&self.music_gain);
            let _ = src.start_with_when(self.music_next_time.max(now));
            self.scheduled.push(src);
            self.music_next_time += BLOCK_SIZE as f64 / 44100.0;
        }
        if self.music.is_none() && !self.scheduled.is_empty() {
            // 排空当帧即到此处 (fix round 2 后本分支真正可达): 丢弃已排入
            // 的节点包装。浏览器侧已 start() 的尾块照常播完 (音频图持有
            // 节点直到播完), Rust 侧只丢包装, 不泄漏。
            self.scheduled.clear();
        }
    }

    /// 渲染一个 BLOCK_SIZE 帧块（交错的 L/R）进新 AudioBuffer。
    /// 序列已排空（第一个采样都取不到）时返回 None；提前结束时尾部补零。
    fn render_block_buffer(&mut self) -> Option<web_sys::AudioBuffer> {
        let engine = self.music.as_mut()?;
        let first = engine.next_interleaved()?;
        let buf = self
            .ctx
            .create_buffer(2, BLOCK_SIZE as u32, 44100.0)
            .ok()?;
        // get_channel_data 返回的是 Vec 拷贝 (写它等于扔掉) --
        // 先在本地暂存, 再用 copy_to_channel 写入 AudioBuffer
        // (fix round 1 裁定修正).
        let mut l = vec![0f32; BLOCK_SIZE];
        let mut r = vec![0f32; BLOCK_SIZE];
        // 消费序：L0, R0, L1, R1, …, R(BLOCK-1)，恰好 2*BLOCK 个采样。
        l[0] = first;
        for i in 0..BLOCK_SIZE {
            let Some(s) = engine.next_interleaved() else { break };
            r[i] = s;
            if i + 1 < BLOCK_SIZE {
                let Some(s2) = engine.next_interleaved() else { break };
                l[i + 1] = s2;
            }
        }
        buf.copy_to_channel(&l, 0).ok()?;
        buf.copy_to_channel(&r, 1).ok()?;
        Some(buf)
    }

    /// 引擎控制的 SFX 起播（trait start_sound 的本体实现）。
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        if channel >= 8 {
            return false;
        }
        let Some((sample_rate, samples)) = decode_doom_sfx(data) else {
            return false;
        };
        let (Ok(buf), Ok(gain), Ok(panner)) = (
            self.ctx.create_buffer(1, samples.len() as u32, sample_rate as f32),
            self.ctx.create_gain(),
            self.ctx.create_stereo_panner(),
        ) else {
            return false;
        };
        // get_channel_data 返回的是 Vec 拷贝, 写它等于扔掉 --
        // copy_to_channel 才真正写入 AudioBuffer (fix round 1 裁定修正).
        if buf.copy_to_channel(&samples, 0).is_err() {
            return false;
        }
        gain.gain().set_value(vol_gain(vol));
        panner.pan().set_value(sep_to_pan(sep));
        let Ok(src) = self.ctx.create_buffer_source() else {
            return false;
        };
        src.set_buffer(Some(&buf));
        let _ = src.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&panner);
        let _ = panner.connect_with_audio_node(&self.ctx.destination());
        let _ = src.start();
        // 结束时刻簿记：pump_music 里到点清槽（替代 onended 回调，
        // 因为 AudioBufferSourceNode 没有 ended 轮询属性，且回调拿不到 &mut self）。
        let end_time = self.ctx.current_time() + samples.len() as f64 / sample_rate as f64;
        self.channels[channel] = Some(SfxChannel { source: src, gain, panner, end_time });
        true
    }

    /// 停止一个逻辑声道（trait stop_sound 的本体实现）。
    fn stop_sound(&mut self, channel: usize) {
        if channel >= 8 {
            return;
        }
        if let Some(ch) = self.channels[channel].take() {
            let _ = ch.source.stop();
        }
    }

    /// 更新声道音量/声像（trait update_sound_params 的本体实现）。
    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        if channel >= 8 {
            return;
        }
        if let Some(ch) = &self.channels[channel] {
            ch.gain.gain().set_value(vol_gain(vol));
            ch.panner.pan().set_value(sep_to_pan(sep));
        }
    }

    /// 槽非空 = 声源未到 end_time（pump_music 每帧清槽）。
    /// 语义与原生 `!player.empty()` 对齐；8 之外的声道恒 false。
    fn is_playing(&self, channel: usize) -> bool {
        channel < 8 && self.channels[channel].is_some()
    }

    /// 引擎侧不会在 wasm 上调用（find_soundfont_path 依赖的 Path::exists()
    /// 恒 false）；保留 trait 完整性，字体在构造时已从 VFS 预载。
    fn load_sound_font(&mut self, _path: &std::path::Path) {
        log::info!("load_sound_font 在 wasm 上为 no-op：字体已在构造时从 VFS 预载");
    }

    /// 起播 MIDI（trait play_music 的本体实现）。
    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) {
        let Some(font) = self.font.clone() else {
            log::warn!("无已预载 SF2，play_music 忽略");
            return;
        };
        match SynthEngine::new(&font, midi_bytes, looping) {
            Some(engine) => {
                for s in self.scheduled.drain(..) {
                    let _ = s.stop();
                }
                self.music = Some(engine);
                self.music_next_time = self.ctx.current_time() + 0.05;
            }
            None => log::warn!("I_PlaySong: SynthEngine 构建失败"),
        }
    }

    /// 停止音乐并排空已调度源（trait stop_music 的本体实现）。
    fn stop_music(&mut self) {
        for s in self.scheduled.drain(..) {
            let _ = s.stop();
        }
        self.music = None;
    }

    /// 音乐总线音量（trait set_music_volume 的本体实现）。
    fn set_music_volume(&self, vol: i32) {
        self.music_gain.gain().set_value(vol_gain(vol));
    }
}

// new/pump_music 在 Task 7（工厂安装 / woom24_tick 接线）前无调用方。
#[allow(dead_code)]
impl WebAudioBackend {
    /// 工厂入口：Web Audio 不可用时返回 Err（room 记日志并走 NoopBackend，
    /// 即 spec 的"降级静音"契约）。构造成功即在 LAST_BUILT 留泵句柄：
    /// 返回本体与 LAST_BUILT 里的句柄是同一 core 的两个 Rc（见 BackendCore 注）。
    pub fn new() -> Result<Self, String> {
        let ctx = web_sys::AudioContext::new().map_err(|e| format!("AudioContext: {e:?}"))?;
        let music_gain = ctx
            .create_gain()
            .map_err(|e| format!("create_gain: {e:?}"))?;
        music_gain
            .connect_with_audio_node(&ctx.destination())
            .map_err(|e| format!("connect: {e:?}"))?;
        let mut core = BackendCore {
            ctx,
            channels: std::array::from_fn(|_| None),
            music_gain,
            font: None,
            music: None,
            music_next_time: 0.0,
            scheduled: Vec::new(),
            paused: Cell::new(false),
        };
        core.preload_sound_font();
        let backend = Self {
            core: std::rc::Rc::new(RefCell::new(core)),
        };
        LAST_BUILT.with_borrow_mut(|slot| {
            *slot = Some(std::rc::Rc::new(RefCell::new(backend.clone())));
        });
        Ok(backend)
    }

    /// 泵入口：转递给共享 core（SFX 槽清理 + 音乐块调度）。
    fn pump_music(&mut self) {
        self.core.borrow_mut().pump_music();
    }
}

impl AudioBackend for WebAudioBackend {
    fn start_sound(&mut self, data: &[u8], vol: i32, sep: i32, channel: usize) -> bool {
        self.core.borrow_mut().start_sound(data, vol, sep, channel)
    }

    fn stop_sound(&mut self, channel: usize) {
        self.core.borrow_mut().stop_sound(channel);
    }

    fn update_sound_params(&self, channel: usize, vol: i32, sep: i32) {
        self.core.borrow().update_sound_params(channel, vol, sep);
    }

    fn is_playing(&self, channel: usize) -> bool {
        self.core.borrow().is_playing(channel)
    }

    fn load_sound_font(&mut self, path: &std::path::Path) {
        self.core.borrow_mut().load_sound_font(path);
    }

    fn play_music(&mut self, midi_bytes: &[u8], looping: bool) {
        self.core.borrow_mut().play_music(midi_bytes, looping);
    }

    fn stop_music(&mut self) {
        self.core.borrow_mut().stop_music();
    }

    fn set_music_volume(&self, vol: i32) {
        self.core.borrow().set_music_volume(vol);
    }

    fn pause_music(&self) {
        let core = self.core.borrow();
        core.paused.set(true);
        let _ = core.ctx.suspend();
    }

    fn resume_music(&self) {
        let core = self.core.borrow();
        core.paused.set(false);
        let _ = core.ctx.resume();
    }

    fn is_music_playing(&self) -> bool {
        let core = self.core.borrow();
        core.music.is_some() || !core.scheduled.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vol_gain_matches_doom_scale() {
        assert_eq!(vol_gain(0), 0.0);
        assert!((vol_gain(127) - 1.0).abs() < 1e-6);
        assert!((vol_gain(64) - 64.0 / 127.0).abs() < 1e-6);
        assert_eq!(vol_gain(200), 1.0, "超范围钳制");
    }

    #[test]
    fn sep_to_pan_is_centred_at_128() {
        assert!(sep_to_pan(128).abs() < 1e-6);
        assert!(sep_to_pan(254) > 0.0, "全右为正");
        assert!(sep_to_pan(0) < 0.0, "全左为负");
        assert!(sep_to_pan(-5).abs() <= 1.0, "越界钳制后仍在 [-1,1]");
    }
}
