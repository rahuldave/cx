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
