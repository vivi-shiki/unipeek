# Building the Workspace

- Only run `copilotBuild.ps1` to build the workspace.
- DO NOT run `cargo build` by yourself on Windows.
- By default the script builds every crate in the workspace with all targets (libraries, binaries, examples and tests), so test code is type-checked too.

## Executing copilotBuild.ps1

Make sure no preview window started by `copilotExecute.ps1 -Mode Preview` is still running, otherwise the linker cannot replace `preview.exe`.
Preview windows quit by themselves after `-Seconds`, so usually waiting is enough.

Then run this script to build the workspace:

```
cd WORKSPACE-ROOT
& REPO-ROOT\.github\Scripts\copilotBuild.ps1
```

## Ensure Target Configuration

- `-Package CRATE-NAME` builds one crate only. Use it when a task only touches one crate and the full build is slow.
- `-Release` builds with the release profile. Use it only when a task measures performance.
- `-Fast` builds `preview` against gpui-fast (`--features fast`) into `REPO-ROOT/target-fast`.
  - It uses a separate target folder because switching the GPUI backend in `REPO-ROOT/target` would rebuild everything each time.
  - Build it when a task changes `crates/preview`, because `crates/preview/src/fast.rs` is only compiled with this feature.
- Pick the default option (no arguments) when there is no specific requirement.

## The Correct Way to Read Compiler Result

- The only source of trust is the raw output of the compiler.
- Wait for the script to finish before reading the log file.
  - DO NOT need to read the output from the script.
  - The first build of the workspace takes several minutes (wgpu, GPUI and the ufbx C code). DO NOT hurry.
  - When the script finishes, the result is saved to `REPO-ROOT/.github/Scripts/Build.log`.
  - A temporary file `Build.log.unfinished` is created during building. It will be automatically renamed to `Build.log` as soon as the building finishes. If you see this file, it means the building is not finished yet.
- The first line of `Build.log` is the exact cargo command that ran.
- Diagnostics use `--message-format=short`: one line per diagnostic, `path:line:column: error[EXXXX]: message`. Open the file at that location to see the context.
- When the build finishes, the last lines of `Build.log` are in the following pattern:
  - "Build succeeded." or "Build failed."
  - "X Warning(s)"
  - "Y Error(s)"
- DO NOT delete the log file by yourself unless explicitly required.
