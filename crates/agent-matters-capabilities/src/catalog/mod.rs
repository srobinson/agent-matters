//! Catalog discovery use cases.

mod discovery;
mod index;
mod overlays;

pub use discovery::{
    CapabilityDiscoverySource, CatalogDiscovery, DiscoveredCapabilityManifest, DiscoveredManifest,
    DiscoveredProfileManifest, discover_catalog,
};
pub use index::{
    CatalogIndexError, CatalogIndexStatus, LoadCatalogIndexRequest, LoadCatalogIndexResult,
    build_catalog_index, catalog_index_path, load_or_refresh_catalog_index,
};
