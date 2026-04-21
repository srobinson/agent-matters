//! `agent-matters profiles` subcommand surface.
//!
//! Handler stubs return `not yet implemented` until the relevant use case
//! ships. Concrete behaviors land in ALP-1942 (list, show), ALP-1937
//! (compile), and ALP-1943 (use).

use std::path::PathBuf;

use clap::Subcommand;

use super::{Runtime, generated_help, help_text};

/// Verbs for `agent-matters profiles`.
#[derive(Debug, Subcommand)]
pub enum ProfilesCmd {
    /// List profiles discovered in the catalog.
    #[command(
        long_about = generated_help::PROFILES_LIST_ABOUT,
        after_help = help_text::PROFILES_LIST_AFTER_HELP
    )]
    List {
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::PROFILES_LIST_JSON_HELP)]
        json: bool,
    },
    /// Show a single profile and its resolved inventory.
    #[command(
        long_about = generated_help::PROFILES_SHOW_ABOUT,
        after_help = help_text::PROFILES_SHOW_AFTER_HELP
    )]
    Show {
        /// Profile identifier.
        #[arg(help = generated_help::PROFILES_SHOW_PROFILE_HELP)]
        profile: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::PROFILES_SHOW_JSON_HELP)]
        json: bool,
    },
    /// Compile a runtime home for the given profile without activating it.
    #[command(
        long_about = generated_help::PROFILES_COMPILE_ABOUT,
        after_help = help_text::PROFILES_COMPILE_AFTER_HELP
    )]
    Compile {
        /// Profile identifier.
        #[arg(help = generated_help::PROFILES_COMPILE_PROFILE_HELP)]
        profile: String,
        /// Target runtime.
        #[arg(long, value_enum, help = generated_help::PROFILES_COMPILE_RUNTIME_HELP)]
        runtime: Runtime,
    },
    /// Activate the given profile for the target runtime.
    #[command(
        long_about = generated_help::PROFILES_USE_ABOUT,
        after_help = help_text::PROFILES_USE_AFTER_HELP
    )]
    Use {
        /// Profile identifier.
        #[arg(help = generated_help::PROFILES_USE_PROFILE_HELP)]
        profile: String,
        /// Optional path to activate the profile in. Defaults to the current
        /// working directory when omitted.
        #[arg(help = generated_help::PROFILES_USE_PATH_HELP)]
        path: Option<PathBuf>,
        /// Target runtime.
        #[arg(long, value_enum, help = generated_help::PROFILES_USE_RUNTIME_HELP)]
        runtime: Runtime,
    },
}

/// Dispatch a parsed `profiles` subcommand to its handler.
pub fn dispatch(cmd: ProfilesCmd) -> anyhow::Result<i32> {
    match cmd {
        ProfilesCmd::List { json } => run_list(json),
        ProfilesCmd::Show { profile, json } => run_show(&profile, json),
        ProfilesCmd::Compile { profile, runtime } => run_compile(&profile, runtime),
        ProfilesCmd::Use {
            profile,
            path,
            runtime,
        } => run_use(&profile, path.as_deref(), runtime),
    }
}

fn run_list(_json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("profiles list: not yet implemented (ALP-1942)")
}

fn run_show(_profile: &str, _json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("profiles show: not yet implemented (ALP-1942)")
}

fn run_compile(_profile: &str, _runtime: Runtime) -> anyhow::Result<i32> {
    anyhow::bail!("profiles compile: not yet implemented (ALP-1937)")
}

fn run_use(
    _profile: &str,
    _path: Option<&std::path::Path>,
    _runtime: Runtime,
) -> anyhow::Result<i32> {
    anyhow::bail!("profiles use: not yet implemented (ALP-1943)")
}
