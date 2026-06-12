# cx — Conditional eXecution for `just`

## 1. Overview

`cx` stands for **Conditional eXecution**. It adds input/output-aware
incremental execution to `just` **without replacing `just` as the task runner
and without pruning its dependency graph.**

- `just` owns recipe ordering. When you run `just build`, the entire recipe
  dependency chain runs, exactly as it always does. No recipe is ever skipped.
- `cx` is a **command prefix used on individual lines inside recipe bodies.** It
  gates a single expensive step: when that step's declared inputs, declared
  outputs, and command text are all unchanged, the `cx` invocation is a **no-op**.

The result is incrementality at **command-line granularity** rather than target
granularity. Unlike `make`, the full DAG still executes top to bottom on every
run; the only things that short-circuit are the `cx`-prefixed lines whose work is
already done.

`cx` never re-parses the justfile. Everything structural is read from
`just --dump --dump-format json`, which has already done the parsing.

---

## 2. Division of responsibility

| Concern | Owner |
|---|---|
| Recipe-to-recipe ordering | `just` (its `dependencies`) |
| Running the full recipe chain | `just` |
| Parameter interpolation of recipe bodies | `just` |
| File-level input/output dependency edges | `cx` (overlay) |
| "Is this step's work already done?" | `cx` |
| Checksums / staleness state | `cx` (`.cx/`) |

`cx` computes **no ordering of its own**. It overlays file nodes onto the graph
`just` already defines and uses that overlay only to (a) lint and (b) decide
per-line staleness.

---

## 3. Concepts and terminology

- **Recipe node** — a `just` recipe. Supplied by the dump. Ordering edges between
  recipe nodes come from each recipe's `dependencies`.
- **`cx` line** — a single line inside a recipe body of the form
  `cx --in … --out … -- COMMAND …`.
- **File node** — added by `cx`, one per distinct `--in`/`--out` value. A file
  node is *template-valued* in the static graph (e.g. `warehouse/{{part}}.parquet`)
  and *concrete* at runtime (e.g. `warehouse/2024-01.parquet`).
- **Overlay graph** — `just`'s recipe DAG augmented with file nodes and file edges
  (recipe →produces→ file, file →consumed-by→ recipe).
- **Command identity** — a content hash of a `cx` line's *template* (flags +
  `-- COMMAND`), taken pre-interpolation. Editing the transform logic changes it;
  passing a new argument value does not.
- **Staleness record** — a `.cx/` sidecar entry for one concrete output set:
  the input stamps, output hashes, and command identity from the last successful
  run.

---

## 4. The `cx` line

### 4.1 Syntax

```
cx [--in PATH]... [--out PATH]... [--CONFIG]... -- COMMAND [ARG]...
```

- `--in PATH` — a declared input file. Repeatable. May be a source file or
  another `cx` line's output.
- `--out PATH` — a declared output file. Repeatable. **At least one `--out` is
  required** (the output set is the sidecar key; see §10).
- `-- COMMAND …` — the actual command to run when the step is stale.

### 4.2 Two resolutions of the same line

Because the line lives in a recipe body, it exists in two forms, and `cx` uses
both:

- **At runtime**, `just` interpolates the line before executing it, so the `cx`
  process receives **concrete** argv: `--in data/2024-01.parquet
  --out warehouse/2024-01.parquet -- transform 2024-01`. No interpolation logic is
  needed inside `cx`.
- **Statically**, the dump's `body` for that recipe contains the same line as a
  **template** (with `{{part}}` preserved as an interpolation fragment). The graph
  construction pass reads it from there.

> Note: `just` does **not** interpolate recipe parameters into attributes, and
> the dump is produced without invoking any recipe, so attribute metadata can
> never carry per-invocation values. That is why in/out edges live on the `cx`
> line (which `just` *does* interpolate at runtime) and not in a `[metadata(...)]`
> attribute.

### 4.3 Recipe-form compatibility (where `cx` lines are valid)

`just` has three recipe forms, and they differ in exactly the property `cx`
depends on — whether `{{…}}` is interpolated and whether the body is executed as
independent per-line commands:

| Form | `{{…}}` interpolation | Execution | Args reach a subprocess via |
|---|---|---|---|
| **Linewise** (default) | **on** | each line is its own shell process | `{{param}}`, interpolated by `just` |
| **Shebang** (`#!…`) | **on** | whole body → temp file → interpreter | `{{param}}`, interpolated into the script text |
| **`[script]`** | **off** | whole body → temp file → interpreter | `$1`, `$2` (positional always on); just vars only if exported |

The governing facts: in `[script]` recipes the parser does not look for `{{…}}`,
positional arguments are always on (`$1`, `$2`, …), and just variables must be
exported to be visible; and in linewise recipes every line is run by a **new
shell instance**, so each `cx` line is a clean, independent process.

**Static `cx` is a linewise-only feature, by design.** The reasons, per form:

- **Linewise** — the happy path. `{{}}` is on, each line is a separate process,
  so a `cx` line yields concrete argv at runtime and a clean template in the dump
  `body`. Full overlay + lints apply.
- **Shebang** — `{{}}` is still interpolated, so `cx` *runs* correctly inside the
  script, but the call lives inside arbitrary code (loops, conditionals,
  shell-computed args). A `cx` token spotted in the body cannot be trusted to be a
  single top-level invocation whose `--in`/`--out` templates are the real runtime
  values. Static extraction is unreliable.
- **`[script]`** — fatal to static analysis: `{{}}` is **off**, so
  `--in data/{{part}}.parquet` is written verbatim (broken); you must write
  `--in "data/$1.parquet"`, making in/out **runtime shell expressions** that are
  invisible to `just --dump`. `cx` gets concrete paths at runtime, but there is
  nothing to extract, lint, or graph statically.

This is fundamental, not an implementation gap: **arbitrary key computation and
static analysis are mutually exclusive.** If the key is computed by running a
script, the only way to know it is to run the script — but the DAG and the
ordering/§7.5 lints all require in/out to be visible *before* execution. Reusing
`[metadata]` does not rescue the script case: metadata is literal (cx would have
to interpolate it), recipe-scoped (cannot describe multiple or looped cx calls),
and static (cannot express the dynamic keys that motivate using a script at all).
Anything expressible in metadata is equally expressible as a linewise `cx` line.

#### Classification (from the dump)

`cx` reliably identifies recipe form before scanning:

- **Shebang** — the recipe's first `body` line starts with `#!`.
- **`[script]`** — the recipe carries the `script` attribute.
- **Linewise** — neither of the above.

#### Recognizing a `cx` call (linewise only)

A body line is a `cx` invocation iff its first shell token, after stripping an
optional leading `@` (just's quiet-line prefix), is literally `cx`. Forms that are
**not** clean top-level invocations are not treated as static `cx` calls:
`VAR=x cx …`, `foo && cx …`, `cx` inside `$(…)`, or a `cx` token produced by
interpolation. Line continuations (`\`) are handled by reading logical lines from
the dump `body` rather than physical lines.

#### Behavior for script recipes

Two configured options; **forbid is the default**:

1. **Forbid** (default) — if a `cx` token appears inside a shebang or `[script]`
   body, lint errors: "cx is not supported inside script recipes; use a linewise
   recipe." Classification above makes this reliable.
2. **Opaque mode** (opt-in flag) — `cx` still self-gates at runtime using the
   concrete `--in`/`--out` the script computed, keyed on the concrete output set,
   with **command identity = a hash of the whole script body** (conveniently fully
   static in a `[script]` recipe, since `{{}}` is off). It is **excluded from the
   overlay graph and from the ordering/§7.5 lints** — no static safety net; the
   annotations are trusted. This drops exactly the guarantees the lints provide, so
   it is off by default.

---

## 5. Data read from the dump (no re-parsing)

`cx` shells `just --dump --dump-format json` and reads, per recipe:

- **`body`** — the recipe's lines as template fragments. Used to (a) locate `cx`
  lines and (b) compute each `cx` line's command identity by hashing its fragments.
- **`dependencies`** — recipe→recipe ordering edges. Consumed verbatim; `cx` adds
  no ordering.
- **`parameters`** — used by lints (§7) to detect arguments that influence work
  but do not appear in any output path.

`cx` performs only **argv-level flag extraction** on a `cx` line (find `--in` /
`--out`, take operands). It does not parse justfile grammar — `just` did that.

---

## 6. The overlay graph

Built once per justfile state (see §8), from **all** `cx` lines across **all**
recipes simultaneously:

1. Start from `just`'s recipe nodes and ordering edges (`dependencies`).
2. For every `cx` line in every recipe body:
   - add a file node for each `--out` template → edge `recipe →produces→ file`.
   - add a file node for each `--in` template → edge `file →consumed-by→ recipe`.
3. Match producers to consumers **by file path**, independent of which recipe they
   sit in. A file that is one line's `--out` and another's `--in` becomes an
   internal edge; an `--in` that matches no `--out` is a **source leaf** that must
   exist on disk.

Because matching is global and path-based, a recipe with two `cx` lines each
consuming a different output of a prior recipe resolves correctly: four file
edges, each consumer's `--in` matching exactly one producer's `--out`, covered by
a single recipe→recipe ordering edge.

---

## 7. Lints (`construct → lint → run`)

Lints run against the overlay graph during construction. **Strictness is
configurable; the default is hard error** (refuse to run on any violation). See
§14 for configuration.

1. **Dangling input** — every `--in` is either an existing source file or some
   `cx` line's `--out`. Otherwise error.
2. **Double producer** — no two `cx` lines declare the same `--out` (template
   unification). This invariant also makes output→line matching unambiguous (§10).
3. **Acyclic file subgraph** — the induced file dependency graph has no cycles.
4. **Ordering subset (the important one)** — if line B consumes a file that line
   A produces, B's recipe must **transitively depend on A's recipe in `just`'s
   dependency DAG.** If it does not, `just` gives no ordering guarantee and the two
   steps race. This check is only possible because the dump hands `cx` both graphs
   at once.
5. **Output-key coverage** — every recipe parameter that influences a `cx` line's
   work should appear in at least one of that line's `--out` paths. If a parameter
   changes the computation but not the output path, two parameter values collide on
   the same sidecar key (§10). Default: error.
6. **Template collision (hazard)** — two `cx` lines whose `--out` *templates* can
   unify to the same concrete path (e.g. `out/{{part}}` and `out/{{region}}`) are
   flagged statically as a warning. The airtight version is the **runtime collision
   guard** in §11.

---

## 8. Two cache lifetimes

These are deliberately separate because they are functions of different inputs.

### 8.1 Overlay graph + lint result — keyed on justfile state

Pure function of the (fully resolved) justfile. Recomputed only when the justfile
changes:

- on the **first `cx` invocation ever**, and
- on the **first `cx` invocation after any justfile edit** (including imported /
  `mod` files, since the dump reflects the resolved justfile).

Cached in `.cx/graph.json` together with a **source fingerprint**.

Fast path to avoid shelling `just --dump` on every line: `cx` records the set of
justfile source paths and their `(mtime, size, hash)` stamps. On each invocation
it stats those recorded paths; if all match, it trusts the cached graph and does
**not** shell `just`. If any differ (or no cached set exists, i.e. first run), it
runs `just --dump`, recomputes the graph + lints, and refreshes the fingerprint and
source set. (Robust discovery of the full source-file set on the very first run is
an implementation detail; see Open Questions.)

### 8.2 Per-line staleness — checked every invocation

Depends on input file contents and concrete argv, not on the justfile, so it
cannot be cached across runs. Keyed on the **interpolated output path set** (§10).
Stored in `.cx/state.json`.

---

## 9. Staleness algorithm (mtime + checksum)

Per stored input, `cx` keeps `(mtime_ns, size, hash)`. The decision for one `cx`
line:

```
key      = sorted, joined interpolated --out paths        # sidecar key
identity = command-identity hash of the matched template line   # §10
prev     = state[key]                                     # may be absent

stale =
       prev is absent
    OR prev.command_hash != identity
    OR any --out path missing on disk
    OR any --in changed                                   # see input check

input check, per --in path p with prior stamp s:
    (mtime, size) = stat(p)
    if s exists and s.mtime == mtime and s.size == size:
        unchanged              # fast path, no hashing
    else:
        h = hash(p)
        if s exists and s.hash == h:
            unchanged          # touch / git-checkout case; refresh stored mtime
        else:
            changed
```

- **Not stale** → exit `0`, **log `up-to-date: <outputs>` to stderr**, run nothing.
- **Stale** → run `-- COMMAND`. On success, record fresh input stamps, the command
  identity, and freshly hashed outputs under `key`.

This gives `checkexec`'s speed on the common path (stat only) without its false
rebuilds (a `touch` with identical content does not trigger a run), and unlike
`tup` the confirmation is content-based, not timestamp-only.

---

## 10. Command identity and output→line matching

The command-identity hash must be **partition-independent** (editing `transform`
invalidates all partitions; a new `--out` value does not). It is therefore taken
from the **template**, not from interpolated argv.

`cx` does not need to know which recipe or line invoked it. It recovers the
template by **unifying its concrete output set against the graph's output
templates**:

1. During graph construction, store for each `cx` line: its `--out` templates and
   its command-identity hash.
2. At runtime, `cx` takes the concrete `--out` paths from its argv and finds the
   `cx` line whose `--out` templates unify with them.
3. The **double-producer lint (§7.2)** guarantees output templates are distinct,
   so the match is unique (template-collision hazards are caught by §7.6 / §11).
4. The matched line yields the command-identity hash to compare against
   `prev.command_hash`.

Inputs and outputs themselves are taken **concretely from argv** — `cx` already
has them post-interpolation and needs no template resolution for them.

---

## 11. Runtime collision guard

The airtight counterpart to the static template-collision lint (§7.6):

> When writing a staleness record, if `key` (the interpolated output set) already
> exists but with a **different** command identity than the one being written,
> that is a collision — two distinct `cx` lines resolved to the same concrete
> output. This is a hard error regardless of lint configuration.

This catches the case where two templates that *could* alias actually *do* alias
at a specific argument value, which static analysis can only warn about.

---

## 12. End-to-end execution flow (one `cx` invocation)

```
1. Resolve graph:
     stat recorded source files.
     if fingerprint matches .cx/graph.json -> load cached graph + lint result.
     else -> just --dump --dump-format json; build overlay; run lints;
             write .cx/graph.json + fingerprint + source set.
2. If lint result has violations:
     honor configured strictness (default: error and abort).
3. Read concrete --in / --out / -- COMMAND from own argv.
4. Match concrete --out set to a graph line -> command identity.
5. key = sorted interpolated --out set; prev = state[key].
6. Compute staleness (§9).
7. If not stale: log "up-to-date: <outputs>" to stderr; exit 0.
8. If stale: exec -- COMMAND.
     on success: record input stamps + identity + output hashes under key
                 (apply collision guard §11); save state.
     on failure: propagate non-zero exit; do not record.
```

Plain recipes (no `cx` line) are untouched: `just` runs them normally and `cx` is
never in the process tree.

---

## 13. `.cx/` directory layout

```
.cx/
  graph.json     # cached overlay graph + lint result + source fingerprint
                 #   { source_files: {path: {mtime_ns,size,hash}},
                 #     lines: [{out_templates, in_templates, command_hash, recipe}],
                 #     lint: {violations: [...]} }
  state.json     # per-output-key staleness records
```

`state.json` record shape:

```json
{
  "warehouse/2024-01.parquet": {
    "command_hash": "…",
    "inputs": {
      "data/2024-01.parquet": { "mtime_ns": 0, "size": 0, "hash": "…" }
    },
    "outputs": {
      "warehouse/2024-01.parquet": "…"
    }
  }
}
```

The key is the sorted, joined interpolated output set; for multi-output lines all
output paths participate in the single key.

---

## 14. CLI surface and configuration

In-recipe usage (the primary mode):

```
cx --in … --out … -- COMMAND …
```

Optional standalone modes (run analysis without executing anything — useful in CI
or pre-commit):

```
cx graph      # print the resolved overlay graph
cx lint       # construct + lint only; exit non-zero on violations
```

Configuration (resolved in this order: CLI flag > env var > .cx config file):

- **Lint strictness** — `error` (default) | `warn` | per-lint overrides.
  - env: `CX_LINT=error|warn`
  - file: `.cx/config.toml` with a `[lint]` table for per-rule strictness.
- No-op logging goes to **stderr** as `up-to-date: <outputs>` (always on).

---

## 15. Worked examples

### 15.1 Plain recipe — no analysis

```just
test:
    pytest
```

`just test` runs `pytest` every time. `cx` is never involved.

### 15.2 Partitioned single step

```just
process part:
    cx --in data/{{part}}.parquet --out warehouse/{{part}}.parquet -- transform {{part}}
```

- `just process 2024-01` → `cx` receives concrete paths; keyed on
  `warehouse/2024-01.parquet`. Each partition has an independent staleness record.
- Editing `transform` changes the **template** identity → every partition reruns
  on next invocation.
- `touch data/2024-01.parquet` with no content change → no rerun.

### 15.3 Two `cx` lines consuming two outputs of a prior recipe

```just
gen:
    cx --out build/a.parquet -- make-a
    cx --out build/b.parquet -- make-b

use: gen
    cx --in build/a.parquet --out out/a.csv -- to-csv build/a.parquet
    cx --in build/b.parquet --out out/b.csv -- to-csv build/b.parquet
```

Overlay: four file edges, matched by path. `use` transitively depends on `gen`
(ordering-subset lint passes). Per-line command identity keeps the two `gen` lines
(and the two `use` lines) from invalidating each other. Touching only `make-a`'s
output reruns only the `build/a.parquet → out/a.csv` step.

---

## 16. Trust model and non-goals

- **Declared, not verified.** `cx` trusts that a step reads only its `--in` and
  writes only its `--out`. It does not intercept syscalls, so (unlike `tup`) it
  cannot detect an undeclared read or a missing write. Adding a verification pass
  (`strace -f`/`fanotify` diff of observed vs declared paths) is future work and
  slots in immediately after `exec -- COMMAND`.
- **No DAG pruning.** `cx` never stops `just` from running the recipe chain. Only
  individual `cx` lines short-circuit.
- **No ordering.** `cx` adds no execution order; it reads `just`'s.
- **No attribute metadata.** In/out edges are on the `cx` line, not in
  `[metadata(...)]`, because `just` does not interpolate attribute arguments.

---

## 17. Open questions

1. **Runtime line disambiguation.** Output→template unification (§10) relies on
   the double-producer lint for uniqueness. If `just` exposes the current recipe
   name via the environment, it could be used to disambiguate edge cases; this is
   not currently assumed.
2. **First-run source discovery.** The graph fast path needs the set of justfile
   source files (including imports/`mod`). On the first run this is unknown until
   the dump is produced; subsequent runs use the cached set. A direct way to
   enumerate source files from `just` would tighten this.
3. **Dump caching within a single build.** Many `cx` processes run during one
   `just build`. The §8.1 stat fast path avoids re-dumping when unchanged, but a
   shared in-build cache keyed on a build id could avoid even the stats.
4. **Variadic / optional parameters** in output paths and their interaction with
   the output-key-coverage lint (§7.5).
```
