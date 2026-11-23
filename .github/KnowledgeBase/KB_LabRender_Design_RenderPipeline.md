# Render Pipeline (lab-render)

## Data Flow

- `Model::load_fbx` (`crates/render/src/model.rs`)
  - Calls `ufbx::load_file` with right-handed Y-up axes, 1.0 meter units, `ufbx::SpaceConversion::ModifyGeometry` and generated missing normals.
  - Walks `scene.nodes`; for each node with a mesh, walks `mesh.material_parts` and triangulates every face with `ufbx::triangulate_face`.
  - Uses `mesh.skinned_position` / `mesh.skinned_normal`, transforming them with `node.geometry_to_world` only when `mesh.skinned_is_local` is set.
  - Flips V (`1.0 - uv.y`) because FBX UVs start bottom-left.
  - Emits a non-indexed `Vec<Vertex>` (position, normal, uv) and one `Part` per material part: material name, mesh node name, first vertex, vertex count.
  - Fails with `anyhow` when there is no mesh data.
- `Renderer::new` (`crates/render/src/renderer.rs`)
  - Builds one bind group layout for globals (`Globals`: view-projection, eye, light) and one for a texture plus sampler.
  - Creates two pipelines from `shader.wgsl` with a closure taking `blend: bool`.
  - Uploads all vertices into one vertex buffer.
  - For each distinct material name: `texture::guess_texture` → `texture::load_mips` → a texture with a full mip chain; or a 4×4 light grey texture when nothing matches. Each `Part` becomes a `Draw` pointing at its bind group.
  - `blend` for a part is `material.to_lowercase().contains("alpha")`.
- `Renderer::render`
  - `ensure_targets` (re)creates the MSAA color and depth targets when the size changes.
  - Writes `Globals`; the light direction is derived from the camera eye plus a fixed offset.
  - One render pass: MSAA color resolved into the caller's single-sample view, depth cleared to 1.0, both discarded after the pass.
  - Draws all opaque draws, then all blended draws.

## Pipelines

| Pipeline | Depth write | Alpha to coverage | Blend |
|---|---|---|---|
| opaque | yes | yes | `REPLACE` |
| blend | no | no | `ALPHA_BLENDING` |

- Culling is off for both (`cull_mode: None`); the shader flips the normal for back faces using `front_facing`.
- Fragments with texture alpha below 0.02 are discarded.
- Blended draws are not sorted. Correct order for overlapping transparent parts is future work (sort by part center, see Snowcase-docs 3D_PREVIEW).

## Shading (`shader.wgsl`)

- Half-Lambert `dot(n, l) * 0.5 + 0.5` through `smoothstep(0.45, 0.55, ...)` gives a soft two-tone step.
- Shadow side is tinted `(0.82, 0.74, 0.80)`, a lilToon-like default, not read from the material yet.
- A hemisphere ambient brightens upward-facing normals; a weak rim `pow(1 - n·v, 4) * 0.12` is added.

## Known Gaps (targets of upcoming tasks)

- Materials are matched by name only; `.meta` `externalObjects`, `.mat` and prefab overrides are not read.
- Transparency is guessed from the material name instead of the render queue or shader variant.
- Texture decoding is synchronous inside `Renderer::new`.
- Input is a file path (`&str`), not a reader.
- No report, no degrade reasons; problems are printed with `eprintln!`.

## Consumers

- `examples/thumb.rs` renders one frame into an `Rgba8UnormSrgb` texture, reads it back and saves a PNG.
- `examples/dump.rs` prints model statistics.
- `crates/preview` renders into DirectComposition surfaces or a BGRA readback texture.
