//! Path constants and the pure tilde expansion helper used by config
//! loaders. These are filesystem path *conventions*, not filesystem I/O.

use std::path::{Path, PathBuf};

/// Directory under the authored repo holding shared defaults.
pub const REPO_DEFAULTS_DIR_NAME: &str = "defaults";

/// Repo default file carrying per-runtime default settings.
pub const RUNTIMES_FILE_NAME: &str = "runtimes.toml";

/// Repo default file carrying project markers.
pub const MARKERS_FILE_NAME: &str = "markers.toml";

/// Repo default file carrying external source trust policy.
pub const SOURCES_FILE_NAME: &str = "sources.toml";

/// User machine config directory (under the user's home directory).
pub const USER_CONFIG_DIR_NAME: &str = ".agent-matters";

/// User machine config file inside [`USER_CONFIG_DIR_NAME`].
pub const USER_CONFIG_FILE_NAME: &str = "config.toml";

/// Expand a leading `~` or `~/` in the path against a supplied home
/// directory. Pure: the caller is responsible for discovering `$HOME`.
///
/// Behavior:
/// * Exactly `"~"` expands to `home`.
/// * `"~/rest"` expands to `home.join("rest")`.
/// * Any path not starting with `~` is returned unchanged.
/// * A path starting with `~user/...` is returned unchanged; MVP does not
///   resolve user specific home directories.
pub fn expand_tilde(path: impl AsRef<Path>, home: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let home = home.as_ref();

    let Some(first) = path.components().next() else {
        return path.to_path_buf();
    };
    let std::path::Component::Normal(first_os) = first else {
        return path.to_path_buf();
    };
    let Some(first_str) = first_os.to_str() else {
        return path.to_path_buf();
    };

    if first_str == "~" {
        let mut out = home.to_path_buf();
        out.extend(path.components().skip(1));
        return out;
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_only_returns_home() {
        let home = Path::new("/users/alice");
        assert_eq!(expand_tilde("~", home), PathBuf::from("/users/alice"));
    }

    #[test]
    fn expand_tilde_slash_joins_remainder() {
        let home = Path::new("/users/alice");
        assert_eq!(
            expand_tilde("~/.agent-matters/config.toml", home),
            PathBuf::from("/users/alice/.agent-matters/config.toml")
        );
    }

    #[test]
    fn absolute_path_passes_through_unchanged() {
        let home = Path::new("/users/alice");
        assert_eq!(
            expand_tilde("/etc/agent-matters/config.toml", home),
            PathBuf::from("/etc/agent-matters/config.toml")
        );
    }

    #[test]
    fn relative_non_tilde_path_passes_through_unchanged() {
        let home = Path::new("/users/alice");
        assert_eq!(
            expand_tilde("defaults/runtimes.toml", home),
            PathBuf::from("defaults/runtimes.toml")
        );
    }

    #[test]
    fn user_specific_tilde_is_not_expanded() {
        let home = Path::new("/users/alice");
        assert_eq!(
            expand_tilde("~bob/config.toml", home),
            PathBuf::from("~bob/config.toml")
        );
    }
}
