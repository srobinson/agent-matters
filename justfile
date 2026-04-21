default:
    @just --list

build:
    cargo build --workspace

release:
    cargo build --workspace --release

test:
    cargo nextest run --workspace

# Run doctests (nextest does not execute doctests)
test-doc:
    cargo test --workspace --doc

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets --fix --allow-dirty -- -D warnings

check: fmt clippy
