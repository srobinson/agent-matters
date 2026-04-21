# Contributing to agent-matters

These standards apply from the first commit. Follow them in every change.

## Module responsibility

* **Single responsibility per module.** A module does one thing. If you find
  yourself naming a module `utils` or `helpers`, split it until the names
  describe what each piece *does*.
* **Domain code stays pure where practical.** `agent-matters-core` performs
  no filesystem I/O and no process orchestration. If a change needs I/O,
  add it in `agent-matters-capabilities`.
* **Filesystem mutation lives in explicit writer modules.** Readers read,
  writers write, and the two are not mixed in one function.
* **CLI presentation contains no domain logic.** `agent-matters-cli` parses
  arguments and renders output; every decision that is not presentation
  calls into `agent-matters-capabilities`.
* **Runtime adapters never parse profile TOML directly.** They consume the
  resolved, validated profile types produced by the profile resolver.
* **Source adapters transform external records into the internal schema.**
  Every external source has its own adapter module; the rest of the code
  base only ever sees normalized `agent-matters` types.

## File size

* Target around **250 lines per file**. At that size the file still fits
  on screen and one module is clearly one idea.
* Hard ceiling is around **500 lines**. If a file is climbing past 500,
  split it *before* adding to it.
* These limits apply to implementation files. Test modules and fixtures
  are not subject to the ceiling, but the same readability considerations
  apply.

If a file is over the ceiling, refactor before adding to it. No "I'll just
add this one more thing". Refactor first.

## Tests

* **Unit tests are colocated with implementation** in a `#[cfg(test)] mod
  tests` block at the bottom of the file. This keeps the test next to the
  code under test and makes private item coverage trivial.
* **Integration tests live in `crates/<crate>/tests/`.** Each file there
  is one integration test binary. Shared helpers go in
  `tests/support/mod.rs` (the subdirectory keeps cargo from treating it
  as a test binary).
* **Fixtures live in `crates/<crate>/tests/fixtures/`** and are loaded via
  the `support::fixture_path` helper. See the
  [fixture README](crates/agent-matters-capabilities/tests/fixtures/README.md)
  for the layout.
* **Test first** for rules, regressions, and compiler behavior whenever
  practical. Bug fixes get a failing test reproducing the bug first, then
  the fix.

## Commits

* Keep commit messages self contained so the next contributor can pick up
  from `git log` alone.
* Commit title format: `<author>[<issue-id>]: <summary>`.
  * Worker commits use `nancy[ALP-XXXX]: ...`.
  * Reviewer commits use `review[ALP-XXXX]: ...`.
* Describe *why* the change is shaped the way it is, not just what it
  does. Mention decisions you ruled out if they are non-obvious.

## Quality gates

Run the full workspace gate before every commit:

```bash
just check && just build && just test
```

* `just check` runs `cargo fmt` plus `cargo clippy --workspace
  --all-targets -- -D warnings`. Warnings are errors.
* `just build` builds the workspace in debug mode.
* `just test` runs the full workspace suite via `cargo nextest`.

A failing gate is a blocker, not a nuisance. Do not commit unverified
code. If `just check` fails, diagnose and fix the underlying issue; do
not use `--no-verify`.

## Toolchain

The Rust toolchain is pinned in `rust-toolchain.toml`. Do not downgrade
the channel in a feature branch without coordination.
