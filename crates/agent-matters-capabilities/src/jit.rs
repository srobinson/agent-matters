//! Local only JIT profile resolution.

use std::io;
use std::path::PathBuf;

mod matching;
mod session_cache;

use agent_matters_core::domain::{CapabilityIdError, Diagnostic, DiagnosticSeverity, IdError};
use serde::Serialize;
use thiserror::Error;

use crate::catalog::{CatalogIndexError, LoadCatalogIndexRequest, load_or_refresh_catalog_index};
use crate::profiles::{
    ProfileBuildPlan, ResolvedProfileBuildPlanRequest, plan_profile_build,
    plan_resolved_profile_build, resolve_profile_record,
};

use self::matching::{
    Query, capability_candidates, is_ambiguous, merge_candidates, profile_candidates,
    select_existing_profile, selected_capabilities,
};
use self::session_cache::{generated_profile_record, write_session_cache_profile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitProfileResolveRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub task_text: String,
    pub runtime: String,
    pub workspace_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JitProfileResolveResult {
    pub task_text: String,
    pub runtime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<JitProfileSelection>,
    pub candidates: Vec<JitProfileCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_cache: Option<JitSessionCache>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_plan: Option<ProfileBuildPlan>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JitProfileSelection {
    pub id: String,
    pub kind: String,
    pub score: u16,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JitProfileCandidate {
    pub id: String,
    pub kind: String,
    pub score: u16,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JitSessionCache {
    pub profile_id: String,
    pub cache_dir: PathBuf,
    pub profile_manifest_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum JitProfileResolveError {
    #[error(transparent)]
    Catalog(#[from] CatalogIndexError),
    #[error("failed to create JIT profile id `{body}`: {source}")]
    ProfileId {
        body: String,
        #[source]
        source: IdError,
    },
    #[error("failed to create runtime id `{body}`: {source}")]
    RuntimeId {
        body: String,
        #[source]
        source: IdError,
    },
    #[error("failed to parse capability id `{id}` for JIT manifest: {source}")]
    CapabilityId {
        id: String,
        #[source]
        source: CapabilityIdError,
    },
    #[error("failed to encode JIT profile manifest `{path}`: {source}")]
    EncodeManifest {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error("failed to write JIT profile manifest `{path}`: {source}")]
    WriteManifest {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub fn resolve_jit_profile(
    request: JitProfileResolveRequest,
) -> Result<JitProfileResolveResult, JitProfileResolveError> {
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: request.repo_root.clone(),
        user_state_dir: request.user_state_dir.clone(),
    })?;
    let mut result = JitProfileResolveResult {
        task_text: request.task_text.clone(),
        runtime: request.runtime.clone(),
        workspace_path: request.workspace_path.clone(),
        selected: None,
        candidates: Vec::new(),
        session_cache: None,
        build_plan: None,
        diagnostics: loaded.diagnostics,
    };
    let query = Query::new(&request.task_text, request.workspace_path.as_ref());

    let profile_candidates = profile_candidates(&loaded.index.profiles, &query);
    if let Some(selected) = select_existing_profile(&profile_candidates) {
        let plan = plan_profile_build(crate::profiles::BuildProfilePlanRequest {
            repo_root: request.repo_root,
            user_state_dir: request.user_state_dir,
            profile: selected.id.clone(),
            runtime: Some(request.runtime),
        })?;
        result.selected = Some(selected);
        result.candidates = profile_candidates;
        result.diagnostics.extend(plan.diagnostics);
        result.build_plan = plan.plan;
        return Ok(result);
    }

    if is_ambiguous(&profile_candidates) {
        result.candidates = profile_candidates;
        result.diagnostics.push(ambiguous_candidates());
        return Ok(result);
    }

    let capability_candidates =
        capability_candidates(&loaded.index.capabilities, &query, &request.runtime);
    let selected_capabilities = selected_capabilities(&capability_candidates);
    if selected_capabilities.is_empty() {
        result.candidates = merge_candidates(profile_candidates, capability_candidates);
        result.diagnostics.push(no_clear_match());
        return Ok(result);
    }

    let capability_ids = selected_capabilities
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let cache = write_session_cache_profile(
        &request,
        &capability_ids,
        &selected_capabilities
            .iter()
            .map(|candidate| candidate.reason.as_str())
            .collect::<Vec<_>>(),
    )?;
    let profile_record = generated_profile_record(
        &cache.profile_id,
        &cache.cache_dir,
        &request.runtime,
        &loaded.index.capabilities,
        &capability_ids,
    );
    let resolved = resolve_profile_record(
        &request.repo_root,
        &request.user_state_dir,
        &loaded.index,
        profile_record,
        cache.profile_manifest_path.clone(),
        Vec::new(),
    );
    let plan = plan_resolved_profile_build(ResolvedProfileBuildPlanRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
        runtime: Some(request.runtime),
        resolved,
    });
    let score = selected_capabilities
        .iter()
        .map(|candidate| candidate.score)
        .max()
        .unwrap_or_default();

    result.selected = Some(JitProfileSelection {
        id: cache.profile_id.clone(),
        kind: "jit-profile".to_string(),
        score,
        reason: "clear local capability composition".to_string(),
    });
    result.candidates = merge_candidates(profile_candidates, capability_candidates);
    result.session_cache = Some(cache);
    result.diagnostics.extend(plan.diagnostics);
    result.build_plan = plan.plan;
    Ok(result)
}

fn ambiguous_candidates() -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "jit.ambiguous",
        "JIT resolver found multiple equally likely local profile candidates",
    )
    .with_recovery_hint("choose a listed profile explicitly or add a more specific task hint")
}

fn no_clear_match() -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "jit.no-clear-match",
        "JIT resolver could not find a clear local profile or capability composition",
    )
    .with_recovery_hint("use an existing profile id or mention specific local capability names")
}
