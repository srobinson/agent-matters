//! `agent-matters sources` subcommand surface.
//!
//! The CLI stays a thin adapter over the source search and import use cases.

use std::path::Path;

use agent_matters_capabilities::sources::{
    ImportSourceRequest, SearchSourceRequest, import_source, search_source,
};
use clap::Subcommand;

use super::{default_catalog_paths, emit_diagnostics, generated_help, help_text};

/// Verbs for `agent-matters sources`.
#[derive(Debug, Subcommand)]
pub enum SourcesCmd {
    /// Search a registered source for entries matching a query.
    #[command(
        long_about = generated_help::SOURCES_SEARCH_ABOUT,
        after_help = help_text::SOURCES_SEARCH_AFTER_HELP
    )]
    Search {
        /// Source identifier (for example `skills.sh`).
        #[arg(help = generated_help::SOURCES_SEARCH_SOURCE_HELP)]
        source: String,
        /// Free form search query.
        #[arg(help = generated_help::SOURCES_SEARCH_QUERY_HELP)]
        query: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::SOURCES_SEARCH_JSON_HELP)]
        json: bool,
    },
    /// Import a capability from a source by locator.
    #[command(
        long_about = generated_help::SOURCES_IMPORT_ABOUT,
        after_help = help_text::SOURCES_IMPORT_AFTER_HELP
    )]
    Import {
        /// Source specific locator.
        #[arg(help = generated_help::SOURCES_IMPORT_LOCATOR_HELP)]
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

fn run_search(source: &str, query: &str, json: bool) -> anyhow::Result<i32> {
    match search_source(SearchSourceRequest {
        source: source.to_string(),
        query: query.to_string(),
    }) {
        Ok(result) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                emit_diagnostics(&result.diagnostics);
                render_search(&result);
            }
            Ok(0)
        }
        Err(err) => {
            let diagnostic = err.to_diagnostic();
            if json {
                println!("{}", serde_json::to_string_pretty(&vec![diagnostic])?);
            } else {
                emit_diagnostics(&[diagnostic]);
            }
            Ok(1)
        }
    }
}

fn run_import(locator: &str) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    match import_source(ImportSourceRequest {
        repo_root: repo_root.clone(),
        user_state_dir,
        locator: locator.to_string(),
        replace_existing: false,
    }) {
        Ok(result) => {
            emit_diagnostics(&result.diagnostics);
            println!("Imported {}", result.capability_id);
            println!("source\t{}:{}", result.source, result.locator);
            println!(
                "manifest\t{}",
                display_path(&repo_root, &result.manifest_path)
            );
            println!("vendor\t{}", display_path(&repo_root, &result.vendor_dir));
            println!("index\t{}", result.index_path.display());
            Ok(0)
        }
        Err(err) => {
            emit_diagnostics(&[err.to_diagnostic()]);
            Ok(1)
        }
    }
}

fn render_search(result: &agent_matters_capabilities::sources::SourceSearchResult) {
    if result.entries.is_empty() {
        println!(
            "No results found for `{}` in `{}`.",
            result.query, result.source
        );
        return;
    }

    for entry in &result.entries {
        println!(
            "{}\t{}\t{}",
            entry.locator,
            entry.version.as_deref().unwrap_or("-"),
            entry.summary.as_deref().unwrap_or("-")
        );
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
