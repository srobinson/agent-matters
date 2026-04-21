//! Show one profile through profile resolution.

use std::path::PathBuf;

use crate::catalog::CatalogIndexError;

use super::{ResolveProfileRequest, ResolveProfileResult, resolve_profile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowProfileRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
}

pub type ShowProfileResult = ResolveProfileResult;

pub fn show_profile(request: ShowProfileRequest) -> Result<ShowProfileResult, CatalogIndexError> {
    resolve_profile(ResolveProfileRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
        profile: request.profile,
    })
}
