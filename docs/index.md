---
title: cx
---

cx runs one line of a [Just](https://just.systems/) recipe only when the line is out of date. The name stands for Conditional eXecution. A recipe is a named list of command lines in a `Justfile`, and `just` runs it.

`just` decides which recipes run and in what order. cx decides only whether one command line must run again. A line that cx runs is a cx line. It names the files that the command reads, the files that it writes and the command:

```just
build: prepare
    cx --in data/message.txt --out build/message.upper -- bash -c 'tr "[:lower:]" "[:upper:]" < data/message.txt > build/message.upper'
```

- **`--in`** names an input, a file that the command reads.
- **`--out`** names an output, a file that the command writes.
- **The command** comes after `--`.

On the first run, cx runs the command. On a later run, cx compares the inputs, the outputs and the command with its record of the last run. If none of them changed, cx does not run the command, and it prints this line:

```text
up-to-date: build/message.upper
```

## When to use cx

Use cx for a slow step that reads files and writes files, for example a data pipeline, a conversion of documents or a generated file. Each step is one cx line. `just` still runs the whole recipe, and a cx line that is up to date does nothing.

Do not use cx for tests, lint or a build tool with its own cache, such as `cargo build`. Also do not use it for a command that reads the network or an environment variable, because cx cannot see such an input.

## Install

Install the latest release with the script `install.sh`. The script needs no Rust:

```bash
curl -fsSL https://raw.githubusercontent.com/rahuldave/cx/main/scripts/install.sh | sh
```

The script downloads the archive of cx for your platform from the GitHub release, and checks its SHA-256 hash. Then it puts `cx` in `~/.local/bin` and prints the version. Releases have archives for macOS on Apple silicon and Intel, and for Linux on x86_64 and arm64. To pick a release or a folder, give `--tag` or `--dir`:

```bash
curl -fsSL https://raw.githubusercontent.com/rahuldave/cx/main/scripts/install.sh | sh -s -- --tag v0.2.0 --dir ~/bin
```

To build from source, use Cargo, the package manager of Rust. Cargo puts the program in `~/.cargo/bin`:

```bash
cargo install --locked --git https://github.com/rahuldave/cx --tag v0.2.0
```

Check that the folder of `cx` is on your `PATH`:

```bash
cx --version
```

cx also needs `just` on your `PATH`, for the commands `cx graph` and `cx lint`.

## Quick start

The repository has an example in `examples/linewise`. Its recipe `build` has two cx lines. The first line writes `build/message.upper`, and the second line reads it and writes `dist/message.txt`.

1. Clone the repository, and go to the example:

   ```bash
   git clone https://github.com/rahuldave/cx.git
   cd cx/examples/linewise
   ```

2. Run the build from a clean state:

   ```bash
   just clean
   just build
   ```

   cx runs both commands, and writes both outputs.

3. Run the build again:

   ```bash
   just build
   ```

   `just` runs the recipe again, and it echoes each line of the recipe. But cx runs neither command, and it prints a line for each output:

   ```text
   up-to-date: build/message.upper
   up-to-date: dist/message.txt
   ```

4. Read the result:

   ```bash
   cat dist/message.txt
   ```

   The file holds the line `Result: HELLO FROM CX`.

If you change `data/message.txt`, the next `just build` runs both commands again. [The quickstart](quickstart.md) makes the same example in a new folder, without the repository.

## Where cx keeps its record

cx keeps its record of each cx line in `.cx/state.json`, in the folder where the line runs. The key of a record is the set of outputs of the line. Add `.cx/state.json` and `.cx/tmp/` to your `.gitignore`, because the record is local to your machine.

## More pages

- [Commands](commands.md) describes each command of cx and its options.
- [With jnterfaces](jnterfaces.md) tells how the cx skill of jnterfaces teaches an agent to write cx lines.
- [Architecture](architecture.md) tells how cx and `just` divide the work, and what is still to come.
