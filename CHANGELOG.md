# Changelog

`unipeek` and `unipeek-unity` are released together with the same version. Before 1.0, a change of
the minor version may break the API; `0.1.x` releases only fix bugs and add things.
`RENDERER_VERSION` changes whenever rendered pixels change, independently of the crate version.

## Unreleased

`RENDERER_VERSION` is 2: PSD textures that used to be drawn white are now drawn.

### unipeek-unity

- `Report::timings` (`Timings`): milliseconds spent indexing, planning, reading assets, parsing models,
  binding materials, decoding textures, rendering and encoding.
- `Degrade::TextureApproximated`: a texture was read with an approximation (CMYK or 16/32-bit PSD).
- `testing::PsdBuilder` builds Photoshop files for tests.

### unipeek

- PSD and PSB textures: the composite image is read (`psd::decode`), 8-bit RGB and grayscale as
  they are; indexed, bitmap, duotone, CMYK, 16-bit and 32-bit are converted and reported as
  `texture_approximated`; files saved without Maximize Compatibility are `texture_unreadable`.
- `LoadOptions::decode_textures`: off for reports. `info` and the serve `info` request now read texture
  sizes from the file headers instead of decoding every texture.
- `unipeek thumb --timings` prints where the time went, including GPU start-up.
- `serve::Worker`: the host side of `serve`. Runs the child process, restarts it after it dies or
  times out, and returns an error only for the request that was in flight.
- Fully transparent thumbnail pixels are always black (they used to keep a little blended color
  that could differ between runs).

## 0.1.0-alpha.2 (2026-11-24)

First release on crates.io. `0.1.0-alpha.1` was only a git tag and was never published; this release
adds everything below that landed after it, plus a README, keywords and license files for each crate.

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
