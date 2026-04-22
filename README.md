# agent-matters

`agent-matters` is a Rust CLI for compiling an authored catalog of agent
capabilities into focused runtime homes for Codex and Claude.

The catalog is the source of truth. Profiles choose capabilities,
instructions, hooks, MCP servers, runtime settings, and launch material.
Generated `.codex` and `.claude` homes are disposable build outputs,
fingerprinted from resolved content and activated per workspace.

For the short version, read [TLDR.md](TLDR.md). For the project model and
repository boundaries, read [PROJECT.md](PROJECT.md).

## Install

```bash
npx agent-matters
```

## Quick Start

```bash
agent-matters doctor
agent-matters profiles list
agent-matters capabilities list
agent-matters profiles show my-profile
agent-matters profiles compile my-profile --runtime codex
agent-matters profiles use my-profile ./some/repo --runtime codex
```

`profiles compile` writes an immutable runtime build without touching an
existing `.codex` or `.claude` directory. `profiles use` compiles as needed,
points the selected runtime at the managed build, and prints the launch
command for the caller.

## Command Surface

| Command | Purpose |
| --- | --- |
| `agent-matters doctor` | Validate catalog discovery, manifest semantics, runtime adapter reachability, generated state, overlays, vendor records, credential allowlists, and required environment variables. |
| `agent-matters profiles list` | List profiles discovered in the local catalog. |
| `agent-matters profiles show <profile>` | Show one profile and its resolved inventory. |
| `agent-matters profiles resolve <task> --runtime <runtime>` | Resolve task text into an existing profile or a local JIT profile. |
| `agent-matters profiles compile <profile> --runtime <runtime>` | Build a runtime home without activating it. |
| `agent-matters profiles use <profile> [path] --runtime <runtime>` | Activate a profile for a workspace and print launch instructions. |
| `agent-matters capabilities list` | List capabilities discovered in the catalog. |
| `agent-matters capabilities show <capability>` | Show one capability and its metadata. |
| `agent-matters capabilities diff <capability>` | Compare a capability overlay against its vendor record. |
| `agent-matters sources search <source> <query>` | Search a registered external source. |
| `agent-matters sources import <locator>` | Normalize and import a capability from an external source. |
| `agent-matters completions <shell>` | Generate shell completions. |

Most operational commands accept `--json` for machine readable output.

## Catalog Model

Profiles live under `catalog/profiles`. A profile manifest declares an `id`,
`kind`, `summary`, selected `capabilities`, selected `instructions`, optional
scope constraints, optional runtime configuration, and optional instruction
output settings.

Capabilities live under kind specific catalog directories:

```text
catalog/agents
catalog/hooks
catalog/instructions
catalog/mcp
catalog/runtime-settings
catalog/skills
```

A capability manifest declares an `id`, `kind`, `summary`, file inventory,
runtime support, optional requirements, and optional origin metadata. Imported
source material is preserved under `catalog/vendor`; local changes can be
tracked through overlays and inspected with `capabilities diff`.

User state defaults to `~/.agent-matters`. Set `AGENT_MATTERS_STATE_DIR` to use
another state directory.

## Repository Shape

| Crate | Responsibility |
| --- | --- |
| `agent-matters-core` | Pure domain types, manifest schemas, catalog constants, runtime build paths, diagnostics, and fingerprints. |
| `agent-matters-capabilities` | Catalog discovery, indexing, source import, profile resolution, build planning, runtime home writing, and doctor checks. |
| `agent-matters-cli` | Argument parsing, command dispatch, human output, JSON output, and shell completions. |

The CLI is intentionally thin. Decisions about catalogs, profiles, runtimes,
sources, and generated state belong below it.

## Development

The Rust toolchain is pinned in `rust-toolchain.toml`.

```bash
just check
just build
just test
```

`just check` runs formatting and Clippy with warnings as errors. `just test`
runs the workspace test suite through `cargo nextest`.
