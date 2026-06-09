# Linewise `just` Example

This example keeps `just` in charge of recipe ordering while `cx` gates the
individual expensive lines.

From this directory, after building `cx`:

```bash
PATH="../../target/debug:$PATH" just build
PATH="../../target/debug:$PATH" just build
```

The first run writes `build/message.upper` and `dist/message.txt`. The second
run still executes the `just` recipe chain, but each `cx` line reports
`up-to-date` and skips its command.
