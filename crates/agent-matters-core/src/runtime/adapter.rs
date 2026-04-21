//! Runtime neutral adapter contract data.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Version of the shared adapter contract data shape.
pub const RUNTIME_ADAPTER_CONTRACT_VERSION: u16 = 1;

/// Public identity for a runtime adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeAdapterMetadata {
    pub id: String,
    pub version: String,
}

/// One deterministic file to materialize inside a generated runtime home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHomeFile {
    pub relative_path: PathBuf,
    pub contents: Vec<u8>,
}

impl RuntimeHomeFile {
    pub fn text(relative_path: impl AsRef<Path>, contents: impl Into<String>) -> Self {
        Self {
            relative_path: relative_path.as_ref().to_path_buf(),
            contents: contents.into().into_bytes(),
        }
    }
}

/// A credential symlink the adapter is allowed to create at launch material time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CredentialSymlinkAllowlistEntry {
    pub source_name: String,
    pub target_path: PathBuf,
}

impl CredentialSymlinkAllowlistEntry {
    pub fn new(source_name: impl Into<String>, target_path: impl AsRef<Path>) -> Self {
        Self {
            source_name: source_name.into(),
            target_path: target_path.as_ref().to_path_buf(),
        }
    }
}

/// Runtime neutral manual launch instructions for human and JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeLaunchInstructions {
    pub env: BTreeMap<String, String>,
    pub args: Vec<String>,
    pub command: String,
}
