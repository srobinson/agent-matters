//! `agent-matters profiles` subcommand surface.
//!
//! Implemented handlers delegate to `agent-matters-capabilities`; remaining
//! verbs return `not yet implemented` until their issue lands.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::profiles::{
    CompileProfileBuildRequest, ListProfilesRequest, ShowProfileRequest, UseProfileRequest,
    compile_profile_build, list_profiles, show_profile, use_profile,
};
use clap::Subcommand;

use super::profile_render::{
    render_profile_compile, render_profile_list, render_profile_show, render_profile_use,
};
use super::{Runtime, default_catalog_paths, emit_diagnostics, generated_help, help_text};

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
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::PROFILES_COMPILE_JSON_HELP)]
        json: bool,
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
        runtime: Option<Runtime>,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::PROFILES_USE_JSON_HELP)]
        json: bool,
    },
}

/// Dispatch a parsed `profiles` subcommand to its handler.
pub fn dispatch(cmd: ProfilesCmd) -> anyhow::Result<i32> {
    match cmd {
        ProfilesCmd::List { json } => run_list(json),
        ProfilesCmd::Show { profile, json } => run_show(&profile, json),
        ProfilesCmd::Compile {
            profile,
            runtime,
            json,
        } => run_compile(&profile, runtime, json),
        ProfilesCmd::Use {
            profile,
            path,
            runtime,
            json,
        } => run_use(&profile, path.as_deref(), runtime, json),
    }
}

fn run_list(json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = list_profiles(ListProfilesRequest {
        repo_root,
        user_state_dir,
    })?;
    emit_diagnostics(&result.diagnostics);

    if json {
        println!("{}", serde_json::to_string_pretty(&result.profiles)?);
    } else if result.profiles.is_empty() {
        println!("No profiles found.");
    } else {
        render_profile_list(result.profiles);
    }

    Ok(0)
}

fn run_show(profile: &str, json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = show_profile(ShowProfileRequest {
        repo_root,
        user_state_dir,
        profile: profile.to_string(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        emit_diagnostics(&result.diagnostics);
        render_profile_show(&result);
    }

    Ok(if result.has_error_diagnostics() { 1 } else { 0 })
}

fn run_compile(profile: &str, runtime: Runtime, json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = compile_profile_build(CompileProfileBuildRequest {
        repo_root,
        user_state_dir,
        native_home_dir: native_home_dir(),
        profile: profile.to_string(),
        runtime: Some(runtime.as_str().to_string()),
        env: current_env_presence(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        emit_diagnostics(&result.diagnostics);
        render_profile_compile(&result);
    }

    Ok(if result.has_error_diagnostics() { 1 } else { 0 })
}

fn run_use(
    profile: &str,
    path: Option<&Path>,
    runtime: Option<Runtime>,
    json: bool,
) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = use_profile(UseProfileRequest {
        repo_root,
        user_state_dir,
        native_home_dir: native_home_dir(),
        profile: profile.to_string(),
        runtime: runtime.map(|runtime| runtime.as_str().to_string()),
        workspace_path: path.map(Path::to_path_buf),
        env: current_env_presence(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        emit_diagnostics(&result.diagnostics);
        render_profile_use(&result);
    }

    Ok(if result.has_error_diagnostics() { 1 } else { 0 })
}

fn current_env_presence() -> BTreeMap<String, String> {
    env_presence_from(std::env::vars_os())
}

fn native_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn env_presence_from(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> BTreeMap<String, String> {
    vars.into_iter()
        .filter_map(|(name, _value)| name.into_string().ok().map(|name| (name, String::new())))
        .collect()
}
