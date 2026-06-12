# Architecture

`cx` means **Conditional eXecution**: a per-line incremental-execution overlay
for `just`.

The design goal is narrow: let `just` keep doing recipe orchestration, while
`cx` gates expensive individual commands with explicit file declarations.

## Responsibility Split

| Concern | Owner |
| --- | --- |
| Recipe order | `just` |
| Recipe dependencies | `just` |
| Parameter interpolation in recipe bodies | `just` |
| Whether the whole recipe chain runs | `just` |
| File input/output declarations for one command line | `cx` |
| Per-line freshness decision | `cx` |
| Runtime staleness state | `cx` |

`cx` does not prune the `just` dependency graph. If a recipe depends on another
recipe, `just` still executes that relationship in order. `cx` only turns a
single command line into a no-op when its declared work is already fresh.

## Runtime Flow

A runtime invocation looks like:

```bash
cx --in data/message.txt --out build/message.upper -- bash -c 'tr "[:lower:]" "[:upper:]" < data/message.txt > build/message.upper'
```

`cx` parses:

- zero or more `--in PATH` declarations;
- one or more `--out PATH` declarations;
- the command after `--`.

At least one `--out` is required because the sorted output set is the key in
`.cx/state.json`.

The MVP freshness check compares:

- whether a prior record exists for the output set;
- whether the command identity changed;
- whether every declared output still exists and has the recorded content hash;
- whether declared input content changed.

For inputs, `cx` records `(mtime_ns, size, hash)`. If mtime and size match, it
uses the previous hash as a fast path. If they differ, it hashes the file; a
same-content rewrite refreshes the timestamp record without rerunning the
command.

When fresh, `cx` prints to stderr:

```text
up-to-date: <outputs>
```

When stale, `cx` runs the command. If the command succeeds, `cx` hashes the
declared outputs and updates `.cx/state.json`. If the command fails, `cx`
propagates failure and does not update the record.

## Static View

`cx graph` and `cx lint` inspect the current `Justfile` by shelling out to:

```bash
just --dump --dump-format json
```

That means `cx` relies on `just` to parse Justfile syntax. The MVP static pass
then reconstructs linewise recipe body text, finds lines whose first shell token
is `cx`, and extracts `--in`, `--out`, and command tokens.

Plain recipe lines are not part of the `cx` static graph. They remain normal
`just` behavior.

The current static pass supports:

- linewise `cx` call extraction;
- template reconstruction for simple `{{variable}}` fragments in the dump;
- `cx lint` failure when a `cx` line has no `--out`;
- rejection of `cx` tokens inside script/shebang recipe forms.

## Mixed Recipes

`cx` is designed to coexist with plain recipes:

```just
prepare:
    mkdir -p out

build: prepare
    cx --in data/input.txt --out out/output.txt -- cp data/input.txt out/output.txt

report: build
    cat out/output.txt
```

`just` runs `prepare`, then `build`, then `report` according to recipe
dependencies. Inside `build`, `cx` decides whether the copy command is stale.
The plain `prepare` and `report` recipes are not skipped by `cx`.

## Current Limitations

The MVP intentionally does not implement the full overlay graph described in
[`cx-spec.md`](../cx-spec.md).

Important follow-ups:

- **Full overlay lints**: dangling inputs, duplicate output producers, file
  dependency cycles, ordering-subset checks, output-key coverage, and template
  collision checks.
- **Template command identity**: the MVP runtime identity is based on concrete
  argv. The fuller design wants runtime invocations to recover the matching
  template line from the static graph so command identity is partition
  independent.
- **Graph cache**: `.cx/graph.json` is reserved in the design but not yet used
  as a persistent static graph cache.
- **Configuration**: lint strictness and `.cx/config.toml` are not implemented
  yet.

## State Files

Runtime state belongs under `.cx/`:

```text
.cx/
  state.json
```

These files are local cache/state and should not be committed.
