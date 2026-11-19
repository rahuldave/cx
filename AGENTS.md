<!-- begin lines of tasker, the tasker role, from agent_gest_git_skills at 58cfdba. -->
<!-- Last written to AGENTS.md by `install_stack.py --target . --profile rust --defaults integration`. The record is parts."AGENTS.md" in .agents/stack.json. -->
Read and follow @.agents/instructions/tasker.md
<!-- end lines of tasker. `just uninstall tasker` removes them. -->
<!-- begin lines of prose, the prose role, from nossaifd at 7ab9e94. -->
<!-- Last written to AGENTS.md by `install_stack.py --target . --defaults`. The record is parts."AGENTS.md" in .agents/stack.json. -->
Read and follow @.agents/instructions/prose.md
<!-- end lines of prose. `just uninstall prose` removes them. -->

# Agent Instructions

The two lines above load the instructions of the tasker role and of the prose
role. The tasker role gives the Gest workflow, the `g*` skills, the branches,
the review and the commit gates. The prose role gives the writing guide. This
file adds the instructions of cx.

## Project Context

- Project name: `cx`
- Main source directory: `src/`
- Primary preliminary spec: `cx-spec.md`
- Command-contract reference: the `just` skill of jnterfaces,
  `.agents/skills/just/SKILL.md`, and its contract
  `.agents/skills/just/references/command_contract.md`
- Usage skill for cx. The `cx` skill of jnterfaces, `.agents/skills/cx/SKILL.md`,
  says when to use cx and how to run `cx lint`. Its source is
  https://github.com/rahuldave/jnterfaces. Change the skill there when the
  behavior of cx changes. Then run `just install --update` here.

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
- `cx` is proposed as a command prefix on individual linewise recipe body lines,
  in the form `cx [--in PATH]... [--out PATH]... -- COMMAND [ARG]...`.
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

## The Stack

cx is set up like any other project of jnterfaces. The installer of
jnterfaces wrote the files in `.agents/`, `.claude/` and `.codex/`, and the
blocks at the top of `Justfile`, `.gitignore` and this file.
`.agents/stack.json` records what is installed:

- The `rust` profile, with the tools `lint`, `typecheck`, `test` and
  `integration`.
- The roles `tasker`, `prose`, `cx` and `jagt-protocol`.

Two parts of the stack are left out:

- **The `docstring` tool.** Its step `docstring-check` fails on each item
  with no doc comment. 114 items of `src/` have no doc comment. Add the doc
  comments first, then run `just install docstring`.
- **The `docs` role.** cx has its own docs site and the recipes `docs`,
  `docs-check` and `docs-build`. The role defines the same recipes, and
  `just install` stops on a recipe that is defined twice.

To install the same set again, run `just install`. To get new versions of the
packages, run `just install --update`. `just install --check` compares the
record with the files and changes nothing. The installer finds the package
clones in the folder of `SKILL_PACKAGES_DIR`. Without that variable, it uses
`~/.local/share/skill-packages`.

Do not copy a recipe or a script of a package into cx. The installer stops on
a module or a recipe that is defined twice, and it names the clash.

## Project Command Contract

The stable command interface is the repo-root `Justfile`. The index of
jnterfaces, `.agents/just/index.just`, gives the recipes of the profile and of
the roles. It also sets `jlog` as the shell of each recipe, so each recipe
needs `jlog` on the `PATH`. The `Justfile` itself has the recipes of cx only.

The recipes of the rust profile:

- `just setup` runs `cargo fetch`.
- `just build` runs `cargo build --workspace`.
- `just fmt` runs `cargo fmt --all`, and `just fmt-check` checks the format.
- `just lint` runs `cargo clippy` and fails on each warning.
- `just typecheck` runs `cargo check`. `just static` is the same check.
- `just test` runs the unit tests under `cargo llvm-cov`.
- `just integration` runs the tests in `tests/` under `cargo llvm-cov`.
- `just coverage-check` fails on each changed line of code that no test runs.
- `just test-paths FILE...` runs the tests of some files in `tests/`.
- `just rust::version` prints the version of `Cargo.toml`.

The recipes of the roles:

- `just release-check` fails when the version of `Cargo.toml` is lower than
  the last tag `vX.Y.Z`, or lower than the bump that the commits ask for. It
  needs no network.
- `just release` tags HEAD with `v` and the version of `Cargo.toml`, pushes
  the tag and checks it. `just release --dry-run` prints the tag and changes
  nothing. The script is `.agents/skills/gpa/scripts/release_tag.py` of the
  tasker role. It runs `just rust::version`, because `.agents/stack.json`
  names the profile `rust`.
- `just prose-check` checks the changed Markdown pages against the writing
  guide.
- `just review`, `just test-review`, `just update-docs` and `just prose` print
  the packets of the commit gates for a fresh agent.

The recipes of cx:

- `just smoke` builds cx and runs `cx --help`.
- `just verify` runs `fmt-check`, `lint`, `test`, `integration`, `smoke` and
  `diff-check`. It is the quick check, and CI runs it.
- `just docs-check` checks the docs site, `just docs` previews it, and
  `just docs-build` builds it.
- `just dist <target>` builds cx for a Rust target, for example
  `aarch64-apple-darwin`, and writes `dist/cx-<target>.tar.gz` and its
  `.sha256`.

`just check` runs every step of the commit gates, in this order. The steps are `tools-check`,
`release-check`, `fmt-check`, `lint`, `typecheck`, `test`, `integration`,
`coverage-check`, `docstring-check`, `docs-check`, `prose-check` and
`diff-check`. cx has no `docstring` tool, so `docstring-check` only logs a
skip. Every commit must pass `just check`. The Git hooks of the tasker role
run the commit gate.

## CI

- `.github/workflows/check.yml` runs `just ci` on each pull request. It
  checks that each commit has the trailer of the commit gate.
- `.github/workflows/verify.yml` runs `just release-check`, `just verify` and
  `just docs-check` on each push and pull request. On each push to `main`,
  after the job `verify` passes, the job `release` runs `just release`. Only
  that job and the job `publish` can write to the repository. So a PR that
  needs a release raises the version of `Cargo.toml`.
- When the job `release` made a tag, the job `binaries` runs `just dist` for
  four targets, and the job `publish` puts the files on the GitHub release of
  the tag. `scripts/install.sh` installs a release, and `tests/install.rs`
  tests it against a fake release.
- Each job that runs `just` installs `jlog` first. The tasker role also wrote
  a starter `release.yml`. cx removed it, because `verify.yml` already has the
  job that tags, and two such jobs would tag twice.

## Change Recipes And Code

When changing Just recipes, consult the Just manual rather than treating it like
Make. The key reference for recipe ordering is:

- Just dependencies: https://just.systems/man/en/dependencies.html
- Just skill reference: https://raw.githubusercontent.com/casey/just/refs/heads/master/skills/just/SKILL.md

For Just, dependency order is meaningful. Dependencies run before the recipe
that depends on them, and in the listed order. Use native recipe dependencies
when one recipe is an ordered composition of other recipes, such as
`verify: fmt-check lint test integration smoke diff-check`. Dependencies with the same
arguments run once per `just` invocation. Such a list is ordered recipe composition,
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
