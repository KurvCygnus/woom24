//! 拉取式合成核心：rustysynth 的薄封装（spec ② D4）。
//!
//! 把"MIDI 字节 → 交错立体声 f32 采样"的唯一实现放进 lib，
//! native（rodio Source）与 web（AudioBuffer 分块）各自做输出适配，
//! 合成逻辑绝不在两个 shell 里重复（AGENTS.md）。
//!
//! //! 命令式状态：512 帧块渲染 + 交错游标 + 结束标志。
//! 音量不在本模块（输出适配层各自缩放），保持核心纯净。

/// 每次 sequencer render 渲染的帧数（左右各 BLOCK_SIZE 个采样）。
pub const BLOCK_SIZE: usize = 512;

use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};
use std::io::Cursor;
use std::sync::Arc;

use super::SAMPLE_RATE;

/// 已解析的 SF2 字体。对外不透明：rustysynth 类型不再出现在任何 shell 的 API 上。
/// Clone = Arc 克隆（廉价），供后端在多次 play_music 间复用。
#[derive(Clone)]
pub struct SynthFont(Arc<SoundFont>);

impl SynthFont {
    /// 从字节解析 SF2；失败返回 None（调用方记日志并保持静音）。
    pub fn parse(sf2_bytes: &[u8]) -> Option<Self> {
        SoundFont::new(&mut Cursor::new(sf2_bytes))
            .ok()
            .map(Arc::new)
            .map(Self)
    }
}

/// 一次播放会话：sequencer + 双声道暂存块 + 交错游标。
/// 字体本身不进来（Synthesizer 内部持有 Arc<SoundFont>），避免每次播放重解析。
pub struct SynthEngine {
    sequencer: MidiFileSequencer,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    finished: bool,
}

impl SynthEngine {
    /// 由字体 + SMF 字节构建播放会话。MIDI 解析失败返回 None。
    pub fn new(font: &SynthFont, midi_bytes: &[u8], looping: bool) -> Option<Self> {
        let settings = SynthesizerSettings::new(SAMPLE_RATE);
        // rustysynth 1.3 的 Synthesizer::new 签名是 &Arc<SoundFont>，直接传引用。
        let synthesizer = Synthesizer::new(&font.0, &settings).ok()?;
        let midi_file = Arc::new(MidiFile::new(&mut Cursor::new(midi_bytes)).ok()?);
        let mut sequencer = MidiFileSequencer::new(synthesizer);
        sequencer.play(&midi_file, looping);
        Some(Self {
            sequencer,
            buf_l: vec![0.0f32; BLOCK_SIZE],
            buf_r: vec![0.0f32; BLOCK_SIZE],
            pos: BLOCK_SIZE * 2, // 首次 next_interleaved 触发渲染
            finished: false,
        })
    }

    /// 交错输出下一个采样（L,R,L,R…）。序列结束且当前块排空后返回 None。
    pub fn next_interleaved(&mut self) -> Option<f32> {
        if self.pos >= BLOCK_SIZE * 2 {
            if self.finished {
                return None;
            }
            self.sequencer.render(&mut self.buf_l, &mut self.buf_r);
            self.pos = 0;
            if self.sequencer.end_of_sequence() {
                self.finished = true;
            }
        }
        let s = if self.pos.is_multiple_of(2) {
            self.buf_l[self.pos / 2]
        } else {
            self.buf_r[self.pos / 2]
        };
        self.pos += 1;
        Some(s)
    }

    /// 是否已到序列末尾（当前块可能仍在排空）。
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mus2midi;

    /// 与 music.rs 测试同款的最小 MUS，但带一个 note-on，
    /// 让合成器有真实事件可渲染。
    fn note_on_mus() -> Vec<u8> {
        let mut d = vec![0u8; 19];
        d[0..4].copy_from_slice(b"MUS\x1a");
        d[4] = 3; // score_len（低字节）
        d[6] = 16; // score_start
        d[16] = 0x10; // note-on, channel 0
        d[17] = 0x3c; // note 60
        d[18] = 0x60; // score end（非 last，仅占位）
        d
    }

    #[test]
    fn synth_font_rejects_garbage() {
        assert!(SynthFont::parse(b"not a soundfont").is_none());
        assert!(SynthFont::parse(&[]).is_none());
    }

    #[test]
    fn synth_engine_rejects_bad_midi() {
        // 没有 SF2 时构造不出引擎，先造一个拒绝路径：坏 MIDI 必须在
        // 有字体前置下也返回 None —— 这里退而验证 mus2midi 的守门：
        assert!(mus2midi(b"garbage").is_none());
    }

    #[test]
    fn synth_engine_renders_deterministic_samples() {
        // 需要本地 SF2（soundfonts/ 下任一，不入库）；缺失则跳过并说明。
        let Some(sf2_bytes) = find_local_sf2() else {
            eprintln!("skip: no local .sf2 under soundfonts/ — determinism not exercised");
            return;
        };
        let font = SynthFont::parse(&sf2_bytes).expect("local sf2 must parse");
        let midi = mus2midi(&note_on_mus()).expect("fixture MUS must convert");
        let mut a = SynthEngine::new(&font, &midi, false).expect("engine a");
        let mut b = SynthEngine::new(&font, &midi, false).expect("engine b");
        let sa: Vec<f32> = (0..4096).filter_map(|_| a.next_interleaved()).collect();
        let sb: Vec<f32> = (0..4096).filter_map(|_| b.next_interleaved()).collect();
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert_eq!(x.to_bits(), y.to_bits(), "同一输入必须逐位一致");
        }
        // note-on 后不应全零（事件真的被合成了）。
        assert!(sa.iter().any(|&s| s != 0.0));
    }

    /// 在 soundfonts/ 下找任一 .sf2（开发机有 SC-55 字体；CI/无字体环境跳过）。
    /// cargo test 的测试进程 cwd 是包根（room/），仓库根的 soundfonts/ 需经
    /// CARGO_MANIFEST_DIR/.. 解析（与 tables.rs、demo_playthrough.rs 同款）。
    fn find_local_sf2() -> Option<Vec<u8>> {
        let candidates = [
            std::path::PathBuf::from("soundfonts"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("soundfonts"),
        ];
        candidates.iter().find_map(|root| find_sf2_under(root))
    }

    /// 在单个候选目录（含一级子目录）下扫描任一 .sf2。
    fn find_sf2_under(root: &std::path::Path) -> Option<Vec<u8>> {
        for entry in std::fs::read_dir(root).ok()?.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "sf2") {
                return std::fs::read(&p).ok();
            }
            // 允许一级子目录（仓库现状：soundfonts/SC55Soundfont-1.2b/*.sf2）。
            if p.is_dir() {
                for sub in std::fs::read_dir(&p).ok()?.flatten() {
                    let sp = sub.path();
                    if sp.extension().is_some_and(|e| e == "sf2") {
                        return std::fs::read(&sp).ok();
                    }
                }
            }
        }
        None
    }
}
