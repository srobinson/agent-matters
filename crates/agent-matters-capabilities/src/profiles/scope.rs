//! Validate profile scope constraints against a target workspace path.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::MANIFEST_FILE_NAME;
use agent_matters_core::domain::{
    Diagnostic, DiagnosticLocation, DiagnosticSeverity, ScopeConstraints, ScopeEnforcement,
};
use serde::Serialize;

use super::ResolveProfileResult;
use super::scope_git::{detect_github_repo, matched_github_repo};

#[derive(Debug, Clone)]
pub struct ProfileScopeValidationRequest<'a> {
    pub resolved: &'a ResolveProfileResult,
    pub repo_root: PathBuf,
    pub workspace_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProfileUseScopeValidationRequest<'a> {
    pub resolved: &'a ResolveProfileResult,
    pub repo_root: PathBuf,
    pub workspace_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileScopeValidationResult {
    pub profile: String,
    pub requested_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    pub scope: ScopeConstraints,
    pub status: ProfileScopeValidationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_scope: Option<MatchedScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_github_repo: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ProfileScopeValidationResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileScopeValidationStatus {
    NotResolved,
    Unrestricted,
    NotEnforced,
    InScope,
    OutOfScope,
    PathMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchedScope {
    pub kind: String,
    pub value: String,
}

pub fn validate_profile_use_scope(
    request: ProfileUseScopeValidationRequest<'_>,
) -> ProfileScopeValidationResult {
    let workspace_path = request
        .workspace_path
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    validate_profile_scope(ProfileScopeValidationRequest {
        resolved: request.resolved,
        repo_root: request.repo_root,
        workspace_path,
    })
}

pub fn validate_profile_scope(
    request: ProfileScopeValidationRequest<'_>,
) -> ProfileScopeValidationResult {
    let scope = request
        .resolved
        .record
        .as_ref()
        .map(|record| record.scope.clone())
        .unwrap_or_default();
    let requested_path = absolutize_workspace_path(&request.workspace_path);
    let mut result = ProfileScopeValidationResult {
        profile: request.resolved.profile.clone(),
        requested_path: path_string(&requested_path),
        canonical_path: None,
        scope: scope.clone(),
        status: ProfileScopeValidationStatus::NotResolved,
        matched_scope: None,
        detected_github_repo: None,
        diagnostics: Vec::new(),
    };

    let Some(record) = request.resolved.record.as_ref() else {
        return result;
    };
    let manifest_path = PathBuf::from(&record.source_path).join(MANIFEST_FILE_NAME);

    if !scope.has_allowed_targets()
        && let Some(diagnostic) = missing_scope_targets(&result.profile, &scope, &manifest_path)
    {
        result.diagnostics.push(diagnostic);
    }

    let canonical_path = match fs::canonicalize(&requested_path) {
        Ok(path) => path,
        Err(source) => {
            result.status = ProfileScopeValidationStatus::PathMissing;
            result.diagnostics.push(path_missing(
                &result.profile,
                &result.requested_path,
                &scope,
                &source.to_string(),
            ));
            return result;
        }
    };
    result.canonical_path = Some(path_string(&canonical_path));

    if !scope.has_allowed_targets() {
        result.status = ProfileScopeValidationStatus::Unrestricted;
        return result;
    }

    if scope.enforcement == ScopeEnforcement::None {
        result.status = ProfileScopeValidationStatus::NotEnforced;
        return result;
    }

    if let Some(path) = matched_path_scope(&canonical_path, &scope.paths, &request.repo_root) {
        result.status = ProfileScopeValidationStatus::InScope;
        result.matched_scope = Some(MatchedScope {
            kind: "path".to_string(),
            value: path,
        });
        return result;
    }

    if let Some(repo) = detect_github_repo(&canonical_path) {
        result.detected_github_repo = Some(repo.clone());
        if let Some(allowed) = matched_github_repo(&repo, &scope.github_repos) {
            result.status = ProfileScopeValidationStatus::InScope;
            result.matched_scope = Some(MatchedScope {
                kind: "github_repo".to_string(),
                value: allowed,
            });
            return result;
        }
    }

    result.status = ProfileScopeValidationStatus::OutOfScope;
    result.diagnostics.push(out_of_scope(
        &result.profile,
        &result.requested_path,
        &scope,
        &manifest_path,
    ));
    result
}

fn matched_path_scope(
    workspace: &Path,
    allowed_paths: &[String],
    repo_root: &Path,
) -> Option<String> {
    let repo_root = normalize_existing_path(repo_root);
    allowed_paths.iter().find_map(|raw| {
        let allowed = normalize_scope_path(raw, &repo_root);
        workspace.starts_with(&allowed).then(|| raw.clone())
    })
}

fn normalize_scope_path(raw: &str, repo_root: &Path) -> PathBuf {
    let expanded = expand_tilde(Path::new(raw));
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        repo_root.join(expanded)
    };
    normalize_existing_path(&absolute)
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn absolutize_workspace_path(path: &Path) -> PathBuf {
    let expanded = expand_tilde(path);
    if expanded.is_absolute() {
        expanded
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(expanded)
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let rendered = path.to_string_lossy();
    if rendered == "~" {
        return home_dir().unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = rendered.strip_prefix("~/") {
        return home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| path.to_path_buf());
    }
    path.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn out_of_scope(
    profile: &str,
    requested_path: &str,
    scope: &ScopeConstraints,
    manifest_path: &Path,
) -> Diagnostic {
    Diagnostic::new(
        match scope.enforcement {
            ScopeEnforcement::Warn => DiagnosticSeverity::Warning,
            ScopeEnforcement::Fail => DiagnosticSeverity::Error,
            ScopeEnforcement::None => DiagnosticSeverity::Info,
        },
        "profile.scope.out-of-scope",
        format!(
            "profile `{profile}` is not scoped for path `{requested_path}`; allowed scopes: {}",
            allowed_scope_text(scope)
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(manifest_path, "scope"))
    .with_recovery_hint("use a path inside the allowed scope or update the profile manifest")
}

fn path_missing(
    profile: &str,
    requested_path: &str,
    scope: &ScopeConstraints,
    source: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.scope.path-not-found",
        format!(
            "profile `{profile}` cannot use missing path `{requested_path}`; allowed scopes: {}; source: {source}",
            allowed_scope_text(scope)
        ),
    )
    .with_location(DiagnosticLocation::field("path"))
    .with_recovery_hint("create the path or pass an existing workspace path")
}

fn missing_scope_targets(
    profile: &str,
    scope: &ScopeConstraints,
    manifest_path: &Path,
) -> Option<Diagnostic> {
    let severity = match scope.enforcement {
        ScopeEnforcement::Warn => DiagnosticSeverity::Warning,
        ScopeEnforcement::Fail => DiagnosticSeverity::Error,
        ScopeEnforcement::None => return None,
    };

    Some(
        Diagnostic::new(
            severity,
            "profile.scope.missing-targets",
            format!(
                "profile `{profile}` declares scope enforcement `{}` without any allowed targets in `paths` or `github_repos`",
                scope.enforcement
            ),
        )
        .with_location(DiagnosticLocation::manifest_field(manifest_path, "scope"))
        .with_recovery_hint(
            "add at least one scope path or GitHub repository, or set enforcement to none",
        ),
    )
}

fn allowed_scope_text(scope: &ScopeConstraints) -> String {
    format!(
        "paths [{}], github_repos [{}]",
        render_list(&scope.paths),
        render_list(&scope.github_repos)
    )
}

fn render_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
