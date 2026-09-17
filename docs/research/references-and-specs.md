# 参考源与兼容规范调研 — woom24

> **状态:讨论稿**。上游与 WAD 解析已决(§9,2026-09-17),其余决策点仍待定。
> 调研时间:2026-09-17。所有 GitHub 事实(star/活动/许可证)来自对仓库页与 GitHub API 的直接核查,非转述。

---

## 0. 结论速览

- 你推荐的 **`sunsided/room` 真实存在**(顺带查证:`sunsided/woom` 是 404,不存在)。它是 doomgeneric 的
  逐模块 Rust 重写,**完整 Vanilla 引擎**(不只是渲染器),带 C/Rust 逐位回归测试,GPL-2.0,非常活跃。
  它是"游戏逻辑核心"问题的现成答案;woom24 剩下的工作可以映射为"把 room 的 winit/wgpu 平台层换成
  wasm-bindgen 层 + 兼容等级扩展"。
- **MBF21 有正式 spec**(kraflab/mbf21,v1.4),**ID24 有正式 spec**(doom-cross-port-collab/id24,
  v0.99.2 草案)——这两级走 spec-first,符合你的策略。
- **Boom/MBF 无 spec**,但 `doom-cross-port-collab` 组织托管了 **Boom 2.02 与 MBF 2.03 官方源码**,
  外加 prboom-plus 的 complevel 文档(prboom-plus 已归档,继承者是 dsda-doom)。
- 浏览器端特性(uncapped FPS / 自定义分辨率 / 宽高比 / 单文件自包含部署)已有 C 侧先例
  **GMH-Code/Dwasm**(PrBoom+ → Emscripten),可作部署模型参考,而非代码参考。
- **决策(2026-09-17)**:上游定为 **fork `sunsided/room`**;WASM 层借鉴 `crustyview`(WAD 加载 UX、
  automap)与 `Dwasm`(uncapped/分辨率/单文件部署);其余 Rust Doom port 不作参考;成熟 source port 与
  兼容规范仍是必须参考项。同日已决:WASM 目标 `wasm32-unknown-unknown`、渲染后端(WebGL2 默认 +
  Canvas2D 兜底,WebGPU 实验性)、音频(MIDI 优先、逐层 fallback)、双入口 wasm API——详见 §9。

---

## 1. `sunsided/room` 深查(你指定的学习对象)

https://github.com/sunsided/room —"Doomgeneric re-implemented in Rust. Rip and tear down undefined behavior."

| 维度 | 事实 |
|---|---|
| 范围 | **完整引擎**:g_game、d_loop、d_main、全部 p_*(mobj/敌人/地图/存档/特殊…)、r_* 渲染、HUD/自动地图/过场、菜单、w_wad、z_zone、系统层(i_video/i_sound/i_input/i_scale/i_timer)。README 宣称"Port complete":所有 vendored C 模块已删除并全部替换为 Rust |
| 正确性 | `room/src/doom/c_tests/` 回归装置:**原始 C 函数与 Rust 移植并排运行,逐位比对**。这是本项目 golden demo 测试思想的现成范本 |
| 音频 | 有:SFX 走 rodio(立体声 pan);音乐走 rustysynth + Roland SC-55 SoundFont |
| 存档 | 有(p_saveg 已移植) |
| Demo | README 未提及 demo/tics/35fps;但 demo 逻辑位于 g_game/d_loop(均已移植且逐位测试),**大概率可播,但无人在案验证**——woom24 需自行验证 |
| 联网 | 无(单机;FEATURE_MULTIPLAYER 未编译) |
| 输入 | 仅键盘(winit → VecDeque,每 tic 弹出) |
| 分辨率/FPS | 固定 640×400 最近邻放大;i_scale 已移植但**无 uncapped、无任意分辨率/宽高比** |
| WASM | **无**(winit + wgpu 原生平台层,无 wasm-bindgen/emscripten) |
| 兼容等级 | 仅 Vanilla(doomgeneric 血统) |
| 许可证 | **GPL-2.0**(Doom 源码继承)。复用其代码 ⇒ woom24 整体 GPL-2.0 |
| 活跃度 | 2026-05 建,最近 push 2026-08-28,160 commits,3★/3 fork,27 open issues,10 PR |

## 2. Rust / WASM Doom 仓库全景

| 仓库 | 语言 | 范围 | Demo | Uncapped/高分辨率/宽高比 | WASM | 许可证 | 状态 | 可借鉴点 |
|---|---|---|---|---|---|---|---|---|
| [sunsided/room](https://github.com/sunsided/room) | Rust | 完整引擎(Vanilla) | 大概率有,未验证 | 无(固定 640×400) | 无 | GPL-2.0 | 活跃 | 逐位正确的 Rust 游戏逻辑核心;C-vs-Rust 差分回归范式 |
| [cristicbz/rust-doom](https://github.com/cristicbz/rust-doom) | Rust | 仅渲染器 | 无 | FOV/分辨率旗标 | 无(glium/glutin) | **Apache-2.0** | 停滞(2024-01) | 唯一宽松许可证的成规模 Rust Doom 代码;engine/game/math/wad workspace 布局 |
| [Henrique194/iron-doom](https://github.com/Henrique194/iron-doom) | Rust | 完整移植(Chocolate 系重写) | 自述 demo 可能失同步 | 未提及 | 无 | GPL-3.0 | 停滞(12 commits) | 可读性 vs demo 同步的取舍讨论 |
| [Patryk27/doome](https://github.com/Patryk27/doome) | Rust | gamejam 渲染器 | 无 | 无 | 不明 | — | 死(2023-02) | 教学级渲染/WAD 代码 |
| [lord-helicon/ferrum-doom](https://github.com/lord-helicon/ferrum-doom) | Rust+TS | 浏览器 Doom(Rust/WASM 引擎 + Vite/TS 宿主) | ? | ? | 是 | **无许可证** | 全新,0★ | 引擎/宿主拆分的先例(只能看,不能抄) |
| [LinusCDE/doomgeneric-rs](https://github.com/LinusCDE/doomgeneric-rs) | Rust | C doomgeneric 的 FFI 绑定 | 有(随引擎) | 引擎的 | 可间接移植 | — | 活跃 | DG_* 五函数接口 = WASM 边界的理想形状 |
| [masriamir/crustyview](https://github.com/masriamir/crustyview) | TS+Rust | Web WAD 查看器(基于 crustywad) | N/A | N/A | 是 | MIT/Apache | 活跃 | 浏览器端 WAD 加载 UX |
| [ozkl/doomgeneric](https://github.com/ozkl/doomgeneric) | C | 完整引擎,极小移植接口 | 有 | 无(Vanilla) | **有 Emscripten 后端含音频** | GPL-2.0 | 稳定,2.1k★ | DG_Init/DG_DrawFrame/DG_GetTicksMs/DG_GetKey = wasm-bindgen trait 面的蓝本 |

明确排除/不存在:`sunsided/woom`(404)、`diekmann/Doom`(404,实为 wasm-fizzbuzz 教程)、
`iron-doom`/`ferrum-doom`(太早期,后者无许可证,均不作主参考)。

## 3. C 侧 WASM 部署模型参考(非代码参考)

| 仓库 | 要点 |
|---|---|
| [GMH-Code/Dwasm](https://github.com/GMH-Code/Dwasm) | **与 woom24 特性集重合度最高的现存证明**:PrBoom+/PrBoomX → Emscripten,宽屏宽高比、超 320×200 自定义分辨率、>35 FPS uncapped、纹理放大、OPL2/Timidity MIDI、手柄、浏览器存档、运行时加载 PWAD+DeHackEd;部署 = 4 个静态文件,或 `-DSINGLE_HTML_FILE=1` → **单个自包含 HTML**。GPL-2.0,37★,在线 demo:dwasm.m-h.org.uk |
| [Darkstrier/dsda-wasm-doom](https://github.com/Darkstrier/dsda-wasm-doom) | dsda-doom → Emscripten,静态 .wasm/.js/.bin 部署;证明 demo 精确型引擎可上浏览器 |
| [mesmotronic/web-doom](https://github.com/mesmotronic/web-doom) | 明确"挣脱 320×200"的 Emscripten 移植,高分辨率渲染思路参考 |
| [somoore/hellbox](https://github.com/somoore/hellbox) | **反面教材**:Lambda MicroVM 依赖服务端,违反自包含约束 |

## 4. WAD 解析 crate(crates.io 实查)

| crate | 版本/更新 | 许可证 | 备注 |
|---|---|---|---|
| [crustywad](https://crates.io/crates/crustywad) | 0.9.6 / 2026-08-26 | MIT OR Apache-2.0 | "Performant, safe, typed Doom WAD file I/O",活跃维护,已在 web 语境(crustyview)中工作 |
| [oxiwad](https://crates.io/crates/oxiwad) | 1.0.0 / 2026-07-11 | MIT | **零依赖** id Tech 1 解析:lump/地图/BSP/调色板/图形 |
| [wad](https://crates.io/crates/wad) | 0.3.2 / 2019 | MIT | 工具向,陈旧 |
| tinywad | 0.1.3 / 2024 | — | WAD 改包向(modding) |
| rs_wad | GitHub 2022 | — | 简易加载库 |

名称核查:`doom-wad` crate **不存在**(404);`wadm` 是 wasmCloud 的东西,与 Doom 无关。
注意:Boom/MBF/MBF21 的扩展行为终归要自己实现,crate 只解决 Vanilla 级解析。

## 5. 兼容等级:规范与参考实现

关键发现:**Doom Cross Port Collab 组织**(https://github.com/doom-cross-port-collab)已成为 Boom/MBF
谱系的规范之家,托管:`mbf21`(spec)、`id24`(spec)、`umapinfo`(spec)、`boom`(**Boom 2.02 源码**)、
`mbf`(**MBF 2.03 源码**)、`issue-tracker`(跨 port 兼容性 bug 追踪)。`elf-alchemist/boom-standards`
已归档(2026-06)并指向该组织。

### Vanilla(必须)
- 语义基准:https://github.com/id-Software/DOOM(linuxdoom-1.10)——有 bug 但权威。
- 行为校验清单:**Chocolate Doom**(https://github.com/chocolate-doom/chocolate-doom),
  `PHILOSOPHY`/`NOT-BUGS` 文档:"复刻 DOS Doom,包括 bug"。

### Limit-Removing(必须)
- 基准:**Crispy Doom**(https://github.com/fabiangreffrath/crispy-doom):"limit-removing,
  enhanced-resolution, 基于 Chocolate",保持 vanilla 存档/demo 兼容。
- 技术含义(doomwiki "Limit removing"):解除静态数组上限——VISPLANE 溢出(VPO)、SPECHITS、
  NOFIT、MAXPLATS、MAXOPENINGS、dropoff/thing-block 表、存档缓冲等。demo 由输入序列定义,
  解除上限不破坏 vanilla demo 回放。

### Boom / MBF(必须;无 spec,代码为准)
- Boom 2.02 源码:https://github.com/doom-cross-port-collab/boom(原始发行:idgames
  `themes/TeamTNT/boom/boom202s`,1998-10-22)。附带最接近 spec 的文档:**BOOMREF.TXT /
  BOOMDEH.TXT / BOOMLUMP.TXT**。
- MBF 2.03 源码:https://github.com/doom-cross-port-collab/mbf(Killough,直接构建于 Boom 2.02 之上)。
- complevel 语义表:prboom-plus `doc/prboom-plus-usage.txt`:0=Doom 1.2,2=Doom 1.9,3=Ultimate,
  4=Final Doom,**9=Boom 2.02**,**11=MBF**,**21=MBF21**(后加)。
- ⚠ coelckers/prboom-plus 已归档(2023-06,终版 2.6.66),只读快照;继承者 **dsda-doom**
  (https://github.com/kraflab/dsda-doom)。

### MBF21(目标;spec-first)
- 正式 spec:**https://github.com/kraflab/mbf21**(镜像 doom-cross-port-collab/mbf21),
  `docs/spec.md`(modder 向)+ `docs/developer_spec.md`(port 开发者向,带代码链接)+
  `docs/level_editor_spec.md` + `docs/options.md`。当前 **v1.4**。
- 相对 MBF 增量:广义 sector type(位 12/13)、linedef flags、line type 1024–1026、DEHACKED
  thing groups 与新 thing flags(LOGRAV/NORADIUSDMG/RIP/FULLVOLSOUNDS)、weapon flags、
  state Args1–8 + SKILL5FAST、**约 50 个新 codepointer**(A_SpawnObject、A_MonsterProjectile、
  A_SeekTracer、A_JumpIf*、A_WeaponBulletAttack…)、comp 选项(comp_ledgeblock 等)、OPTIONS lump。
- 实现者:dsda-doom(首个)、**Woof!**(默认 complevel)、Eternity 4.04.00、Doom Retro。
- DEHEXRA 相关但独立,MBF21 port 通常捆绑。

### ID24(名字所系;spec-first,最后实现)
- 正式 spec:**https://github.com/doom-cross-port-collab/id24**(0.99.1 / 0.99.2 / markdown 版 /
  community_version / source_code_reference)。Xaser Acheron 为 2024 KEX 官方重制版(Legacy of Rust)
  设计,源于 Rum and Raisin Doom 的 RnR24,**公开 spec 为净室重写**。
- 范围 = vanilla + Boom + MUSINFO + MBF + DEHEXTRA + MBF21 + DSDHacked(ID24HACKED)+ UMAPINFO,
  另加:JSON 控制 lump(DEMOLOOP/GAMECONF/SBARDEF/SKYDEFS)、UMAPINFO 扩展、任意面贴任意纹理/平坦、
  地板/天花纹理偏移与旋转(2048–2056)、音乐切换器(2057–2098)、reset 出口(2069–2074)、sector 染色
  (2075–2081)、双 sidedef 滚动(2082–2086)、负 editor number 新怪物等。
- ⚠ **明确与 Boom/MBF21 不完全兼容**(demo、武器行为、火焰速度、彩色血、sector 666)。
- 成熟度:截至 2026 仍 **v0.99.2 草案**(repo 活跃,2026-05 有 push,12 open issues),但有商业发行版背书。
- 实现者:Woof! 15.0.0→16.0.0(最完整的社区实现)、Helion 0.9.5.0、Doom Retro 5.6、Odamex 11、KEX 本尊;
  **dsda-doom 尚不支持**(kraflab/dsda-doom#893)。"complevel 32" 说法未在 spec/发布说明中得到证实,勿采信。

## 6. Demo(LMP)格式要点

- **Vanilla v1.9**:头 = skill/episode/map/deathmatch/respawn/fast/nomonsters/consoleplayer 8 字节 +
  4 字节视角度量;之后每 tic 4 字节 ticcmd(forwardmove/sidemove/angleturn+strafe高位/buttons)。
  v1.2(complevel 0)头更短。
- **Longtics**:ticcmd 内 16 位转身(来自 v1.10 beta,Chocolate/prboom 系以 `-longtics` 暴露)。
- **Boom**:扩展头 **109 字节**(vanilla 13 + 版本/complevel 字段,约 0x1c/0x1d 处);**MBF** 再扩展
  (demo version 字节 203 + complevel 字段)。字节图:doomworld 主题帖 72033。
- **MBF21**:complevel 21 demo,MBF 式扩展头;OPTIONS lump 状态与 demo 交互。
- ⚠ **UMAPINFO 陷阱**:prboom-plus 在所有 complevel 强制扩展 demo 头(prboom-plus#522)——失同步源。
- **ID24**:新增 DEMOLOOP(自定义 demo/cast lump)与 GAMEVERS。
- 跨等级回放要点:录制 complevel 必须与回放一致;自动探测可能误判(pwad 强制 Ultimate → v1.9 demo
  失同步,需手动 `-complevel`);Final Doom 常量与 Doom2 1.9 有差异;Boom 仅在 vanilla 兼容模式下播
  vanilla demo。

**参考**:主 = Chocolate Doom `d_main.c`(vanilla + longtics 的规范实现);副 = dsda-doom
(0→21 全 complevel 自动探测、严格模式、TAS 工具,整个 port 以 demo 为中心)。

## 7. Uncapped FPS / 自定义分辨率 / 宽高比的实现技法

- **架构共识(prboom+ 引入,dsda/crispy/woof 沿用)**:模拟恒定 **35 tics/s**;渲染按显示刷新率跑,
  对最后两个 tic 之间做**插值**(mover/automap 均有插值修正)。woom24 对应:`TIC`(整数精确)与
  render 解耦,mobj/player 保留插值快照。
- **分辨率**:prboom+ 任意窗口分辨率(渲染列函数按分辨率参数化)——比 Crispy 的固定 640×400 内部
  分辨率更适合 woom24 的"自定义分辨率"目标。
- **宽高比**:vanilla 是 320×200 经 mode-13h 非方像素 → 4:3(等效 320×240 方像素)。Crispy 做方像素
  4:3 校正 + 宽屏至 24:9(垂直 FOV 信箱);prboom+/Woof 渲染额外水平视野。woom24:精确 4:3 校正为
  正确性基线,宽屏为纯渲染侧扩展(永不进模拟)。

## 8. 兼容矩阵(port × complevel)

| Port | URL | 语言 | Vanilla | Limit-removing | Boom | MBF | MBF21 | ID24 |
|---|---|---|---|---|---|---|---|---|
| Chocolate Doom | github.com/chocolate-doom/chocolate-doom | C | ✓(基准) | ✗(有意) | ✗ | ✗ | ✗ | ✗ |
| Crispy Doom | github.com/fabiangreffrath/crispy-doom | C | ✓ | ✓(基准) | ✗ | ✗ | ✗ | ✗ |
| PrBoom+(冻结) | github.com/coelckers/prboom-plus(2023 归档) | C | ✓ | ✓ | ✓(cl 9) | ✓(cl 11) | ✗ | ✗ |
| dsda-doom | github.com/kraflab/dsda-doom | C | ✓ | ✓ | ✓(cl 9) | ✓(cl 11) | ✓(首个,cl 21) | ✗(#893) |
| Woof! | github.com/fabiangreffrath/woof | C | ✓ | ✓ | ✓ | ✓ | ✓(默认) | ✓ 部分(15→16) |
| Helion | github.com/Helion-Engine/helion | C# | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ 部分(0.9.5.0) |
| Eternity | github.com/team-eternity/eternity | C | ✓ | ✓ | ✓ | 部分 | ✓(4.04.00) | ✗ |

(ZDoom 系按项目策略排除;Helion 虽是 C#,作为语义参考与 Rust 移植的架构镜像仍有阅读价值。)

## 9. 决策记录(2026-09-17 起;已决条目回填 AGENTS.md 的 Decision Log)

1. **上游策略与许可证 — ✅ 已决:Fork `sunsided/room` 直接开发**,GPL-2.0 传染被接受。
   理由:room 的代码与逐位回归测试是完美脚手架;DOOM 社区的传统本就是开源互相学习——严肃 port 生态
   (chocolate/crispy/prboom+/dsda/woof/doomgeneric)全部 GPL-2.0+,GPL 化在此领域几乎没有额外代价。
   执行纪律见 AGENTS.md "Fork Discipline":保留 upstream remote 定期合并、不重写历史、模块布局与命名
   尽量不动以降合并摩擦、通用改进(含 WASM 层本身)保持可上游化并回馈 PR。
   fork 后的里程碑 ①验证 demo 回放现状并用黄金测试钉死;②把 winit/wgpu/rodio/rustysynth 平台层收拢为
   shell trait,原生 shell 留作开发、web shell 新建。
2. **WAD 解析 — ✅ 已决**:以 room 自带的 `w_wad`(Rust 移植)为基,按 Boom/MBF/MBF21/ID24 扩展自研;
   **不引入第二套解析器**。`crustyview` 作为浏览器端 WAD 加载 UX(file picker/IndexedDB)与 automap
   渲染的借鉴对象;其底层 crustywad 为 MIT/Apache,若某天确需引入也不受许可证阻碍,但默认不引。
3. **WASM 目标 — ✅ 已决:`wasm32-unknown-unknown` + wasm-bindgen**。理由:该目标兼容性最强;
   DOOM 生态除系统 I/O 交互外与底层几乎无耦合,此目标难度合理。Emscripten 出局,Dwasm 仅作部署形态
   (单文件 HTML)参考。
4. **渲染后端 — ✅ 已决:默认 WebGL2 + Canvas2D 兜底;WebGPU 作实验性实现**(feature flag 隔离)。
   WebGPU 仍在发展、实现确实不简单,但值得尝试;三种后端全部藏在渲染 trait 之后,core 不感知具体后端。
5. **兼容等级路线图确认**:Vanilla(含 demo 黄金测试)→ Limit-Removing → Boom(cl 9)→ MBF(cl 11)
   → MBF21(cl 21)→ ID24(最后,DEHACKED 层按 DSDHacked-ready 设计)。
6. **音频 — ✅ 已决:MIDI 优先(FluidSynth 角色),之后逐层 fallback**。实现取 room 现成的 rustysynth
   (纯 Rust SF2 软合成,即 FluidSynth 的同位素)——C 版 FluidSynth 与 unknown-unknown 及"无 C 依赖"
   约束冲突,不引入。fallback 链:rustysynth + 用户自载 SF2 → 内置 OPL2 仿真(免外部资源)→ 静音。
   SF2/MIDI 资产经配置 UI 加载,运行时不 fetch。
7. **wasm 入口 API — ✅ 已决:双显式导出,行为不同故必须分离**(AGENTS.md "Web Entry Contract"):
   - 最少参入口:设备元信息(至少含宿主允许的最大渲染分辨率,用于限制引擎 framebuffer 上限)+ 已加载的
     IWAD → 进入**启动器模式**:先出配置 UI,用户自行加载 PWAD、SF2/MIDI 及选项,之后才开局。
   - 标准入口:完整 boot profile(IWAD、PWAD 及顺序、选项、资产)→ 跳过配置 UI 直接开局,供自带 UI 的
     宿主嵌入。
   - 分离只发生在导出契约面:内部两条入口汇入同一条 init 流水线,仅 boot profile 不同——绝不允许出现
     两套分叉的初始化逻辑。PWAD/资产集合在 boot 时冻结,游戏中途换资产属确定性风险,未设计前不做。

## 10. Spec-first 阅读顺序(实现时的动作清单)

1. `id-Software/DOOM`(linuxdoom-1.10)——一切语义的地面真相。
2. Chocolate Doom `PHILOSOPHY`/`NOT-BUGS`——vanilla 正确性清单;`d_main.c` demo 代码(vanilla/longtics)。
3. Crispy Doom——limit-removing 解除哪些限制、640×400 + 宽高比校正的最小实现。
4. `kraflab/mbf21` spec.md + developer_spec.md——唯一必须的正式规范;对照 dsda-doom/Woof! 验证。
5. Boom 2.02 源码 + BOOMREF/BOOMDEH/BOOMLUMP + prboom-plus complevel 表——cl 9 语义;
   MBF 2.03 源码——cl 11 增量。
6. Demo 栈:Chocolate(vanilla/longtics)→ prboom+/dsda-doom 自动探测(Boom 109 字节头、MBF 203 头、
   cl 21)+ UMAPINFO 失同步陷阱。
7. ID24 0.99.2——最后实现;DEHACKED 层按 DSDHacked-ready 设计,以 Woof! 16.x 为工作参考,预期规范仍会变。
