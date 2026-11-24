# Changelog

`unipeek` and `unipeek-unity` are released together with the same version. Before 1.0, a change of
the minor version may break the API; `0.1.x` releases only fix bugs and add things.
`RENDERER_VERSION` changes whenever rendered pixels change, independently of the crate version.

## 0.1.0-alpha.1 (unreleased)

First version a host can depend on (roadmap M1 to M3). `RENDERER_VERSION` is 1.

### unipeek-unity

- `PackageIndex`: one streaming pass over a `.unitypackage` (gzip tar). Keeps `pathname`, `.meta` and the
  Unity YAML worth reading, measures everything else; flags hostile paths, duplicates, missing pathnames
  and truncation. `read_assets` fetches FBX and texture bytes in a second pass.
- `Package::open`: unitypackages from a path, bytes or a one-shot reader; zips (the largest
  unitypackage inside, or loose FBX and images with Shift-JIS names); folders; a single FBX with the
  textures next to it. `Input::from_location` understands `a.zip#b.unitypackage#Assets/x.prefab`.
- Unity YAML reader, `.meta` (material remapping, normal maps), `.mat` and the approximate `Look`
  (mode, color, shadow color, cull, cutoff, lilToon outline).
- Shader recognition from the lilToon 1.8.3 GUID table, Unity built-ins, and shader names found in the
  package (Poiyomi, MToon).
- `Plan`: chooses the main prefab (the plain prefab with the package's largest model before variants
  and extras), follows nested prefabs, and binds every FBX material slot through prefab renderers,
  `externalObjects`, material names and texture names. Lists GUIDs missing from the package.
- `Report` with degrade reasons and `Status` (ok, degraded, unsupported, error). Binary-serialized or
  oversized `.mat` files are reported as `material_unreadable`.

### unipeek

- `Scene::load`: CPU-only loading of models (ufbx), bindings and textures (decoded on worker threads,
  scaled and mip-mapped), with triangle, bone and blend shape counts in the report.
- Renderer for scenes: opaque, cutout, transparent and inverted-hull outline passes, per-material cull.
- `Gpu::headless` tries DX12, then Vulkan or Metal, then any backend, and keeps the failed attempts in
  `Gpu::fallback`; `serve` replies name the backend that rendered.
- `render_thumbnail` / `write_thumbnail` on a headless device, PNG and
  lossless WebP, transparent or solid background, views front, side, back and 3q.
- `serve`: one JSON request per line (thumb, info, version) for a host's child process.
- `unipeek` command line (feature `cli`, on by default): `info [--json]`, `ls`, `thumb`, `serve`,
  `version`. Exit codes 0 ok, 1 degraded, 2 unsupported, 3 error, 64 usage.

### Known limits

- Material overrides that a prefab applies to an instance of an FBX (`m_Materials` modifications on
  the model's renderers) are not mapped yet; they are reported as `material_overrides_ignored`.
- PSD textures, 7z archives, Unity projects, glTF and VRM are not read yet.
- No interactive `Viewport` yet (roadmap M4) and no cross-package `Resolver` yet (M5).
