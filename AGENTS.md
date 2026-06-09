# Agent Instructions

This repository uses Gest to track substantial implementation work. Use the
project-local Codex skill family under `.agents/skills/`, especially `gtw`, for
coding, debugging, implementation, refactoring, documentation, verification, and
project planning.

The user may invoke the router as `$gtw`, `gtw:`, or `/gtw`. Use `gsu` for
repository bootstrap, setup refresh, tool selection, ignore rules, installs,
command-contract mapping, and Justfile creation.

If a request is substantial enough for Gest tracking but no `g*` command was
explicitly invoked, still use the appropriate Gest workflow. If an agent chooses
not to use Gest for a coding/debugging/refactoring/documentation/verification
request, it must say why in the final response.

## Project Context

- Project name: `cx`
- Main source directory: `src/`
- Primary preliminary spec: `cx-spec.md`
- Detailed workflow playbook: `docs/gest_codex_workflow.md`
- Command-contract reference: `docs/just_command_contract.md`
- Tag/dependency workflow: `docs/tag_dependency_workflow.md`

`cx-spec.md` is a preliminary design proposal. Treat it as material to agree,
disagree, revise, or promote into a Gest-tracked spec. Do not assume it is an
accepted implementation plan until a Gest spec/task explicitly adopts it.

`cx` is currently intended as a Rust CLI project. The proposed product shape is
a per-line incremental-execution overlay for `just`, where `just` remains the
task runner and owns recipe ordering.

## Preliminary `cx` Design Guardrails

These guardrails summarize the current proposal in `cx-spec.md`; revisit them
before implementation and update this section when the accepted design changes.

- `just` owns recipe-to-recipe ordering and runs the full recipe chain.
- `cx` must not prune the Just dependency graph or skip entire recipes.
- `cx` is proposed as a command prefix on individual linewise recipe body lines:
  `cx [--in PATH]... [--out PATH]... -- COMMAND [ARG]...`.
- At least one `--out` is required for a `cx` line because the output set is the
  staleness sidecar key.
- File-level input/output edges belong to `cx`; recipe ordering edges belong to
  `just`.
- Static analysis should use `just --dump --dump-format json`; do not reparse
  Justfile grammar when the dump already exposes the needed structure.
- Static `cx` extraction is proposed to be linewise-only by default. Shebang and
  `[script]` recipes should be rejected for static `cx` use unless the accepted
  design adds an explicit opaque mode.
- Runtime state belongs under `.cx/`. Cache/state files such as
  `.cx/graph.json` and `.cx/state.json` should not be committed.

## Tag And Dependency Impact

Before creating or splitting Gest tasks, collect the existing project tag
vocabulary and classify the work against it. Record selected existing tags, new
dynamic tags, and near-miss rejected tags when useful. Store machine-readable
metadata such as `classification.tags.reviewed=true`,
`classification.tags.new=<comma-separated-new-tags>`, and
`impact.ast_grep.required=true|false`. Use `docs/tag_dependency_workflow.md` for
the exact workflow.

For code-facing changes, identify changed semantic contracts and use `ast-grep`
to inspect dependers when useful. If a task changes one surface of a coupled
concept, expand the task or create/link a child task for the other surface
before completion. Completion notes should include `Tag classification:` and
`Dependency impact:` for code-facing work.

## Gest Workflow

Before creating new tasks, search and inspect existing work:

```bash
gest search "cx" --all --json
gest task list --all --json
gest iteration list --all --json
```

If Gest is not initialized for this repository yet, initialize it from the repo
root:

```bash
gest init --local
```

Use native Gest `child-of` / `parent-of` links for hierarchy. Tags are filters,
not hierarchy. Claim one leaf task at a time, verify before completion, and keep
long-lived outline parents open until the whole subtree is done.

For any Gest-tracked work that writes files, choose a VCS branch model and
execution model before editing. Branch names should be keyed to the highest
meaningful Gest task for the workstream, for example
`gest/<task-id-short>-two-word-summary` or
`session/<task-id-short>-two-word-summary`.

Use a normal session/development branch for one coherent workstream. Use stacked
branches for multiple meaty dependent slices that should be separately
reviewable. Use physical git worktrees for multiple independent write tasks
running at the same time.

GitButler parallel branches and stacked branches share one managed workspace.
They are sequential branch-curation tools for agents, not an agent-parallelism
primitive. Do not launch parallel write agents in one GitButler workspace or use
GitButler parallel lanes for agent parallelism. If parallel work is needed, use
separate physical worktrees first and integrate the results into the intended
branch or stack afterward.

When GitButler owns the workspace, use current `but` CLI write commands such as
`but branch new`, `but stage`, `but commit`, `but push`, and `but pr`. Do not
use raw `git commit`, `git switch`, `git checkout`, or branch-mutating git
commands in GitButler mode. If a workflow has explicitly left GitButler mode to
use physical git worktrees, mark raw worktree commands with
`GEST_VCS_EXECUTION=git-worktrees`.

For non-trivial completed leaf tasks, add a Gest task note before completion:

```bash
gest task note add <task-id-or-prefix> --agent codex --body "Done: ...\nVerification: ...\nFollow-up: ..."
gest task complete <task-id-or-prefix> --quiet
```

Use task metadata for machine-queryable facts, not prose work logs.

## Commit Cadence

Committing is VCS hygiene, not a Gest task by itself. Do not create a Gest task
whose only purpose is making a normal commit.

Session work should not commit every small leaf by default. Commit when the user
asks, when a coherent checkpoint helps, or when a long-lived parent/subtree
reaches a stable point.

Session classification alone is not a reason to skip `gcm`. A verified slice is
a commit-required checkpoint when it changes deployment/runtime configuration,
persistence, migrations, schemas, public APIs, user-visible UI, reusable
workflow material, publishable docs/templates, or a non-trivial multi-file
changeset. After verification and review, run `git status --short --branch`
before final response. If it shows Codex-owned changes and a commit-required
trigger applies, route through `gcm` before completing the handoff. If `gcm` is
intentionally skipped despite a dirty worktree, record the concrete no-commit
reason in the Gest note and final response.

Development work should commit at verified durable checkpoints such as a
completed depth-1 workstream, coherent depth-2 implementation subtree, handoff,
risky bug/migration fix, or GitHub issue/PR sync.

Stage explicit files and do not put Gest IDs in commit messages.

After every Codex-created commit, make the push/sync decision explicit. Run
`git status --short --branch`; if the user has not asked for local-only work,
push the checkpoint. If the branch has no upstream, set one with
`git push -u origin <branch>` or the repo's equivalent.

When Codex pushes a branch other than the repository's mainline branch, create
or update the pull request for that branch, run `gpa`, report the PR review
findings/state to the user, and ask whether to merge. Do not merge unless the
user explicitly asked for that merge in the current turn or gives approval after
the `gpa` review packet.

At every durable checkpoint, run checkpoint hygiene: regenerate the overall Gest
graph and focused graph for the latest relevant iteration, make the GitHub issue
promotion decision with `gpr` when appropriate, verify push state, and run an
explicit review pass with `grv` or code-review stance.

## Project Command Contract

The stable command interface is the repo-root `Justfile`.

```bash
just setup
just fmt [path]
just fmt-check
just lint
just typecheck
just static
just build
just test
just smoke
just diff-check
just verify
git diff --check
```

Current command mappings:

- Setup: `just setup`, which runs `cargo fetch`.
- Format: `just fmt [path]`, which runs `cargo fmt --all`. The optional path is
  accepted for interface consistency but Cargo formatting is crate-wide.
- Format check: `just fmt-check`, which runs `cargo fmt --all --check`.
- Lint: `just lint`, which runs `cargo clippy --all-targets --all-features -- -D warnings`.
- Typecheck: `just typecheck`, which runs `cargo check --all-targets --all-features`.
- Static/compile check: `just static`, which currently depends on `typecheck`.
- Build: `just build`, which runs `cargo build`.
- Tests: `just test`, which runs `cargo test --all-targets --all-features`.
- Smoke: `just smoke`, which runs `cargo run -- --help`.
- Diff hygiene: `just diff-check`, which runs `git diff --check`.
- Full local verification: `just verify`, which depends on `fmt-check`,
  `lint`, `test`, `smoke`, and `diff-check`.

When changing Just recipes, consult the Just manual rather than treating it like
Make. The key reference for recipe ordering is:

- Just dependencies: https://just.systems/man/en/dependencies.html
- Just skill reference: https://raw.githubusercontent.com/casey/just/refs/heads/master/skills/just/SKILL.md

For Just, dependency order is meaningful: dependencies run before the recipe
that depends on them, and in the listed order. Use native recipe dependencies
when one recipe is an ordered composition of other recipes, such as
`verify: fmt-check lint test smoke diff-check`. Dependencies with the same
arguments run once per `just` invocation. This is ordered recipe composition,
not Make-style file freshness analysis.

Use `gfm` for formatting, linting, typechecking, compile/static checks, and
diff hygiene. Use `gte` for unit tests, API regression tests, smoke checks, and
integration tests. Use `gdo` to check and update user-facing docs,
developer-facing docs, and in-code docs.

Recommended Rust layout:

- `src/`: CLI implementation and library modules.
- `tests/`: integration tests that exercise the compiled CLI or public API.
- `fixtures/`: Justfile and file-tree fixtures for graph/lint/staleness tests.

For CLI behavior changes, prefer focused tests around argv parsing, graph
construction, lint behavior, and staleness decisions before broad end-to-end
fixtures.
