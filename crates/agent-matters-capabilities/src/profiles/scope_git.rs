//! Lightweight GitHub repository detection for profile scope checks.

use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn matched_github_repo(detected: &str, allowed_repos: &[String]) -> Option<String> {
    let detected = normalize_repo_id(detected);
    allowed_repos
        .iter()
        .find(|raw| normalize_allowed_repo(raw) == detected)
        .cloned()
}

pub(super) fn detect_github_repo(workspace: &Path) -> Option<String> {
    let config_path = git_config_path(workspace)?;
    let config = fs::read_to_string(config_path).ok()?;
    config
        .lines()
        .filter_map(|line| line.trim().strip_prefix("url =").map(str::trim))
        .find_map(normalize_github_repo)
}

fn normalize_allowed_repo(raw: &str) -> String {
    normalize_github_repo(raw).unwrap_or_else(|| normalize_repo_id(raw))
}

fn normalize_repo_id(raw: &str) -> String {
    raw.trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_ascii_lowercase()
}

fn git_config_path(workspace: &Path) -> Option<PathBuf> {
    for ancestor in workspace.ancestors() {
        let dot_git = ancestor.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git.join("config"));
        }
        if dot_git.is_file() {
            return git_file_config_path(&dot_git);
        }
    }
    None
}

fn git_file_config_path(dot_git: &Path) -> Option<PathBuf> {
    let body = fs::read_to_string(dot_git).ok()?;
    let gitdir = body.trim().strip_prefix("gitdir:")?.trim();
    let gitdir = if Path::new(gitdir).is_absolute() {
        PathBuf::from(gitdir)
    } else {
        dot_git.parent()?.join(gitdir)
    };
    let config = gitdir.join("config");
    if config.exists() {
        return Some(config);
    }
    let common = fs::read_to_string(gitdir.join("commondir")).ok()?;
    let common = if Path::new(common.trim()).is_absolute() {
        PathBuf::from(common.trim())
    } else {
        gitdir.join(common.trim())
    };
    Some(common.join("config"))
}

fn normalize_github_repo(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let repo = trimmed
        .strip_prefix("git@github.com:")
        .or_else(|| trimmed.split_once("github.com/").map(|(_, repo)| repo))?;
    let mut parts = repo.split('/');
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim().trim_end_matches(".git");
    (!owner.is_empty() && !name.is_empty()).then(|| format!("{owner}/{name}").to_ascii_lowercase())
}
