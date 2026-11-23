# 需求目录

结论：本库是**懂 Unity 资产包的预览库**，给资产管理器调用，不是又一个通用 3D 查看器。Snowcase 是第一个用户；库放在独立仓库，发布到 crates.io，许可 MIT OR Apache-2.0。

状态：需求稿，2026-11-23。标 ★ 的是第一版要做的。来源是 Snowcase-docs 的 [RENDER_REQUIREMENTS](https://github.com/Map1en/Snowcase-docs) 和 RENDER_PLUGIN，以及 [开源实现调研](PRIOR_ART.md)。

## 定位

现有开源项目各缺一块（详见 [PRIOR_ART](PRIOR_ART.md)）：

| 类别 | 例子 | 缺什么 |
|---|---|---|
| 通用查看器 | slerp、polyloupe、f3d | 不懂 unitypackage 的 GUID、prefab、`.mat`，FBX 只能显示白模 |
| VRChat 向查看器 | popone | 是给人手动打开的桌面程序，别的程序没法调用 |
| 资产管理器 | KonoAsset 等 | 不做 3D |

本库补在中间：输入是资产包，输出是给程序用的缩略图、视口和报告。

| 做 | 不做 |
|---|---|
| 解析资产包，找出该显示哪个 prefab、用哪些材质 | 资产管理、数据库、GUID 归属（调用方负责） |
| 卡通近似渲染：无头出图、交互视口 | 和 Unity 一模一样的画面；执行 Unity shader 源码 |
| 信息报告和降级原因 | 编辑、导出回 Unity |
| 命令行和常驻进程，供其他工具调用 | `.vrca` 等已上传 avatar；通用的几百种格式 |

## 输入

| 输入 | 要求 | 阶段 |
|---|---|---|
| `.unitypackage` | 流式读取，常量内存，3 GB 以上的包也能扫完 | ★ |
| zip、文件夹里的 FBX / PNG / PSD | FBX 二进制和 ASCII；PSD 取合成图；zip 无 UTF-8 标志时按 Shift-JIS 识别文件名 | ★ |
| 字节流 | 除了路径，还接收 `Read` 或字节（冷归档文件、包内条目没有独立路径） | ★ |
| `Resolver` | 按 GUID 查文件，由调用方实现，用来补跨包依赖（素体、shader） | ★ |
| Unity 工程目录 | 只看 `Assets/` 和 `Packages/`，跳过 `Library/` | P2 |
| glTF / GLB / VRM、OBJ、7z、包中包 | 包中包限制深度和总大小 | 后做 |

## 解析与画面

| 项 | 要求 | 阶段 |
|---|---|---|
| 材质绑定 | prefab → FBX `.meta` `externalObjects` → `.mat` → 贴图；名字匹配作兜底。参考 popone 的实现 | ★ |
| lilToon | 主色、阴影、透明 / cutout、剔除、描边；透明和描边按 shader 变体决定，不看开关 | ★ |
| Poiyomi、MToon、Standard | 近似：主贴图加阴影 | 后做 |
| MatCap、法线、自发光 | 法线贴图需要 MikkTSpace 切线 | 后做 |
| 降级 | 认不出就显示白模加主贴图，记下原因，不报错 | ★ |

## 输出：Snowcase 需要的

| 输出 | 格式 | 阶段 |
|---|---|---|
| 缩略图 | 512×512，透明背景，WebP 或 PNG；尺寸、视角可选 | ★ |
| 交互视口 `Viewport` | 画到调用方给的 DComp visual 或窗口；相机和输入事件由调用方传入；读回静态帧兜底 | ★ |
| 信息报告 | Rust 结构体，`--json` 输出 JSON：三角形、骨骼、BlendShape、材质、shader、贴图尺寸、缺失 GUID | ★ |
| 降级原因 | 报告里的列表，例如「缺素体」「shader 未识别」「PSD 读不了」 | ★ |
| 退出码 | 区分 成功 / 降级成功 / 不支持 / 出错 | ★ |
| 转台帧 | 12–24 帧 PNG / WebP | P1 |
| 骨骼映射 | 骨骼名到 Humanoid 的对应，只给数据 | 未排期 |
| 多视角小图 | 224² / 384²，给以图搜图用 | P5 |

## 输出：给其他人用的

| 输出 | 理由 | 阶段 |
|---|---|---|
| 原始 RGBA 字节，加 PNG / JPEG | 最通用，几乎不花额外工作 | ★ |
| 独立窗口 `view` | 命令行用户直接看一个包 | ★ |
| 常驻进程 `serve`（stdin / stdout 一行一个 JSON） | 非 Rust 程序调用，不用每次初始化 GPU；Snowcase 的缩略图子进程也用它隔离驱动崩溃 | 接 Snowcase 缩略图时 |
| `raw-window-handle` 嵌入 | egui、iced、winit 程序也能嵌 | 后做 |
| 导出 GLB | 把 prefab 连材质转成 GLB；开源里目前只有 unidot 能做，还要依赖 FBX2glTF 和 Godot | 后做 |
| 联系表、深度 / 法线 / 遮罩图 | 商品检查、发帖、AI 抠图 | 有人要再做 |

## 命令行

```
<bin> info  <输入> [--json]
<bin> ls    <输入>
<bin> thumb <输入> -o out.webp [--size 512] [--view front|side|back|3q] [--bg transparent]
<bin> turn  <输入> -o dir/ [--frames 24]
<bin> view  <输入>
<bin> serve
```

`<输入>` 可以指向包内路径，例如 `Avatar.unitypackage#Assets/Avatar/Avatar.prefab`；`--with <包或目录>` 可以重复，用来补充 GUID 来源。

## 约束

| 约束 | 说明 |
|---|---|
| 只读 | 不修改、不移动输入文件，不往输入目录写东西 |
| 路径安全 | 包内路径带 `../` 或是绝对路径时，落点不能出目标目录 |
| CI 无 GPU | 解析和报告要能脱离 GPU 测试 |
| 许可 | 不引入 GPL 依赖 |
| 平台 | Windows 优先，三平台能编译 |
| GPUI | 嵌入方案依赖 `gpui-pre` 0.3.12 及以上（DComp target 为 `topmost = false`），见 [PRIOR_ART](PRIOR_ART.md#对已有方案的修正和补充) |
| 日志 | 错误信息和日志用英文 |

## 发布

| 项 | 决定 |
|---|---|
| 仓库 | 独立仓库，不放进 Snowcase workspace，不受 Snowcase 的 CLA 约束 |
| 许可 | MIT OR Apache-2.0（见 Snowcase-docs DECISIONS 2026-11-21） |
| crate | 拆成不依赖 GPU 的解析层 `unipeek-unity` 和渲染层 `unipeek`（feature `cli`、`7z` 等），对齐 Snowcase-docs 里 `formats` 复用解析 crate 的决定，见 [INTEGRATION](INTEGRATION.md#依赖关系) |
| 名字 | `unipeek`（Unity + peek）。2026-11-23 查过：crates.io 上 `unipeek` 和相近名字都没人用；GitHub 上只有一个无关的空 CSS 仓库同名。原工作名 `snowcase-render` 不再使用 |
| 版本 | 接口稳定、能出缩略图后发 0.1 |
| 注意 | ufbx 是 C 库，用户机器需要 C 编译器 |

## 待定

- [x] 正式名字：`unipeek`。
- [x] `Cargo.toml` 已改成 MIT OR Apache-2.0，加了 LICENSE-MIT 和 LICENSE-APACHE。
- [ ] 窗口内子 HWND 作为方案 D 的退路，需要实测。
