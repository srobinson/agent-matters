default:
    @just --list

AGENT_MATTERS_LOCAL_BIN := env_var_or_default("AGENT_MATTERS_LOCAL_BIN", "/Users/alphab/Dev/LLM/DEV/helioy/agent-matters-worktrees/nancy-ALP-1960/target/release/agent-matters")

build:
    cargo build --workspace

release:
    cargo build --workspace --release

install: release
    cargo install --path crates/agent-matters-cli

install-local:
    cargo build --release -p agent-matters-cli
    @set -eu; \
    src="$(pwd)/target/release/agent-matters"; \
    dest="{{AGENT_MATTERS_LOCAL_BIN}}"; \
    case "$dest" in /*) ;; *) dest="$(pwd)/$dest";; esac; \
    if [ "$src" = "$dest" ]; then \
        echo "Built $src"; \
    else \
        mkdir -p "$(dirname "$dest")"; \
        install -m 755 "$src" "$dest"; \
        echo "Installed $dest"; \
    fi

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
