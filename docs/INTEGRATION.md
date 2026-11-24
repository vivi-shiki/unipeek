# 接入 Snowcase

日期 2026-11-23。本库（unipeek）如何接进 [Map1en/Snowcase](https://github.com/Map1en/Snowcase)。需求见 [REQUIREMENTS](REQUIREMENTS.md)，调研见 [PRIOR_ART](PRIOR_ART.md)。

标记：**✓** 已核实（读过对方源码、Cargo.lock、CI 配置、文档，或 GitHub API 返回值）；**～** 推断，没有实测。对方代码以 Snowcase `master` 的 `1f0fe8d`、Snowcase-docs `master` 的 `ab8e461` 为准。

## 结论

1. **对方还没有任何 3D、缩略图代码，接口可以由我们先定。** Snowcase 现在做到库扫描、包文件树、BOOTH 链接；README 的 TODO 里「Thumbnails」「3D model preview」都没勾 ✓。
2. **第一步用读回就够了，不需要 DComp 外挂。** 详情是固定 460 px 宽的右侧抽屉 ✓，按 150% DPI 算约 690² 物理像素；lab 实测 1100×760 读回每帧 3.4 ms，512² 只多约 0.6 ms ✓。零拷贝留给以后的大窗口或全屏查看。
3. **库不依赖 GPUI。** GPUI 版本由 Snowcase 锁定（gpui-kit 0.9.1 → gpui-pre 0.3.12 ✓），我们只出 RGBA 帧和「画到某个 DComp visual / HWND」的接口，GPUI 胶水代码写在 Snowcase 的 `app` 里。
4. **crate 要拆成「解析」和「渲染」两层，和 Snowcase-docs 已定的方案对齐。** RENDER_REQUIREMENTS 已经定了 Snowcase 的 `formats` 会依赖渲染库里「只管解析、不依赖 GPU 的那两个 crate」✓，这和我们 REQUIREMENTS 里「先发一个 crate 用 feature 开关」冲突，以对方为准。
5. **`Resolver` 要按批、按包设计。** Snowcase 的 `cache.db` 已有 GUID → 包的索引，但不存条目内容 ✓；`.unitypackage` 是 tar.gz，没法随机读，按 GUID 一个个取字节会把同一个包重读很多遍 ～。
6. **Snowcase 直接把本库当外部依赖调用，接入代码由 Map1en 写。** 本库的交付物是公开 API 和版本号；不往 Snowcase 仓库开 issue、提 PR，所以 CONTRIBUTING 的「先开 issue」和 CLA 都不涉及本库（2026-11-24 Vivi 确认）。下面「接入决定」里的几条是和 Map1en 约定的 API 边界。

## 接入决定（M0，2026-11-24）

Vivi 定下，需要告诉 Map1en。开发路线见 [ROADMAP](ROADMAP.md)。

| # | 决定 | 理由 | 被否掉的做法 |
|---|---|---|---|
| 1 | **两个 crate，同一个仓库**：`unipeek-unity`（解析：unitypackage 索引、Unity YAML、prefab → `.mat` → 贴图、`Report`，不依赖 GPU）和 `unipeek`（渲染，依赖并重新导出 `unipeek-unity`）。Snowcase `formats` 依赖前者，`app` 依赖后者 | `formats` 不该被迫编译 wgpu 和 ufbx，CI 也没有 GPU；crate 越多，两边要同步的版本越多。以后真要细拆，可以在 `unipeek-unity` 里重新导出，不破坏调用方 | 一个 crate 用 feature 开关；按 Snowcase-docs 字面拆三个 |
| 2 | **缩略图子进程是 Snowcase 自己的 exe**（`snowcase.exe --thumb-worker`），里面跑库函数 `unipeek::serve(stdin, stdout)` | 详情预览本来就要在主进程链接 wgpu 和 ufbx，单独发 exe 省不了什么；只发一个 exe，版本永远一致；驱动崩溃照样被隔离 | 单独发 `unipeek.exe` |
| 3 | **交互预览第一版只给 `Viewport::offscreen` 读回**（BGRA，给 GPUI 的 `RenderImage`） | 460 px 抽屉读回约 1 ms ✓；不依赖 gpui-pre 内部细节，不怕 #66676 把 DComp target 改回 `topmost = true` | 第一版就做 DComp / 子 HWND 零拷贝（以后做全屏查看时再加，只是多一个构造函数） |
| 4 | **缩略图：每个商品自动一张，单个 prefab 按需**。本库负责从一个包里自动选出「主 prefab」；Snowcase 只给可见商品排队，跟在「探查包」之后跑。包文件树里的 prefab 在用户点开或悬停时才渲，之前先用包里的 128 px `preview.png` | 封面优先级里 3D 排最后（WORKFLOWS：BOOTH 图 → 压缩包宣传图 → `preview.png` → 3D）✓，批量渲染全部 prefab 占机械硬盘和 GPU，收益低。对本库来说两种情况是同一个函数，只是多一个包内路径 | 后台批量渲染每个 prefab |
| 5 | **首个发布不做转台帧** | 详情里已能交互旋转；缩略图的 `view` 和 `size` 参数设计好，以后转台帧就是循环调用，不改 API | 按 RENDER_REQUIREMENTS 列为 P1 |

## 对方现状

### 仓库和流程

| 项 | 现状 | |
|---|---|---|
| 许可 | GPL-3.0-only，workspace 统一 | ✓ |
| CLA | contributor-assistant v2.6.1，PR 里评论一句话签名，签名记在 `signatures/version1/cla.json`；`Map1en` 和 bot 在白名单 | ✓ |
| CLA 内容 | 不转让版权，但授予维护者以任何许可再授权的权利；第三方材料要标明来源和许可，与再授权不兼容的材料不能提交 | ✓ |
| CONTRIBUTING | 早期开发、变化快；超过小修先开 issue；不要放付费素材、解包内容；AI 写的代码自己要读懂 | ✓ |
| PR 模板 | What does this PR do / How to test / Screenshots | ✓ |
| issue / PR | 目前一个都没有（`gh issue list`、`gh pr list` 都为空） | ✓ |
| 活跃度 | 5 星；主仓库和文档仓库 2026-11-23 当天都有推送 | ✓ |
| 文档 | Snowcase-docs 单独一个仓库；`master` 之外还有一个 `docs/3d-preview-research` 分支，里面是 3D_PREVIEW.md（2026-11-14），对应的 PR #1 当天关闭、没合并、没有评论 | ✓ |
| RENDER_PLUGIN.md | Vivi 2026-11-14 写的渲染库设计（`b5bce38`），11-15 被 `a5cf175` 改写成 RENDER_REQUIREMENTS.md，已进 `master`；原文只在 git 历史里 | ✓ |

### crate 和代码

workspace 共 6 个 crate，约 1.07 万行 Rust ✓：

| crate | 职责 | 和我们相关的地方 |
|---|---|---|
| `formats` | 文件类型、zip / 7z 目录、unitypackage 内部、指纹和 CRC | `list_unitypackage(impl Read)` 流式读 tar.gz，只记每个 GUID 的 `pathname`、有没有 `asset`、有没有 `preview.png`，**不读 `.meta`、`.mat`、prefab 内容** ✓ |
| `store` | `library.db`（Diesel）和可重建的 `cache.db` | `cache.db` 有 `package_asset(fingerprint, member, guid, path, is_file, has_preview)`，就是现成的 GUID → 包索引 ✓；`cache.db` 自带版本号（现为 1），可删重建 ✓ |
| `core` | 快照、唯一写入端 `Library`、入库规则、后台任务（指纹、CRC、读包） | 缩略图生成会是这里的一类新后台任务 ～ |
| `booth` | BOOTH 公开页 | 无关 |
| `app` | GPUI 界面和编排 | 网格封面用 `img(path)` 显示文件（`library_view/widgets.rs:81`）✓；详情抽屉 `DETAIL_WIDTH = 460.`（`library_view/detail/mod.rs`）✓ |
| `testing` | UI 测试框架、测试数据 | 只做 dev-dependency |

依赖方向 `formats ← store ← core ← app`，`booth ← app` ✓（CODE_STRUCTURE.md）。

### 版本和构建

| 项 | Snowcase | 本库（lab） | |
|---|---|---|---|
| gpui-kit | 0.9.1 | 0.9.1 | ✓ |
| gpui-pre / gpui-pre-windows | 0.3.12 | 0.3.12 | ✓ |
| wgpu | 30.0.1（Cargo.lock 里有，来自 gpui-pre-wgpu / gpui-fast-wgpu） | 30.0.1 | ✓ |
| image | 0.25.10 | 0.25 | ✓ |
| tar / flate2 / zip / sevenz-rust2 / encoding_rs / blake3 | 0.4.46 / 1.1.11 / 9.8.1 / 0.25.0 / 0.8.44 / 1.8.7 | 本库以后要用，版本对齐即可 | ✓ |
| Rust 版本 | 没有 rust-toolchain，CI 用 stable；edition 2024 | 锁在 1.99.1；edition 2024 | ✓ |
| CI | windows-latest：`cargo test --workspace --locked`、`cargo build -p snowcase`、`--smoke` 冒烟启动；另一个 job 检查 Diesel schema | 无 | ✓ |

gpui-pre-wgpu 大概是 Linux 后端，Windows 上 GPUI 走 D3D11，所以 Snowcase 的 Windows 构建现在可能根本不编译 wgpu；接入后会多出 wgpu 和 ufbx（C 代码）的编译时间 ～。windows-latest 自带 MSVC，ufbx 的 C 编译不用额外配置 ～。

### 文档里已经定了的

| 决定 | 出处 | |
|---|---|---|
| 渲染库单独建仓库，MIT OR Apache-2.0；工作名 `snowcase-render` | DECISIONS 2026-11-21；RENDER_REQUIREMENTS「待定」 | ✓ |
| Snowcase 决定渲染什么（哪个文件、哪个 prefab、配哪个素体），负责 GUID 查询、缓存索引和界面 | RENDER_REQUIREMENTS「范围」 | ✓ |
| `formats` 复用渲染库的 unitypackage / Unity YAML 解析，依赖其中两个不碰 GPU 的 crate，按版本号锁定，升级时两边一起跑测试；编译期依赖，不做运行时开关 | RENDER_REQUIREMENTS「待定」 | ✓ |
| 3D 路线：离线缩略图（覆盖 90%）→ 详情页读回 `RenderImage` → 退路独立窗口 | TECH_STACK「3D 预览路线」 | ✓ |
| 缩略图放库目录的 `thumbs/`，属于缓存，可删可重建；图片来源有 `package`（包内预览） | DATA_MODEL | ✓ |
| 后台任务以机械硬盘为底线：顺序读、同盘不并发、可暂停续跑、部分结果立即可用、单个失败不中断 | DECISIONS 2026-11-20 | ✓ |
| 错误信息和日志用英文；不引入 GPL 依赖 | DECISIONS | ✓ |
| P1 首个发布要有「包文件树、缩略图、白模预览」 | ROADMAP | ✓ |

3D_PREVIEW.md（未合并分支）里的「自建 topmost DComp」方案、缩略图缓存键（资产 blake3 + 材质集合 hash + 渲染器版本 + 视角）和 S0–S4 原型阶段，是 preview-lab 的出发点 ✓，但它的 PR 已关闭、没有进 `master`。M0 决定第一版只用读回（见「接入决定」第 3 条），所以不再依赖这份文档。

## 接入方式

### 依赖关系

```
unipeek-unity   解析：unitypackage 流式索引、Unity YAML、prefab → .mat → 贴图、报告    不依赖 GPU
unipeek         渲染：wgpu、ufbx、缩略图、Viewport；feature `cli` 出命令行和 serve       依赖 unipeek-unity

Snowcase formats  ──依赖──▶ unipeek-unity        （以后替换自己的 list_unitypackage）
Snowcase app      ──依赖──▶ unipeek              （缩略图任务、详情预览、GPUI 胶水）
```

- Snowcase 文档说的是「两个」解析 crate ✓，具体怎么拆没写。建议先拆成上面这两个，`unipeek-unity` 里再用模块分包格式和 YAML；如果对方坚持要两个解析 crate，再把包格式拆成 `unipeek-package` ～。
- 依赖方式：开发期用 git 依赖锁 rev，发了 0.1 以后改成 crates.io 版本号。Snowcase CI 用 `--locked`，所以每次升级要在对方仓库提交 Cargo.lock ✓。
- 许可方向只能单向：MIT / Apache 代码进 GPL 的 Snowcase 没问题；**反过来不行**，不能把 Snowcase 的 `list_unitypackage` 等 GPL 代码抄进本库，除非 Map1en 把那部分另外授权给我们（他是版权方，CLA 也给了他再授权的权利）✓。

### 接口

| 接口 | 形状（建议） | Snowcase 怎么用 |
|---|---|---|
| 输入 | `Source::Path(PathBuf)`、`Source::Reader(Box<dyn Read + Send>)`、`Source::Bytes(Vec<u8>)`，加可选的包内路径（`Avatar.unitypackage#Assets/…/Avatar.prefab`） | 普通文件给路径；zip 里的 unitypackage、以后的冷归档给 Reader |
| `Resolver` | `fn locate(&self, guids: &[Guid]) -> Vec<(Guid, Located)>`，`Located` 是「某个包文件（路径或 Reader 工厂）+ 包内 GUID」或「工程里的某个文件」；库按包分组，每个包只读一遍 | 实现时查 `cache.db` 的 `package_asset`，再用 `library.db` 把 `fingerprint` 映射回磁盘路径 ～ |
| 报告 | `Report`（`serde::Serialize`）：三角形、骨骼、BlendShape、材质和 shader、贴图尺寸、缺失 GUID、降级原因列表 | 详情抽屉里显示；以后做「项目健康检查」 |
| 缩略图 | `render_thumbnail(&Scene, ThumbOptions { size, view, background }) -> RgbaImage`；另出 `RENDERER_VERSION` 常量 | 编码成 PNG / WebP 写进 `thumbs/`，网格照旧 `img(path)` |
| 交互视口 | `Viewport::offscreen(w, h)` → `render() -> &[u8]`（BGRA，给 `RenderImage`）；以后加 `Viewport::composition(visual)` 和 `Viewport::hwnd(hwnd)` | 第一步只用 offscreen，Snowcase 自己转 `RenderImage` |
| 相机和输入 | `Orbit` 的 `rotate / pan / zoom / reset / preset(View)`，由调用方喂鼠标增量 | GPUI 的鼠标事件转成这几个调用 |
| `serve` | stdin / stdout 一行一个 JSON：`{"op":"thumb","source":…,"out":…}` | 见下面「缩略图子进程」 |

缓存键由 Snowcase 拼：`fingerprint + member + 包内路径 + 解析出的依赖 GUID 集合 + RENDERER_VERSION + 视角 + 尺寸`。Snowcase 的快速指纹是「大小 + 首尾 hash」✓，不是全量 blake3，够做缓存键 ～；`RENDERER_VERSION` 一变，旧缩略图就失效重画。

### 缩略图子进程

需求里说缩略图走子进程，用来隔离显卡驱动崩溃（3D_PREVIEW「坑」：作者本机 UHD 630 跑 wgpu DX12 直接段错误 ✓）。两种做法：

| 做法 | 好处 | 坏处 |
|---|---|---|
| **snowcase.exe 自己带 `--thumb-worker` 参数，以子进程方式启动自己**（建议） | 只发一个 exe；版本永远一致 | snowcase.exe 要链接 wgpu 和 ufbx |
| 单独发一个 `unipeek.exe`，Snowcase 调它的 `serve` | Snowcase 主程序不链接渲染代码 | 安装包多一个文件，两个版本要对上 |

不管哪种，子进程里跑的都是本库的 `serve` 循环，所以 `serve` 要做成库函数（`unipeek::serve(stdin, stdout)`），不只是 CLI 的一部分 ～。调度（只排可见项、可暂停、同盘不并发）是 Snowcase 的事：按 CODE_STRUCTURE，启动子进程这类编排在 `app::library_store`，写结果在 `core::library::jobs`；缩略图要用包索引，适合做成「整理文件」「探查包」之后的第三段 ✓。我们只保证单个任务对一个包顺序读一遍。

已定第一种（见「接入决定」第 2 条）。

### 详情页预览

| 阶段 | 做法 | 依据 |
|---|---|---|
| 第一步 | 抽屉里放一个固定尺寸的读回视口：`Viewport::offscreen`，相机动了才渲染，每帧一个新 `RenderImage` | 460 px 抽屉的读回开销约 1 ms ✓；不依赖 gpui-pre 内部细节 |
| 以后 | 大窗口或全屏查看时换零拷贝：在 gpui-pre ≥ 0.3.12 上用 topmost DComp（lab 的 `overlay`）或窗口内子 HWND | PRIOR_ART「对已有方案的修正」：0.3.12 起 GPUI 的 target 是 `topmost = false` ✓；上游 #66676 如果合并并打开，会改回 `true` ✓ |
| 退路 | 独立窗口（本库的 `view`） | TECH_STACK「退路」✓ |

贴图加载和 mip 生成在 lab 里要 6–9 秒、跑在 UI 线程上 ✓，接进 Snowcase 前必须挪到后台线程，界面先显示 `preview.png` 或白模 ～。

## 谁改什么

### Snowcase 那边（Map1en 写）

| 改动 | 位置 | 阶段 |
|---|---|---|
| 加依赖 `unipeek`（缩略图、预览）和 `unipeek-unity`（以后给 `formats`） | workspace `Cargo.toml` | 接缩略图时 |
| 缩略图任务：排队、调子进程、写 `thumbs/`、记索引 | `app::library_store`（编排、子进程）、`core::library::jobs`、`store::cache`（加一张表，`cache.db` 版本号 +1） | P1 |
| 网格封面优先用生成的缩略图，没有时退回 `preview.png` 和现在的封面顺序 | `app::library_view::grid` | P1 |
| 详情抽屉加 3D 预览和报告 | `app::library_view::detail` 下新模块 | P1 |
| 实现 `Resolver` | `core` 或 `app`，读 `cache.db` + `library.db` | P1 降级，P2 补全 |
| `formats::list_unitypackage` 换成 `unipeek-unity` 的索引 | `formats` | 本库解析稳定以后 |
| CI：构建时间变长；测试不能要求有 GPU | `.github/workflows/ci.yml` | 接入时 |

### 本库这边

| 改动 | 现状 | 顺序 |
|---|---|---|
| 许可改成 MIT OR Apache-2.0，加两份 LICENSE | 已做（本次提交） | ✓ 完成 |
| 拆 crate：`lab-render` → `unipeek-unity` + `unipeek`；`preview` 留作示例，不发布 | 现在是 `lab-render` 和 `preview` | 1 |
| 输入改成 `Source`，支持 `Read` 和包内路径 | 现在只接文件路径（`Model::load_fbx(&str)`） | 2 |
| unitypackage 流式索引 + prefab → `.mat` → 贴图绑定（移植 popone） | 现在按材质名猜贴图（`texture::guess_texture`） | 3 |
| `Report` 和降级原因 | 没有 | 3 |
| 缩略图库函数 + CLI `thumb` + `serve` | 只有 `examples/thumb.rs` | 4 |
| 贴图加载移出 UI 线程，大贴图先缩到 ≤1024 | `MAX_SIZE = 2048`，同步加载 | 4 |
| `Viewport::offscreen` 公开接口，输出 BGRA | lab 的 `readback` 模式已验证 | 5 |
| 解析部分的测试不依赖 GPU；Rust 版本不高于 stable | 锁在 1.99.1 | 一直 |

## 先后顺序

分成 M0–M6 七个里程碑，每步的详细需求、完成标准和对 Snowcase 的依赖见 [ROADMAP](ROADMAP.md)。

## 风险

| 风险 | 影响 | 对策 |
|---|---|---|
| Snowcase 架构还在快速变化，CONTRIBUTING 明说大 PR 可能被拒 ✓ | 接入代码白写 | 接入代码由 Map1en 写，我们只保证 API；GPUI 胶水尽量薄，主要逻辑留在本库 |
| 两边各有一份 unitypackage 解析，直到第 6 步 | 行为不一致（比如 `pathname` 只取首行、跳过非 GUID 目录，对方已经这样做 ✓） | 本库的测试覆盖对方现有的行为；第 6 步时两边一起跑测试 |
| 不能抄对方的 GPL 代码 | 写重复代码 | 自己写，或请 Map1en 把相关文件另外授权 |
| gpui-pre 升级把 DComp target 改回 `topmost = true`（#66676）✓ | 零拷贝方案失效 | 第一步只用读回；零拷贝启动时自检，失败退回读回 |
| 本库被 Snowcase 的 gpui-kit / wgpu 版本拖住 | 升级不同步 | 本库不依赖 GPUI；wgpu 大版本跟随 Snowcase 锁的版本 ～ |
| ufbx 是 C 库，wgpu 编译慢 | Snowcase CI 和开发者本机构建变慢 | 解析层不带 GPU；Snowcase 只在 `app` 里依赖渲染层 |
| 显卡驱动崩溃 | 拖垮 Snowcase 主进程 | 缩略图放子进程；DX12 失败退 Vulkan，记住结果 |
| 机械硬盘上并发读 | 违反对方的后台任务原则 ✓ | 本库单任务只顺序读一遍包；并发由 Snowcase 调度 |
| Rust 版本 | 本库锁 1.99.1，对方用 stable ✓ | 本库标明 `rust-version`，不用比 stable 新的特性 |
