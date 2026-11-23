# 3D 预览：开源实现调研

日期 2026-11-23。只读调研，未改代码。补充 Snowcase-docs 里的渲染库设计稿和需求，已有结论不重做。需求目录见 [REQUIREMENTS](REQUIREMENTS.md)。

标记：**✓** 已核实（读过 README、源码、Cargo.toml 或 GitHub / crates.io API 返回值）；**～** 推断或没有亲手验证。星数和最近推送时间取自 2026-11-23 的 GitHub API。

## 结论

1. **没有能直接拿来用的现成库。** 没有任何项目同时做到「流式读 unitypackage → 按 GUID 解析 prefab / .meta / .mat → 卡通近似渲染 → 无头缩略图 → 嵌进 GPUI」。最接近的是 **popone**，它把前三步做成了一个 egui 桌面程序，但不是库。
2. **最值得借鉴的是 popone（0BSD）。** unitypackage 里 prefab → FBX `.meta` → `.mat` → 贴图这条链，以及 lilToon / Poiyomi / UTS2 → MToon 的参数近似，它都已经做完了。0BSD 不要求署名，代码可以直接搬进 GPL 或宽松许可的仓库。建议按模块移植，不当依赖。
3. **可以直接依赖的只有基础 crate。** `ufbx`、`gltf`、`wgpu`、`image`、`tar` / `flate2` / `zip` / `sevenz-rust2`、`encoding_rs`，再加上 popone 用的 `mikktspace`（法线贴图要用的切线，新增建议）。
4. **还得自己做的部分：** 常量内存的流式包索引和跨包 `Resolver`、按 shader 变体决定渲染方式、缩略图服务（缓存键、失效、子进程）、信息报告和降级原因，以及 GPUI 零拷贝嵌入。最后这一项**没有找到任何开源项目在上游 GPUI 的 Windows 版上做过**，preview-lab 的方案 D 已经走在前面。
5. **要修正两处已有结论：** Zed 上游的合成 PR #62379 已经关闭、没有合并；gpui-pre 从 0.3.12 起才把 DComp target 改成 `topmost = false`。方案 D 因此要锁定版本，另外多出一条可选的退路「窗口内子 HWND」。详见最后一节。

## 按功能分层看现有实现

| 层 | 现有开源实现 | 和我们要的差距 |
|---|---|---|
| 资产包解析（unitypackage、zip、7z） | popone ✓、slerp ～、各种 Rust 解包 CLI ✓ | 都是先整包解开或者读进内存（popone 上限 2 GB ✓），没有常量内存的流式索引，也没有跨包 GUID 查询 |
| Unity YAML（prefab、.meta、.mat） | popone ✓（5 种 prefab 格式、Variant 链）、unidot_importer ✓（GDScript）、slerp ～（要求有工程目录） | 只取主贴图、法线和少数参数；不读 shader 变体，不读阴影、描边参数 |
| FBX | ufbx ✓、popone 自写解析器 ✓、fbx-rs ✓（纯 Rust，两周前才建） | ufbx 仍是首选，原结论不变 |
| glTF / VRM | `gltf` crate ✓、popone ✓、weebui ✓、bevy_vrm / bevy_vrm1 ✓ | VRM 扩展解析都很薄，自己写或抄成本都低 |
| 卡通着色 | popone（MToon 近似 WGSL）✓、weebui（MToon WGSL）✓、three-liltoon（lilToon 移植，TS）✓ | 没有现成的 Rust lilToon 实现；popone 自评 lilToon 还原度 60%，Poiyomi 45% ✓ |
| 无头缩略图 | slerp（辅助进程常驻 GPU）✓、polyloupe ✓、glimpse（软光栅）✓ | 都是给资源管理器做缩略图，单个文件、无材质绑定，没有缓存键和失效 |
| 嵌入 GPUI | rbx-native、weebui、bevy-gpui，全部走读回 → `RenderImage` ✓；GPUI-3D、gpui-fusion 是 GPUI fork ✓ | 上游 GPUI 的 Windows 版上没有零拷贝实现 |

## 逐项评估

### 和我们最接近的

| 项目 | 许可 | 活跃度 | 做了什么 | 能否依赖 | 建议 |
|---|---|---|---|---|---|
| [tinatsu-nomy/popone](https://github.com/tinatsu-nomy/popone) | 0BSD ✓ | 2 星，2026-03 建，最近推送 2026-09-19，v0.5.18 ✓ | egui + wgpu 24 查看器，CLI 转 PMX。读 VRM / FBX（自写解析器，二进制和 ASCII）/ PMX / OBJ / unitypackage / zip / 7z / rar，嵌套压缩包展开一层 ✓。unitypackage 里按 prefab → `.meta` `externalObjects` → `.mat` → 贴图解析，识别 New / Old / Unpacked / Mixed / Variant 五种 prefab，Variant 链有环检测和 32 层上限 ✓。lilToon / Poiyomi / UTS2 自动识别后映射成 MToon 参数，有描边（反向外壳）、4 阶段绘制顺序、alpha-to-coverage ✓ | 不行：它是单体程序，GUI、PMX 转换、日志都混在一起，库接口不稳定，只有一个维护者 | **重点借鉴。** 移植 `src/unitypackage.rs` 的 prefab 解析和三级材质匹配、`convert/material.rs` 的 shader 识别和参数映射、MToon WGSL、FBX 坐标换算的经验。FBX 解析仍用 ufbx |
| [eric-frost/slerp](https://github.com/eric-frost/slerp) | MIT OR Apache-2.0 ✓ | 1 星，2026-10 建，README 写明全部由 AI 生成 ✓ | 约 200 种格式的查看器和缩略图程序。拆成 `slerp-render` / `scene` / `models` / `textures` 等 crate，wgpu 29，FBX 用 ufbx 0.11 ✓。缩略图由一个常驻辅助进程渲染，GPU 只初始化一次 ✓。Unity prefab 要放在工程目录里，靠扫 `.meta` 建 GUID 索引 ✓ | 不建议：太新，范围太散，没有 lilToon 相关代码（代码搜索 0 结果 ✓，但仓库可能还没被索引） | **借鉴架构。** crate 拆分方式、`serve` 式辅助进程、wgpu 后端按平台开关，和 RENDER_PLUGIN 的设计基本一致，可以当参照。unitypackage 它大概只是把里面的图片拼成联系表，不渲染 3D ～ |
| [Rukafuu/weebui](https://github.com/Rukafuu/weebui)（`gpui-vrm`） | MIT ✓ | 3 星，2026-11 建 ✓ | GPUI 里显示 VRM：wgpu 离屏、MSAA 4×、MToon WGSL（阴影 shift / toony、边缘光、反向外壳描边），帧通过 `RenderImage` 交给 GPUI ✓ | 不行：依赖 `gpui 0.2.2` 和 `wgpu 25`，与 Snowcase 的 gpui-pre、wgpu 30 不一致 ✓ | 参考 MToon WGSL 和读回的写法（MIT，抄代码要保留版权声明） |
| [mochiya-labs/three-liltoon](https://github.com/mochiya-labs/three-liltoon) | MIT ✓ | 53 星，2026-09 建，最近推送 10-17 ✓ | lilToon 的非官方 Three.js / WebGL2 移植：多层主色、三段阴影、MatCap、边缘光、自发光、描边、Fur、Gem ✓。材质要先用作者的 Unity 插件导出成 `MOCHIYA_materials_liltoon` glTF 扩展，它不从 `.mat` 读，也不按名字猜 ✓ | 不行（TypeScript） | **lilToon 着色公式的最佳参考。** 它的 FEATURE_MATRIX 列了每个变体的 pass 组合，可以直接对照来定「按 shader 变体决定渲染方式」那张表 |

### Unity 包浏览器和资产管理器

| 项目 | 许可 | 活跃度 | 3D 预览 | 结论 |
|---|---|---|---|---|
| [V-Sekai/unidot_importer](https://github.com/V-Sekai/unidot_importer) | MIT ✓ | 984 星，2026-10 仍在推送 ✓ | 把 unitypackage 转成 Godot 场景。FBX 靠外部的 FBX2glTF；材质只支持 Standard ✓ | 参考它处理 Unity YAML 的边界情况：文本和二进制序列化、Unpacked prefab、Humanoid ✓ |
| [barcoderdev/unitypackage_util](https://github.com/barcoderdev/unitypackage_util) | MIT ✓ | 26 星，2023-09 后无推送 ✓ | 无。Rust CLI，能把 YAML 转成 JSON，FBX 也靠 FBX2glTF ✓ | 已停更，只作参考 |
| [ticpu/rust-unityextractor](https://github.com/ticpu/rust-unityextractor) 等 Rust 解包 CLI | BSD-3 / MIT ✓ | 少量星 ✓ | 无 | 只解包，没有可复用的东西 |
| [pixel-clover/uview](https://github.com/pixel-clover/uview) | Apache-2.0 ✓ | 13 星，11-16 有推送 ✓ | 无，只能看和改文本资产 ✓ | 不相关 |
| [m35/UnityPackageViewer](https://github.com/m35/UnityPackageViewer) | AGPL-3.0 ✓ | 4 星 ✓ | 无 ～ | 许可不能用 |
| [wh0am15533/UnityPackageBrowser](https://github.com/wh0am15533/UnityPackageBrowser) | MIT ✓ | 2020 年后无推送 ✓ | 竞品表写它能交互看 3D；记忆里记着源码不公开 ～ | 无法参考 |
| [siloneco/KonoAsset](https://github.com/siloneco/KonoAsset)、[HakuSystems/EasyExtractUnitypackage](https://github.com/HakuSystems/EasyExtractUnitypackage) | MIT ✓ | 活跃 ✓ | 无 ～ | 竞品，没有 3D 可借鉴 |
| [AssetRipper/AssetRipper](https://github.com/AssetRipper/AssetRipper) | GPL-3.0 ✓ | 8,989 星 ✓ | 针对已构建的游戏资源，不是源资产包 | 方向不同，许可也不行 |

### 通用查看器和缩略图

| 项目 | 许可 | 说明 | 结论 |
|---|---|---|---|
| [Fr3nezy/polyloupe](https://github.com/Fr3nezy/polyloupe) | GPL-3.0 ✓ | Rust，Windows 查看器加资源管理器缩略图，glTF / FBX / OBJ 等，PBR 和转台渲染 ✓ | 和 Snowcase 的 GPL 兼容，但违反「不引入 GPL 依赖」的约定；只看思路，不抄代码 |
| [codename-B/glimpse](https://github.com/codename-B/glimpse) | MIT ✓ | Rust 写的 `IThumbnailProvider` COM DLL，软光栅，不需要 GPU ✓ | 「CI 没有 GPU」时的思路参考；不支持 FBX |
| [f3d-app/f3d](https://github.com/f3d-app/f3d) | BSD-3 ✓ | C++ / VTK，5,093 星 ✓ | 太重，没法嵌进 Rust；至多当外部命令行兜底 ～ |

### Rust 渲染和 VRM 生态

| 项目 | 许可 | 结论 |
|---|---|---|
| [ufbx/ufbx-rust](https://github.com/ufbx/ufbx-rust) 0.11.5 | MIT OR Unlicense ✓ | 保持首选。crates.io 最新仍是 0.11.5（2026-09-25）✓ |
| [lo48576/fbxcel](https://github.com/lo48576/fbxcel) | Apache-2.0 ✓ | 2026-04 有推送 ✓，修正原文档的「2022 年后无更新」；但 `fbxcel-dom` 仍停在 2023 年 ✓，否决结论不变 |
| [Tavrin/fbx-rs](https://github.com/Tavrin/fbx-rs) | Apache-2.0 ✓ | 纯 Rust，号称与 ufbx 逐位一致，只支持静态场景 ✓；两周前才建，先观察 |
| [unavi-xyz/bevy_vrm](https://github.com/unavi-xyz/bevy_vrm)、[not-elm/bevy_vrm1](https://github.com/not-elm/bevy_vrm1) | Apache-2.0 ✓ | 绑死 bevy，不依赖；`serde_vrm` 0.5（MIT OR Apache-2.0 ✓）可以用来解析 VRM 扩展 JSON，也可以自己写 |
| [pixiv/three-vrm](https://github.com/pixiv/three-vrm)、[vrm-c/UniVRM](https://github.com/vrm-c/UniVRM)、[Santarh/MToon](https://github.com/Santarh/MToon) | MIT ✓ | MToon 的权威实现，用来核对公式 |
| [schell/renderling](https://github.com/schell/renderling) | MIT OR Apache-2.0 ✓ | wgpu + rust-gpu 的 GPU 驱动渲染器，253 星，活跃 ✓；对预览器来说太重，和 bevy 同理 |
| [FyroxEngine/Fyrox](https://github.com/FyroxEngine/Fyrox)、[bevyengine/bevy](https://github.com/bevyengine/bevy) | MIT / Apache-2.0 ✓ | 整个引擎，原否决结论不变 |
| [poiyomi/PoiyomiToonShader](https://github.com/poiyomi/PoiyomiToonShader) | GitHub API 报告 MIT ✓（仓库根目录没找到 LICENSE 文件 ～） | 可以关掉原文档里「Poiyomi 许可待验证」这一项，但只针对免费版；Pro 版不开源 |

### GPUI 里放 3D

| 项目 | 做法 | 结论 |
|---|---|---|
| [chteau/rbx-native](https://github.com/chteau/rbx-native)（MIT，24 星，活跃） | 和 Snowcase 同一套栈：gpui-kit、gpui-pre 加 wgpu。视口用读回 → `RenderImage`（`crates/rbx_studio/src/render_image.rs` 里转 BGRA）✓。还 vendor 了 gpui-pre 和 gpui-pre-windows 来打补丁 ✓ | 证明读回这条路在真实项目里可行。也提醒一点：一旦要改 GPUI，它的做法是 vendor 补丁，不是长期 fork |
| [r4ai/bevy-gpui](https://github.com/r4ai/bevy-gpui) | bevy 无头渲染，读回给 GPUI ✓ | 同上 |
| [RandazzoMaxime/GPUI-3D](https://github.com/RandazzoMaxime/GPUI-3D)（0BSD） | 自带 GPUI fork（`gpui-ce`），UI 和 3D 共用一个设备，三缓冲 surface，零拷贝 ✓ | 等于方案 B / C 的极端版：整个 UI 后端都换掉。不适合 Snowcase，但它的 ARCHITECTURE.md 讲了 D3D12 / Vulkan 共享设备的同步规则，方案 B 可以参考 |
| [cradiy/gpui-fusion](https://github.com/cradiy/gpui-fusion) | GPUI fork，带 `gpui_3d`，但 3D 视口目前只支持 Linux WGPU ✓ | 不可用 |

## 剩下要自己做的

| 部分 | 为什么没有现成的 | 可抄的起点 |
|---|---|---|
| 流式 unitypackage 索引，常量内存，支持 3 GB 以上 | popone 整包读进内存，上限 2 GB ✓ | 原设计（单遍扫描，大文件进内容寻址缓存） |
| `Resolver`：跨包、跨工程、查 `cache.db` | 都只在单个包或单个工程里解析 | slerp 的「`.meta` 扫描 + GUID 索引缓存」～ |
| 按 shader 变体决定透明和描边，加已知 shader GUID 表 | popone 靠名字和属性识别 shader ✓，不看变体 | three-liltoon 的变体表 ✓ |
| lilToon 阴影和描边参数直读 | popone 是转成 MToon 后再画 | popone 的映射加 three-liltoon 的公式 |
| 缩略图服务：缓存键、失效、子进程隔离 | 没有人做到资产库这个层面 | slerp 的辅助进程 |
| 信息报告和降级原因 | 没有 | popone 的材质列表和缺贴图回退（白色 1×1）✓ |
| GPUI 零拷贝嵌入 | 所有 GPUI 项目都只用读回 | preview-lab 的方案 D |
| zip 的 Shift-JIS 文件名 | popone 只在 PMD 和文本里用 encoding_rs ～ | 原设计 |

## 对已有方案的修正和补充

1. **Zed #62379 已经关闭，没有合并（2026-11-21）✓。** 3D_PREVIEW.md 的风险表里写着「关注 zed#62379，状态待验证」，现在可以定了：上游短期不会有合成接口。更早的 #60573（外部合成器、零拷贝 wgpu 纹理）在 2026-07-10 也关闭了，没有合并 ✓。
2. **方案 D 依赖的是一个很新的实现细节。** 已核实 crates.io 上的源码：`gpui-pre-windows` 0.3.11 用的是 `CreateTargetForHwnd(hwnd, true)`，0.3.12 到 0.3.14 改成 `false` ✓。这个改动来自上游 [#66368](https://github.com/zed-industries/zed/pull/66368)（2026-11-02 合并，目的是让 WebView2 这类子窗口显示在 GPUI 上面）✓。还有一个没合并的 [#66676](https://github.com/zed-industries/zed/pull/66676) 准备加 `draw_above_child_windows` 选项，打开后 GPUI 会改回 `true` ✓。所以：
   - `gpui-pre` 版本下限锁在 0.3.12，永远不要打开那个选项。
   - 启动时，`CreateTargetForHwnd(hwnd, true)` 一旦失败，就自动退回读回或独立窗口。
3. **多了一条退路：窗口内子 HWND ～。** 0.3.12 起子窗口画在 GPUI 内容的上面，所以可以在详情页区域放一个子 HWND，让 wgpu 用普通的 HWND surface 来 present。它不依赖 topmost 这个位置，限制和方案 D 一样：弹窗会被挡住。值得在 preview-lab 里和 D 并排测一次。
4. **版本：** gpui-kit 已经出到 0.10.0（11-16），gpui-pre 到 0.3.14（11-23），wgpu 到 30.0.2 ✓。Snowcase 现在用 gpui-kit 0.9，preview-lab 升级时要确认上面第 2 条仍然成立。
5. **新增依赖建议：** `mikktspace`（MIT OR Apache-2.0，popone 在用 ✓），用来给没有切线的 FBX 生成切线，法线贴图要靠它。7z 用 `sevenz-rust2`，原来那个 `sevenz-rust` 已经没人维护 ～，popone 用的是前者 ✓。
6. **RENDER_PLUGIN.md 里的 fbxcel「2022 年后无更新」不准确：** fbxcel 本体 2026-04 还有推送，停更的是 fbxcel-dom ✓。结论不变。

## 建议的下一步

把 popone 的 `unitypackage.rs` 和材质映射移植进 preview-lab，在本机样本包上和现在按名字匹配贴图的结果对比。0BSD 不需要署名，但建议在源码注释里注明出处，方便以后追溯。
