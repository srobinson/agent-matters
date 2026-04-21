use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use agent_matters_core::catalog::{CapabilityIndexRecord, ProfileIndexRecord};

use super::{JitProfileCandidate, JitProfileSelection};

const EXISTING_PROFILE_SELECTION_SCORE: u16 = 90;
const CAPABILITY_SELECTION_SCORE: u16 = 16;
const STOP_WORDS: &[&str] = &[
    "and", "for", "the", "this", "that", "with", "need", "needs", "use", "using",
];

#[derive(Debug)]
pub(super) struct Query {
    text: String,
    tokens: BTreeSet<String>,
}

impl Query {
    pub(super) fn new(task_text: &str, workspace_path: Option<&PathBuf>) -> Self {
        let mut combined = task_text.to_lowercase();
        if let Some(path) = workspace_path {
            combined.push(' ');
            combined.push_str(&path.to_string_lossy().to_lowercase());
        }
        Self {
            tokens: tokens(&combined),
            text: combined,
        }
    }
}

pub(super) fn profile_candidates(
    profiles: &BTreeMap<String, ProfileIndexRecord>,
    query: &Query,
) -> Vec<JitProfileCandidate> {
    ranked_candidates(profiles.values().filter_map(|profile| {
        let (score, reason) = score_profile(profile, query);
        (score > 0).then(|| JitProfileCandidate {
            id: profile.id.clone(),
            kind: "existing-profile".to_string(),
            score,
            reason,
        })
    }))
}

pub(super) fn capability_candidates(
    capabilities: &BTreeMap<String, CapabilityIndexRecord>,
    query: &Query,
    runtime: &str,
) -> Vec<JitProfileCandidate> {
    ranked_candidates(capabilities.values().filter_map(|capability| {
        if !capability
            .runtimes
            .get(runtime)
            .is_some_and(|runtime| runtime.supported)
        {
            return None;
        }
        let (score, reason) = score_capability(capability, query);
        (score > 0).then(|| JitProfileCandidate {
            id: capability.id.clone(),
            kind: "capability".to_string(),
            score,
            reason,
        })
    }))
}

pub(super) fn select_existing_profile(
    candidates: &[JitProfileCandidate],
) -> Option<JitProfileSelection> {
    let candidate = candidates.first()?;
    if candidate.score < EXISTING_PROFILE_SELECTION_SCORE {
        return None;
    }
    if candidates
        .get(1)
        .is_some_and(|next| next.score == candidate.score)
    {
        return None;
    }
    Some(JitProfileSelection {
        id: candidate.id.clone(),
        kind: candidate.kind.clone(),
        score: candidate.score,
        reason: candidate.reason.clone(),
    })
}

pub(super) fn is_ambiguous(candidates: &[JitProfileCandidate]) -> bool {
    candidates.len() > 1 && candidates[0].score == candidates[1].score && candidates[0].score > 0
}

pub(super) fn selected_capabilities(
    candidates: &[JitProfileCandidate],
) -> Vec<JitProfileCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.score >= CAPABILITY_SELECTION_SCORE)
        .cloned()
        .collect()
}

pub(super) fn merge_candidates(
    profiles: Vec<JitProfileCandidate>,
    capabilities: Vec<JitProfileCandidate>,
) -> Vec<JitProfileCandidate> {
    ranked_candidates(profiles.into_iter().chain(capabilities))
}

fn ranked_candidates(
    candidates: impl IntoIterator<Item = JitProfileCandidate>,
) -> Vec<JitProfileCandidate> {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.id.cmp(&right.id))
    });
    candidates
}

fn score_profile(profile: &ProfileIndexRecord, query: &Query) -> (u16, String) {
    if query.text.contains(&profile.id.to_lowercase()) {
        return (
            100,
            format!("task text mentions profile id `{}`", profile.id),
        );
    }
    if all_id_tokens_match(&profile.id, query) {
        return (
            70,
            format!("task text matches profile id tokens `{}`", profile.id),
        );
    }
    let overlap = token_overlap(&profile.summary, query);
    (
        overlap * 12,
        format!("{overlap} task token(s) match the profile summary"),
    )
}

fn score_capability(capability: &CapabilityIndexRecord, query: &Query) -> (u16, String) {
    let body = capability
        .id
        .split_once(':')
        .map_or(capability.id.as_str(), |(_, body)| body);
    if query.text.contains(&capability.id.to_lowercase()) {
        return (
            60,
            format!("task text mentions capability id `{}`", capability.id),
        );
    }
    if query.tokens.contains(body) || query.text.contains(&body.replace('-', " ")) {
        return (45, format!("task text mentions capability name `{body}`"));
    }
    let overlap = token_overlap(&capability.summary, query);
    (
        overlap * 8,
        format!("{overlap} task token(s) match the capability summary"),
    )
}

fn all_id_tokens_match(id: &str, query: &Query) -> bool {
    let id_tokens = tokens(id);
    !id_tokens.is_empty() && id_tokens.iter().all(|token| query.tokens.contains(token))
}

fn token_overlap(text: &str, query: &Query) -> u16 {
    tokens(text)
        .intersection(&query.tokens)
        .filter(|token| !STOP_WORDS.contains(&token.as_str()))
        .count() as u16
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(str::to_lowercase)
        .collect()
}
