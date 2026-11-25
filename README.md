# unipeek

A preview library for Unity asset packages: read a `.unitypackage`, zip or folder, work out which prefab and materials to show, and render a toon-style thumbnail or an interactive viewport. [Snowcase](https://github.com/Map1en/Snowcase) is the first user; any other tool can call it too.

> **Status: 0.1.0-alpha.2 on [crates.io](https://crates.io/crates/unipeek).** Thumbnails, reports, the `serve` protocol and the command line work (roadmap M1 to M3). The interactive viewport (M4) and cross-package dependencies (M5) are next. See [CHANGELOG.md](CHANGELOG.md).

Model files are never committed. Pass the paths on the command line.

## Docs

- [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md): what the render library is for, its inputs and outputs (Chinese).
- [docs/PRIOR_ART.md](docs/PRIOR_ART.md): survey of related open-source projects (Chinese).
- [docs/INTEGRATION.md](docs/INTEGRATION.md): how this library plugs into Snowcase (Chinese).
- [docs/USAGE.md](docs/USAGE.md): how a host calls the library, the `serve` protocol and the command line (Chinese).
- [docs/ROADMAP.md](docs/ROADMAP.md): milestones M0 to M6 (Chinese).
- [CLAUDE.md](CLAUDE.md), [Project.md](Project.md) and [.github/](.github/README.md): instructions for AI coding agents, adapted from GacUI.

## Layout

- `crates/unity` (`unipeek-unity`): GPU-free. Streams a `.unitypackage`, reads Unity YAML, `.meta` and `.mat`, chooses the prefab and binds materials and textures, and writes the report.
- `crates/render` (`unipeek`): loads FBX with `ufbx`, decodes textures off the caller's thread, renders toon-style thumbnails on a headless wgpu device, runs `serve`, and builds the `unipeek` command line.
- `crates/preview`: research GPUI app with three ways to put frames on screen. Never published.

| mode | GPUI backend | how |
|---|---|---|
| `overlay` | upstream (gpui-kit default) | our own DirectComposition device, `CreateTargetForHwnd(hwnd, topmost = true)`, wgpu presents to the visual |
| `fast` | gpui-fast (`--features fast`) | `window.enable_window_composition()?.create_native_surface()`, wgpu presents to that visual |
| `readback` | upstream | offscreen render, read back, new `RenderImage` each frame |

## Run

`<input>` is a `.unitypackage`, `.zip`, folder or `.fbx`, optionally followed by `#<path inside>`.

```
cargo run --release -p unipeek -- info  <input> [--json]
cargo run --release -p unipeek -- thumb <input> -o out/thumb.webp --size 512 --view 3q
cargo run --release -p unipeek -- view  <input>
cargo run --release -p unipeek -- serve
cargo run -p preview -- readback <input> 1100x800
cargo run -p preview --features fast --target-dir target-fast -- fast <input> 1100x800
```

Exit codes: 0 ok, 1 degraded, 2 unsupported, 3 error, 64 usage.
In the preview, left drag rotates, middle drag pans, wheel zooms. `Popup test` cycles a GPUI panel over the viewport.
Env: `LAB_SPIN=1` rotates continuously, `LAB_SECS=n` quits after n seconds, `LAB_POPUP=0|1|2` sets the popup state at start.

## Tests

Tests that run anywhere build tiny packages, zips and ASCII FBX files in the test itself; GPU tests print `skipped:` when there is no adapter.
Real samples are paid assets and stay outside the repo: set `UNIPEEK_SAMPLES` to their folder, which also holds the manifest `unipeek-samples.json`, to run `crates/render/tests/local_samples.rs`. `UNIPEEK_BIG=1` adds a 3 GB streaming test.

## Results (2026-11-14, i7-8750H, RTX 2060, 75 Hz display, dev build)

Test model (a BOOTH avatar FBX, not included): 69,640 triangles, parsed in about 90 ms. Loading eight 4K PNGs and building mips takes 6 to 9 s on the UI thread and needs to move off it.

| mode | 1100x760 | 2400x940 | popup over the 3D area |
|---|---|---|---|
| `overlay` | 1.1 ms/frame | 1.2 ms/frame | hidden behind the 3D layer |
| `fast` | 1.3 ms/frame | 1.3 ms/frame | drawn above it (needs `deferred`) |
| `readback` | 3.4 ms/frame | 7.2 ms/frame | drawn above it |

The time is what our code spends per frame: render and present, or render, read back and build the `RenderImage`. GPUI's own upload of the read-back image into its atlas is not included. All modes held the display's 76 fps.

Not tested: dragging with the mouse, live window resizing, DPI changes, device loss, the Intel iGPU.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

