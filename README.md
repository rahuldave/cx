# cx

`cx` is a fresh Rust CLI project for exploring a per-line incremental-execution
overlay for `just`.

The current design note is [`cx-spec.md`](cx-spec.md). It is still the broader
proposal, but the repository now contains a first working MVP:

- runtime `cx [--in PATH]... [--out PATH]... -- COMMAND [ARG]...` execution;
- content-hash staleness records under `.cx/state.json`;
- no-op logging to stderr as `up-to-date: <outputs>`;
- `cx graph` and `cx lint` groundwork built from `just --dump --dump-format json`;
- a linewise `just` example in [`examples/linewise`](examples/linewise).

## Repository Shape

- `src/`: Rust crate source.
- `cx-spec.md`: preliminary product/design proposal.
- `.agents/skills/`: vendored Git/Gest Codex skills.
- `docs/`: reusable Gest workflow documentation copied with the skills.
- `Justfile`: stable command contract for agents and humans.

## Quick Start

```bash
just setup
just verify
```

Run a single `cx` line directly:

```bash
cargo run -- --in input.txt --out output.txt -- cp input.txt output.txt
```

Run the example with `just`:

```bash
cargo build
cd examples/linewise
PATH="../../target/debug:$PATH" just build
PATH="../../target/debug:$PATH" just build
```

The first example run writes `build/message.upper` and `dist/message.txt`. The
second run still lets `just` execute the recipe chain, while each `cx` line
short-circuits if its declared inputs, outputs, and command identity are fresh.

## Rust Preparation Checklist

- Confirm the MSRV or accept the repo default stable toolchain in
  `rust-toolchain.toml`.
- Continue expanding `cx graph` and `cx lint` toward the full overlay lints in
  `cx-spec.md`, including dependency ordering, duplicate producers, dangling
  inputs, and template collision checks.
- Decide whether runtime command identity should match concrete argv, as in the
  current MVP, or fully recover the template identity from the static graph.
- Decide whether `.cx/config.toml` should be committed as project config or
  kept local, while keeping `.cx/graph.json` and `.cx/state.json` ignored.
