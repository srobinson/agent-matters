//! Provenance records for authored, imported, derived, and generated content.

use serde::{Deserialize, Serialize};

/// Origin metadata for catalog entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Provenance {
    /// Local authored content. This is optional in manifests because absence
    /// also means local authored content in the MVP.
    Local,
    /// Content imported from an external source.
    External {
        source: String,
        locator: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    /// Content derived from another local or external capability.
    Derived {
        source: String,
        locator: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    /// Content produced by a generator inside agent-matters.
    Generated {
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locator: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
}

impl Provenance {
    pub fn local() -> Self {
        Self::Local
    }

    pub fn external(
        source: impl Into<String>,
        locator: impl Into<String>,
        version: Option<String>,
    ) -> Self {
        Self::External {
            source: source.into(),
            locator: locator.into(),
            version,
        }
    }

    /// Imported or derived capabilities require explicit provenance and a
    /// vendor record in later catalog checks.
    pub const fn requires_vendor_record(&self) -> bool {
        matches!(self, Self::External { .. } | Self::Derived { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_provenance_serializes_as_tag_only() {
        let encoded = serde_json::to_string(&Provenance::local()).unwrap();

        assert_eq!(encoded, r#"{"type":"local"}"#);
    }

    #[test]
    fn external_provenance_parses_manifest_shape() {
        let src = r#"
            type = "external"
            source = "skills.sh"
            locator = "linear"
            version = "1.4.2"
        "#;

        let provenance: Provenance = toml::from_str(src).unwrap();

        assert_eq!(
            provenance,
            Provenance::external("skills.sh", "linear", Some("1.4.2".to_string()))
        );
        assert!(provenance.requires_vendor_record());
    }

    #[test]
    fn provenance_variants_have_strict_tags() {
        let derived: Provenance = toml::from_str(
            r#"
                type = "derived"
                source = "skill:playwright"
                locator = "skill:playwright/headless"
            "#,
        )
        .unwrap();
        let generated: Provenance = toml::from_str(
            r#"
                type = "generated"
                source = "agent-matters"
            "#,
        )
        .unwrap();
        let err = toml::from_str::<Provenance>(r#"type = "imported""#).unwrap_err();

        assert!(derived.requires_vendor_record());
        assert!(!generated.requires_vendor_record());
        assert!(err.to_string().contains("imported"));
    }
}
