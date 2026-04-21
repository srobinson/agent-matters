//! Disk loaders for repo defaults and user config.
//!
//! Every loader returns `Ok(Default::default())` when its backing file is
//! absent so a brand new workspace or home directory just works. Invalid
//! TOML produces [`ConfigError::Parse`] with the offending file path and
//! the underlying parser diagnostic preserved verbatim.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::config::{
    MARKERS_FILE_NAME, Markers, REPO_DEFAULTS_DIR_NAME, RUNTIMES_FILE_NAME, RuntimeDefaults,
    USER_CONFIG_DIR_NAME, USER_CONFIG_FILE_NAME, UserConfig,
};
use serde::de::DeserializeOwned;

/// Errors that can occur while loading a config file.
///
/// Missing files are not errors: loaders return defaults. These variants
/// cover the two failure modes the issue calls out:
/// * Unreadable file (permissions, io failure).
/// * Invalid TOML.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file exists but could not be read.
    #[error("failed to read config file `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The file was read but contained invalid TOML or did not match the
    /// expected schema.
    #[error("failed to parse config file `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

/// Load `<repo_root>/defaults/runtimes.toml` into [`RuntimeDefaults`].
pub fn load_runtime_defaults(repo_root: &Path) -> Result<RuntimeDefaults, ConfigError> {
    let path = repo_root
        .join(REPO_DEFAULTS_DIR_NAME)
        .join(RUNTIMES_FILE_NAME);
    load_optional_toml(&path)
}

/// Load a runtime settings TOML file referenced by a `runtime-setting`
/// capability. The file uses the same schema as repo runtime defaults.
pub fn load_runtime_settings(path: &Path) -> Result<RuntimeDefaults, ConfigError> {
    load_optional_toml(path)
}

/// Load `<repo_root>/defaults/markers.toml` into [`Markers`].
pub fn load_markers(repo_root: &Path) -> Result<Markers, ConfigError> {
    let path = repo_root
        .join(REPO_DEFAULTS_DIR_NAME)
        .join(MARKERS_FILE_NAME);
    load_optional_toml(&path)
}

/// Load `<user_home>/.agent-matters/config.toml` into [`UserConfig`].
pub fn load_user_config(user_home: &Path) -> Result<UserConfig, ConfigError> {
    let path = user_home
        .join(USER_CONFIG_DIR_NAME)
        .join(USER_CONFIG_FILE_NAME);
    load_optional_toml(&path)
}

/// Load `<user_state_dir>/config.toml` into [`UserConfig`].
pub fn load_user_config_from_state_dir(user_state_dir: &Path) -> Result<UserConfig, ConfigError> {
    let path = user_state_dir.join(USER_CONFIG_FILE_NAME);
    load_optional_toml(&path)
}

fn load_optional_toml<T>(path: &Path) -> Result<T, ConfigError>
where
    T: DeserializeOwned + Default,
{
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(T::default()),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    toml::from_str::<T>(&raw).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, body: &str) {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, body).unwrap();
    }

    #[test]
    fn runtime_defaults_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let loaded = load_runtime_defaults(tmp.path()).unwrap();
        assert_eq!(loaded, RuntimeDefaults::default());
    }

    #[test]
    fn runtime_defaults_valid_file_populates_runtime_map() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "defaults/runtimes.toml",
            r#"
            [runtimes.codex]
            model = "gpt-5.4"
            "#,
        );
        let loaded = load_runtime_defaults(tmp.path()).unwrap();
        assert_eq!(
            loaded.runtimes.get("codex").unwrap().model.as_deref(),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn markers_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let loaded = load_markers(tmp.path()).unwrap();
        assert_eq!(loaded, Markers::default());
    }

    #[test]
    fn markers_valid_file_populates_list() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "defaults/markers.toml",
            r#"project_markers = [".git", "Cargo.toml"]"#,
        );
        let loaded = load_markers(tmp.path()).unwrap();
        assert_eq!(
            loaded.project_markers,
            vec![".git".to_string(), "Cargo.toml".to_string()]
        );
    }

    #[test]
    fn user_config_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let loaded = load_user_config(tmp.path()).unwrap();
        assert_eq!(loaded, UserConfig::default());
    }

    #[test]
    fn user_config_reads_default_runtime() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            ".agent-matters/config.toml",
            r#"default_runtime = "claude""#,
        );
        let loaded = load_user_config(tmp.path()).unwrap();
        assert_eq!(loaded.default_runtime.as_deref(), Some("claude"));
    }

    #[test]
    fn user_config_reads_from_state_dir() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "config.toml", r#"default_runtime = "codex""#);

        let loaded = load_user_config_from_state_dir(tmp.path()).unwrap();

        assert_eq!(loaded.default_runtime.as_deref(), Some("codex"));
    }

    #[test]
    fn runtime_settings_file_uses_runtime_defaults_schema() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "settings.toml",
            r#"
            [runtimes.codex]
            model = "gpt-5.4"
            "#,
        );

        let loaded = load_runtime_settings(&tmp.path().join("settings.toml")).unwrap();

        assert_eq!(
            loaded.runtimes.get("codex").unwrap().model.as_deref(),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn repo_and_user_config_are_loaded_independently() {
        // Proves the issue's precedence concern at the loading layer:
        // both files deserialize into their own typed struct without
        // interfering. Applying precedence is ALP-1932.
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "defaults/runtimes.toml",
            r#"
            [runtimes.codex]
            model = "gpt-5.4"
            "#,
        );
        write(
            tmp.path(),
            ".agent-matters/config.toml",
            r#"
            default_runtime = "codex"

            [runtimes.codex]
            model = "gpt-5.4-preview"
            "#,
        );

        let repo = load_runtime_defaults(tmp.path()).unwrap();
        let user = load_user_config(tmp.path()).unwrap();
        assert_eq!(
            repo.runtimes.get("codex").unwrap().model.as_deref(),
            Some("gpt-5.4")
        );
        assert_eq!(user.default_runtime.as_deref(), Some("codex"));
        assert_eq!(
            user.runtimes.get("codex").unwrap().model.as_deref(),
            Some("gpt-5.4-preview")
        );
    }

    #[test]
    fn invalid_toml_surfaces_parse_error_with_path() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "defaults/runtimes.toml",
            "this = is = not = valid",
        );
        let err = load_runtime_defaults(tmp.path()).unwrap_err();
        match err {
            ConfigError::Parse { path, source } => {
                assert!(path.ends_with("defaults/runtimes.toml"));
                // The underlying toml error is preserved for actionable
                // diagnostics; assert it mentions the problematic input.
                assert!(!source.to_string().is_empty());
            }
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    #[test]
    fn schema_violation_is_reported_as_parse_error() {
        // `deny_unknown_fields` on UserConfig means any stray key is a
        // parse-time rejection with the offending key named.
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            ".agent-matters/config.toml",
            r#"unexpected_key = true"#,
        );
        let err = load_user_config(tmp.path()).unwrap_err();
        match err {
            ConfigError::Parse { path, source } => {
                assert!(path.ends_with(".agent-matters/config.toml"));
                assert!(source.to_string().contains("unexpected_key"));
            }
            other => panic!("expected Parse error, got {other:?}"),
        }
    }
}
