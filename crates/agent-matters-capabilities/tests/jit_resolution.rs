mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use agent_matters_capabilities::jit::{JitProfileResolveRequest, resolve_jit_profile};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use support::fixture_path;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

fn resolve(
    repo: &Path,
    state: &Path,
    task: &str,
) -> agent_matters_capabilities::jit::JitProfileResolveResult {
    resolve_jit_profile(JitProfileResolveRequest {
        repo_root: repo.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        task_text: task.to_string(),
        runtime: "codex".to_string(),
        workspace_path: Some(repo.to_path_buf()),
    })
    .unwrap()
}

fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, body).unwrap();
}

#[test]
fn exact_profile_hint_selects_existing_profile() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = resolve(
        repo.path(),
        state.path(),
        "use github-researcher for this repository investigation",
    );

    let selected = result.selected.as_ref().unwrap();
    assert_eq!(selected.kind, "existing-profile");
    assert_eq!(selected.id, "github-researcher");
    assert!(selected.reason.contains("profile id"));
    assert!(result.session_cache.is_none());
    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.build_plan.as_ref().map(|plan| plan.profile.as_str()),
        Some("github-researcher")
    );
}

#[test]
fn clear_local_capability_match_creates_session_cache_profile() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = resolve(
        repo.path(),
        state.path(),
        "I need browser automation with Linear issue context",
    );

    let selected = result.selected.as_ref().unwrap();
    assert_eq!(selected.kind, "jit-profile");
    assert!(selected.id.starts_with("jit/"));
    let cache = result.session_cache.as_ref().unwrap();
    assert!(cache.profile_manifest_path.ends_with("manifest.toml"));
    assert!(cache.profile_manifest_path.starts_with(state.path()));

    let manifest = fs::read_to_string(&cache.profile_manifest_path).unwrap();
    assert!(manifest.contains("kind = \"task\""));
    assert!(manifest.contains("\"skill:playwright\""));
    assert!(manifest.contains("\"mcp:linear\""));
    assert!(!repo.path().join("profiles").join(&selected.id).exists());
    assert_eq!(
        result
            .build_plan
            .as_ref()
            .map(|plan| plan.effective_capabilities.len()),
        Some(2)
    );
}

#[test]
fn hyphenated_capability_name_selects_local_capability() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = resolve(repo.path(), state.path(), "use session-logger");

    let selected = result.selected.as_ref().unwrap();
    assert_eq!(selected.kind, "jit-profile");
    let cache = result.session_cache.as_ref().unwrap();
    let manifest = fs::read_to_string(&cache.profile_manifest_path).unwrap();
    assert!(manifest.contains("\"hook:session-logger\""));
    assert!(result.candidates.iter().any(|candidate| {
        candidate.id == "hook:session-logger" && candidate.reason.contains("capability name")
    }));
}

#[test]
fn ambiguous_matches_return_candidates_without_writing_session_cache() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    write(
        repo.path(),
        "profiles/research-reviewer/manifest.toml",
        r#"id = "research-reviewer"
kind = "task"
summary = "Research review profile."
capabilities = ["mcp:linear"]
instructions = []

[runtimes.codex]
enabled = true
"#,
    );

    let result = resolve(repo.path(), state.path(), "research");

    assert!(result.selected.is_none());
    assert!(result.session_cache.is_none());
    assert!(result.build_plan.is_none());
    assert!(result.candidates.len() >= 2);
    assert!(
        result
            .candidates
            .iter()
            .any(|c| c.id == "github-researcher")
    );
    assert!(
        result
            .candidates
            .iter()
            .any(|c| c.id == "research-reviewer")
    );
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == DiagnosticSeverity::Warning && diagnostic.code == "jit.ambiguous"
    }));
}

#[test]
fn jit_resolution_does_not_call_external_source_tools() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let poison_bin = TempDir::new().unwrap();
    let marker = poison_bin.path().join("npx-was-called");
    write(
        poison_bin.path(),
        "npx",
        &format!(
            "#!/bin/sh\n/bin/echo called > {}\nexit 42\n",
            marker.display()
        ),
    );
    fs::set_permissions(
        poison_bin.path().join("npx"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let old_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", poison_bin.path());
    }

    let result = resolve(repo.path(), state.path(), "linear");

    unsafe {
        match old_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
    }
    assert!(result.selected.is_some());
    assert!(!marker.exists());
}

#[test]
fn session_cache_output_shape_is_stable() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = resolve(repo.path(), state.path(), "linear");
    let cache = result.session_cache.as_ref().unwrap();
    let cache_dir = cache.profile_manifest_path.parent().unwrap();

    assert_eq!(cache.cache_dir, cache_dir);
    assert_eq!(
        cache_dir
            .strip_prefix(state.path())
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str(),
        "session-cache"
    );
    assert!(cache_dir.ends_with(cache.profile_id.replace('/', "-")));
    assert!(result.candidates.iter().all(|candidate| {
        candidate.kind == "capability" || candidate.kind == "existing-profile"
    }));
}
