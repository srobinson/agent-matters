use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn native_home_with_codex_auth(root: &Path) -> PathBuf {
    let home = root.join("native-home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(home.join(".codex/auth.json"), br#"{"token":"test"}"#).unwrap();
    home
}

pub(crate) fn native_home_with_claude_auth(root: &Path) -> PathBuf {
    let home = root.join("native-home");
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::write(
        home.join(".claude/.credentials.json"),
        br#"{"claude":"test"}"#,
    )
    .unwrap();
    home
}
