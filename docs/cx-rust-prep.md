# Rust Project Preparation

`cx` is already initialized as a Cargo binary crate. Before implementation, make
these decisions explicitly:

- MSRV: either accept the current `stable` toolchain or pin a minimum supported
  Rust version.
- Crate shape: keep a binary-only crate for the first spike, or add `src/lib.rs`
  for testable graph/lint/staleness modules.
- CLI parsing: choose `clap` or keep manual parsing during the first prototype.
- Data formats: choose `serde`, `serde_json`, and `toml` for Just dump, graph
  cache, state, and config files.
- Path handling: decide between `std::path`, `camino`, or `typed-path`.
- Hashing: choose `blake3` or `sha2` for content and command-identity hashes.
- Error handling: choose `thiserror` for library errors and `anyhow` for the CLI
  boundary, or keep a single lightweight error type.
- Test fixtures: add Justfile fixtures for linewise extraction, script rejection,
  ordering-subset linting, duplicate producers, dangling inputs, parameter/output
  key coverage, and stale/up-to-date execution.
- Runtime state: keep `.cx/graph.json` and `.cx/state.json` out of git; decide
  later whether `.cx/config.toml` should be project config.
