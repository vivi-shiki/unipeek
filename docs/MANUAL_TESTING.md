# 手动验收清单

日期 2026-11-26，按 `master` 的 `655e7b5`（M1–M6 本库侧全部合并）整理。列出自动测试替代不了、要 Vivi 在本机亲手看或操作的行为。

**结论**：

1. 自动测试管的是「对不对」：解析、报告、退出码、`serve` 协议、子进程重启、像素可重复、内存不涨、读包遍数。它们管不了「看起来对不对」「手感对不对」「别的机器上行不行」。
2. **A 部分（Snowcase 第一阶段要用的）共 13 项**，对应 P1 的「缩略图」「白模预览」，加上 ★ 的 PSD、包内路径、`Resolver`、独立窗口 `view`。发第一阶段要用的新版本之前做完。
3. **B 部分（之后再测）共 9 项**：零拷贝视口、转台帧、多视角小图、更多 shader、7z / 包中包 / Unity 工程、glTF / VRM、别的显卡和系统、机械硬盘。
4. 自动测试已经覆盖得够的行为不在这里，见文末「不用手动测的」。

标记沿用 [ROADMAP](ROADMAP.md)：**✓** 已核实；**～** 推断或待验证。

## 准备

| 项 | 做法 |
|---|---|
| 机器 | Windows，有独显（现在的基准机是 i7-8750H + RTX 2060，还带一块 UHD 630 核显） |
| 构建 | `cargo build --release -p unipeek`，下面的 `unipeek` 都指 `target\release\unipeek.exe` |
| 样本 | 本机付费样本文件夹（含 `unipeek-samples.json`），下面写作 `<样本>`；`$env:UNIPEEK_SAMPLES = "<样本>"` |
| 输出 | 一律写到 `out\manual\`（`out` 已被 git 忽略）。付费素材的截图和缩略图不提交、不贴进文档 |
| 记录 | 每项后面的方框打勾；不通过的写一句现象和样本（不写付费素材名，写「样本 3」之类）记在本文「验收记录」里 |

批量出图可以用这段 PowerShell（后面几项都用到）：

```powershell
New-Item -ItemType Directory -Force out\manual | Out-Null
Get-ChildItem $env:UNIPEEK_SAMPLES -Recurse -Include *.unitypackage,*.zip,*.7z | ForEach-Object {
    & target\release\unipeek.exe thumb $_.FullName -o "out\manual\$($_.BaseName).webp" --view 3q --timings 2>&1 |
        Out-File -Append out\manual\thumb.log
    "$LASTEXITCODE`t$($_.Name)" | Out-File -Append out\manual\codes.tsv
}
```

## A. Snowcase 第一阶段要用的

### A1 自动测试确实跑了，没有被跳过

- [ ] 通过

- **前置**：设好 `UNIPEEK_SAMPLES`，独显可用，有桌面会话（不是远程无显示的会话）。
- **步骤**：`cargo test --workspace --release -- --nocapture 2>&1 | Tee-Object out\manual\test.log`，然后 `Select-String "skipped:" out\manual\test.log`。
- **预期**：全部通过；`skipped:` 一行都没有（尤其是 `thumbnail`、`viewport`、`surface`、`cli` 的 `view_opens_a_window_or_says_why_not` 和 `local_samples`）。
- **为什么要手动**：没有 GPU、没有窗口、没设样本时，这些测试打印 `skipped:` 然后**算通过**。只看「全绿」分不出是真的测过还是跳过了，要人看一眼日志。

### A2 缩略图整体观感（全部样本）

- [ ] 通过

- **前置**：A1 通过；用上面的批量脚本把 `<样本>` 下所有包出一遍 3q 缩略图。
- **步骤**：在资源管理器里用大图标浏览 `out\manual\`，逐张看。
- **预期**：
  - 模型完整在画面里，不被裁掉头脚，也不缩成一个小点；居中，留白大致一样。
  - 面朝镜头（3q 是正面偏侧），不是背面、不是躺倒或倒立。
  - 贴图在正确的部位，没有整件衣服是白的或颜色串到别的部位。
  - 没有明显的破面、穿模之外的大块黑色或洋红色。
  - `codes.tsv` 里退出码和 manifest 里记的状态一致；没有 3（出错）。
- **为什么要手动**：`local_samples::thumbnails_render` 只检查能出图、状态和尺寸，`thumbnail.rs` 用的是合成的几个四边形。「构图好不好、朝向对不对、贴图贴没贴对」只能人眼判断。

### A3 四个视角和背景

- [ ] 通过

- **前置**：挑 3 个样本：一个头像、一个衣装、一个小物件。
- **步骤**：每个样本分别 `unipeek thumb <包> -o out\manual\v-front.webp --view front`，再换 `side`、`back`、`3q`；再出一张 `--bg "#ffffff"` 和一张 `--bg "#202020"` 的 PNG。
- **预期**：front 是正面，back 是背面，side 是侧面（记下是左侧还是右侧，以后别改），3q 在正面和侧面之间；四张大小和位置一致。透明背景的图放到白色和深色底上看（比如拖进浏览器、换底色的看图软件），**头发、裙边等半透明边缘没有黑边或白边**。
- **为什么要手动**：`view_names` 和 `side_view_differs…` 只证明视角不同、可重复，不证明方向符合直觉；半透明边缘叠到 Snowcase 网格的底色上好不好看，要放在真实底色上看。

### A4 lilToon 着色近似

- [ ] 通过

- **前置**：挑用 lilToon 的样本，最好分别带透明（头发、纱）、cutout（睫毛、蕾丝）、描边的材质；准备好这些商品的 BOOTH 商品图或 Unity 里的截图做对照。
- **步骤**：`unipeek thumb <包> -o out\manual\lil.png --size 1024 --view front`，和商品图并排看。
- **预期**：主色、阴影色调大体对得上（不追求和 Unity 一样）；cutout 的边缘干净，没有大块该透明却实心的；透明部分能透出后面的东西，不出现前后颠倒；描边粗细合理、只在轮廓上，没有整张脸被描黑；双面材质背面不消失，单面材质背面不出鬼影。
- **为什么要手动**：着色是近似，测试只能用合成材质检查「描边画在轮廓外」「透明变体会混合」这类规则，判断「像不像这个商品」只能对照真图。

### A5 认不出的 shader 和降级原因

- [ ] 通过

- **前置**：找一个用了 lilToon / Poiyomi / MToon / Standard 以外 shader 的样本（或在 `info --json` 的 `materials` 里找 shader 未识别的槽）。
- **步骤**：`unipeek info <包> --json > out\manual\info.json`，看 `status`、`degrades`（降级原因）和材质列表；再出缩略图。
- **预期**：不报错；这些槽显示白模加主贴图；降级原因写的是 shader 未识别，并能对上是哪个材质。整份报告里的三角形数、骨骼数、BlendShape 数、贴图尺寸，和 Unity 里看到的大致一致。
- **为什么要手动**：报告字段格式有测试，但「原因写得对不对、普通用户在 Snowcase 抽屉里看得懂吗」要人判断；manifest 只覆盖本机已有的几个包。

### A6 PSD 贴图

- [ ] 通过

- **前置**：本机样本里带 PSD 贴图的包（`local_samples::psd_composites_decode` 用到的那些），另外最好找一两个不在 manifest 里的。
- **步骤**：出 front 缩略图；用 Photoshop 或别的看图软件打开对应 PSD 对照颜色。有条件的话把一个 PSD 另存为「关闭最大兼容」再打包测一次。
- **预期**：颜色和 PSD 的合成图一致，没有偏暗（16/32 位换算）、没有红蓝对调、透明区域正确；关了最大兼容的那个显示白色，报告写 `texture_unreadable`，其余照常出图；CMYK / 16 位等报 `texture_approximated`。
- **为什么要手动**：自动测试用的是 psd-tools 的 4×4 小图和自己生成的 PSD，证明解码正确；BOOTH 上真实 PSD 的图层、色彩模式和压缩千奇百怪，颜色「看起来对」要人对照。

### A7 自动选的主 prefab

- [ ] 通过

- **前置**：最好有几个**不在 manifest 里**的新包（manifest 是我们自己写的预期，等于自己出题自己答）。
- **步骤**：`unipeek info <包>`，看选中的 prefab 和候选列表。
- **预期**：选中的是商品图上那个「本体」（不带颜色、变体后缀，比如没有 `_Quest`、`_PB`、`_Black`）；候选列表里能找到其他颜色和变体；只有 FBX 没有 prefab 的包选了最大的 FBX。
- **为什么要手动**：选择规则是按路径结构猜的，测试只能证明规则按写的那样执行，不能证明对新包猜得对。Snowcase 网格里每个商品自动一张图，选错了用户第一眼就能看到。

### A8 包内路径、包中包里的 prefab、日文文件名

- [ ] 通过

- **前置**：一个 zip 里装着 `.unitypackage` 的商品；一个文件名没有 UTF-8 标志的日文 zip（日本作者的 zip 大多如此）。
- **步骤**：
  1. `unipeek ls <zip>`：看文件名。
  2. 从候选列表里挑一个非主 prefab：`unipeek thumb "<zip>#<包内的 unitypackage>#Assets/…/X.prefab" -o out\manual\one.webp`。
- **预期**：日文文件名显示正常，不是乱码；指定的那个 prefab 被渲出来（不是主 prefab），报告里 prefab 路径一致。
- **为什么要手动**：Shift-JIS 和 `#` 路径都有合成测试；真实 zip 的编码、多层目录和长路径以真实商品为准。这正是 Snowcase 包文件树里点开某个 prefab 时的调用。

### A9 独立窗口 `unipeek view`（人眼确认不是黑屏）

- [ ] 通过

- **前置**：本机桌面，独显；挑一个 7 万三角形左右的头像包和一个小包。
- **步骤**：`unipeek view <包>`，依次：
  1. 看打开瞬间：先出白模，几秒内换成有贴图的。标题栏依次显示加载百分比、「loading textures」、最后是 prefab 名、三角形数和状态。
  2. 左键拖动旋转；右键或中键拖动平移；滚轮缩放。
  3. 按 `R` 复位，`1`/`2`/`3`/`4` 切正面、侧面、背面、3q。
  4. 拖窗口边改大小、最大化、还原；把窗口拖到另一个 DPI 不同的显示器（有的话）。
  5. `Esc` 关闭，看退出码（`$LASTEXITCODE`）。
- **预期**：画面不是黑的；旋转跟手，方向符合直觉（往右拖模型往右转），不会翻转到底下；缩放不会穿进模型或缩没；改大小后画面不拉伸、不变黑；白模换贴图时相机不跳；降级的包在终端打印 `degraded:` 原因；退出码和 `thumb` 同一个包一致。
- **为什么要手动**：`cli::view_opens_a_window_or_says_why_not` 只开 2 秒、读 stderr 里的帧数，看不到画面。**之前用 `shot.ps1` 截图出来是全黑的**，可能是截图工具截不到 wgpu 交换链，也可能真的是黑屏，只能人眼看实际窗口来分辨 ～。鼠标拖动、实时改大小、DPI 变化在 README「Results」里也写着没测过。

### A10 详情预览（读回视口，模拟 Snowcase 抽屉）

- [ ] 通过

- **前置**：preview-lab 的 `readback` 模式已经只用 `unipeek::Viewport`（M4 完成标准里改过），可以当 Snowcase 抽屉的替身。
- **步骤**：`cargo run --release -p preview -- readback <包> 690x690`（150% DPI 下 460 px 抽屉的物理尺寸）；拖动旋转、缩放，点 `Popup test` 让面板盖到 3D 区域上；再用 `1100x800` 跑一次。看 stderr 的 `[stats]` 行。
- **预期**：首帧白模 1 秒内出来，贴图随后换上；拖动流畅，每帧（渲染加读回加转 `RenderImage`）不超过 5 ms，参考值 1.7 ms；弹出面板画在 3D 上面；颜色和 `unipeek view` 一致（没有 BGRA 红蓝对调、没有变暗变亮）；模型背景和透明边缘正常。
- **为什么要手动**：`local_samples::viewport_speed` 量的是库内的渲染和读回，不含 GPUI 上屏，也没有真实鼠标；红蓝对调、伽马错、帧率掉这类问题要在真的 GPUI 窗口里看。

### A11 跨包素体（`--with` / `Resolver`）

- [ ] 通过

- **前置**：**需要找一对样本**：一个衣装包，它的 prefab 通过 GUID 引用素体包里的 prefab 或网格（典型的「对应 ○○ 的衣装」，衣装 prefab 里嵌套了素体 prefab）。本机现有样本里没有这样的一对（见 ROADMAP M5）。
- **步骤**：
  1. `unipeek thumb <衣装包> -o out\manual\outfit-alone.webp`，看退出码和 `info` 里的 `missing_guids`。
  2. `unipeek thumb <衣装包> --with <素体包> -o out\manual\outfit-with.webp`；`unipeek view <衣装包> --with <素体包>`。
  3. `--with` 换成放着素体包的文件夹再来一次。
- **预期**：第 1 步照常出图，退出码 1（降级），原因 `missing_dependency`，列出缺的 GUID；第 2、3 步衣服穿在素体身上，位置对得上，素体有自己的贴图；`info` 的 `dependencies` 列出用到的素体包。
- **为什么要手动**：自动测试用合成的衣装包和素体包证明了流程；真实衣装对素体的引用方式（嵌套 prefab、Prefab Variant、直接拿网格）每家作者不一样，还没在真实包上见过成功的例子。Snowcase P1 允许这里降级，但要知道降级的样子能不能接受。

### A12 全库批量跑（`serve` 和子进程）

- [ ] 通过

- **前置**：用 Snowcase 实际会扫的整个素材库，而不只是 manifest 里的样本。
- **步骤**：用开头的批量脚本跑一遍；另外开一个 `unipeek serve`，手工粘几行请求（`{"id":1,"op":"thumb","source":"…","out":"…"}`、`{"op":"info",…}`、`{"op":"version"}`）看回复；跑的过程中在任务管理器里结束一次 `unipeek.exe` 进程。
- **预期**：没有卡死、没有崩溃窗口；退出码 3（出错）的包逐个能说出原因（损坏、不是 Unity 包等），不是本库的 bug；耗时在 `thumb.log` 里没有离群的（单包超过 2 分钟要记下）；任务管理器里内存不持续上涨；`serve` 回复格式和 [USAGE](USAGE.md) 一致。
- **为什么要手动**：`serve_runs_the_whole_set` 和 `serve_memory_stays_flat` 只跑 manifest 里的十几个样本，真实素材库里的怪包（加密 zip、超大包、空包、只有 Shader 的包）只有全库跑一遍才会冒出来。

### A13 核显和没有显卡的情况

- [ ] 通过

- **前置**：在 Windows「设置 → 系统 → 屏幕 → 显卡」里把 `unipeek.exe` 设为「节能」（用 UHD 630 核显）。
- **步骤**：对同一个包跑 `thumb --timings`、`view`；再在 `serve` 里跑几个请求。改回「高性能」后再跑一次对比。
- **预期**：要么正常出图（`serve` 回复里写着渲染用的后端；DX12 失败退到别的后端时，`thumb` 在 stderr 打印 `fallback:` 行），要么干净地报错（退出码 3，错误信息说明原因），**不能**整个进程段错误闪退；如果段错误，至少 `serve` 的子进程方式能把它隔离，父进程继续下一个（这就是 Snowcase 用子进程的原因）。核显上出的图和独显上的看起来一样。
- **为什么要手动**：Snowcase-docs 3D_PREVIEW 记过 UHD 630 跑 wgpu DX12 直接段错误 ✓；`gpu_fallback_is_recorded` 只能证明「有回退记录」，测不了真的驱动崩溃。Snowcase 用户的机器什么显卡都有，第一阶段发布前至少要在核显上看一次。

## B. 之后再测

### B1 零拷贝视口（DComp visual 和子 HWND）

- [ ] 通过

- **前置**：等 Snowcase 要做全屏或大窗口查看时再测；或者先把 preview-lab 的 `overlay` / `fast` 改成调 `unipeek::surface`（现在还没改，见文末「已知遗留」）。
- **步骤**：在宿主里用 `ViewportKind::open(Target::CompositionVisual(…))` 和 `Target::Hwnd(…)` 各开一次，全屏、改大小、拖动旋转；GPUI 弹出层盖上去；设 `UNIPEEK_NO_ZERO_COPY=1` 再开一次；睡眠唤醒或更新显卡驱动后再看。
- **预期**：画面正常、不闪；改大小后宿主 `Commit` 了画面就跟上；弹出层在 3D 上面（visual 方式需要 gpui-fast 的 native surface）；强制关掉时退回读回并在日志写原因；设备丢失后要么恢复要么退回读回，不崩。
- **为什么要手动**：`surface.rs` 只测了子窗口能出帧、空句柄会退回读回；DComp visual 路径没有自动测试，也还没有宿主真正用过它。

### B2 转台帧 `unipeek turn`

- [ ] 通过

- **步骤**：`unipeek turn <包> -o out\manual\turn\ --frames 24`，把帧按顺序放成动画看（看图软件幻灯片或拖进网页）。
- **预期**：转一整圈、方向一致，最后一帧接回第一帧不跳；每帧构图大小一样，模型不在画面里漂移；24 张都在，名字 `frame_000` 到 `frame_023`。
- **为什么要手动**：测试检查帧数、文件名、帧和帧不同，动起来顺不顺只能看。首个发布不做转台帧（M0 决定 5）。

### B3 多视角小图 `render_views`（224² / 384²）

- [ ] 通过

- **步骤**：写个小例子或等 Snowcase P5 接入时，出 224² 和 384² 的一组视角。
- **预期**：这么小的尺寸下模型仍然认得出，描边不会糊成一团黑，贴图没有严重摩尔纹。
- **为什么要手动**：给 AI 用的小图「够不够看清」是主观判断，要等 P5 的需求定下再定标准。

### B4 更多 shader：Poiyomi、MToon、Standard、法线、MatCap、自发光

- [ ] 通过

- **前置**：各找一个用这些 shader 的样本，准备商品图对照。
- **步骤**：出 front 和 3q 的 1024² 缩略图，和商品图对照。
- **预期**：Poiyomi 的透明 / cutout 模式对；MToon 的阴影色、描边对；Standard 的透明模式对；法线贴图的凹凸方向对（**不是反的**：DirectX 和 OpenGL 的法线贴图绿通道相反，凸起的地方看起来像凹进去就是反了 ～）；MatCap 的光泽在头发、金属上合理，不是满身发白；自发光的部分发亮但不过曝。
- **为什么要手动**：都是近似，测试用合成材质只证明「平的法线贴图不改变画面、斜的改变」「三种 MatCap 混合方式不同」，像不像要对照真图。

### B5 7z、包中包、Unity 工程目录

- [ ] 通过

- **前置**：一个 7z 商品；一个 zip 里套 zip 再套 unitypackage 的商品；Vivi 自己的一个 Unity 工程文件夹（以及把它整个打成 zip 的版本）。
- **步骤**：`unipeek ls`、`info`、`thumb` 各跑一遍；工程目录用 `info --json` 看选中的 prefab 和 GUID。
- **预期**：7z 和多层包能读到（超过三层或超出大小预算时报原因，不卡死）；工程目录只看 `Assets/` 和 `Packages/`，不读 `Library/`（大工程也不应该很慢）；工程里的 prefab 和材质按 `.meta` 的 GUID 绑上贴图。
- **为什么要手动**：测试用的都是合成的小归档和小工程。真实 Unity 工程动辄几万个文件，速度和选 prefab 的结果要在真东西上看。Unity 工程扫描是 Snowcase P2。

### B6 glTF / GLB / VRM

- [ ] 通过

- **前置**：VRM 0.x 和 1.0 各一个（VRoid Hub 上可下载的公开模型即可）；一个 `.gltf` 加外部贴图的文件夹；一个贴图内嵌为 data URI 的 `.gltf`。
- **步骤**：`thumb` 和 `view` 各跑一遍。
- **预期**：VRM 面朝镜头（0.x 已转向 +Z）；静止姿势（T / A 字），网格没有炸开或缩成一团；MToon 阴影和描边合理；外部贴图读到；**data URI 内嵌贴图读不了**，应该照常出图（白色）并报贴图读不了的原因，不报错退出（见「已知遗留」）。
- **为什么要手动**：测试用合成的 GLB 和 VRM；真实 VRM 的蒙皮、骨骼层级和 MToon 参数组合多，看起来对不对要真模型。

### B7 别的显卡、Vulkan、别的系统

- [ ] 通过

- **步骤**：有机会时在 AMD 显卡、只有核显的轻薄本、Windows on ARM 上跑 A2、A9、A10 的简化版；macOS / Linux 上至少 `cargo build --workspace` 和 `cargo test -p unipeek-unity`。
- **预期**：能出图或干净地报错；回退到 Vulkan 时 stderr 有 `fallback:` 行，`serve` 回复里的后端也对得上。
- **为什么要手动**：只有一台基准机。

### B8 机械硬盘和 3 GB 以上的真实包

- [ ] 通过

- **前置**：把几个大包（含 3 GB 以上的，如果有）放到机械硬盘上。
- **步骤**：`unipeek thumb <包> --timings`，看读包时间和 `Report::read` 的字节数；同时开着资源监视器看磁盘队列。
- **预期**：顺序读，不来回寻道；第二遍取齐需要的条目就停；读包时间和 SSD 上的差距在合理范围（记下数字，给 Snowcase 的后台任务排期做参考）。
- **为什么要手动**：`big_package_streams` 用合成的 3.2 GB 包证明了常量内存，但跑在 SSD 上、内容是假的；机械硬盘是 Snowcase 后台任务的底线（DECISIONS 2026-11-20 ✓），只能在真盘上量。

### B9 高分屏、多显示器、大窗口里的 `view`

- [ ] 通过

- **步骤**：4K 屏、200% 缩放下全屏 `unipeek view`；两个 DPI 不同的显示器之间拖动窗口。
- **预期**：清晰、不模糊、不黑屏；帧率跟得上显示器；跨屏后尺寸正确。
- **为什么要手动**：没有自动测试碰 DPI 和多显示器。

## 已知遗留

| 遗留 | 影响 | 由哪项检查 |
|---|---|---|
| 透明材质（两套重叠的脸和眼网格）两次渲染有 4–6 个像素差 1 | GPU 舍入，人眼看不出；Snowcase 的缓存键用 `RENDERER_VERSION`，不比像素，所以不影响缓存 ✓。测试已允许最多 16 个像素差 1 | 不用手动测；如果以后要用像素 hash 判断「图变没变」，再重新评估 |
| `.gltf` 用 data URI 内嵌的贴图读不了 | 这类模型显示白色、报贴图读不了；GLB 和外部贴图不受影响 | B6 |
| preview-lab 的 `overlay` / `fast` 还没改用 `unipeek::surface` 的零拷贝视口 | DComp visual 路径没有真实宿主验证过 | B1 |
| 之前 `unipeek view` 的截图是全黑的 | 分不清是截图工具截不到还是真黑屏 | A9（人眼看窗口） |
| crates.io 上的 `0.1.0-alpha.2` 还没有 `Viewport`、`Resolver`、PSD、`Worker` | Snowcase 第一阶段的「白模预览」要用 `Viewport`，要先发新版本 ～ | 不是测试项；发版前按 A 部分验收 |

## 不用手动测的

这些已经有自动测试，A1 确认它们真的跑了就够：

- 读包：流式索引、两遍读、3 GB 常量内存、损坏 / 截断 / 恶意路径（`unipeek-unity` 的单元测试、`big_package_streams`）。
- 绑定：prefab → `.meta` → `.mat` → 贴图、prefab 材质覆盖、按网格 fileID 对节点（`scene.rs`、`bindings_beat_name_matching`）。
- 报告和退出码：`info --json` 字段、0 / 1 / 2 / 3 / 64（`cli.rs`、`reports_match_the_manifest`）。
- `serve` 协议、`Worker` 重启和超时（`serve.rs`、`worker.rs`）。
- 同一输入两次出图像素一致（`side_view_differs_and_rendering_is_deterministic`、本机样本两设备对比）。
- `Viewport` 只在变化时重渲、先白模后贴图、复位和预设视角（`viewport.rs`）。
- `Resolver` 批量查询、每个包只开一次（`resolver.rs`）。
- PSD 各色彩模式的解码（`psd_fixtures.rs`、`psd/tests.rs`）。

## 验收记录

| 日期 | 项 | 结果 | 备注 |
|---|---|---|---|
| | | | |
