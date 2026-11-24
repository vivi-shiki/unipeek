# Knowledge Base

## Guidance

The goal comes first. Before choosing an API, check `## Development Goal` in `REPO-ROOT/Project.md`.

The following choices are preferred:

- Use `std::io::Read` (plus `std::io::Seek` only when the format needs it, like zip) for every input. A path is just one way to get a reader.
- Use `glam::Vec3`, `glam::Mat4` for math. Use `bytemuck::Pod` structs with `#[repr(C)]` for anything uploaded to the GPU.
- Use `image::RgbaImage` for decoded pixels. Textures on the GPU are `Rgba8UnormSrgb`; frames for GPUI are `Bgra8UnormSrgb`.
- Use `ufbx` for FBX, never a hand-written parser.
- Use `tar` + `flate2` for `.unitypackage`, `zip` for zip, `sevenz-rust2` for 7z, `encoding_rs` for Shift-JIS names. Align versions with Snowcase.
- Use `anyhow` only in binaries, examples and `preview`. Library crates expose their own error type.
- Use `pollster::block_on` only at the edge (examples, CLI, device creation). Do not spread async through the library.
- Degrade, don't fail: an unknown shader, a missing texture or a missing dependency becomes a reason in the report, not an `Err`.

[Learning of Coding Preference](./Learning.md)

## unipeek-unity

Folder: `REPO-ROOT/crates/unity`

GPU-free parsing. Opens any input as a `PackageIndex`, decides what to show (`Plan`), binds every FBX material slot, and writes the `Report`. It never depends on `wgpu`.

### Choosing APIs

#### Opening Inputs

- Use `Input::from_location` to split `a.zip#b.unitypackage#Assets/x.prefab` into a source and inner paths; `Source` is a path, bytes or a one-shot reader.
- Use `Package::open` to index a unitypackage, zip, folder or single FBX; loose files get GUIDs from their paths. Use `Package::read_assets` for FBX and texture bytes.
- Use `PackageIndex::read` / `read_assets` directly for a `.unitypackage` stream.

#### Unity Assets

- Use `yaml::parse_documents` / `yaml::parse` for Unity YAML, `Meta::parse`, `Material::parse`, `Prefab::parse` for the typed views.
- Use `shader::identify` for a shader reference (lilToon GUID table, Unity built-ins, names found in the package) and `Look::of` for the approximate look of a material.

#### Planning and Reporting

- Use `Plan::new` to choose the main prefab and its models, and `Plan::bind_slot` (renderer slots first) or `Plan::bind` (names) for one FBX material slot.
- Use `Report::new`, `Report::add_binding` and `Report::degrade` to collect what was found; `Status::exit_code` maps it to the CLI.
- Use the `testing` feature (`PackageBuilder`, `zip`, `fbx_ascii`, `material_yaml`, `model_meta`, `prefab_instancing`, `prefab_with_renderer`) to build synthetic inputs in tests.

## unipeek

Folder: `REPO-ROOT/crates/render`

Rendering. `Scene::load` prepares everything on the CPU (ufbx, bindings, textures decoded on worker threads with mips); the renderer draws a `Scene`; thumbnails, `serve` and the `unipeek` binary sit on top.

### Choosing APIs

#### GPU Device

- Use `Gpu::headless` for offscreen work (DX12, then Vulkan or Metal, then any backend); `Gpu::new` with `Gpu::instance` when a surface is involved (DX12 for DirectComposition).
- Use `Gpu::read_texture` to copy a single-sample RGBA or BGRA target back to the CPU.

#### Scenes and Models

- Use `Scene::load` with `LoadOptions` (`max_texture`) to get geometry, `SceneMaterial`s with `Look` and mips, bounds and the report. It never touches the GPU.
- Use `Model::from_fbx_bytes` / `Model::load_fbx` for raw FBX geometry; `Part::slot` is the submesh index a Unity renderer uses.

#### Rendering and Thumbnails

- Use `Renderer::new` with a `Scene` and a target format, `Renderer::render` per frame, `Renderer::clear` for the background.
- Use `render_thumbnail` for an RGBA image, `write_thumbnail` to load, render and write a `.png` or `.webp` file, `encode` for bytes. Bump `RENDERER_VERSION` whenever pixels change.
- Use `serve::serve` for the JSON line protocol and the `unipeek` binary (feature `cli`) for the command line.

#### Camera

- Use `Orbit::fit` to frame a bounding box; `Orbit::rotate`, `Orbit::pan`, `Orbit::zoom` for mouse input; `Orbit::view_proj` and `Orbit::eye` for shader constants.

### Design Explanation

#### Render Pipeline

- Passes: solid (opaque and cutout, depth write), inverted-hull outline (front faces culled, extruded along normals by the lilToon width in meters), then alpha-blended. One pipeline per pass and cull mode.
- Per-material uniform: color, shadow color, cutoff and mode; cutout discards below `_Cutoff`, opaque forces alpha 1.
- Thumbnails clear to transparent black and un-premultiply the resolved edges after readback.
- The key light follows the camera; shading is half-Lambert through a soft step tinted by the material's shadow color, plus a weak rim.

[Design Explanation](./KB_LabRender_Design_RenderPipeline.md)

## preview

Folder: `REPO-ROOT/crates/preview`

A GPUI application that compares three ways to show `unipeek` frames inside a GPUI window on Windows. It is research code and will never be published.
It is the evidence behind the "readback first, zero-copy later" decision in `REPO-ROOT/docs/INTEGRATION.md`.

### Design Explanation

#### Putting wgpu Frames into a GPUI Window

The three presentation paths and why each one behaves as measured.

- `overlay`: our own DirectComposition device creates a `topmost = true` target on GPUI's HWND. It works only because gpui-pre 0.3.12+ uses `topmost = false` for itself. GPUI popups end up behind the 3D layer.
- `fast`: gpui-fast gives a native composition surface between its base scene and its overlay, so popups draw above the 3D area (they need `deferred`). The whole UI must switch to the gpui-fast backend.
- `readback`: render offscreen as BGRA, copy to a buffer, map it, and hand GPUI a new `RenderImage` each frame. Simplest and always correct, cost grows with the viewport size.
- `VisualSurface` wraps a wgpu surface on an `IDCompositionVisual`; wgpu only calls `SetContent`, so the owner must commit the DComp device after configure.

[Design Explanation](./KB_Preview_Design_GpuiEmbedding.md)

## wgpu (external)

Version 30 (`Cargo.lock` has 30.0.1). Snowcase's lock has the same version.

### Choosing APIs

#### Surfaces on DirectComposition

- Use `wgpu::SurfaceTargetUnsafe::CompositionVisual` with an `IDCompositionVisual` pointer to present without copying. Only the DX12 backend supports it.
- Use `wgpu::Surface::get_capabilities` to pick an sRGB format and Mailbox when available.

#### Reading Frames Back

- Use `wgpu::CommandEncoder::copy_texture_to_buffer` into a `MAP_READ` buffer, with rows padded to `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`, then `wgpu::Buffer::map_async` and poll the device.
- Strip the row padding before handing pixels to `image` or GPUI.

## ufbx (external)

Version 0.11 (MIT OR Unlicense). C code compiled by the crate's build script.

### Choosing APIs

#### Loading and Converting

- Use `ufbx::load_file` (later `ufbx::load_memory` for entries without a path) with `ufbx::LoadOpts` setting `target_axes`, `target_unit_meters` and `space_conversion`.
- Use `ufbx::triangulate_face` per face with a buffer of `mesh.max_face_triangles * 3` indices.
- Use `mesh.material_parts` to group faces by material, `mesh.skinned_position` / `mesh.skinned_normal` for positions in bind pose.

## gpui-kit / gpui-pre (external)

gpui-kit 0.9.1 re-exports gpui-pre 0.3.12 (same as Snowcase). On Windows GPUI renders with D3D11 into its own DirectComposition target.

### Design Explanation

#### What the Embedding Depends On

- gpui-pre-windows 0.3.11 used `CreateTargetForHwnd(hwnd, true)`; 0.3.12 to 0.3.14 use `false`. The `overlay` path needs `false`. Never use 0.3.11 or older.
- Upstream PR #66676 (not merged on 2026-11-23) would add an option that switches GPUI back to `true`. Never enable it.
- See `REPO-ROOT/docs/PRIOR_ART.md` section 对已有方案的修正和补充 for sources.

## Snowcase (host application)

The first user, a GPL-3.0 GPUI application. Its code is never copied here.

### Design Explanation

#### Integration Contract

- Snowcase decides what to render and owns the GUID index (`cache.db` `package_asset`), thumbnails folder and UI. unipeek takes sources and a `Resolver`, and returns images, frames and reports.
- Snowcase's `formats` crate will depend on unipeek's GPU-free parsing crate. Keep that crate free of wgpu.
- The detail drawer is 460 px wide, so the first viewport is readback.
- Thumbnails run in a child process to survive driver crashes; `serve` must be a library function.

[Design Explanation](../../docs/INTEGRATION.md)

## Experiences and Learnings

- Measured numbers belong in `REPO-ROOT/README.md` `## Results`, with the date, hardware and build profile.
- Learnings from past tasks are in `./Learning.md`, `../Learning/Learning_Coding.md` and `../Learning/Learning_Testing.md`.

# External References

| Document | What it is good for |
|---|---|
| `REPO-ROOT/docs/REQUIREMENTS.md` | Full requirement list with stages (Chinese) |
| `REPO-ROOT/docs/INTEGRATION.md` | Snowcase's current state, interfaces, who changes what (Chinese) |
| `REPO-ROOT/docs/PRIOR_ART.md` | Related open-source projects and what to borrow from each (Chinese) |
| https://github.com/Map1en/Snowcase-docs | Snowcase architecture, decisions, data model (Chinese) |
| https://github.com/tinatsu-nomy/popone | unitypackage → prefab → `.mat` → texture chain, lilToon / Poiyomi → MToon mapping (0BSD, may be ported with a source comment) |
| https://github.com/mochiya-labs/three-liltoon | lilToon shading formulas and the per-variant feature matrix (MIT) |
| https://docs.rs/wgpu/30.0.1 | wgpu API |
| https://docs.rs/ufbx | ufbx API |
