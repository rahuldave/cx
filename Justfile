set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: verify

setup:
    cargo fetch

fmt path="":
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --all-targets --all-features -- -D warnings

typecheck:
    cargo check --all-targets --all-features

static: typecheck

build:
    cargo build

test:
    cargo test --all-targets --all-features

smoke:
    cargo run -- --help

diff-check:
    git diff --check

verify: fmt-check lint test smoke diff-check

# Every check of the repository: the release version, verify, then the docs site. CI runs each of them
check: release-check verify docs-check

# The release tags. scripts/release_tag.py is a copy of .agents/skills/gpa/scripts/release_tag.py of
# agent_gest_git_skills at the commit 799a44b. The job release of verify.yml runs just release on each push to main.
# The script runs just <profile>::version for each name of the key profiles of .agents/stack.json, here rust, so the
# tag is v plus the version of Cargo.toml.
release_tag := "uv run --no-project --quiet --no-python-downloads --python '>=3.9' python scripts/release_tag.py"

# rust::version prints the version of Cargo.toml. The rust profile of jnterfaces gives the same recipe. When cx
# installs that profile, take out this line and scripts/rust.just, because just stops on a module defined twice
mod rust 'scripts/rust.just'

# Tag HEAD with v plus the version of Cargo.toml, push the tag and check it. Takes --dry-run first
[positional-arguments]
release *args:
    @{{release_tag}} "$@"

# Fail when the version of Cargo.toml is below the last tag or the bump that the commits ask for. It needs no network
release-check:
    {{release_tag}} --check

# Build the release binary for TARGET, a Rust target triple, and pack it as dist/cx-TARGET.tar.gz with cx at its top
# level, plus dist/cx-TARGET.tar.gz.sha256. The job binaries of verify.yml runs it, and scripts/install.sh installs it
dist target:
    rustup target add {{target}}
    cargo build --release --locked --target {{target}}
    mkdir -p dist
    COPYFILE_DISABLE=1 tar -czf dist/cx-{{target}}.tar.gz -C target/{{target}}/release cx
    cd dist && shasum -a 256 cx-{{target}}.tar.gz > cx-{{target}}.tar.gz.sha256

# The docs site in docs/, which smalldocs builds. The workflow docs-site.yml publishes it to https://rahuldave.com/cx/
# The docs role of jnterfaces defines the same recipe names. If cx installs that role, take out these recipes first,
# because just stops on a recipe defined twice
smalldocs := "uvx --quiet smalldocs@0.5.12"

# Check the pages of the site: ignored keys, Pandoc-only syntax and broken links
docs-check:
    {{smalldocs}} check docs

# Build the site, serve it on localhost, and rebuild it on each change
docs:
    {{smalldocs}} preview docs

# Build the site in strict mode into docs/_site
docs-build:
    {{smalldocs}} render docs --strict
