# Task

- Check out `Accessing Task Documents` and `Accessing Script Files` in `REPO-ROOT/.github/copilot-instructions.md` for context about mentioned `*.md` and `*.ps1` files.
- All `*.md` and `*.ps1` files should exist; you should not create any new files unless explicitly instructed.
- Read `## Development Goal` in `REPO-ROOT/Project.md`. The change must not break anything listed in `### Never Do`.

## Goal and Constraints

- You must ensure the source code compiles.
- You must ensure all tests pass.

## Step 1. Implement Request

- Follow the chat message to implement the task.

## Step 2. Make Sure the Code Compiles

- Check out `External Tools Environment and Context` in `REPO-ROOT/.github/copilot-instructions.md` for accessing scripts for building.
  - Strictly follow the instruction above, so that logs end up where this document expects them.
- Each attempt of build-fix process should be executed in a sub agent.
  - One build-fix process includes one attempt with the following instructions.
  - The main agent should call different sub agent for each build-fix process.
  - Do not build and retrieve build results in the main agent.
  - If sub agents are not available in your environment, run each attempt yourself, one at a time.

### Use a sub agent to run all following instructions (`Build the Workspace`, `Fix Compile Errors`, `Code Generation`, `Finishing Code Change`)

#### Build the Workspace

- Check out `REPO-ROOT/Project.md` to find out what you need to build.
  - Pay attention to the `Code Generation Projects` section.
  - Projects you need to run may dynamically changing according to what you have done.
  - Build `preview` with `--features fast` as well when you changed `crates/preview`.
- Find out if there is any warning or error.
  - `External Tools Environment and Context` in `REPO-ROOT/.github/copilot-instructions.md` has the instruction about how to check compile result.

#### Fix Compile Errors

- If there is any compilation error, address all of them:
  - If there is any compile warning, only fix warnings that caused by your code change. Do not fix any other warnings.
  - If there is any compile error, you need to carefully identify, is the issue in the callee side or the caller side. Check out similar code before making a decision.
- After finishing fixing, exit the current sub agent and tell the main agent to go back to `Step 2. Make Sure the Code Compiles`

### Code Generation

- Check out `## Projects for Verification` in `REPO-ROOT/Project.md`.
  - Pay attention to the `### Code Generation Projects` section, `code generation projects` below references this section.
- Find out if any code generation is necessary.
  - If there is no need to run any code generation, you can skip this step.
  - Otherwise, pay attention to:
    - What features or profile are needed to build the workspace.
    - What crate or example should be executed, and with which arguments.
    - It is possible that building is required between two runs of code generation projects.
      - The building and future code generation project execution should be handled by the next sub agent.

### Finishing Code Change

- Exit the current sub agent and tell the main agent to go back to `Step 2. Make Sure the Code Compiles`.
  - If there is any unfinished work according to `Code Generation`, make sure to tell the main agent about them.

## Step 3. Run Unit Test

- Check out `External Tools Environment and Context` in `REPO-ROOT/.github/copilot-instructions.md` for accessing scripts for testing and debugging.
  - Strictly follow the instruction above, so that logs end up where this document expects them.
- Each attempt of test-fix process should be executed in a sub agent.
  - One test-fix process includes one attempt following `Execute Unit Test` and `Fix Failed Test Cases`.
  - The main agent should call different sub agent for each test-fix process.
  - Do not test and retrieve test results in the main agent.

### Use a sub agent to run the following instructions (`Execute Unit Test`, `Identify the Cause of Failure` and `Fix Failed Test Cases`)

#### Execute Unit Test

- Check out `REPO-ROOT/Project.md` to find out what crates you need to test.
- Run the unit test and see if they passed. If everything is good, you will see every executed test with `... ok`.
  - Make sure added test cases are actually executed.
  - If any test case fails on an assertion, the message of `assert!` / `assert_eq!` and the panic location are printed to the output.
  - If any test case crashes, the test binary dies without a summary. Re-run that test alone to find which one it is. In this case, you might need to add logging to the code.
    - `eprintln!` in tests and library code is printed when the test fails, or always with `-NoCapture`.
    - When added logging is no longer necessary, you should remove all of them.

#### Identify the Cause of Failure

- Dig into related source code carefully, make your assumption about the root cause.
- `assert!` with a formatted message in test cases or `eprintln!` in the source code would help.
  - They can make sure the expected code path is executed.
  - They can print variable values with `{:?}`.
- Follow `REPO-ROOT/.github/Guidelines/Debugging.md` to get a backtrace or wgpu validation messages if you are not confident of the assumption.
- When you have made a few guesses but did not progress, narrow the failing input down to the smallest fixture that still fails.
- Even when a failure is not related to your change, you should take care of it.

#### Fix Failed Test Cases

- Apply fixes to source files.
- DO NOT delete any test case. DO NOT mark a test `#[ignore]` to make it pass.
- After finishing fixing, exit the current sub agent and tell the main agent to go back to `Step 2. Make Sure the Code Compiles`
  - `Step 2. Make Sure the Code Compiles` and `Step 3. Run Unit Test` are absolutely no problem. If you didn't see any progress, the only reason is that your change is not correct.

## Step 4. Check it Again

- Go back to `Step 2. Make Sure the Code Compiles`, follow all instructions and all steps again.
