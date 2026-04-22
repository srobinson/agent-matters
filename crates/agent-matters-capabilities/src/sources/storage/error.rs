use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SourceImportStorageError {
    #[error("import manifest for `{capability}` must include external or derived provenance")]
    MissingProvenance { capability: String },
    #[error(
        "import provenance `{origin_source}:{origin_locator}` does not match source result `{result_source}:{locator}`"
    )]
    ProvenanceMismatch {
        origin_source: String,
        origin_locator: String,
        result_source: String,
        locator: String,
    },
    #[error("import for `{source_id}:{locator}` must include at least one raw vendor file")]
    MissingVendorRecord { source_id: String, locator: String },
    #[error("refusing to replace existing import path `{path}`")]
    AlreadyExists { path: PathBuf },
    #[error(
        "source import is partially published: `{existing}` exists but `{missing}` is missing and staged contents do not match"
    )]
    PartialPublishConflict { existing: PathBuf, missing: PathBuf },
    #[error("relative import path `{path}` must stay inside its target directory")]
    InvalidRelativePath { path: PathBuf },
    #[error("source import file path `{path}` is reserved for generated metadata")]
    ReservedPath { path: PathBuf },
    #[error("failed to serialize manifest for `{capability}`: {source}")]
    SerializeManifest {
        capability: String,
        #[source]
        source: toml::ser::Error,
    },
    #[error("failed to create directory `{path}`: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write file `{path}`: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to publish source import path `{from}` to `{to}`: {source}")]
    PublishPath {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to inspect source import path `{path}`: {source}")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to remove source import path `{path}`: {source}")]
    RemovePath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
