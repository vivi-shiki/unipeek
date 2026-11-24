# Project Specific Instruction

## Development Goal

unipeek is a preview library for Unity asset packages.
It is not yet another general purpose 3D viewer.
Given an asset package, it works out what should be shown (which prefab, which materials, which textures),
renders a toon-style approximation, and reports what it found and why it had to degrade.
[Snowcase](https://github.com/Map1en/Snowcase) is the first user, but the library must stay usable by any other tool.

The authoritative requirement list is `REPO-ROOT/docs/REQUIREMENTS.md` (Chinese).
The integration contract with Snowcase is `REPO-ROOT/docs/INTEGRATION.md` (Chinese).
The survey of related open-source projects is `REPO-ROOT/docs/PRIOR_ART.md` (Chinese).
The milestones M0–M6 with detailed requirements are in `REPO-ROOT/docs/ROADMAP.md` (Chinese).
How a host such as Snowcase calls the library is in `REPO-ROOT/docs/USAGE.md` (Chinese). Keep it in sync with the public API.
When this section and those documents disagree, those documents win, and you should point out the conflict.

Items marked with ★ are required for the first release. Everything else must not be started unless a task asks for it.

### Input

| Input | Requirement | Stage |
|---|---|---|
| `.unitypackage` | Streamed in a single pass with constant memory. Packages over 3 GB must be indexed. Entry order inside the tar is not fixed, resolve GUIDs after the scan. Only the first line of `pathname` counts. | ★ |
| FBX / PNG / PSD in a zip or a folder | Binary and ASCII FBX. PSD uses the composite image. Zip file names without the UTF-8 flag are decoded as Shift-JIS. | ★ |
| Byte streams | Besides paths, accept `Read` or bytes. Members of an archive and cold-archived files have no path of their own. | ★ |
| Inner path | `Avatar.unitypackage#Assets/Avatar/Avatar.prefab` selects one asset inside a package. | ★ |
| `Resolver` | Implemented by the caller. Locates files by GUID to complete cross-package dependencies (base avatar, shader). Must be batched per package, because a `.unitypackage` cannot be read at random. | ★ |
| Unity project folder | Only `Assets/` and `Packages/`, always skip `Library/`. | P2 |
| glTF / GLB / VRM, OBJ, 7z, nested archives | Nested archives are limited in depth and total size. | Later |

### Output

| Output | Format | Stage |
|---|---|---|
| Thumbnail | 512×512 by default, transparent background, PNG or WebP. Size and view (front, side, back, 3/4) are options. | ★ |
| Raw frame | RGBA8 bytes, plus PNG / JPEG encoding. | ★ |
| Interactive `Viewport` | Offscreen frame read back as BGRA first. Later: draw into a DirectComposition visual or a window handle given by the caller. Camera and input events come from the caller. | ★ |
| `Report` | A Rust struct with `serde::Serialize`, printed as JSON by `--json`: triangles, bones, BlendShapes, materials, shaders, texture sizes, missing GUIDs. | ★ |
| Degrade reasons | A list inside `Report`, e.g. missing base avatar, unknown shader, unreadable PSD. Degrading is never an error. | ★ |
| Exit codes | Distinguish success / degraded success / unsupported / error. | ★ |
| Standalone window `view` | For a command line user to look at one package. | ★ |
| `serve` | A long-running process, one JSON object per line on stdin / stdout. Also a library function so a host can run it in a child process. | When Snowcase takes thumbnails |
| Turntable frames | 12–24 PNG / WebP frames. | P1 |
| `RENDERER_VERSION` | A constant the caller puts into its thumbnail cache key. Bump it whenever the rendered pixels change. | ★ |

### Command Line

```
unipeek info  <input> [--json]
unipeek ls    <input>
unipeek thumb <input> -o out.webp [--size 512] [--view front|side|back|3q] [--bg transparent]
unipeek turn  <input> -o dir/ [--frames 24]
unipeek view  <input>
unipeek serve
```

`--with <package or folder>` can be repeated to add more GUID sources.

### Never Do

- Never modify, move or write next to input files. unipeek is read-only on its inputs.
- Never let an inner path with `../` or an absolute path escape the output folder.
- Never fail because a material is not recognized. Render white plus the main texture, and record the reason.
- Never aim for pixel parity with Unity, and never execute Unity shader source code.
- Never add a GPL dependency. The library is MIT OR Apache-2.0.
- Never copy code from Snowcase (GPL-3.0) into this repo.
- Never depend on GPUI from the library crates. GPUI glue belongs to the host application.
- Never commit model files, textures, packages, extracted package contents, or screenshots of paid assets.
- Never commit local absolute paths (e.g. a drive letter path to a sample folder) or names of paid sample assets.
- Never do asset management, databases or GUID ownership. That belongs to the caller.

## Workspace to Work On

You are working on the Cargo workspace `REPO-ROOT/Cargo.toml`,
therefore `WORKSPACE-ROOT` is `REPO-ROOT`.

| Crate | Folder | Purpose | Published |
|---|---|---|---|
| `lab-render` | `REPO-ROOT/crates/render` | UI-agnostic loading and rendering. Will be split into `unipeek-unity` (parsing, no GPU) and `unipeek` (rendering). | Not yet |
| `preview` | `REPO-ROOT/crates/preview` | GPUI test application with three ways to put frames on screen: `overlay`, `fast`, `readback`. Research code, never published. | Never |

Planned crates, follow `REPO-ROOT/docs/INTEGRATION.md` before creating them:
- `unipeek-unity`: unitypackage streaming index, Unity YAML, prefab → FBX `.meta` → `.mat` → texture binding, `Report`. MUST build and test without a GPU.
- `unipeek`: wgpu renderer, ufbx loading, thumbnails, `Viewport`; feature `cli` for the command line and `serve`.

## Files not Allowed to Modify

- `REPO-ROOT/Cargo.lock` must only be changed by cargo, never by hand.
- `REPO-ROOT/target`, `REPO-ROOT/target-fast`, `REPO-ROOT/out` and `REPO-ROOT/assets` are local and ignored by git. Never add them.
- `REPO-ROOT/LICENSE-MIT` and `REPO-ROOT/LICENSE-APACHE` must not be changed.
- `REPO-ROOT/docs/*.md` record decisions made with me. Only change them when the task asks for it.

## Sample Assets

Real samples are paid BOOTH assets and live outside the repo.
- Tests that need a real sample must read its location from the environment variable `UNIPEEK_SAMPLES`, and skip with a printed message when it is not set.
- Tests that run in CI must use generated fixtures: build a tiny `.unitypackage`, zip, FBX or PNG in the test itself, or commit a small file you created yourself.
- Never print the content of a sample into logs or task documents beyond names and sizes.

## Projects for Verification

You are required to follow the guideline to run anything in this workspace,
do not call `cargo` directly unless a guideline says so.

- When any `*.rs`, `*.wgsl` or `Cargo.toml` file is changed, build the workspace and run the unit tests of the affected crates.
- `unipeek-unity` (and today the parsing part of `lab-render`) must pass its tests on a machine without a GPU.
- Tests that need a GPU adapter must skip with a printed message when `wgpu::Instance::request_adapter` returns nothing.
- When any test case fails, you must fix the issue immediately, even if the error is unrelated to the issue you are working on.
- The `preview` crate has no unit tests. It is verified by running it, see `REPO-ROOT/.github/Guidelines/Running-Preview.md`. Only run it when the task changes `crates/preview` or asks for a measurement.

### Code Generation Projects

There are no code generation projects at the moment.
If one is added (for example a generated table of known shader GUIDs), it must be listed here with the files that trigger it.

## Platform

- Windows first. The workspace must still compile on macOS and Linux.
- Code that only works on Windows (DirectComposition, `windows` crate) must sit behind `#[cfg(windows)]` in a module of its own, as small as possible.
- The toolchain is pinned in `REPO-ROOT/rust-toolchain.toml`. Do not use language features newer than the latest stable Rust, because Snowcase builds with stable.
- `ufbx` is a C library. Building needs a C compiler (MSVC on Windows).
