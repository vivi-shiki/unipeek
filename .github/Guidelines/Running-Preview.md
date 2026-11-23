# Running the GPUI Preview Application

`crates/preview` is a research application. It opens a GPUI window and shows the rendered model in one of three ways.
It has no unit tests. Run it only when a task changes `crates/preview`, or asks for a measurement.

| Mode | GPUI backend | How frames reach the screen |
|---|---|---|
| `overlay` | upstream GPUI (gpui-kit default) | Our own DirectComposition device, `CreateTargetForHwnd(hwnd, topmost = true)`, wgpu presents to that visual |
| `fast` | gpui-fast (`-Fast`) | `window.enable_window_composition()?.create_native_surface()`, wgpu presents to that visual |
| `readback` | upstream GPUI | Offscreen render, read back, a new `RenderImage` every frame |

## Executing copilotExecute.ps1

```
cd WORKSPACE-ROOT
& REPO-ROOT\.github\Scripts\copilotExecute.ps1 -Mode Preview -Executable readback -Arguments "MODEL.fbx","TEXTURE-DIR","1100x800"
```

- `-Executable` is the mode: `overlay`, `fast` or `readback`. `fast` always builds with `--features fast` into `REPO-ROOT/target-fast`.
- `-Seconds N` closes the window after N seconds (default 10). It sets `LAB_SECS`. The script never leaves a window open.
- Other switches of the application are environment variables, set them before calling the script:
  - `LAB_SPIN=1` rotates the model continuously, so frame times are measured under load.
  - `LAB_POPUP=0|1|2` sets the state of the popup test panel at start, to check whether GPUI popups draw above the 3D area.
- The model and texture folder are paid assets outside the repo, see `## Sample Assets` in `REPO-ROOT/Project.md`.
- The application only works on Windows.

## The Correct Way to Read the Result

- The application prints `[stats]` lines with frame timings, and `[overlay]` / `[fast]` / `[readback]` lines for setup and resize times, to stderr. When the script finishes, they are in `REPO-ROOT/.github/Scripts/Execute.log`.
- Compare timings only between runs on the same machine, the same build profile and the same window size.
- Whether a popup is hidden behind the 3D layer cannot be read from the log. Ask me to look, or take a screenshot with `REPO-ROOT/shot.ps1` into `REPO-ROOT/out` and crop it with `REPO-ROOT/crop.ps1`. Never commit screenshots of paid assets.
- Known results are in the `## Results` section of `REPO-ROOT/README.md`. Update that table only when a task asks for a new measurement.
