# Workspace and Crate File Structure

- `REPO-ROOT/Cargo.toml` is the workspace manifest. `members = ["crates/*"]`, so every folder in `REPO-ROOT/crates` with a `Cargo.toml` is a crate of the workspace.
- Shared settings live in the workspace manifest:
  - `[workspace.package]`: version, edition, license, repository. Crates inherit them with `version.workspace = true` and so on.
  - `[workspace.dependencies]`: every third-party dependency and its version. Crates use `name.workspace = true`, optionally adding `features = [...]`.
  - `[profile.*]`: build profiles. Dependencies are optimized even in debug builds, because unoptimized wgpu and image decoding are too slow to test with.
- A crate's root module is `src/lib.rs` (library) or `src/main.rs` (binary). Every other file is a module that must be declared with `mod` in its parent.

## Adding a Source File

- Create the file next to its parent module and add `mod name;` (or `pub mod name;`) to the parent.
- Re-export only what callers need with `pub use` in `lib.rs`.
- Unit tests of a module go into a `#[cfg(test)] mod tests` block at the end of the file, or into `name/tests.rs` declared with `#[cfg(test)] mod tests;` when they are long.
- Integration tests that use only the public API go into `crates/CRATE/tests/*.rs`. Each file there is a separate test binary.
- Examples go into `crates/CRATE/examples/*.rs`. Each file there is a separate example, run with `-Mode Example`.
- Shaders are `*.wgsl` files next to the module that loads them with `wgpu::include_wgsl!`.

## Adding a Crate

- Create `crates/FOLDER/Cargo.toml` with `version.workspace = true`, `edition.workspace = true`, `license.workspace = true`.
- Folder names are short (`render`, `unity`); crate names are the published names (`unipeek`, `unipeek-unity`).
- Add the crate to `[workspace.dependencies]` as `name = { path = "crates/FOLDER" }` when other crates depend on it.
- Add `publish = false` to crates that must never be published (`preview`, test helpers).
- Update the crate table in `## Workspace to Work On` of `REPO-ROOT/Project.md` and the knowledge base.

## Adding a Dependency

- Add it to `[workspace.dependencies]` first, then reference it from the crate. Use `default-features = false` and turn on only the features you need.
- Prefer the version already used by Snowcase when the crate is shared (tar, flate2, zip, sevenz-rust2, encoding_rs, blake3, image, wgpu), see `REPO-ROOT/docs/INTEGRATION.md`.
- Check the license as described in `Writing Rust Code` of `REPO-ROOT/.github/copilot-instructions.md`.
- Let cargo update `Cargo.lock`. Never edit it by hand.

## Renaming and Removing Source Files

- Update every `mod` declaration, `use` path and `pub use` re-export that refers to the file.
- When renaming a crate, update `[workspace.dependencies]`, every dependent `Cargo.toml`, every `use crate_name::` path (hyphens become underscores), `REPO-ROOT/Project.md`, `REPO-ROOT/README.md` and the knowledge base.
- Remove a file with git (`git rm`) only when the task says so.
