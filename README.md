# agent-matters

`agent-matters` manages generated runtime homes for agent CLI runtimes.

It compiles authored profiles and capabilities into focused `.codex` and
`.claude` homes for Codex and Claude. The catalog is the source of truth;
runtime homes are disposable build artifacts.

For operating notes, read [TLDR.md](TLDR.md). For the project model and
repository boundaries, read [PROJECT.md](PROJECT.md).

## Install

```bash
npx agent-matters
```

## Development

This repository is a Rust workspace. The toolchain is pinned in
`rust-toolchain.toml`.

The CLI should stay thin. Catalog discovery, profile resolution, runtime home
generation, source import, and doctor checks belong below the CLI boundary.

Local quality gates and install workflows are defined in the `justfile`.
