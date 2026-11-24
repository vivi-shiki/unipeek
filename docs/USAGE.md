# 调用 unipeek

结论：Snowcase 在 `Cargo.toml` 里按名字依赖 `unipeek`，不用复制代码。`formats` 依赖只管解析的 `unipeek-unity`，`app` 依赖带渲染的 `unipeek`。缩略图在 `snowcase.exe --thumb-worker` 子进程里调 `unipeek::serve`，详情预览用 `Viewport::offscreen` 读回给 `RenderImage`。

状态：2026-11-24，接口是计划稿，第一个可依赖版本（[ROADMAP](ROADMAP.md) 的 M3）发布后以 crate 文档为准。写给 Snowcase 的接入方（Map1en），决定的来由见 [INTEGRATION「接入决定」](INTEGRATION.md#接入决定m02026-11-24)。

仓库：<https://github.com/vivi-shiki/unipeek>，MIT OR Apache-2.0，可以进 GPL-3.0 的 Snowcase；反过来 Snowcase 的代码不能进 unipeek。

## 依赖

| crate | 内容 | 谁依赖 |
|---|---|---|
| `unipeek-unity` | 流式读 unitypackage、Unity YAML、prefab → `.mat` → 贴图、`Report`；不依赖 GPU，CI 能测 | `formats`（以后换掉自己的 `list_unitypackage` 时） |
| `unipeek` | wgpu + ufbx 渲染：缩略图、`Viewport`、`serve`；重新导出 `unipeek-unity`，不用两个都写 | `app` |

发到 crates.io 以后：

```toml
# app/Cargo.toml
unipeek = "0.1"

# formats/Cargo.toml（以后）
unipeek-unity = "0.1"
```

发布前的过渡期用 git 依赖，锁 tag 或 rev，不要跟分支：

```toml
unipeek = { git = "https://github.com/vivi-shiki/unipeek", tag = "v0.1.0-alpha.1" }
```

- Snowcase CI 用 `--locked`，升级时连 `Cargo.lock` 一起提交。
- 仓库公开以后才能这样直接拉。私有期间本机要让 Cargo 用 git 命令行和已登录的凭据（`.cargo/config.toml` 里 `[net] git-fetch-with-cli = true`），CI 还要配访问令牌，所以私有期间不要接进 Snowcase 的 `master`。
- unipeek 的 wgpu 大版本跟 Snowcase 的 `Cargo.lock` 走（现在都是 30）；不依赖 GPUI，GPUI 版本完全由 Snowcase 决定。

## 缩略图

在子进程里跑，显卡驱动崩溃不会拖垮主程序。`main` 里先认参数：

```rust
fn main() {
    if std::env::args().any(|a| a == "--thumb-worker") {
        let stdin = std::io::stdin().lock();
        let stdout = std::io::stdout().lock();
        std::process::exit(unipeek::serve(stdin, stdout));
    }
    // 正常启动 GPUI
}
```

主进程启动 `snowcase.exe --thumb-worker`，往 stdin 写一行一个请求，从 stdout 读一行一个回复：

```json
{"op":"thumb","source":"D:/Assets/Example.unitypackage","inner":null,"out":"D:/Library/thumbs/ab/abcd.webp","size":512,"view":"3q"}
{"status":"degraded","reasons":["missing_base_avatar"],"missing_guids":["…"],"prefab":"Assets/…/Outfit.prefab"}
```

- `inner` 为空时 unipeek 自己选主 prefab，回复里写选了哪个；给包内路径就渲那一个（包文件树里点开某个 prefab 时用）。
- `status` 是 `ok` / `degraded` / `unsupported` / `error`。降级也出图，原因放在 `reasons` 里。
- 子进程退出或没有回复就重启它，跳过那个包，记成失败。
- 调度（只排可见商品、可暂停、同盘不并发）由 Snowcase 负责，适合放在「整理文件」「探查包」之后作为第三段。unipeek 保证一个请求对一个包只顺序读。

不想开子进程时（比如测试）也能直接调：

```rust
let scene = unipeek::Scene::load(unipeek::Source::Path(path.into()), None)?;
let image = unipeek::render_thumbnail(&gpu, &scene, &unipeek::ThumbOptions::default())?; // RgbaImage，512²，透明背景
```

## 详情预览

第一版只有读回：unipeek 离屏渲染，交出 BGRA 字节，Snowcase 包成 GPUI 的 `RenderImage`。460 px 的抽屉每帧约 1 ms。

```rust
let mut view = unipeek::Viewport::offscreen(&gpu, width, height);
let loading = view.load(source, inner); // 后台线程；先出白模，贴图解码完再换上

// 鼠标事件
view.orbit().rotate(dx, dy);
view.orbit().zoom(steps);

// 每次重绘
if let Some(bgra) = view.render() { // 相机和场景都没变时返回 None，不重渲
    // 转成 RenderImage 显示
}
```

报告：`view.report()` 或 `serve` 的 `info` 请求返回 `unipeek::Report`（`serde::Serialize`）：选中的 prefab 和候选、三角形 / 骨骼 / BlendShape 数、材质和 shader、贴图尺寸、缺失的外部 GUID、降级原因。

## 跨包依赖（素体）

Snowcase 不维护词表，从名字认不出衣装适配哪个 Avatar，所以要靠 GUID：unipeek 列出 prefab 引用了但包里没有的 GUID，Snowcase 实现 `Resolver` 告诉它在哪个包。

```rust
impl unipeek::Resolver for LibraryResolver {
    fn locate(&self, guids: &[unipeek::Guid]) -> Vec<(unipeek::Guid, unipeek::Located)> {
        // 查 cache.db 的 package_asset 拿到 (fingerprint, member)，
        // 再用 library.db 映射回磁盘路径
        todo!()
    }
}
```

unipeek 按包分组，每个包只读一遍。找不到时照常出图，记「缺素体」。shader 靠内置的 GUID 表识别，不需要找到 shader 文件。

## 版本约定

| 项 | 约定 |
|---|---|
| 版本号 | 1.0 之前，`0.x` 的次版本号变了就可能不兼容；`0.1.x` 内只修 bug 和加东西 |
| 两个 crate | 版本号一起发，`unipeek` 依赖同版本的 `unipeek-unity` |
| `unipeek::RENDERER_VERSION` | 画面像素会变的改动都加一。Snowcase 把它放进缩略图缓存键，一变旧缩略图就失效重画 |
| 缓存键 | `fingerprint + member + 包内路径 + 依赖 GUID 集合 + RENDERER_VERSION + 视角 + 尺寸`，由 Snowcase 拼 |
| 变更 | 记在本仓库的 CHANGELOG；接口变了同步改本文 |

## 不做

- 不写输入文件，不在资产旁边写东西；输出只写到调用方给的路径。
- 不做资产管理、数据库、GUID 归属，这些是 Snowcase 的事。
- 不追求和 Unity 一样的画面；认不出的材质显示白模加主贴图，不报错。
- 首个发布没有转台帧，也没有零拷贝视口。
