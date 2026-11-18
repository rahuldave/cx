---
title: With jnterfaces
---

[jnterfaces](https://rahuldave.github.io/jnterfaces/) is a package of agent skills for the command interface of a project. An agent skill is a folder with a `SKILL.md` file that Codex, Claude and other agents read. In jnterfaces, the recipes of the `Justfile` are the commands of a project. cx lets a slow line of a recipe run again only when its files or its command change.

jnterfaces has a skill for cx. The skill teaches an agent when to write a cx line, how to write it and how to check it. The skill does not install the program cx.

## Install the cx skill

The cx skill comes with the role `cx` of jnterfaces. A role names a job, and a package does that job. The role is off by default. In a project that has the jnterfaces installer, turn it on:

```bash
just install cx
```

The installer copies the skill into `.agents/skills/cx/`, and records the role in `.agents/stack.json`. A later `just install` keeps the role. To take the skill out, run `just uninstall cx`.

Then install the program cx, as [Install](index.md#install) says. If an agent finds no `cx` on the `PATH`, the skill tells it to ask you before it installs the program.

## What the skill tells an agent

The skill gives an agent these rules, in `.agents/skills/cx/SKILL.md`:

- **When to use cx.** Use it for a step that reads files and writes durable files, for example a data pipeline, a conversion or a generated file.
- **When not to use cx.** Do not use it for tests, lint, a build tool with its own cache, or a command that reads the network.
- **How to write a line.** Give each line at least one `--out`, and declare each file that the command reads with `--in`. Put cx lines only in linewise recipes, and not in a script recipe.
- **How to check a pipeline.** Run it twice from clean outputs, and confirm the `up-to-date` lines. Then change one input, confirm that only the later steps run, and run `cx lint`.

The skill also has a reference with worked examples and a checklist for a review, in `references/incremental_pipelines.md`.

## How the other skills use it

The `just` skill of jnterfaces is the only skill that a project must have. It points to the cx skill for slow steps that read and write files. If a project does not have the cx skill, the `just` skill tells the agent to write each step as an ordinary line of the recipe. So a project works without cx, and its steps run each time.

`just` keeps the order of the recipes in each case. cx adds only the choice to skip a line that is up to date.
