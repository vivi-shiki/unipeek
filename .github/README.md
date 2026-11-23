# AI Agent Documents

Everything in this folder is written for AI coding agents (Claude Code, Codex, Copilot, Cursor).
The entry points are `REPO-ROOT/CLAUDE.md`, `REPO-ROOT/AGENTS.md` and `REPO-ROOT/.cursorrules`; they route a request by its first word to a prompt in `prompts/`.

| Folder / file | Purpose |
|---|---|
| `copilot-instructions.md` | General rules: tools, Rust coding rules, task documents, knowledge base layout |
| `../Project.md` | Development goal (input, output, never do), workspace, verification |
| `prompts/` | One file per keyword: `scrum`, `design`, `plan`, `summarize`, `execute`, `verify`, `ask`, `investigate`, `code`, `kb`, `refine`, `review` |
| `Guidelines/` | How to build, test, run, debug, manage files, and read Unity asset formats |
| `KnowledgeBase/` | `Index.md` plus API and design explanations of each crate and key dependency |
| `Learning/` | Project-specific learnings, and backups of finished task documents waiting for `refine` |
| `TaskLogs/` | Working documents of the current task (`Copilot_*.md`) |
| `Scripts/` | PowerShell wrappers around cargo that write `Build.log` / `Execute.log` |

## Typical Flow

1. `scrum` + `# Problem ...`: break a goal into tasks in `TaskLogs/Copilot_Scrum.md`.
2. `design` + `# Problem Next`: design the next task in `Copilot_Task.md`.
3. `plan`: write the code-level plan in `Copilot_Planning.md`.
4. `summarize`: condense the plan into `Copilot_Execution.md`.
5. `execute and verify`: apply the plan, build, test, mark `# !!!VERIFIED!!!`.
6. `scrum` + `# Learn`: compare my manual edits with the plan and back up the task documents.
7. `refine`: turn the backups into learnings.

Each document can be corrected with `# Update ...` at any step. `review` runs a multi-model review panel on any of the documents.

## Attribution

The structure, workflow and much of the wording are adapted from the `.github` folder of [vczh-libraries/GacUI](https://github.com/vczh-libraries/GacUI),
whose LLM supporting files are shared under the [Apache License 2.0](https://github.com/vczh-libraries/License/blob/master/README.md).
They were rewritten for a Rust / Cargo workspace and for unipeek's goal; the scripts were rewritten for cargo.
