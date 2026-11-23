# Running Unit Tests

- Only run `copilotExecute.ps1` to run unit tests.
- DO NOT call `cargo test` or test executables yourself on Windows.

## Executing copilotExecute.ps1

`CRATE-NAME` is the name of the crate (the `name` in its `Cargo.toml`, e.g. `lab-render`), or `all` for the whole workspace.

Build first following `Building.md`, then run the tests:

```
cd WORKSPACE-ROOT
& REPO-ROOT\.github\Scripts\copilotExecute.ps1 -Mode UnitTest -Executable CRATE-NAME
```

`-Mode` and `-Executable` are required.

Optional arguments:
- `-Filter TEXT` only runs tests whose path contains `TEXT` (the normal libtest filter, e.g. `package::tests` or `pathname_only_first_line`).
- `-NoCapture` prints `println!` / `eprintln!` output of passing tests too.
- `-Release` runs the tests with the release profile. Only use it when a test is too slow in debug.

## Ensure Expected Test Cases are Selected

- When you only work on a few tests, use `-Filter` to run just those while fixing, then run the whole crate once at the end.
- DO NOT use `-Filter` for the final verification of a task. The final run covers every crate listed in `# AFFECTED PROJECTS`.
- A test marked `#[ignore]` is not executed. Never add `#[ignore]` to hide a failure.
- Tests that need a real sample read `UNIPEEK_SAMPLES` (see `## Sample Assets` in `REPO-ROOT/Project.md`) and print a line starting with `skipped:` when it is not set. Run with `-NoCapture` to see which ones skipped.
- Tests that need a GPU print a line starting with `skipped:` when no adapter is found. On a machine with a GPU they must not skip.

## The Correct Way to Read Test Result

- The only source of trust is the raw output of the test process.
- Wait for the script to finish before reading the log file.
  - DO NOT need to read the output from the script.
  - Testing includes building the test binaries and may take a long time. DO NOT hurry.
  - When the script finishes, the result is saved to `REPO-ROOT/.github/Scripts/Execute.log`.
  - A temporary file `Execute.log.unfinished` is created during testing. It will be automatically renamed to `Execute.log` as soon as the testing finishes. If you see this file, it means the testing is not finished yet.
- Each test binary (unit tests of a crate, each file in `tests/`, doc-tests) prints its own `test result:` line.
- When all test cases pass, the last lines of `Execute.log` should be in the following pattern:
  - "Exit code: 0"
  - "Passed test binaries: X/X"
  - "Passed test cases: Y/Y"
- When a test binary crashes (for example inside a native driver or ufbx), it prints no `test result:` line. The last test name printed before the crash is the suspect. Run it alone with `-Filter`.
- DO NOT delete the log file by yourself unless explicitly required.
