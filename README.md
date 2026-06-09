# cx

`cx` is a fresh Rust CLI project for exploring a per-line incremental-execution
overlay for `just`.

The current design note is [`cx-spec.md`](cx-spec.md). It is preliminary: treat
it as a proposal to evaluate and revise before turning it into a Gest-tracked
implementation plan.

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

The crate is intentionally only a scaffold right now. The next project session
should first decide which parts of `cx-spec.md` are accepted, then create Gest
tasks/specs before implementation.

## Rust Preparation Checklist

- Confirm the MSRV or accept the repo default stable toolchain in
  `rust-toolchain.toml`.
- Decide whether the crate should stay a binary-only crate or split into
  `src/lib.rs` plus a thin `src/main.rs` for testable core logic.
- Choose CLI/config dependencies before implementation, likely `clap`,
  `serde`, `serde_json`, `toml`, `camino` or `typed-path`, `blake3` or `sha2`,
  and `thiserror`/`anyhow`.
- Decide how to invoke and validate `just --dump --dump-format json` in tests.
- Create fixture Justfiles for linewise recipes, dependency ordering, script
  rejection, duplicate outputs, dangling inputs, and staleness records.
- Decide whether `.cx/config.toml` should be committed as project config or
  kept local, while keeping `.cx/graph.json` and `.cx/state.json` ignored.
