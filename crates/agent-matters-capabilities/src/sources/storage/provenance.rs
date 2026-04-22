use agent_matters_core::domain::Provenance;

use super::super::contract::SourceImportResult;
use super::SourceImportStorageError;

pub(super) fn validate_provenance(
    import: &SourceImportResult,
) -> Result<(), SourceImportStorageError> {
    let Some(origin) = import.manifest.origin.as_ref() else {
        return Err(SourceImportStorageError::MissingProvenance {
            capability: import.manifest.id.to_string(),
        });
    };
    let Some((origin_source, origin_locator)) = provenance_source_locator(origin) else {
        return Err(SourceImportStorageError::MissingProvenance {
            capability: import.manifest.id.to_string(),
        });
    };
    if origin_source != import.source || origin_locator != import.locator {
        return Err(SourceImportStorageError::ProvenanceMismatch {
            origin_source: origin_source.to_string(),
            origin_locator: origin_locator.to_string(),
            result_source: import.source.clone(),
            locator: import.locator.clone(),
        });
    }
    Ok(())
}

fn provenance_source_locator(origin: &Provenance) -> Option<(&str, &str)> {
    match origin {
        Provenance::External {
            source, locator, ..
        }
        | Provenance::Derived {
            source, locator, ..
        } => Some((source.as_str(), locator.as_str())),
        _ => None,
    }
}
