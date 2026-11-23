# Unity Asset Formats

A brief introduction to the formats unipeek reads. It is the equivalent of a syntax guide: read it before touching parsing code.
Facts marked ✓ were checked against real packages, Snowcase's code or popone's code. Facts marked ～ are from memory or documentation and must be checked against a real file before code depends on them.
When you confirm or correct a ～ fact, update this file.

## `.unitypackage`

- A gzip-compressed tar archive ✓. There is no index and no random access: reading one entry means decompressing everything before it.
- Every asset is a folder named by its 32-hex-digit GUID ✓:
  - `GUID/pathname`: the asset's path in the Unity project, e.g. `Assets/Shop/Avatar/Avatar.prefab`. Only the first line counts, and a trailing `\r` must be trimmed ✓ (Snowcase does exactly this in `formats::list_unitypackage`).
  - `GUID/asset`: the file content. Folders have no `asset` ✓.
  - `GUID/asset.meta`: the importer settings, Unity YAML ～.
  - `GUID/preview.png`: an optional 128×128 preview made by Unity ✓. Good enough as a fallback cover, not as a thumbnail.
- Entry names may start with `./` ✓. Folders whose name is not a GUID must be skipped ✓.
- Entry order is not fixed ✓. `pathname` may come after `asset`, so decide what to keep by file size and the extension of `pathname` once it is known, and resolve references only after the whole scan.
- Packages are often 1–5 GB ✓; indexing must use constant memory. Keep small text entries (`pathname`, `.meta`, `.mat`, `.prefab`) in memory, and only hash or skip big binary entries.
- `pathname` comes from the package author. Treat `../`, absolute paths and drive letters as hostile when building any output path.

## Unity YAML (`.meta`, `.mat`, `.prefab`, `.asset`)

- Text serialization starts with `%YAML 1.1` and `%TAG !u! tag:unity3d.com,2011:` ～.
- A file is a stream of documents, each with a header like `--- !u!21 &2100000`: `21` is the class ID, `2100000` is the file-local ID ～.
  - Class IDs that matter: `1` GameObject, `4` Transform, `21` Material, `23` MeshRenderer, `137` SkinnedMeshRenderer, `1001` PrefabInstance ～.
- References are `{fileID: N, guid: GUID, type: T}`. A reference without `guid` points into the same file ～.
- Unity can also save these files in binary form. Detect it and degrade with a reason instead of failing ～.
- Do not use a general YAML library blindly: Unity uses custom tags and some files are huge. A small line-based reader for the few fields needed is acceptable, popone does this ✓.

## Material Binding

The chain unipeek must follow, from strongest to weakest ✓ (popone's three-level matching, Snowcase-docs 3D_PREVIEW):

1. Prefab: `m_Materials` of the renderers, and `m_Modifications` of a `PrefabInstance` overriding them.
2. FBX `.meta`: `externalObjects` maps a material name inside the FBX to a `.mat` GUID.
3. Material name matching against `.mat` files and textures, as the last fallback. This is what `texture::guess_texture` in `crates/render` does today.

- Prefabs come in five forms: New, Old, Unpacked, Mixed and Variant ✓ (popone). Variants form a chain; detect cycles and stop at 32 levels ✓ (popone's limit).

## `.mat`

- `m_Shader` is a reference to the shader; the shader itself is often not in the package (lilToon and Poiyomi are installed through VPM) ✓. Recognize shaders by a table of known shader GUIDs, and by name as a fallback.
- `m_SavedProperties` holds `m_TexEnvs` (textures), `m_Floats` and `m_Colors` ～.
- Common properties: main texture `_MainTex` / `_BaseMap`, color `_Color` / `_BaseColor`, normal map `_BumpMap`, culling `_Cull` ✓ (Snowcase-docs 3D_PREVIEW).
- Render queue: 2450 and above is cutout, 3000 and above is transparent ✓ (same source).
- lilToon decides transparency and outline by shader variant, not by a float switch ✓ (PRIOR_ART, three-liltoon's feature matrix). Look at the shader GUID / name first.
- An unknown shader is not an error: render white plus the main texture and record the reason.

## FBX

- Read with `ufbx`. Ask it for right-handed Y-up meters and modified geometry, like `Model::load_fbx` in `crates/render/src/model.rs` ✓.
- FBX UV origin is bottom-left; flip V for top-left textures ✓ (same file).
- The `.meta` of an FBX may set a scale factor and axis conversion; apply it on top of ufbx's conversion ～.
- Both binary and ASCII FBX appear in real packages ✓.

## Textures

- Mostly 4K PNG, often with a layered PSD next to it ✓. PSD uses the composite image; 16-bit, CMYK or huge PSDs degrade with a reason ～.
- Decode and build mips off the UI thread. In the lab, eight 4K PNGs take 6 to 9 seconds ✓ (README results).

## Zip and Other Containers

- Zip file names without the UTF-8 flag are Shift-JIS in Japanese packages ✓ (Snowcase-docs sample survey).
- A zip may contain a whole Unity project with a `Library/` folder of more than 1 GB ✓. Only read `Assets/` and `Packages/`.
