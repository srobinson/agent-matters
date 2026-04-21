//! Catalog discovery use cases.

mod discovery;

pub use discovery::{
    CatalogDiscovery, DiscoveredCapabilityManifest, DiscoveredManifest, DiscoveredProfileManifest,
    discover_catalog,
};
