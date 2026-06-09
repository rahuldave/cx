# Linewise `just` Example

This example keeps `just` in charge of recipe ordering while `cx` gates the
individual expensive lines.

It has one plain setup recipe and one recipe with two `cx` lines:

```just
prepare:
    mkdir -p build dist

build: prepare
    cx --in data/message.txt --out build/message.upper -- bash -c 'tr "[:lower:]" "[:upper:]" < data/message.txt > build/message.upper'
    cx --in build/message.upper --out dist/message.txt -- bash -c 'printf "Result: " > dist/message.txt; cat build/message.upper >> dist/message.txt'
```

If `cx` is installed on your `PATH`, run:

```bash
just clean
just build
just build
```

The first run writes `build/message.upper` and `dist/message.txt`. The second
run still executes the `just` recipe chain, but each `cx` line reports
`up-to-date` and skips its command.

Check the output:

```bash
cat dist/message.txt
```

Expected:

```text
Result: HELLO FROM CX
```

To recreate this example outside the repository, see
[`../../docs/quickstart.md`](../../docs/quickstart.md).
