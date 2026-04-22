//! [`RuntimeId`]. Each runtime adapter registers its own id, for example
//! `codex` or `claude`. Shape follows the shared simple-id rules so
//! registrations stay predictable.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::id::{IdError, validate_path_segment_id_body};

/// Identifier for a runtime adapter, for example `codex` or `claude`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeId(String);

impl RuntimeId {
    pub fn new(body: impl Into<String>) -> Result<Self, IdError> {
        let body = body.into();
        validate_path_segment_id_body(&body)?;
        Ok(Self(body))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for RuntimeId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RuntimeId::new(s)
    }
}

impl Serialize for RuntimeId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RuntimeId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_and_claude_are_valid() {
        assert_eq!(RuntimeId::new("codex").unwrap().as_str(), "codex");
        assert_eq!(RuntimeId::new("claude").unwrap().as_str(), "claude");
    }

    #[test]
    fn empty_is_rejected() {
        assert_eq!(RuntimeId::new(""), Err(IdError::Empty));
    }

    #[test]
    fn uppercase_is_rejected() {
        let err = RuntimeId::new("Codex").unwrap_err();
        assert!(matches!(err, IdError::InvalidChar { .. }));
    }

    #[test]
    fn slash_is_rejected() {
        let err = RuntimeId::new("codex/custom").unwrap_err();
        assert!(matches!(err, IdError::PathSeparator { .. }));
        assert!(err.to_string().contains("single path segment"));
    }

    #[test]
    fn serde_round_trips_as_string() {
        let id = RuntimeId::new("codex").unwrap();
        let encoded = serde_json::to_string(&id).unwrap();
        assert_eq!(encoded, "\"codex\"");
        let decoded: RuntimeId = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, id);
    }
}
