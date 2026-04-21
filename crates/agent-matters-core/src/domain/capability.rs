//! [`CapabilityKind`] and [`CapabilityId`], the kind prefixed id used
//! throughout the catalog. An id renders as `<kind>:<body>`, for example
//! `skill:playwright` or `agent:github-researcher`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::id::{IdError, validate_id_body};

/// MVP capability kinds. Profile manifests reference capabilities through
/// ids that begin with one of these prefixes. Additional kinds may be
/// added; unknown kinds are rejected at parse time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityKind {
    Skill,
    Mcp,
    Hook,
    Instruction,
    Agent,
    RuntimeSetting,
}

impl CapabilityKind {
    /// Canonical lowercase kebab representation used in ids and manifests.
    pub const fn as_str(&self) -> &'static str {
        match self {
            CapabilityKind::Skill => "skill",
            CapabilityKind::Mcp => "mcp",
            CapabilityKind::Hook => "hook",
            CapabilityKind::Instruction => "instruction",
            CapabilityKind::Agent => "agent",
            CapabilityKind::RuntimeSetting => "runtime-setting",
        }
    }

    /// All kinds in declaration order. Useful for exhaustive iteration
    /// in tests and in `doctor` output.
    pub const fn all() -> &'static [CapabilityKind] {
        &[
            CapabilityKind::Skill,
            CapabilityKind::Mcp,
            CapabilityKind::Hook,
            CapabilityKind::Instruction,
            CapabilityKind::Agent,
            CapabilityKind::RuntimeSetting,
        ]
    }
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CapabilityKind {
    type Err = CapabilityIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for kind in CapabilityKind::all() {
            if kind.as_str() == s {
                return Ok(*kind);
            }
        }
        Err(CapabilityIdError::UnknownKind {
            kind: s.to_string(),
        })
    }
}

/// Error produced when parsing a [`CapabilityId`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityIdError {
    /// Input did not contain the required `:` separator.
    #[error("capability id `{input}` must be `<kind>:<body>`")]
    MissingSeparator { input: String },
    /// Kind portion did not match a known capability kind.
    #[error(
        "capability id uses unknown kind `{kind}`; expected one of: \
         skill, mcp, hook, instruction, agent, runtime-setting"
    )]
    UnknownKind { kind: String },
    /// Body portion failed id validation.
    #[error("capability id body is invalid: {source}")]
    InvalidBody {
        #[source]
        source: IdError,
    },
}

/// Kind prefixed capability id, for example `skill:playwright`.
///
/// Parsing is strict: an unknown kind or malformed body returns a
/// [`CapabilityIdError`] carrying the offending input so diagnostics are
/// actionable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityId {
    kind: CapabilityKind,
    body: String,
}

impl CapabilityId {
    /// Construct from an already validated kind and body. Returns an
    /// error if the body fails id validation.
    pub fn new(kind: CapabilityKind, body: impl Into<String>) -> Result<Self, CapabilityIdError> {
        let body = body.into();
        validate_id_body(&body).map_err(|source| CapabilityIdError::InvalidBody { source })?;
        Ok(Self { kind, body })
    }

    pub fn kind(&self) -> CapabilityKind {
        self.kind
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.as_str(), self.body)
    }
}

impl FromStr for CapabilityId {
    type Err = CapabilityIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((kind_part, body_part)) = s.split_once(':') else {
            return Err(CapabilityIdError::MissingSeparator {
                input: s.to_string(),
            });
        };
        let kind = CapabilityKind::from_str(kind_part)?;
        Self::new(kind, body_part)
    }
}

impl Serialize for CapabilityId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips_via_string() {
        for kind in CapabilityKind::all() {
            assert_eq!(
                CapabilityKind::from_str(kind.as_str()).unwrap(),
                *kind,
                "kind {} should round trip",
                kind
            );
        }
    }

    #[test]
    fn runtime_setting_uses_kebab_case() {
        assert_eq!(
            CapabilityKind::RuntimeSetting.to_string(),
            "runtime-setting"
        );
    }

    #[test]
    fn valid_capability_ids_parse() {
        let id: CapabilityId = "skill:playwright".parse().unwrap();
        assert_eq!(id.kind(), CapabilityKind::Skill);
        assert_eq!(id.body(), "playwright");
        assert_eq!(id.to_string(), "skill:playwright");
    }

    #[test]
    fn namespaced_body_is_allowed() {
        let id: CapabilityId = "skill:helioy/mail".parse().unwrap();
        assert_eq!(id.body(), "helioy/mail");
    }

    #[test]
    fn missing_separator_is_actionable() {
        let err: CapabilityIdError = "playwright".parse::<CapabilityId>().unwrap_err();
        match err {
            CapabilityIdError::MissingSeparator { input } => {
                assert_eq!(input, "playwright");
            }
            other => panic!("expected MissingSeparator, got {other:?}"),
        }
    }

    #[test]
    fn unknown_kind_lists_accepted_kinds() {
        let err = "widget:foo".parse::<CapabilityId>().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("widget"));
        assert!(msg.contains("skill"));
        assert!(msg.contains("runtime-setting"));
    }

    #[test]
    fn invalid_body_surfaces_id_error() {
        let err = "skill:Foo".parse::<CapabilityId>().unwrap_err();
        assert!(matches!(
            err,
            CapabilityIdError::InvalidBody {
                source: IdError::InvalidChar { .. }
            }
        ));
    }

    #[test]
    fn serde_round_trips_as_string() {
        let id: CapabilityId = "agent:github-researcher".parse().unwrap();
        let encoded = serde_json::to_string(&id).unwrap();
        assert_eq!(encoded, "\"agent:github-researcher\"");
        let decoded: CapabilityId = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, id);
    }

    #[test]
    fn serde_reports_unknown_kind_for_unknown_prefix() {
        let err = serde_json::from_str::<CapabilityId>("\"widget:foo\"").unwrap_err();
        assert!(err.to_string().contains("widget"));
    }
}
