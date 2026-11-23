# Debugging a Project

There is no scripted debugger for this workspace.
Debug with backtraces, logging, validation messages and smaller inputs, in this order.

## Backtraces

- `copilotExecute.ps1` always sets `RUST_BACKTRACE=1`, so every panic in `Execute.log` comes with a backtrace.
- Read the first frames that are inside `crates/`. Frames inside `std`, `core` or a dependency only tell you how the panic travelled.

## Logging

- Add `eprintln!("[TAG] ...")` lines with a short tag, e.g. `[upkg]`, `[mat]`, `[render]`, so you can find them in `Execute.log`.
- In tests, run with `-NoCapture` to see logging of passing tests.
- Remove every added log line when it is no longer necessary. Keep only logging that existed before or that the task asks for.

## wgpu Validation and Device Errors

- wgpu reports validation errors as a panic with the message `wgpu error: Validation Error`, followed by the label of the resource. Give resources a `label` when you create them, so the message says which one.
- When a GPU test fails only on one backend, set `WGPU_BACKEND=dx12`, `WGPU_BACKEND=vulkan` or `WGPU_BACKEND=gl` before running the script and compare. Known issue: some old Intel drivers crash on DX12 while Vulkan works (see `REPO-ROOT/docs/INTEGRATION.md`).
- `WGPU_ADAPTER_NAME=<part of the name>` picks a specific GPU on machines with two GPUs.
- A crash inside a driver kills the test binary without a `test result:` line. That is a native crash, not a Rust panic. Narrow it down with `-Filter`.

## ufbx

- `ufbx::load_file` returns an error with a `description`. Print it with `{:?}` together with the file name.
- ufbx is C code. A crash inside it is usually caused by invalid indices passed back to it (for example to `ufbx::triangulate_face`). Check the sizes of the buffers you pass first.

## Smaller Inputs

- When a real sample fails, find the smallest piece that still fails: one entry of the package, one mesh, one material.
- Turn that piece into a generated fixture in a test whenever its content is not paid material (structure, names and numbers are fine, textures and meshes are not).

## Native Debugger

- If I ask for a native debugger session, build with the default debug profile and attach Visual Studio or WinDbg to the process yourself only when you can interact with it. Otherwise ask me to do it.
- Never leave a debugger attached, it keeps `target\debug\*.exe` locked and the next build fails.
