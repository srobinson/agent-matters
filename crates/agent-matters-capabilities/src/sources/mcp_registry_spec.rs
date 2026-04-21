//! Design shape for a future MCP Registry source adapter.
//!
//! This module is intentionally spec only. Keep [`SOURCE_ID`] out of
//! `search_source` and `import_source` until live registry support is
//! implemented and tested.
//!
//! The registry publishes `server.json` records from
//! `https://registry.modelcontextprotocol.io/`. A list or version response is
//! an envelope containing the upstream `server` document plus registry owned
//! metadata under [`REGISTRY_OFFICIAL_META_KEY`].
//!
//! ## Upstream concepts to preserve
//!
//! A future adapter should keep the whole response envelope as raw vendor
//! data. The normalized capability is smaller than the upstream record, and
//! losing registry fields would make future sync and doctor checks weaker.
//!
//! * `server.json`: `$schema`, `name`, `title`, `description`, `version`,
//!   `repository`, `websiteUrl`, `icons`, `packages`, and `remotes`.
//! * Packages: `registryType`, `identifier`, `version`, `runtimeHint`,
//!   `runtimeArguments`, `packageArguments`, `environmentVariables`,
//!   `fileSha256`, and a package transport.
//! * Transports: local `stdio` plus remote `sse` and `streamable-http`.
//! * Inputs: required flags, repeated flags, positional arguments, defaults,
//!   value hints, secret fields, environment variables, headers, and template
//!   variables.
//! * Registry metadata: status, status change time, publish time, update time,
//!   latest version marker, and version history.
//!
//! ## Capability transformation
//!
//! Search entries should use [`SOURCE_ID`] as the source and
//! `<server.name>@<server.version>` as the source specific locator. The raw
//! locator keeps the registry name untouched so version endpoints can be
//! addressed without lossy reverse mapping.
//!
//! Imported capabilities should be `kind = "mcp"` with an id body derived from
//! `server.name`:
//!
//! * Preserve `/` namespace separators.
//! * Lowercase ASCII letters.
//! * Convert dots, underscores, spaces, and other invalid segment characters
//!   to `-`.
//! * Collapse repeated `-` and trim segment boundary `-`.
//! * Reject the import if two registry names normalize to the same capability
//!   id. A later alias workflow can solve collisions explicitly.
//!
//! For a single launch target, use `mcp:<normalized-server-name>`. If one
//! registry server has distinct launch variants, import each variant as a
//! separate capability. Variant ids append a normalized selector segment such
//! as `mcp:<normalized-server-name>/npm-<package-leaf>` or
//! `mcp:<normalized-server-name>/remote-<transport>`.
//!
//! The normalized manifest should set external provenance to
//! `source = "mcp-registry"`, `locator = "<server.name>@<server.version>"`,
//! and `version = "<server.version>"`. The summary should prefer
//! `server.title`, then `server.description`.
//!
//! ## Normalized files
//!
//! The catalog side should contain a `manifest.toml` plus a runtime neutral
//! `server.toml` describing the selected local package or remote endpoint.
//! Required package environment variables and required secret header inputs
//! should become capability env requirements where the current manifest model
//! can represent them. Inputs that cannot be represented must remain in
//! `server.toml` and produce diagnostics until runtime adapters know how to
//! render them.
//!
//! The vendor side should contain the raw registry envelope and extracted
//! `server.json` so doctor can later validate vendor presence, status, and
//! version freshness without consulting hidden state.

pub const SOURCE_ID: &str = "mcp-registry";
pub const REGISTRY_BASE_URL: &str = "https://registry.modelcontextprotocol.io";
pub const SERVER_SCHEMA_URL: &str =
    "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json";
pub const REGISTRY_OFFICIAL_META_KEY: &str = "io.modelcontextprotocol.registry/official";
pub const CAPABILITY_KIND: &str = "mcp";
pub const CATALOG_SERVER_FILE: &str = "server.toml";
pub const VENDOR_RECORD_FILE: &str = "record.json";
pub const VENDOR_SERVER_FILE: &str = "server.json";

pub const MCP_REGISTRY_ADAPTER_SPEC: McpRegistryAdapterSpec = McpRegistryAdapterSpec {
    source_id: SOURCE_ID,
    registry_base_url: REGISTRY_BASE_URL,
    server_schema_url: SERVER_SCHEMA_URL,
    registry_meta_key: REGISTRY_OFFICIAL_META_KEY,
    search_locator: "<server.name>@<server.version>",
    import_locator: "<server.name>@<server.version>",
    capability_id: "mcp:<normalized-server-name>",
    variant_capability_id: "mcp:<normalized-server-name>/<normalized-variant>",
    provenance_source: SOURCE_ID,
    provenance_locator: "<server.name>@<server.version>",
    provenance_version: "<server.version>",
    catalog_files: &[CATALOG_SERVER_FILE],
    vendor_files: &[VENDOR_RECORD_FILE, VENDOR_SERVER_FILE],
    unsupported_until_adapter_exists: &[
        "live registry search",
        "live registry import",
        "registry doctor reachability checks",
        "CLI advertised MCP Registry source support",
    ],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpRegistryAdapterSpec {
    pub source_id: &'static str,
    pub registry_base_url: &'static str,
    pub server_schema_url: &'static str,
    pub registry_meta_key: &'static str,
    pub search_locator: &'static str,
    pub import_locator: &'static str,
    pub capability_id: &'static str,
    pub variant_capability_id: &'static str,
    pub provenance_source: &'static str,
    pub provenance_locator: &'static str,
    pub provenance_version: &'static str,
    pub catalog_files: &'static [&'static str],
    pub vendor_files: &'static [&'static str],
    pub unsupported_until_adapter_exists: &'static [&'static str],
}
