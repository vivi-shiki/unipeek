# Putting wgpu Frames into a GPUI Window (preview)

## Modes

Selected by the first command line argument of `preview` (`crates/preview/src/main.rs`).

### `overlay` (upstream GPUI)

- `TopmostLayer` in `crates/preview/src/backend.rs` creates its own DirectComposition device and a target with `CreateTargetForHwnd(hwnd, topmost = true)` on the window handle GPUI exposes through `HasWindowHandle`.
- A child visual of that target is passed to `VisualSurface::new`, which creates a wgpu surface with `wgpu::SurfaceTargetUnsafe::CompositionVisual`. This requires the DX12 backend.
- `VisualSurface::resize` configures the swap chain; because wgpu only calls `SetContent`, the layer commits its DComp device after every resize and after moving the visual.
- Why it works: each HWND can have one `topmost = false` and one `topmost = true` target. gpui-pre 0.3.12+ takes `false`, leaving `true` free. On 0.3.11 GPUI took `true` and this path fails.
- Consequence: the 3D layer is above all GPUI content, so GPUI popups over the 3D area are hidden. Mouse input still reaches GPUI because visuals do not take input.

### `fast` (gpui-fast, `--features fast`)

- `FastLayer` in `crates/preview/src/fast.rs` calls `window.enable_window_composition()` and `create_native_surface()`; the platform handle is downcast to `IUnknown` and handed to `VisualSurface::new`.
- gpui-fast stacks this visual between its base scene and its deferred / overlay surface, so popups drawn with `deferred` appear above the 3D area.
- `FastLayer::place` configures first and then calls `set_bounds`, which commits the tree.
- The whole application must use the gpui-fast backend, which Snowcase does not.

### `readback` (upstream GPUI)

- `Readback` in `crates/preview/src/backend.rs` renders into an offscreen `Bgra8UnormSrgb` texture (GPUI's `RenderImage` is BGRA), copies it to a buffer with padded rows, maps it, strips the padding and returns the bytes.
- The view builds a new `RenderImage` from those bytes every frame.
- Always correct with respect to popups and layout. Cost grows with the pixel count, and GPUI's own atlas upload is not included in the measured time.

## Measurements

See `## Results` in `REPO-ROOT/README.md` (2026-11-14, RTX 2060, dev build): overlay 1.1–1.2 ms, fast 1.3 ms, readback 3.4 ms at 1100×760 and 7.2 ms at 2400×940 per frame of our own work. All modes held the display refresh rate.

## Not Covered Yet

- Dragging with the mouse, live window resizing, DPI changes, device loss, the Intel iGPU.
- A child HWND inside the GPUI window as an alternative zero-copy path (suggested in `REPO-ROOT/docs/PRIOR_ART.md`).

## Decision Derived from This

Snowcase's detail drawer is 460 px wide, so readback is used first; zero-copy is for a large or full-screen view later (`REPO-ROOT/docs/INTEGRATION.md`).
