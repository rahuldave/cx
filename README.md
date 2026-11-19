# cx

`cx` stands for **Conditional eXecution**. It adds file-aware incremental
execution to individual lines in a [`just`](https://just.systems/) recipe.

The documentation site is https://rahuldave.com/cx/. It is built from `docs/`,
and `just docs` previews it.

`just` still owns the recipe graph. It decides which recipes run and in what
order. `cx` only decides whether one declared command line is already fresh.

```just
build: prepare
    cx --in data/message.txt --out build/message.upper -- bash -c 'tr "[:lower:]" "[:upper:]" < data/message.txt > build/message.upper'
```

On the first run, `cx` runs the command after `--`. On a later run, if the
declared inputs, outputs, and command identity are unchanged, `cx` exits
successfully without running the command and prints:

```text
up-to-date: build/message.upper
```

## Mental Model

Think of `cx` as a checkpoint around one expensive shell command.

- `just` runs the whole recipe chain every time.
- Plain recipe lines keep behaving exactly like plain `just`.
- `cx` lines declare the files they read with `--in`.
- `cx` lines declare the files they write with `--out`.
- The command after `--` runs only when those declarations are stale.
- Cache state lives beside the project in `.cx/state.json`.

The model is command-level incrementality, not Make-style target pruning. A recipe
that depends on another recipe still runs through the normal `just` order; only
individual `cx` lines can become no-ops.

## Install

Install the latest release with the script `scripts/install.sh`. The script needs no Rust:

```bash
curl -fsSL https://raw.githubusercontent.com/rahuldave/cx/main/scripts/install.sh | sh
```

The script downloads the archive of cx for your platform from the GitHub release, and checks its SHA-256 hash. Then
it puts `cx` in `~/.local/bin` and prints the version. Releases have archives for macOS on Apple silicon and Intel,
and for Linux on x86_64 and arm64. To pick a release or a folder, give `--tag` or `--dir`:

```bash
curl -fsSL https://raw.githubusercontent.com/rahuldave/cx/main/scripts/install.sh | sh -s -- --tag v0.2.2 --dir ~/bin
```

To build from source, use Cargo, the package manager of Rust:

```bash
cargo install --locked --git https://github.com/rahuldave/cx --tag v0.2.2
```

Check that the folder of `cx` is on your `PATH`:

```bash
cx --version
```

## Quick Start

Use the included example:

```bash
cd examples/linewise
just clean
just build
just build
```

The first `just build` creates `build/message.upper` and `dist/message.txt`.
The second `just build` still runs the `just` recipe chain, but both `cx` lines
should report `up-to-date`.

You can also reconstruct the same example from scratch without cloning this
repository. See [docs/quickstart.md](docs/quickstart.md).

## Commands

Run or skip one declared command:

```bash
cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND [ARG]...
```

Inspect linewise `cx` calls in the current `Justfile`:

```bash
cx graph
```

Check the current `Justfile` for the MVP lint rules:

```bash
cx lint
```

`cx` requires at least one `--out`. The output set is the key for the staleness
record.

## Documentation

- [Quickstart](docs/quickstart.md): run the bundled example or recreate it in a
  new directory.
- [Architecture](docs/architecture.md): how `cx` and `just` divide
  responsibility, what is implemented now, and what remains from the broader
  design.
- [Initial design proposal](cx-spec.md): the fuller design note this MVP is
  growing toward.

## Current MVP

Implemented:

- runtime `cx --in ... --out ... -- COMMAND ...`;
- content-hash staleness records under `.cx/state.json`;
- no-op logging to stderr as `up-to-date: <outputs>`;
- `cx graph` and `cx lint` groundwork from `just --dump --dump-format json`;
- examples and tests for direct runtime use, static extraction, mixed plain and
  `cx` recipes, and the linewise `just` example.

Known follow-ups:

- implement full overlay graph lints from `cx-spec.md`, including dangling
  inputs, duplicate producers, ordering-subset checks, and template collisions;
- make runtime command identity recover the template identity from `cx graph`
  rather than using the current concrete argv identity;
- add configuration for lint strictness and future `.cx/config.toml` behavior.

## Develop

`cx` uses the stack of [jnterfaces](https://github.com/rahuldave/jnterfaces).
The stack is the shared recipes for `just` and the skills of the agents, and
the installer of jnterfaces writes them into the repository. `cx` uses the
`rust` profile of jnterfaces, which gives the recipes for Rust.

You need `git`, `gh`, `just`, `uv`, `jlog`, `gest`, `jagt`, `rustup` and `cx`
itself. The page
[Start a project](https://github.com/rahuldave/jnterfaces/blob/main/docs/new-project.md)
of jnterfaces says where to get each tool. `just install` installs the rest of
the Rust tools, such as the toolchain and `cargo-llvm-cov`. `just tools-check`
names each program that is missing.

After you clone the repository, run these commands in the clone. The first one
installs `cx` from the clone.

```bash
cargo install --locked --path .
just setup
just install
```

`just install` also installs the Git hooks of the commit gate. The commit gate
is the set of steps that each commit must pass, and the hooks run it on each
commit. `just check` runs all of these steps, and `just verify` is a quicker
check. [AGENTS.md](AGENTS.md) lists each recipe.

## License

MIT. See [LICENSE](LICENSE).
