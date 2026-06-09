# Quickstart

This guide assumes `cx` is already installed somewhere on your `PATH`.

Check:

```bash
cx --help
just --version
```

## Run The Bundled Example

From this repository:

```bash
cd examples/linewise
just clean
just build
just build
```

Expected behavior:

1. The first build creates `build/message.upper` and `dist/message.txt`.
2. The second build still runs the `just` recipe chain.
3. Each `cx` line reports that its declared output is fresh:

```text
up-to-date: build/message.upper
up-to-date: dist/message.txt
```

Inspect the result:

```bash
cat dist/message.txt
```

Expected output:

```text
Result: HELLO FROM CX
```

## Recreate The Example From Scratch

You do not need this repository to try the mental model. Create a new directory:

```bash
mkdir cx-demo
cd cx-demo
mkdir -p data
printf 'hello from cx\n' > data/message.txt
```

Create a `Justfile`:

```just
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: build

prepare:
    mkdir -p build dist

build: prepare
    cx --in data/message.txt --out build/message.upper -- bash -c 'tr "[:lower:]" "[:upper:]" < data/message.txt > build/message.upper'
    cx --in build/message.upper --out dist/message.txt -- bash -c 'printf "Result: " > dist/message.txt; cat build/message.upper >> dist/message.txt'

clean:
    rm -rf build dist .cx
```

Run it:

```bash
just build
just build
```

The first run executes both commands. The second run prints:

```text
up-to-date: build/message.upper
up-to-date: dist/message.txt
```

Now change the input:

```bash
printf 'changed input\n' > data/message.txt
just build
cat dist/message.txt
```

Expected output:

```text
Result: CHANGED INPUT
```

Only the `cx` lines decide whether to rerun. The `prepare` recipe remains a
plain `just` recipe and still runs normally.

## Inspect The Static View

From a directory with a `Justfile`, run:

```bash
cx graph
```

`cx graph` uses `just --dump --dump-format json` and reports the linewise `cx`
calls it can see. Plain recipe lines are left alone.

Run:

```bash
cx lint
```

The MVP lint pass checks basic `cx` line shape, such as requiring at least one
`--out`.

## What To Look For

After running a `cx` line, a `.cx/` directory appears:

```text
.cx/
  state.json
```

That file records the declared output set, command identity, input stamps, and
output hashes. It is runtime state and should not be committed.
