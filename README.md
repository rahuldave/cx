# cx

`cx` adds file-aware incremental execution to individual lines in a
[`just`](https://just.systems/) recipe.

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

This is command-level incrementality, not Make-style target pruning. A recipe
that depends on another recipe still runs through the normal `just` order; only
individual `cx` lines can become no-ops.

## Install

From this repository:

```bash
cargo install --path . --force
```

That installs a release build, usually at:

```text
~/.cargo/bin/cx
```

Verify:

```bash
cx --help
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
cx [--in PATH]... [--out PATH]... -- COMMAND [ARG]...
```

Inspect linewise `cx` calls in the current `Justfile`:

```bash
cx graph
```

Check the current `Justfile` for the MVP lint rules:

```bash
cx lint
```

At least one `--out` is required. The output set is the key for the staleness
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
