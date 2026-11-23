# General Instruction

- `REPO-ROOT` refers to the root directory of the repository.
- `WORKSPACE-ROOT` refers to the root directory of a Cargo workspace (the folder containing the `Cargo.toml` with `[workspace]`).
- The workspace and crates you need to work on could be found in `REPO-ROOT/Project.md`.
- The development goal (input, output, things never to do) is in `REPO-ROOT/Project.md`. Every design decision must serve that goal.
- Following `Leveraging the Knowledge Base`, find knowledge and documents for this project in `REPO-ROOT/.github/KnowledgeBase/Index.md`.
- Before writing to a source file, read it again and make sure you respect my parallel editing.
- If any `*.prompt.md` file is referenced, take immediate action following the instructions in that file.

## External Tools Environment and Context

- If you are on Windows OS:
  - Always prefer the offered script files instead of direct CLI commands.
  - DO NOT call `cargo build`, `cargo test` or `cargo run` directly. Use the scripts described in `Coding Guidelines and Tools`.
  - `cargo add`, `cargo remove`, `cargo update -p <crate>` and `cargo fmt` may be called directly when a task needs them.
  - DO NOT create or delete any file unless explicitly directed.
  - MUST run any PowerShell script in this format: `& absolute-path.ps1 parameters...`.
  - Multiple powershell commands are concatenated with `;` to be executed in one line.

- If you are on Linux or macOS, offered powershell script files may not work and here are replacements:
  - Run every command in `WORKSPACE-ROOT`.
  - Building: `cargo build --workspace --all-targets 2>&1 | tee REPO-ROOT/.github/Scripts/Build.log`.
  - Unit testing: `cargo test --workspace 2>&1 | tee REPO-ROOT/.github/Scripts/Execute.log`, or `cargo test -p <crate>` for one crate.
  - The `preview` crate and everything behind `#[cfg(windows)]` cannot run there. Only make sure they still compile when the code is not Windows-only.
  - DO NOT call `gdb` or `lldb` unless you can interact with it.

## Coding Guidelines and Tools

The Rust workspace in this repo is built and tested through its own wrapper scripts.
You must strictly follow the instructions in the following documents,
otherwise logs will not be where the prompts expect them.

- Adding/Removing/Renaming Source Files and Crates: `REPO-ROOT/.github/Guidelines/SourceFileManagement.md`
- Building the Workspace: `REPO-ROOT/.github/Guidelines/Building.md`
- Running a Project:
  - Unit Test: `REPO-ROOT/.github/Guidelines/Running-UnitTest.md`
  - CLI Application and Examples: `REPO-ROOT/.github/Guidelines/Running-CLI.md`
  - GPUI Preview Application: `REPO-ROOT/.github/Guidelines/Running-Preview.md`
- Debugging a Project: `REPO-ROOT/.github/Guidelines/Debugging.md`
- Unity Asset Formats (unitypackage, `.meta`, `.mat`, prefab, GUID): `REPO-ROOT/.github/Guidelines/UnityAssetFormats.md`
- Requirements, integration contract and prior art: `REPO-ROOT/docs/`

## Accessing Task Documents

If you need to find any document for the current working task, they are in the `REPO-ROOT/.github/TaskLogs` folder:
- `Copilot_Scrum.md`
- `Copilot_Task.md`
- `Copilot_Planning.md`
- `Copilot_Execution.md`
- `Copilot_KB.md`
- `Copilot_Investigate.md`

### Important Rules for Writing Markdown Files

- Do not print "````````" or "````````markdown" in a markdown file.
- It is totally fine to have multiple top level `# Topic`.
- When mentioning a Rust name in markdown file:
  - If it is defined in the standard library or a third-party crate, use the full path, e.g. `std::io::Read`, `wgpu::Device`, `ufbx::Scene`.
  - If it is defined in this workspace, use the full path if there is ambiguity, and then mention the file containing its definition.
- Task documents are written in English. Quote my words exactly as I wrote them, even when they are in Chinese.

## Accessing Script Files

If you need to find any script or log files, they are in the `REPO-ROOT/.github/Scripts` folder:
- `copilotPrepare.ps1`
- `copilotPrepareReview.ps1`
- `copilotBuild.ps1`
- `copilotExecute.ps1`
- `copilotGitCommit.ps1`
- `Build.log`
- `Execute.log`

## Writing Rust Code

- This project uses Rust edition 2024. Use new language features where they make code shorter and clearer (let-else, let chains, `is_none_or`), but nothing newer than the latest stable Rust.
- All code should be cross-platform. When an OS feature is needed, put the Windows version in a module guarded by `#[cfg(windows)]`, keep it as small as possible, and give other platforms a fallback or a clear "unsupported" error.
- Library crates return errors, they never panic on bad input. Malformed packages, broken FBX files and unreadable textures are normal input.
  - Use `anyhow` only in binaries, examples and the `preview` crate.
  - Library crates that are published must expose their own error type (`thiserror` is fine, it is MIT OR Apache-2.0).
- Recoverable problems with an asset are not errors. Record a degrade reason in the report and keep going.
- Error messages and logs are in English.
- Keep the UI-agnostic crates free of GPUI, winit and any window system types. The GPU-free crate must also be free of `wgpu`.
- Do not read a whole package into memory. Use `std::io::Read` and stream. Large entries are skipped or hashed while reading.
- Heavy work (texture decoding, mip generation) must be callable off the UI thread. Do not hide it behind a blocking call in a constructor that a UI would call.
- Prefer `glam` for math, `bytemuck` for GPU buffers, `image` for decoding.
- Comments say why, not what. Most code needs no comments. Module level `//!` doc comments describe what the module is for, like the existing files.
- There is no rustfmt configuration yet. Do not reformat code you are not changing. Keep the style of the surrounding code.
- Every new dependency must be MIT, Apache-2.0, BSD, Zlib, ISC, Unlicense, 0BSD or a combination of these. Check the license before adding it and write it in the task document.
- When porting code from another project (for example popone, 0BSD), keep a comment with the source repository and file at the top of the ported module.

## Leveraging the Knowledge Base

- When making design or coding decisions, you must leverage the knowledge base to make the best choice.
- The main entry is `REPO-ROOT/.github/KnowledgeBase/Index.md`, it is organized in this way:
  - `## Guidance`: A general guidance that plays a super important part repo-wide.
  - Each `## Project`: A brief description of each crate (or external dependency) and its purpose.
    - `### Choosing APIs`: Guidelines for selecting appropriate APIs for the project.
    - `### Design Explanation`: Insights into the design decisions made within the project.
  - `## Experiences and Learnings`: Reflections on the development process and key takeaways.
  - `# External References`: Pointers to documents outside this repo (Snowcase-docs, crate documentation, prior art) and what each one is good for.

### Project/Choosing APIs

There are multiple categories under `Choosing APIs`. Each category begins with a short and accurate title `#### Category`.
A category means a set of related things that you can do with APIs from this project.

Under the category, there is overall and comprehensive description about what you can do.

Under the description, there are bullet points and each item follow the format:  `- Use TYPE-NAME for blahblahblah` (If a function does not belong to a type, you can generate `Use FUNCTION-NAME ...`).
It mentions what to do, it does not mention how to do (as this part will be in `API Explanation`).
If many types or functions serve the same, or very similar purpose, one bullet point will mention them together.

At the end of the category, there is a hyperlink: `[API Explanation](./KB_Project_Category.md)` (no space between file name, all pascal case).

### Project/Design Explanation

There are multiple topics under `Design Explanation`. Each topic begins with a short and accurate title `#### Topic`.
A topic means a feature of this project, it will be multiple components combined.

Under the topic, there is overall and comprehensive description about what does this feature do.

Under the description, there are bullet points to provide a little more detail, but do not make it too long. Full details are supposed to be in the document from the hyperlink.

At the end of the topic, there is a hyperlink: `[Design Explanation](./KB_Project_Design_Topic.md)` (no space between file name, all pascal case).

### Experiences and Learnings

- Cross-project learnings are in `REPO-ROOT/.github/KnowledgeBase/Learning.md`.
- Learnings about this project's source code are in `REPO-ROOT/.github/Learning/Learning_Coding.md`.
- Learnings about this project's test code are in `REPO-ROOT/.github/Learning/Learning_Testing.md`.
- Items in `# Orders` are sorted by a counter. A high counter means a mistake that was made many times. Read them before coding.
