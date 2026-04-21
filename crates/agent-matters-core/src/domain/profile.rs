//! [`ProfileKind`] and [`ProfileId`]. Profile ids are simple public
//! identifiers without a kind prefix; the kind is metadata declared inside
//! the manifest, not part of the id.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::id::{IdError, validate_id_body};

/// MVP profile kinds. Kind is metadata in MVP and does not influence
/// resolution; future work may attach behavior to specific kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileKind {
    Persona,
    Task,
    Launcher,
}

impl ProfileKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ProfileKind::Persona => "persona",
            ProfileKind::Task => "task",
            ProfileKind::Launcher => "launcher",
        }
    }

    pub const fn all() -> &'static [ProfileKind] {
        &[
            ProfileKind::Persona,
            ProfileKind::Task,
            ProfileKind::Launcher,
        ]
    }
}

impl fmt::Display for ProfileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProfileKind {
    type Err = ProfileKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for kind in ProfileKind::all() {
            if kind.as_str() == s {
                return Ok(*kind);
            }
        }
        Err(ProfileKindError::Unknown {
            kind: s.to_string(),
        })
    }
}

/// Error produced when parsing a [`ProfileKind`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileKindError {
    #[error("profile kind `{kind}` is not recognized; expected one of: persona, task, launcher")]
    Unknown { kind: String },
}

/// Simple profile id, for example `github-researcher`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileId(String);

impl ProfileId {
    /// Validate and wrap the given string as a profile id.
    pub fn new(body: impl Into<String>) -> Result<Self, IdError> {
        let body = body.into();
        validate_id_body(&body)?;
        Ok(Self(body))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ProfileId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ProfileId::new(s)
    }
}

impl Serialize for ProfileId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ProfileId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_kind_round_trips() {
        for kind in ProfileKind::all() {
            assert_eq!(ProfileKind::from_str(kind.as_str()).unwrap(), *kind);
        }
    }

    #[test]
    fn unknown_profile_kind_is_reported() {
        let err = ProfileKind::from_str("service").unwrap_err();
        assert!(err.to_string().contains("service"));
    }

    #[test]
    fn profile_id_accepts_simple_kebab() {
        let id: ProfileId = "github-researcher".parse().unwrap();
        assert_eq!(id.as_str(), "github-researcher");
        assert_eq!(id.to_string(), "github-researcher");
    }

    #[test]
    fn profile_id_rejects_empty() {
        assert_eq!(ProfileId::new(""), Err(IdError::Empty));
    }

    #[test]
    fn profile_id_rejects_uppercase() {
        let err = ProfileId::new("GitHubResearcher").unwrap_err();
        assert!(matches!(err, IdError::InvalidChar { .. }));
    }

    #[test]
    fn profile_id_serde_round_trips_as_string() {
        let id: ProfileId = "linear-triage".parse().unwrap();
        let encoded = serde_json::to_string(&id).unwrap();
        assert_eq!(encoded, "\"linear-triage\"");
        let decoded: ProfileId = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, id);
    }

    #[test]
    fn profile_kind_serde_uses_kebab_case() {
        let encoded = serde_json::to_string(&ProfileKind::Persona).unwrap();
        assert_eq!(encoded, "\"persona\"");
        let decoded: ProfileKind = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ProfileKind::Persona);
    }
}
