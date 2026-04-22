# Project

`agent-matters` exists to make agent runtime configuration authored,
inspectable, and reproducible.

The working assumption is simple: runtime homes should be generated artifacts.
Codex and Claude need concrete files in `.codex` and `.claude`, but canonical
design belongs in a catalog of profiles and capabilities.

## System Model

A capability is a portable unit of agent behavior. It can carry instructions,
skills, hooks, MCP server definitions, runtime settings, requirements, and
origin metadata. A profile is a deliberate composition of capabilities and
instructions for a class of work.

The CLI resolves a profile against the local catalog, validates runtime
compatibility and requirements, builds a fingerprinted runtime home, and then
optionally activates that build for a workspace. This gives the system two
useful properties: generated homes can be rebuilt from source material, and
activation can be changed without rewriting the authored catalog.

The current runtime adapters target Codex and Claude. The adapter contract is
kept explicit so future runtimes can consume the same resolved profile model.

## Main Workflows

`agent-matters doctor` runs the local integrity checks. It validates discovery,
manifest semantics, profile requirements, required environment variables,
overlay and vendor consistency, runtime adapter reachability, generated state,
and credential allowlist presence.

`agent-matters profiles resolve` maps task text to local profile material. When
the catalog has a clear local composition, the resolver can produce a generated
JIT profile in the session cache without mutating authored profile files.

`agent-matters profiles compile` writes an immutable runtime home under managed
state. The build is fingerprinted from resolved content.

`agent-matters profiles use` compiles if needed, validates workspace scope,
points the selected runtime at the managed build, and prints launch
instructions. Runtime launch stays with the caller.

`agent-matters sources import` normalizes external source material into the
internal catalog schema, preserves raw vendor material, and records provenance
for later drift checks.

## Repository Boundaries

`agent-matters-core` owns the pure model: domain identifiers, manifest schemas,
catalog constants, diagnostics, runtime build paths, fingerprints, and runtime
adapter contracts. It should remain free of filesystem mutation and process
orchestration.

`agent-matters-capabilities` owns behavior over the model: catalog discovery,
indexing, source adapters, source import, profile resolution, build planning,
runtime home rendering, generated state writing, and doctor checks.

`agent-matters-cli` owns the command interface: Clap definitions, dispatch,
human rendering, JSON rendering, and shell completions. It should not contain
catalog or runtime decision logic.

The module boundaries matter because this project is about controlled
generation. If the CLI starts deciding semantics, or if the domain crate starts
performing I/O, the system becomes harder to test and harder to extend.

## Catalog Shape

The catalog contains authored profiles and capabilities:

```text
catalog/profiles
catalog/agents
catalog/hooks
catalog/instructions
catalog/mcp
catalog/runtime-settings
catalog/skills
```

Each profile has a `manifest.toml` with an identifier, kind, summary,
capability selection, instruction selection, optional scope constraints,
optional runtime settings, and optional instruction output settings.

Each capability has a `manifest.toml` with an identifier, kind, summary, file
inventory, runtime support, optional requirements, and optional origin metadata.
Imported source material is preserved under `catalog/vendor`; local changes can
be represented as overlays and compared against the vendor record.

Repo defaults live under `defaults`. User state defaults to `~/.agent-matters`
and can be redirected with `AGENT_MATTERS_STATE_DIR`.

## Release And Distribution

The Rust workspace is built as `agent-matters-cli` with the binary name
`agent-matters`. Release automation uses `cargo-dist` for platform artifacts
and release attestation. The npm package named `agent-matters` is a small
installer that downloads the matching GitHub release binary where supported.

Local installation remains straightforward:

```bash
cargo install --path crates/agent-matters-cli
```

## Engineering Standards

Keep the core model pure where practical. Put filesystem reads, writes, and
process interaction in the capabilities crate. Keep CLI presentation separate
from domain decisions. Runtime adapters should consume resolved profile types,
not parse profile manifests directly.

The normal quality gate is:

```bash
just check
just build
just test
```

The toolchain is pinned in `rust-toolchain.toml`. `just check` formats and runs
Clippy with warnings as errors. `just test` runs the workspace suite through
`cargo nextest`.
