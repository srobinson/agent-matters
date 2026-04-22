//! Shared id validation used by [`CapabilityId`], [`ProfileId`], and
//! [`RuntimeId`]. Capability ids may use `/` separated locator segments, while
//! profile and runtime ids must remain single path segments.
//!
//! Centralizing the rule here keeps the three id types aligned so users
//! see consistent diagnostics regardless of which field rejected their
//! input.

/// Validation failure produced by id body validators.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    /// The id was empty or a segment was empty (e.g. `""`, `"a//b"`, `"/a"`).
    #[error("id must not be empty")]
    Empty,
    /// A segment started or ended with `-`.
    #[error("id segment `{segment}` must not start or end with `-`")]
    BoundaryHyphen { segment: String },
    /// A segment contained an unexpected character.
    #[error("id segment `{segment}` contains invalid character `{ch}`; allowed: [a-z0-9-]")]
    InvalidChar { segment: String, ch: char },
    /// A filesystem path component id contained `/`.
    #[error("id `{body}` must be a single path segment; `/` is not allowed")]
    PathSeparator { body: String },
}

/// Validate the body portion of a simple id.
///
/// Rules:
/// * non-empty
/// * one or more `/` separated segments
/// * each segment matches `[a-z0-9](?:[a-z0-9-]*[a-z0-9])?`
///
/// Examples of valid ids: `playwright`, `helioy-core`, `helioy/mail`,
/// `openai/gpt-4o`.
pub fn validate_id_body(body: &str) -> Result<(), IdError> {
    if body.is_empty() {
        return Err(IdError::Empty);
    }
    for segment in body.split('/') {
        validate_segment(segment)?;
    }
    Ok(())
}

/// Validate an id body that will be used as one filesystem path segment.
pub fn validate_path_segment_id_body(body: &str) -> Result<(), IdError> {
    if body.contains('/') {
        return Err(IdError::PathSeparator {
            body: body.to_string(),
        });
    }
    validate_id_body(body)
}

fn validate_segment(segment: &str) -> Result<(), IdError> {
    if segment.is_empty() {
        return Err(IdError::Empty);
    }
    if segment.starts_with('-') || segment.ends_with('-') {
        return Err(IdError::BoundaryHyphen {
            segment: segment.to_string(),
        });
    }
    for ch in segment.chars() {
        let allowed = matches!(ch, 'a'..='z' | '0'..='9' | '-');
        if !allowed {
            return Err(IdError::InvalidChar {
                segment: segment.to_string(),
                ch,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_segment_is_valid() {
        assert!(validate_id_body("playwright").is_ok());
    }

    #[test]
    fn kebab_segment_is_valid() {
        assert!(validate_id_body("helioy-core").is_ok());
    }

    #[test]
    fn namespaced_segment_is_valid() {
        assert!(validate_id_body("helioy/mail").is_ok());
        assert!(validate_id_body("openai/gpt-4o").is_ok());
    }

    #[test]
    fn path_segment_rejects_namespaced_body() {
        assert_eq!(
            validate_path_segment_id_body("helioy/mail"),
            Err(IdError::PathSeparator {
                body: "helioy/mail".to_string()
            })
        );
    }

    #[test]
    fn empty_is_rejected() {
        assert_eq!(validate_id_body(""), Err(IdError::Empty));
    }

    #[test]
    fn empty_segment_is_rejected() {
        assert_eq!(validate_id_body("a//b"), Err(IdError::Empty));
        assert_eq!(validate_id_body("/a"), Err(IdError::Empty));
        assert_eq!(validate_id_body("a/"), Err(IdError::Empty));
    }

    #[test]
    fn leading_or_trailing_hyphen_is_rejected() {
        assert!(matches!(
            validate_id_body("-foo"),
            Err(IdError::BoundaryHyphen { .. })
        ));
        assert!(matches!(
            validate_id_body("foo-"),
            Err(IdError::BoundaryHyphen { .. })
        ));
    }

    #[test]
    fn uppercase_is_rejected() {
        let err = validate_id_body("Playwright").unwrap_err();
        match err {
            IdError::InvalidChar { ch, .. } => assert_eq!(ch, 'P'),
            other => panic!("expected InvalidChar, got {other:?}"),
        }
    }

    #[test]
    fn whitespace_is_rejected() {
        let err = validate_id_body("foo bar").unwrap_err();
        assert!(matches!(err, IdError::InvalidChar { ch: ' ', .. }));
    }
}
