# Running a CLI Application or an Example

- Only run `copilotExecute.ps1` to run a binary crate or an example.
- DO NOT call executables in `target` yourself on Windows.

## Executing copilotExecute.ps1

A binary crate (for example the future `unipeek` command line):

```
cd WORKSPACE-ROOT
& REPO-ROOT\.github\Scripts\copilotExecute.ps1 -Mode CLI -Executable CRATE-NAME -Arguments "info","INPUT","--json"
```

An example of a library crate (today `thumb` and `dump` of `lab-render`):

```
cd WORKSPACE-ROOT
& REPO-ROOT\.github\Scripts\copilotExecute.ps1 -Mode Example -Package lab-render -Executable thumb -Arguments "MODEL.fbx","TEXTURE-DIR","REPO-ROOT\out\front.png","1024","1024"
```

- `-Mode` and `-Executable` are required. `-Package` is required with `-Mode Example`.
- `-Arguments` is a PowerShell array. Quote each argument, separate them with commas.
- `-Release` uses the release profile. Use it for thumbnails of real models, the debug build is slow on big textures.

## Inputs and Outputs

- Real model files are paid assets. Ask me for their location, or read `UNIPEEK_SAMPLES`. Never copy them into the repo.
- Write output files into `REPO-ROOT/out`. It is ignored by git. Never write next to the input files.
- Never paste a local absolute path into a task document, a commit message or a source file. Write `UNIPEEK_SAMPLES\...` or `<model folder>` instead.

## The Correct Way to Read the Result

- The only source of trust is the raw output of the process.
- Wait for the script to finish before reading the log file.
  - When the script finishes, the result is saved to `REPO-ROOT/.github/Scripts/Execute.log`.
  - A temporary file `Execute.log.unfinished` is created while running. If you see this file, it means the process is not finished yet.
- The last line of `Execute.log` is `Exit code: N`.
  - Once the command line defines exit codes for success / degraded success / unsupported / error (see `### Output` in `REPO-ROOT/Project.md`), check that the code matches what the task expects.
- To check an image the process wrote, open it and describe what you see. Do not commit it.
- DO NOT delete the log file by yourself unless explicitly required.
