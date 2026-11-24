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

## unipeek

Folder: `REPO-ROOT/crates/render`

The UI-agnostic part of the lab. It loads an FBX with ufbx into a flat triangle list grouped by material, matches textures to materials by file name, and draws the model with a small wgpu forward renderer (MSAA 4x, toon-ish shading, orbit camera).
It is the seed of the future `unipeek` crate. The parsing half will move to `unipeek-unity`.
It does not read `.unitypackage`, `.meta`, `.mat` or prefabs yet.

### Choosing APIs

#### GPU Device

Create the wgpu instance, adapter, device and queue shared by every front end.

- Use `Gpu::instance` to create a `wgpu::Instance` limited to some backends (DX12 is required for DirectComposition surfaces).
- Use `Gpu::new` to pick a high-performance adapter, optionally compatible with a surface, and create the device and queue.

#### Model Loading

Turn an FBX file into a vertex soup ready for one vertex buffer.

- Use `Model::load_fbx` to load an FBX converted to right-handed, Y-up, meters, with skinning applied and missing normals generated.
- Use `Model::parts` (`Part`) to know which range of vertices uses which material name and comes from which mesh node.
- Use `Model::min` / `Model::max` for the bounding box, and `Model::triangles` for the triangle count.

#### Textures

Find and decode the texture for a material.

- Use `texture::guess_texture` to find the shortest PNG file name ending with the material name, case-insensitive. It is the last-level fallback of material binding.
- Use `texture::load_mips` to decode a PNG, clamp it to `texture::MAX_SIZE` and build a mip chain on the CPU.

#### Rendering

Draw a model into any color target.

- Use `Renderer::new` to build pipelines, upload vertices and one texture per distinct material for a given target format.
- Use `Renderer::render` to record one frame into a single-sample `wgpu::TextureView`; MSAA and depth targets are created and resized internally.
- Use `Renderer::clear` to set the background color (alpha 0 for transparent thumbnails).

#### Camera

Orbit around the model.

- Use `Orbit::fit` to frame a bounding box.
- Use `Orbit::rotate`, `Orbit::pan`, `Orbit::zoom` to feed mouse deltas and wheel steps.
- Use `Orbit::view_proj` and `Orbit::eye` when writing shader constants.

### Design Explanation

#### Render Pipeline

How a frame is produced from a `Model`.

- Two pipelines share one shader: opaque (depth write, alpha-to-coverage) and blended (no depth write, alpha blending). Opaque parts are drawn first.
- A part is blended only when its material name contains `alpha`. This is a placeholder for render queue and shader-variant rules.
- The key light follows the camera so the model is never back-lit; shading is half-Lambert through a soft step with a tinted shadow and a weak rim.
- A material without a matching texture gets a 4×4 light grey texture and a log line.
- Texture decoding happens inside `Renderer::new`, on the caller's thread. This is the 6–9 s stall noted in the README.

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
