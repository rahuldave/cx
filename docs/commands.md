---
title: Commands
---

cx has one command that runs a line and two commands that read a `Justfile`. `cx --help` prints the list:

```text
cx VERSION
Conditional eXecution for linewise just commands.

USAGE:
    cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND [ARG]...
    cx graph
    cx lint
    cx --version
```

| Command | What it does |
|---|---|
| `cx --in PATH --out PATH -- COMMAND` | Runs the command when the line is out of date, and records the run. |
| `cx graph` | Prints each cx line of the `Justfile` as JSON. |
| `cx lint` | Checks the form of each cx line of the `Justfile`. |
| `cx --version`, `cx -V` | Prints the version. |
| `cx --help`, `cx -h`, `cx` | Prints the list of commands. |

## Run a line

```bash
cx [--in PATH]... --out PATH [--out PATH]... -- COMMAND [ARG]...
```

The options come before `--`, and the command comes after it:

- **`--in PATH`** names an input. Give it once for each file that the command reads. A line can have no input. cx uses a repeated path once.
- **`--out PATH`** names an output. Give it once for each file that the command writes. A line needs at least one output. cx uses a repeated path once.
- **`COMMAND [ARG]...`** is the command and its arguments. cx runs the program directly, without a shell. To use a pipe or a redirect, run `bash -c '...'`.

A path is relative to the current folder. `just` runs a recipe in the folder of its `Justfile`, so a path in a cx line is relative to that folder.

### When cx runs the command

cx keeps a record of each line in `.cx/state.json`. The set of outputs of the line is the key of its record. cx runs the command when one of these is true:

- The line has no record, for example on the first run.
- The command, an input path or an output path changed.
- The content of an input changed. cx compares the SHA-256 hash of each input. If the size and the modification time of a file are the same as before, cx uses the hash of its record.
- An output is missing, or its content is not the content that the last run wrote.

Otherwise cx prints `up-to-date:` and the outputs to standard error, and exits with code 0.

### What cx does after the command

- **The command succeeds**, and each output exists. cx writes the new record.
- **The command succeeds**, but an output is missing. cx stops with an error, and does not write the record.
- **The command fails.** cx exits with the code of the command, and does not write the record. So the next run runs the command again. On Unix, a code has 8 bits. On a system with wider codes, such as Windows, cx keeps each bit of the code.
- **A signal stops the command.** cx exits with 128 plus the number of the signal, as a shell does. For example, `SIGTERM` is signal 15, so cx exits with 143. cx does not write the record.

An input that does not exist is an error, and cx does not run the command.

### Errors in the options

cx stops with an error and runs no command in these cases:

- The line has no `--out`.
- The line has no command after `--`.
- An option is not `--in` or `--out`, for example `--input`.
- A word that is not an option comes before `--`.

## Print the graph

```bash
cx graph
```

`cx graph` reads the `Justfile` that `just` finds from the current folder, with `just --dump --dump-format json`. It prints each cx line as JSON, with its recipe, its text, its inputs, its outputs, its command and the hash of the line. It also prints the problems that `cx lint` finds, in the list `violations`.

`cx graph` writes no file.

## Check the cx lines

```bash
cx lint
```

`cx lint` reads the same lines as `cx graph`. It exits with code 0 when it finds no problem. Otherwise it prints one line for each problem, and exits with code 1:

```text
lint error: build:1: cx requires at least one --out PATH
```

`cx lint` reports these problems:

- A cx line has no `--out`, or no command after `--`.
- A cx line has an option that cx does not know, or an `--in` or `--out` without a path.
- A word that is not an option comes before `--`.
- A line whose first word is `cx` cannot be split into words, for example because a quote is not closed. A line whose first word is another program, such as `cxfoo`, is not a cx line, so `cx lint` does not check it.
- A cx line is in a script recipe. A script recipe starts with `#!` or has the attribute `[script]` or `[script(...)]`. Another attribute, such as `[group("scripts")]`, does not make a script recipe. `just` gives such a recipe to one program as one file, so cx cannot run each line by itself.

`cx lint` checks the form of the lines only. It does not run them. To check that a pipeline works, run it twice, and confirm the `up-to-date` lines on the second run.
