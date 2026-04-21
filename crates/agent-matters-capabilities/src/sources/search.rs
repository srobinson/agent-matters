//! Source search use case dispatch.

use super::{
    SkillsShAdapter, SourceAdapter, SourceAdapterError, SourceSearchRequest, SourceSearchResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSourceRequest {
    pub source: String,
    pub query: String,
}

pub fn search_source(
    request: SearchSourceRequest,
) -> Result<SourceSearchResult, SourceAdapterError> {
    match request.source.as_str() {
        "skills.sh" => SkillsShAdapter::default().search(SourceSearchRequest {
            query: request.query,
        }),
        other => Err(SourceAdapterError::search_failed(
            other,
            "unsupported source; supported sources: skills.sh",
        )),
    }
}
