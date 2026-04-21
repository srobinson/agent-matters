use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

pub(crate) fn bin() -> Command {
    Command::cargo_bin("agent-matters").expect("cargo bin available in tests")
}

pub(crate) fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent-matters-capabilities/tests/fixtures")
        .join(relative)
}

pub(crate) fn write_fake_skills_bin(dir: &TempDir) -> PathBuf {
    write_script(
        dir,
        "fake-skills",
        r#"#!/bin/sh
set -eu
case "$1" in
  find)
    if [ "${2:-}" = "none" ]; then
      printf 'No skills found for "none"\n'
      exit 0
    fi
    printf 'owner/repo@playwright 2 installs\n'
    printf '%s\n' '-> https://skills.sh/owner/repo/playwright'
    ;;
  add)
    skill=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "--skill" ]; then
        shift
        skill="$1"
      fi
      shift || true
    done
    mkdir -p ".agents/skills/$skill/docs"
    cat > ".agents/skills/$skill/SKILL.md" <<'SKILL'
---
name: playwright
description: Mock Playwright skill.
metadata:
  version: "2.0.0"
---

# Playwright
SKILL
    printf 'Details.\n' > ".agents/skills/$skill/docs/usage.md"
    printf 'installed\n'
    ;;
  *)
    printf 'unsupported command\n' >&2
    exit 2
    ;;
esac
"#,
    )
}

pub(crate) fn write_script(dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, contents).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
