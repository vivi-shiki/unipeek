# 开发路线

日期 2026-11-24。本库从 M0 到 M6 的里程碑，每步写清要交付什么、做到什么算完成、依赖 Snowcase 哪部分。需求见 [REQUIREMENTS](REQUIREMENTS.md)，接入约定见 [INTEGRATION](INTEGRATION.md)。

标记：**✓** 已核实；**～** 推断或待验证。

**Vivi 的决定（2026-11-26）**：

1. Snowcase 侧的接入代码暂时不管：不往 Snowcase 提任何东西，M3/M4/M5 里「Map1en 写」的部分和 M6 的 Snowcase `formats` 换库都先跳过，本库只做自己能做的部分。
2. PSD 合成图读取放进首个版本（[Project.md](../Project.md) 里标 ★），从 M6「更多输入」挪到 M1 的输入里。
3. 完成 M1–M6 中本库能做的开发。

## 结论

1. **Snowcase 的 P1（首个发布）只差「缩略图」和「白模预览」两项，都是本库的事。** 本库在关键路径上。
2. **M1、M2 不依赖对方，可以马上做。** M2 做完就发第一个可依赖的版本（M3），Map1en 接上就是 P1「缩略图」；M4 做完他接上就是 P1「白模预览」。
3. **M3 发到 crates.io，Snowcase 按名字 `unipeek` 拉取**（Vivi 2026-11-24 的打算）。crates.io 上的源码是公开的，`repository` 指向 GitHub 仓库，所以发布前仓库也该公开 ～。发布前的过渡期可以用 git 依赖，但 Snowcase 是公开仓库，CI 拉不到私有仓库 ～。
4. **接入代码由 Map1en 写。** 本库只交付 API、版本号和用法说明，不往 Snowcase 提 PR。

## Snowcase 现在的进度

以 Snowcase `master` 的 `1f0fe8d`（2026-11-23）、Snowcase-docs 的 `ab8e461` 为准 ✓。

| 部分 | 状态 | 和本库的关系 |
|---|---|---|
| 库扫描、分组、后台指纹和 CRC | 已完成 | 无 |
| 后台读 unitypackage 进 `cache.db`（`package`、`package_asset`） | 已完成 | M5 `Resolver` 的 GUID → 包索引就靠它 |
| 后台任务两段式：「整理文件」→「探查包」 | 已完成（11-23） | 缩略图做成第三段（M3） |
| 详情抽屉、包文件树（`app::library_view::detail::package_tree`） | 已完成 | 预览入口（M4） |
| 包内搜索、店铺识别、合并提示、贴 BOOTH 链接 | 已完成 | 无 |
| 按名字识别类型和适配 | 撤回，不维护内置词表 | 配素体只能靠 GUID（M5） |
| 包内 `preview.png` 做封面 | 未做 | Map1en 那边的封面顺序，缩略图排在它后面 |
| 缩略图、3D 预览 | 未做，代码里没有任何 3D | 本库 M2–M4 |
| Unity 工程扫描（P2） | 未开始 | M5 的工程内依赖要等它 |

## 总览

| 里程碑 | 内容 | 依赖 Snowcase | 交付后 Snowcase 能做 | 状态 |
|---|---|---|---|---|
| M0 | 接入约定 | 无 | — | 已定，待告诉 Map1en |
| M1 | 解析层 `unipeek-unity` | 无 | — | 完成（2026-11-24），prefab 对 FBX 实例的材质覆盖还没对上 |
| M2 | 缩略图和 `serve` | 无 | — | 完成（2026-11-24） |
| M3 | 第一个可依赖版本；Map1en 接缩略图 | 两段式后台任务 ✓、`cache.db` ✓ | P1「缩略图」 | 已发布 `0.1.0-alpha.2` 到 crates.io（2026-11-24） |
| M4 | 交互视口 `Viewport::offscreen` | 详情抽屉、包文件树 ✓ | P1「白模预览」 | 未开始 |
| M5 | 跨包 `Resolver`（素体） | `package_asset` ✓；工程扫描（P2，未开始） | P1 降级，P2 补全 | 未开始 |
| M6 | 以后：`formats` 换用本库、零拷贝、转台帧等 | 视项目而定 | — | — |

## M0 接入约定

已定（2026-11-24），详见 [INTEGRATION「接入决定」](INTEGRATION.md#接入决定m02026-11-24)：

1. 同一仓库两个 crate：`unipeek-unity`（解析，不依赖 GPU）和 `unipeek`（渲染，重新导出前者）。
2. 缩略图子进程用 `snowcase.exe --thumb-worker`，里面跑 `unipeek::serve`。
3. 交互预览第一版只给 `Viewport::offscreen` 读回。
4. 缩略图每个商品自动一张（本库选主 prefab），单个 prefab 按需。
5. 首个发布不做转台帧。

还要做：

- [ ] 把这五条告诉 Map1en。
- [ ] 把调用说明 [USAGE](USAGE.md) 给 Map1en。

## M1 解析层 `unipeek-unity`

目标：给一个包（或 zip、文件夹），不用 GPU 就能说出「该显示哪个 prefab、用了哪些网格、材质、贴图，缺了什么」。

### crate 拆分

- `crates/render`（`lab-render`）拆成 `crates/unipeek-unity` 和 `crates/unipeek`；`preview` 改为依赖 `unipeek`，仍不发布。
- `unipeek-unity` 不依赖 wgpu、ufbx 以外的 GPU 相关东西；ufbx 留在 `unipeek`（解析层只认 FBX 是哪个文件、被谁引用，不解网格）～。
- 库 crate 用自己的错误类型，`anyhow` 只留在 CLI、示例和 `preview`。

### 输入

| 项 | 要求 |
|---|---|
| `Source` | `Path`、`Reader(Box<dyn std::io::Read + Send>)`、`Bytes`；路径只是拿到 reader 的一种方式 |
| 包内路径 | `Avatar.unitypackage#Assets/…/Avatar.prefab`；`#` 后面按包内 `pathname` 匹配 |
| `.unitypackage` | gzip + tar 流式读，常量内存，3 GB 以上也能扫完；条目顺序不固定，扫完再解析 GUID |
| 和 Snowcase 现有行为一致 | 只认 `<32 位十六进制 GUID>/` 目录；`pathname` 只取首行；去掉 `./` 前缀 ✓。照对方文档描述的行为写测试，不看、不抄对方代码 |
| zip、文件夹里的 FBX / PNG / PSD | zip 没有 UTF-8 标志时文件名按 Shift-JIS 解；PSD 读合成图（2026-11-26 起，见上面的决定 2） |

**读几遍包（2026-11-26 定：两遍）**：原来的目标是一遍。prefab、`.mat`、`.meta` 都是小文本，第一遍全部留在内存；FBX 和贴图在知道要哪些之前不能全留。做法二选一，在本机样本上量过再定：

- 一遍：大文件边读边写进临时的内容寻址缓存，用完删掉。
- 两遍：第一遍索引加 YAML，第二遍只取选中的 FBX 和贴图。

不管哪种，`Report` 里记下实际读了几遍、读了多少字节，Snowcase 据此判断对机械硬盘的影响。

定为两遍 ✓。在一个 249 MB 的头像 zip 上（release）：第一遍索引 160–220 ms，第二遍取 2 个 FBX 和 8 张贴图 165–225 ms，两遍一共读了 68 MB 压缩数据，因为第二遍取齐要的条目就停。一遍的话，要先把包里每个 FBX 和贴图都写进临时缓存，大包要写几 GB，比多读一遍更伤机械硬盘。一次性的 reader 不能重读，仍然在第一遍就把二进制留下（`ReadStats::passes` 为 1）。`Report::read` 记遍数和字节数，`Report::timings` 记每步耗时。

### Unity YAML 和材质绑定

- 读 prefab（含 Prefab Variant 链、嵌套 prefab 实例）、FBX 的 `.meta`（`externalObjects` 材质重映射）、`.mat`（shader GUID、`_MainTex`、`_Color`、lilToon 的阴影、透明、描边参数）。
- 绑定链：prefab → FBX `.meta` → `.mat` → 贴图；找不到时退回按材质名匹配（现在 `texture::guess_texture` 的做法）。
- 从 popone（0BSD）按模块移植，源码注释写明出处 ✓。
- shader 不靠解析 shader 文件：内置一张已知 shader GUID 表（lilToon 各变体、Poiyomi、MToon、Standard），shader 本身不在包里也能认出来 ～。

### 选主 prefab

不给包内路径时，本库自己选。规则要确定、可测试，每条配真实包结构（只用路径结构，不提交付费文件）的用例：

1. 只考虑引用了网格的 prefab。
2. 优先不带颜色或变体后缀、路径最短的（研究里 `agent-unitypackage.md` 的封面选图规则 ✓）。
3. 没有 prefab 时，退到最大的 FBX。

`Report` 里写出选中的那个和其余候选，Snowcase 可以列给用户换。

### Report

`serde::Serialize` 结构体：

- 选中的 prefab 和候选列表
- 网格：三角形数、骨骼数、BlendShape 数
- 材质：名字、shader（认出的种类或 GUID）、绑定的贴图和尺寸
- **外部 GUID**：prefab 引用了、包里没有的 GUID，按用途分（网格 / 材质 / 贴图 / 脚本 / 未知）。这是 M5 找素体的唯一途径
- 降级原因列表（缺素体、shader 未识别、贴图读不了等），降级不是错误
- 读包统计（遍数、字节数、耗时）

### 命令行（feature `cli`）

- `unipeek info <输入> [--json]`、`unipeek ls <输入>`。

### 完成标准

- [x] 本机样本包上，`info` 选出的 prefab 和手工判断一致，贴图绑定比现在的按名字匹配更准（列出对比）。2026-11-26：11 个 unitypackage 共 44 个材质槽，现在 34 个绑上贴图，只按名字匹配是 15 个（`local_samples::bindings_beat_name_matching`）。
- [x] 3 GB 以上的包扫完，内存不随包大小增长。2026-11-26：合成的 3.2 GB 包流式扫完，工作集只涨 1 MB（`big_package_streams`，`UNIPEEK_BIG=1`）。
- [x] `unipeek-unity` 的测试在没有 GPU 的机器上全部通过；测试数据在测试里生成，不提交付费素材。

## M2 缩略图和 `serve`

目标：给一个包，出一张 512² 透明背景的缩略图，能在子进程里批量跑。

| 项 | 要求 |
|---|---|
| 场景 | 渲染输入从 `Model::load_fbx(&str)` 换成 M1 解析出的场景（网格加绑好的材质） |
| 着色 | lilToon 基础：主色、阴影、cutout / 透明、剔除、描边；透明和描边按 shader 变体决定。认不出的显示白模加主贴图，记原因 |
| 缩略图 API | `render_thumbnail(&Scene, ThumbOptions { size, view, background }) -> image::RgbaImage`；`view` 有 front / side / back / 3q；编码 PNG 和 WebP |
| `RENDERER_VERSION` | 公开常量，画面像素一变就加一，Snowcase 拼进缓存键 |
| 贴图 | 解码挪到后台线程；按输出尺寸先缩小（缩略图 ≤1024），不再统一 2048 |
| GPU | 无头创建设备，不建窗口；同一个进程里复用设备；DX12 失败退 Vulkan，并在结果里说明 |
| `serve` | 库函数 `unipeek::serve(impl BufRead, impl Write)`，一行一个 JSON。请求：`{"op":"thumb","source":…,"inner":…,"out":…,"size":512,"view":"3q"}` 和 `{"op":"info",…}`；回复：成功 / 降级 / 不支持 / 出错，加降级原因和简要报告。协议写进文档 |
| 命令行 | `unipeek thumb <输入> -o out.webp [--size] [--view] [--bg]`、`unipeek serve`；退出码区分成功 / 降级成功 / 不支持 / 出错 |

### 完成标准

- [x] 本机全部样本包都能出缩略图（降级也算），记录每个包的耗时和读包字节数。
- [x] `serve` 连续跑全部样本不泄漏内存；子进程被杀或崩溃时，父进程能重启它并接着处理下一个。2026-11-26：同一个 serve 循环把全部样本的 thumb 和 info 跑三轮，每轮结束时提交内存 450 / 456 / 455 MB（`serve_memory_stays_flat`）。父进程一侧是 `serve::Worker`：子进程死了或超时，这一个请求返回错误，下一个请求换新的子进程（`tests/worker.rs`）。
- [x] 同一输入、同一 `RENDERER_VERSION` 两次输出的像素一致。2026-11-26：全部样本在两个设备上各渲一次，12 个完全一致；1 个（两套重叠的脸和眼网格）有 4–6 个像素差 1，是 GPU 自己的舍入，测试允许最多 16 个像素差 1。完全透明的像素一律写成黑色，之前混合会在里面留下每次不同的颜色。

## M3 第一个可依赖版本，Map1en 接缩略图

### 本库

| 项 | 要求 |
|---|---|
| 发布 | `unipeek-unity` 和 `unipeek` 都发到 crates.io（先发 `unipeek-unity`，`unipeek` 依赖它的版本号）；第一版用预发布号（如 `0.1.0-alpha.1`），API 稳定后发 0.1.0 |
| 仓库可见性 | 发 crates.io 前公开 GitHub 仓库 |
| 依赖版本 | 和 Snowcase `Cargo.lock` 对齐：wgpu 30.0.1、image 0.25.10、tar 0.4.46、flate2 1.1.11、zip 9.8.1、encoding_rs 0.8.44 ✓ |
| Rust 版本 | 写明 `rust-version`，不用比 stable 新的特性；对方 CI 用 stable、`--locked` ✓ |
| 用法说明 | 给 Map1en 的接入说明写在 [USAGE](USAGE.md)：依赖哪个 crate、`--thumb-worker` 怎么接 `serve`、缓存键怎么拼、`RENDERER_VERSION` 的含义；API 变了要同步改 |
| CHANGELOG | 从这一版开始记 |

### Snowcase（Map1en 写，本库只提供说明）

- workspace 加依赖 `unipeek`。
- 缩略图作为「探查包」之后的第三段后台任务：只排可见商品，可暂停，同盘不并发。编排和子进程在 `app::library_store`，写结果在 `core::library::jobs` ✓。
- `cache.db` 加一张缩略图表，版本号 +1（对方的 `cache.db` 不迁移，版本不对就删了重建 ✓）。
- 缓存键：`fingerprint + member + 包内路径 + 依赖 GUID 集合 + RENDERER_VERSION + 视角 + 尺寸`。
- 网格封面顺序：BOOTH 图 → 压缩包宣传图 → `preview.png` → 3D 缩略图 ✓。

### 完成标准

- [ ] Snowcase 带上本库后，CI（windows-latest、无 GPU）通过。
- [ ] 没有其他封面的商品在网格里显示 3D 缩略图。

## M4 交互视口

目标：Map1en 能在 460 px 的详情抽屉里放一个可以拖动旋转的模型。

| 项 | 要求 |
|---|---|
| API | `Viewport::offscreen(&Gpu, width, height)`、`resize`、`set_scene`、`render() -> &[u8]`（BGRA，直接给 GPUI 的 `RenderImage`）；只有相机或场景变了才重新渲染 |
| 相机 | `Orbit` 的 `rotate`、`pan`、`zoom`、`reset`、`preset(View)`；鼠标增量由调用方喂进来 |
| 加载 | `load(Source, inner)` 在后台线程跑，带进度；先出白模，贴图解码完再换上（现在贴图要 6–9 秒、还卡 UI 线程 ✓） |
| 单个 prefab 的缩略图 | 包文件树里点开或悬停时用，和 M2 是同一个函数，多一个包内路径 |
| 独立窗口 | `unipeek view <输入>`，命令行用户和退路用 |
| 不做 | DComp、子 HWND 零拷贝（M6） |

### Snowcase（Map1en 写）

- 详情抽屉里加预览模块和报告显示；入口放在包文件树的 `.prefab` 节点上 ～（要和 Map1en 确认 UI）。
- GPUI 鼠标事件转成 `Orbit` 调用，帧转成 `RenderImage`。

### 完成标准

- [x] preview-lab 的 `readback` 模式改为只用 `unipeek` 公开 API，功能不变。2026-11-26：改用 `Viewport`，`backend::Readback` 删掉了。
- [x] 460 px 抽屉（150% DPI 下约 690²）里，拖动时每帧渲染加读回不超过 5 ms；首帧白模 1 秒内出来。2026-11-26（RTX 2060，release，本机最大的样本，7 万三角形）：每帧 1.08 ms，白模 0.63 s 上屏，贴图 1.09 s 换上（`local_samples::viewport_speed`）。preview-lab 里 `readback` 每帧 1.7 ms（含转成 `RenderImage`）。

## M5 跨包 `Resolver`

目标：衣装 prefab 能配上素体一起显示。对方不维护词表，从名字认不出适配哪个 Avatar，**只能靠 GUID** ✓。

| 项 | 要求 |
|---|---|
| trait | `fn locate(&self, guids: &[Guid]) -> Vec<(Guid, Located)>`；`Located` 是「包（路径或 reader 工厂）+ 包内 GUID」或「普通文件」 |
| 批量 | 本库把要找的 GUID 按包分组，每个包只顺序读一遍 |
| 素体 | 衣装引用的骨架或身体 GUID 在包外 → 交给 `Resolver` → 把素体网格并进场景 |
| shader | 靠 M1 的已知 GUID 表认，不需要找到 shader 文件 |
| 降级 | `Resolver` 找不到时照常出图，原因写「缺素体」，列出缺的 GUID |
| 命令行 | `--with <包或目录>` 可重复，命令行用户手动补来源 |

### Snowcase（Map1en 写）

- 用 `cache.db` 的 `package_asset` 按 GUID 找到包，再用 `library.db` 把指纹映射回磁盘路径 ✓。
- 工程里的依赖要等 P2 的 Unity 工程扫描。

### 完成标准

- [ ] 本机样本里的衣装包加上对应素体包（`--with`）后，出图包含素体；不给时降级并列出缺的 GUID。

## M6 以后

不排期，按 Snowcase 的需要挑：

| 项 | 说明 |
|---|---|
| Snowcase `formats` 换用 `unipeek-unity` | 对方的 `list_unitypackage` 现在能用，不急；换的时候两边一起跑测试 |
| 零拷贝视口 | 全屏查看时用 DComp visual 或窗口内子 HWND；启动时自检，失败退回读回 |
| 转台帧 `unipeek turn` | 循环调用缩略图 |
| 更多 shader | Poiyomi、MToon、Standard 近似；MatCap、法线贴图（需要 `mikktspace`）、自发光 |
| 更多输入 | Unity 工程目录（P2）、glTF / VRM、7z、包中包（PSD 合成图已挪到首个版本，见文首的决定 2） |
| AI 用的多视角小图 | 224² / 384²（Snowcase P5） |
| 发 crates.io 0.1 | API 稳定以后 |

## 待定

- [ ] **仓库何时公开**：最晚在 M3 发 crates.io 之前。
- [x] 读包一遍还是两遍：两遍（2026-11-26，理由见 M1「读几遍包」）。
- [ ] 详情里预览的入口和布局：Map1en 定，本库两种入口（商品主 prefab、包文件树里的 prefab）都支持。
