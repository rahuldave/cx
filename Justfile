# begin lines of jnterfaces, the installer, from jnterfaces at a0cb68d.
# Last written to Justfile by `install_stack.py --target . --profile rust --defaults integration`. The record is parts."Justfile" in .agents/stack.json.
import? '.agents/install.just'
export JLOG_RUN := env("JLOG_RUN", uuid())
# end lines of jnterfaces. They stay while the project keeps .agents/stack.json.

# The recipes of cx. The block of jnterfaces above imports .agents/install.just, which gives just install, and the
# recipes of the rust profile and of the roles, for example setup, build, fmt, fmt-check, lint, typecheck, test,
# integration, coverage-check, docstring-check, diff-check, tools-check, check, ci, release, release-check,
# prose-check, review, update-docs and rust::version. AGENTS.md lists them. The index sets the
# shell of the Justfile to jlog. Each name below is cx's own, and the index leaves it out.

default: verify

# The static check of the code, as an alias of the step typecheck
static: typecheck

# Run the binary once, as a smoke test of the build
smoke:
    cargo run -- --help

# The quick local check: the format, the lint, the tests, the smoke test and the whitespace of the diff
verify: fmt-check lint test integration smoke diff-check

# Build the release binary for TARGET, a Rust target triple, and pack it as dist/cx-TARGET.tar.gz with cx at its top
# level, plus dist/cx-TARGET.tar.gz.sha256. The job binaries of verify.yml runs it, and scripts/install.sh installs it
dist target:
    rustup target add {{target}}
    cargo build --release --locked --target {{target}}
    mkdir -p dist
    COPYFILE_DISABLE=1 tar -czf dist/cx-{{target}}.tar.gz -C target/{{target}}/release cx
    cd dist && shasum -a 256 cx-{{target}}.tar.gz > cx-{{target}}.tar.gz.sha256

# The docs site in docs/, which smalldocs builds. The workflow docs-site.yml publishes it to https://rahuldave.com/cx/
# The docs role of jnterfaces defines the same recipe names, so cx leaves that role out: just install stops on a
# recipe defined twice
smalldocs := "uvx --quiet smalldocs@0.5.12"

# Check the pages of the site: ignored keys, Pandoc-only syntax and broken links
docs-check:
    {{smalldocs}} check docs
    jlog step docs-check

# Build the site, serve it on localhost, and rebuild it on each change
docs:
    {{smalldocs}} preview docs

# Build the site in strict mode into docs/_site
docs-build:
    {{smalldocs}} render docs --strict
