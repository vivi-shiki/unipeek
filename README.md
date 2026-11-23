# unipeek

A preview library for Unity asset packages: read a `.unitypackage`, zip or folder, work out which prefab and materials to show, and render a toon-style thumbnail or an interactive viewport. [Snowcase](https://github.com/Map1en/Snowcase) is the first user; any other tool can call it too.

> **Status: research prototype.** The code here is still the preview lab: a minimal test of how to show a wgpu-rendered model inside a GPUI window on Windows. Nothing is published yet.

Model files are never committed. Pass the paths on the command line.

## Docs

- [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md): what the render library is for, its inputs and outputs (Chinese).
- [docs/PRIOR_ART.md](docs/PRIOR_ART.md): survey of related open-source projects (Chinese).
- [docs/INTEGRATION.md](docs/INTEGRATION.md): how this library plugs into Snowcase (Chinese).
- [CLAUDE.md](CLAUDE.md), [Project.md](Project.md) and [.github/](.github/README.md): instructions for AI coding agents, adapted from GacUI.

## Layout

- `crates/render`: UI-agnostic. `ufbx` loads the FBX, textures are matched to materials by name, a small wgpu renderer draws it (toon-ish shading, MSAA 4x, orbit camera).
- `crates/preview`: one GPUI app with three ways to put the frames on screen.

| mode | GPUI backend | how |
|---|---|---|
| `overlay` | upstream (gpui-kit default) | our own DirectComposition device, `CreateTargetForHwnd(hwnd, topmost = true)`, wgpu presents to the visual |
| `fast` | gpui-fast (`--features fast`) | `window.enable_window_composition()?.create_native_surface()`, wgpu presents to that visual |
| `readback` | upstream | offscreen render, read back, new `RenderImage` each frame |

## Run

```
set M=<model folder>
cargo run -p preview -- overlay  %M%\model.fbx %M%\textures 1100x800
cargo run -p preview -- readback %M%\model.fbx %M%\textures 1100x800
cargo run -p preview --features fast --target-dir target-fast -- fast %M%\model.fbx %M%\textures 1100x800
cargo run --release -p lab-render --example thumb -- %M%\model.fbx %M%\textures out\front.png 1024 1024
```

Left drag rotates, middle drag pans, wheel zooms. `Popup test` cycles a GPUI panel over the viewport.
Env: `LAB_SPIN=1` rotates continuously, `LAB_SECS=n` quits after n seconds, `LAB_POPUP=0|1|2` sets the popup state at start.

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

