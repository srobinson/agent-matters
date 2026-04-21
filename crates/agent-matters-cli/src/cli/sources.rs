//! `agent-matters sources` subcommand surface.
//!
//! Handler stubs return `not yet implemented` until the relevant use case
//! ships. Concrete behaviors land in ALP-1946 (skills.sh search and import)
//! and ALP-1950 (trust policy).

use clap::Subcommand;

/// Verbs for `agent-matters sources`.
#[derive(Debug, Subcommand)]
pub enum SourcesCmd {
    /// Search a registered source for entries matching a query.
    Search {
        /// Source identifier (for example `skills.sh`).
        source: String,
        /// Free form search query.
        query: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long)]
        json: bool,
    },
    /// Import a capability from a source by locator.
    Import {
        /// Source specific locator.
        locator: String,
    },
}

/// Dispatch a parsed `sources` subcommand to its handler.
pub fn dispatch(cmd: SourcesCmd) -> anyhow::Result<i32> {
    match cmd {
        SourcesCmd::Search {
            source,
            query,
            json,
        } => run_search(&source, &query, json),
        SourcesCmd::Import { locator } => run_import(&locator),
    }
}

fn run_search(_source: &str, _query: &str, _json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("sources search: not yet implemented (ALP-1946)")
}

fn run_import(_locator: &str) -> anyhow::Result<i32> {
    anyhow::bail!("sources import: not yet implemented (ALP-1946)")
}
