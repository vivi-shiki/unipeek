# snowcase-preview-lab

Minimal test of how to show a wgpu-rendered Unity model inside a GPUI window on Windows.
Throwaway code for the Snowcase 3D preview research; not part of Snowcase.

Model files are never committed. Pass the paths on the command line.

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
set K=<model folder>
cargo run -p preview -- overlay  %K%\model.fbx %K%\textures 1100x800
cargo run -p preview -- readback %K%\model.fbx %K%\textures 1100x800
cargo run -p preview --features fast --target-dir target-fast -- fast %K%\model.fbx %K%\textures 1100x800
cargo run --release -p lab-render --example thumb -- %K%\model.fbx %K%\textures out\front.png 1024 1024
```

Left drag rotates, middle drag pans, wheel zooms. `Popup test` cycles a GPUI panel over the viewport.
Env: `LAB_SPIN=1` rotates continuously, `LAB_SECS=n` quits after n seconds, `LAB_POPUP=0|1|2` sets the popup state at start.

## Results (2026-11-14, i7-8750H, RTX 2060, 75 Hz display, dev build)

Test model: 69,640 triangles, parsed in about 90 ms. Loading eight 4K PNGs and building mips takes 6 to 9 s on the UI thread and needs to move off it.

| mode | 1100x760 | 2400x940 | popup over the 3D area |
|---|---|---|---|
| `overlay` | 1.1 ms/frame | 1.2 ms/frame | hidden behind the 3D layer |
| `fast` | 1.3 ms/frame | 1.3 ms/frame | drawn above it (needs `deferred`) |
| `readback` | 3.4 ms/frame | 7.2 ms/frame | drawn above it |

The time is what our code spends per frame: render and present, or render, read back and build the `RenderImage`. GPUI's own upload of the read-back image into its atlas is not included. All modes held the display's 76 fps.

Not tested: dragging with the mouse, live window resizing, DPI changes, device loss, the Intel iGPU.
