# 调用 unipeek

结论：Snowcase 在 `Cargo.toml` 里按名字依赖 `unipeek`，不用复制代码。`formats` 依赖只管解析的 `unipeek-unity`，`app` 依赖带渲染的 `unipeek`。缩略图在 `snowcase.exe --thumb-worker` 子进程里调 `unipeek::serve`，详情预览用 `Viewport::offscreen` 读回给 `RenderImage`。

状态：2026-11-26 按 `dev/m1-m6` 分支更新（下一个版本）。缩略图、`serve`、`info`、命令行和「详情预览」（M4）已经实现并有测试；「跨包依赖」（M5）一节还是计划稿。写给 Snowcase 的接入方（Map1en），决定的来由见 [INTEGRATION「接入决定」](INTEGRATION.md#接入决定m02026-11-24)。

仓库：<https://github.com/vivi-shiki/unipeek>，MIT OR Apache-2.0，可以进 GPL-3.0 的 Snowcase；反过来 Snowcase 的代码不能进 unipeek。

## 依赖

| crate | 内容 | 谁依赖 |
|---|---|---|
| `unipeek-unity` | 流式读 unitypackage、Unity YAML、prefab → `.mat` → 贴图、`Report`；不依赖 GPU，CI 能测 | `formats`（以后换掉自己的 `list_unitypackage` 时） |
| `unipeek` | wgpu + ufbx 渲染：缩略图、`serve`、`Viewport`、命令行；以 `unipeek::unity` 重新导出 `unipeek-unity`，不用两个都写 | `app` |

已经发到 crates.io（`0.1.0-alpha.2` 起）：

```toml
# app/Cargo.toml
unipeek = { version = "0.1.0-alpha.2", default-features = false }   # 关掉 cli 和 view（view 会带进 winit）

# formats/Cargo.toml（以后）
unipeek-unity = "0.1.0-alpha.2"
```

要跟没发布的提交时用 git 依赖，锁 tag 或 rev，不要跟分支：

```toml
unipeek = { git = "https://github.com/vivi-shiki/unipeek", tag = "v0.1.0-alpha.2" }
```

- Snowcase CI 用 `--locked`，升级时连 `Cargo.lock` 一起提交。
- 仓库公开以后才能这样直接拉。私有期间本机要让 Cargo 用 git 命令行和已登录的凭据（`.cargo/config.toml` 里 `[net] git-fetch-with-cli = true`），CI 还要配访问令牌，所以私有期间不要接进 Snowcase 的 `master`。
- unipeek 的 wgpu 大版本跟 Snowcase 的 `Cargo.lock` 走（现在都是 30）；不依赖 GPUI，GPUI 版本完全由 Snowcase 决定。

## 缩略图

在子进程里跑，显卡驱动崩溃不会拖垮主程序。`main` 里先认参数：

```rust,no_run
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
{"id":7,"op":"thumb","source":"D:/Assets/Example.unitypackage","out":"D:/Library/thumbs/ab/abcd.webp","size":512,"view":"3q"}
{"id":7,"status":"degraded","out":"D:/Library/thumbs/ab/abcd.webp","prefab":"Assets/…/Outfit.prefab","reasons":["missing_dependency"],"missing_guids":["…"]}
```

- `source` 不带 `#` 时 unipeek 自己选主 prefab，回复里写选了哪个；写成 `Example.unitypackage#Assets/…/X.prefab` 就渲那一个（包文件树里点开某个 prefab 时用）。zip 里的包写成 `Item.zip#Item/Example.unitypackage#Assets/…`。
- 其他字段：`id` 原样带回；`view` 是 `front` / `side` / `back` / `3q`；`background` 是 `transparent`（默认）或 `#rrggbb`；`out` 只能是 `.png` 或 `.webp`（无损），先写临时文件再改名。
- `status` 是 `ok` / `degraded` / `unsupported` / `error`。降级也出图，原因放在 `reasons` 里；`missing_guids` 只列缺的 prefab 和网格（通常是素体）。没有可显示的模型时是 `unsupported`。
- 另外两个请求：`{"op":"info","source":…}` 回复里多一个完整的 `report`；`{"op":"version"}` 回复 `version` 和 `renderer_version`。
- 子进程退出或没有回复就重启它，跳过那个包，记成失败。`unipeek::serve::Worker` 就是这一侧的现成实现：

```rust,no_run
use std::{process::Command, time::Duration};
let mut worker = unipeek::Worker::new(|| {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.arg("--thumb-worker");
    c
})
.with_timeout(Duration::from_secs(120));
let request = serde_json::json!({"id": 7, "op": "thumb", "source": "D:/Assets/Example.unitypackage", "out": "D:/thumbs/abcd.webp"});
match worker.request(&request) {
    Ok(reply) => println!("{}", reply["status"]),
    // 子进程死了或超时：这一个包记失败，下一个请求会自动换新的子进程
    Err(e) => eprintln!("{e}"),
}
```
- 调度（只排可见商品、可暂停、同盘不并发）由 Snowcase 负责，适合放在「整理文件」「探查包」之后作为第三段。unipeek 保证一个请求对一个包只顺序读。

不想开子进程时（比如测试）也能直接调：

```rust,no_run
fn thumbnail(path: &str) -> Result<unipeek::Report, unipeek::Error> {
    let gpu = unipeek::Gpu::headless()?; // 一个进程建一次
    let scene = unipeek::Scene::load(unipeek::Input::from_location(path), &unipeek::LoadOptions::default())?; // 只用 CPU
    let image = unipeek::render_thumbnail(&gpu, &scene, &unipeek::ThumbOptions::default())?; // RgbaImage，512²，3q，透明背景
    println!("{}x{}", image.width(), image.height());
    Ok(scene.report) // 选了哪个 prefab、降级原因
}
```

`Scene::load` 不碰 GPU，可以放在后台线程；`LoadOptions { decode_textures: false, .. }` 只要报告时不解码贴图（尺寸从文件头读）；`report.timings` 是每一步的毫秒数；`unipeek::write_thumbnail(&gpu, location, out, &opts)` 把加载、渲染、编码和写文件一起做完。

### debug 构建要给依赖开优化

宿主的 debug 构建默认对所有依赖 `opt-level = 0`，PNG 解码和缩放会慢十倍以上：同一个有 7 张 4K PNG 的头像包，一张缩略图 release 约 1.5 s，未优化的 debug 要 17–19 s。在宿主的工作区 `Cargo.toml` 里加：

```toml
[profile.dev.package."*"]
opt-level = 3
```

只想给一部分依赖开也行，至少要 `unipeek`、`unipeek-unity`、`image`、`png`、`fdeflate`、`flate2`、`miniz_oxide`、`ufbx`。

## 命令行

`cargo install unipeek` 或直接用编译出的 `unipeek.exe`：

```text
unipeek info  <输入> [--json]        # 报告，不需要 GPU
unipeek ls    <输入>                 # 包里的文件
unipeek thumb <输入> -o out.webp [--size 512] [--view 3q] [--bg transparent] [--timings]
unipeek view  <输入>                 # 独立窗口：左键转，右键或中键平移，滚轮缩放，R 复位，1-4 视角
unipeek serve
unipeek version
```

退出码：0 正常，1 降级，2 不支持，3 出错，64 参数不对。

## 详情预览（M4）

只有读回：unipeek 离屏渲染，交出 BGRA 字节，Snowcase 包成 GPUI 的 `RenderImage`。150% DPI 下 460 px 的抽屉约 690²，每帧渲染加读回约 1.1 ms（RTX 2060，release）。

```rust,no_run
fn drawer(gpu: &unipeek::Gpu) -> Result<(), unipeek::Error> {
    let mut view = unipeek::Viewport::offscreen(gpu, 690, 690);
    // 后台线程加载；先出白模，贴图解码完再换上，相机不动
    view.load(unipeek::Input::from_location("D:/Assets/Example.unitypackage"), unipeek::LoadOptions::default());

    // 每次重绘前
    view.update(); // 取回加载结果；返回 true 表示场景变了
    let _progress = view.progress(); // Some(0.0..=1.0) 表示还在加载
    // 鼠标事件
    view.orbit().rotate(12.0, 0.0); // 拖动的像素增量
    view.orbit().zoom(1.0); // 滚轮格数
    // 渲染：相机、尺寸、场景、背景都没变时返回 None，不重渲
    if let Some(bgra) = view.render()? {
        let _ = bgra; // width * height * 4 字节，sRGB，straight alpha
    }
    Ok(())
}
```

- `resize(w, h)` 改尺寸（物理像素）；`set_background(Background::Transparent)` 要透明背景，默认是深灰。
- `orbit()` 还有 `pan`、`reset`（正面）、`preset(View::Side)` 等。
- `report()` 是当前场景的报告；`error()` 是上次 `load` 失败的原因，失败时旧场景保留。
- 不想用 `Viewport` 的加载时，`unipeek::Loading::start(input, opts)` 单独在后台加载，`poll()` 先给 `Loaded::Untextured` 再给 `Loaded::Textured`；`set_scene` 把场景交给视口。
- 包文件树里某一个 prefab 的缩略图：`source` 写成 `Example.unitypackage#Assets/…/X.prefab`，和商品主图是同一个请求。

## 跨包依赖（素体，M5，计划稿）

Snowcase 不维护词表，从名字认不出衣装适配哪个 Avatar，所以要靠 GUID：unipeek 列出 prefab 引用了但包里没有的 GUID，Snowcase 实现 `Resolver` 告诉它在哪个包。

```rust,ignore
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

## 已知限制（0.1.0-alpha.2）

- prefab 对 FBX 实例上渲染器材质的覆盖（`m_Materials` 修改）按 Unity 给模型子物体算 fileID 的规则（类型加层级路径的 xxHash64）对到 FBX 节点上；对不上的报 `material_overrides_ignored`，指向 FBX 里已经没有的物体的旧修改记在 `stale_overrides`，Unity 自己也忽略它们。
- PSD 读合成图；CMYK、16 / 32 位、索引色等会换算并报 `texture_approximated`；存的时候关了「最大兼容」的 PSD 没有真正的合成图，报 `texture_unreadable`。
- 还不读 7z、Unity 工程、glTF / VRM。
