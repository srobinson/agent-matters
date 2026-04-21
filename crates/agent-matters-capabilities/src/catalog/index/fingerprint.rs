use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::CATALOG_INDEX_SCHEMA_VERSION;

use super::super::{CapabilityDiscoverySource, CatalogDiscovery};
use super::paths::relative_path;
use super::types::CatalogIndexError;

pub(super) fn content_fingerprint(
    repo_root: &Path,
    discovery: &CatalogDiscovery,
) -> Result<String, CatalogIndexError> {
    let mut paths = BTreeSet::<PathBuf>::new();
    for entry in &discovery.capabilities {
        paths.insert(entry.manifest_path.clone());
        if let CapabilityDiscoverySource::Overlay {
            target_manifest_path,
            ..
        } = &entry.source
        {
            paths.insert(target_manifest_path.clone());
        }
    }
    for entry in &discovery.profiles {
        paths.insert(entry.manifest_path.clone());
    }

    let mut hasher = Fnv64::new();
    hasher.write_u16(CATALOG_INDEX_SCHEMA_VERSION);
    for path in paths {
        hasher.write(relative_path(repo_root, &path).as_bytes());
        hasher.write(&[0]);
        let bytes = fs::read(&path).map_err(|source| CatalogIndexError::Read {
            path: path.clone(),
            source,
        })?;
        hasher.write(&bytes);
        hasher.write(&[0xff]);
    }

    Ok(format!("fnv64:{:016x}", hasher.finish()))
}

struct Fnv64(u64);

impl Fnv64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write_u16(&mut self, value: u16) {
        self.write(&value.to_le_bytes());
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    const fn finish(&self) -> u64 {
        self.0
    }
}
