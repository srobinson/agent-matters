# TLDR

`agent-matters` turns a local catalog of agent capabilities and profiles into
generated runtime homes for Codex and Claude.

The catalog is the source of truth. `.codex` and `.claude` are generated
outputs. Profiles select capabilities, instructions, hooks, MCP servers,
runtime settings, and launch material. The CLI resolves that material,
validates it, writes a fingerprinted runtime build, and activates it for a
workspace when asked.

## Install

```bash
cargo install --path crates/agent-matters-cli
```

Release consumers can use the npm wrapper:

```bash
npm install -g agent-matters
```

## Common Commands

```bash
agent-matters doctor
agent-matters profiles list
agent-matters capabilities list
agent-matters profiles resolve 'debug Playwright browser automation' --runtime codex
agent-matters profiles compile my-profile --runtime codex
agent-matters profiles use my-profile ./some/repo --runtime codex
```

Use `--json` on operational commands when another tool needs structured output.

## Mental Model

A capability is one portable unit of agent behavior. It can contain
instructions, skills, hooks, MCP server definitions, runtime settings, required
environment variables, and provenance.

A profile is a selected composition of capabilities and instructions for a
class of work.

A runtime adapter renders the resolved profile into a concrete home directory
for Codex or Claude.

`doctor` is the first command to run when something feels wrong.
