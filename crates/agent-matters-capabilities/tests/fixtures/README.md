# agent-matters-capabilities fixtures

Fixtures are static input files used by integration tests under
`crates/agent-matters-capabilities/tests/`. Load them with the
`support::fixtures::fixture_path` helper so the path stays relative to this
directory regardless of where cargo is invoked from.

## Layout

* `homes/` — staged user home directories. Each subdirectory mimics a
  real `$HOME` layout (including `.agent-matters/config.toml`) and can
  be passed directly to `load_user_config`.
* `repos/` — staged authored repos. Each subdirectory mimics a real
  repo root (including `defaults/runtimes.toml` and `defaults/markers.toml`)
  and can be passed directly to `load_runtime_defaults` and
  `load_markers`.
* `manifests/` — sample profile and capability manifests. Populated as
  the manifest schemas land under ALP-1911.
* `imports/` — sample external source import records. Populated as the
  source adapters land under ALP-1916.
* `overlays/` — sample capability overlays. Populated as overlay support
  lands under ALP-1912.
* `runtime-homes/` — sample generated runtime homes for adapter
  integration tests. Populated as the runtime compiler and adapters
  land under ALP-1914 and ALP-1915.

## Authoring a fixture

1. Pick the smallest directory layout that reproduces the condition the
   test needs.
2. Mirror real filesystem layout: if the code under test expects
   `<home>/.agent-matters/config.toml`, stage exactly that.
3. Keep fixture files readable. Fixtures double as documentation of the
   expected input shape.
