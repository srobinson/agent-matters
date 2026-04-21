//! Runtime config composition for resolved profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{CapabilityIndexRecord, ProfileIndexRecord};
use agent_matters_core::config::{
    REPO_DEFAULTS_DIR_NAME, RUNTIMES_FILE_NAME, RuntimeSettings, USER_CONFIG_FILE_NAME,
};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::Serialize;

use crate::config::{
    ConfigError, load_runtime_defaults, load_runtime_settings, load_user_config_from_state_dir,
};

use super::{runtime_adapter_ids, runtime_adapters};

const RUNTIME_SETTING_KIND: &str = "runtime-setting";
const SETTINGS_FILE_KEY: &str = "settings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedRuntimeConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeResolution {
    pub runtime_configs: Vec<ResolvedRuntimeConfig>,
    pub selected_runtime: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn resolve_runtime_configs(
    repo_root: &Path,
    user_state_dir: &Path,
    profile: &ProfileIndexRecord,
    effective_capabilities: &[CapabilityIndexRecord],
    profile_manifest_path: &Path,
) -> RuntimeResolution {
    let known = known_runtime_ids();
    let mut diagnostics = Vec::new();
    let mut settings = adapter_defaults();

    match load_runtime_defaults(repo_root) {
        Ok(defaults) => apply_settings_layer(
            &mut settings,
            defaults.runtimes,
            &known,
            &mut diagnostics,
            repo_root
                .join(REPO_DEFAULTS_DIR_NAME)
                .join(RUNTIMES_FILE_NAME),
        ),
        Err(error) => diagnostics.push(config_load_error("repo runtime defaults", &error)),
    }

    let user_config_path = user_state_dir.join(USER_CONFIG_FILE_NAME);
    let user_config = match load_user_config_from_state_dir(user_state_dir) {
        Ok(config) => config,
        Err(error) => {
            diagnostics.push(config_load_error("user config", &error));
            Default::default()
        }
    };
    apply_settings_layer(
        &mut settings,
        user_config.runtimes,
        &known,
        &mut diagnostics,
        user_config_path.clone(),
    );

    for capability in effective_capabilities {
        if capability.kind != RUNTIME_SETTING_KIND {
            continue;
        }
        if let Some(path) = runtime_setting_path(repo_root, capability) {
            match load_runtime_settings(&path) {
                Ok(defaults) => apply_settings_layer(
                    &mut settings,
                    defaults.runtimes,
                    &known,
                    &mut diagnostics,
                    path,
                ),
                Err(error) => {
                    diagnostics.push(config_load_error("runtime-setting capability", &error))
                }
            }
        }
    }

    let runtime_configs = enabled_runtime_configs(
        profile,
        &settings,
        &known,
        &mut diagnostics,
        profile_manifest_path,
    );
    let selected_runtime = select_default_runtime(
        profile,
        user_config.default_runtime.as_deref(),
        &runtime_configs,
        &mut diagnostics,
        profile_manifest_path,
        &user_config_path,
    );

    RuntimeResolution {
        runtime_configs,
        selected_runtime,
        diagnostics,
    }
}

fn adapter_defaults() -> BTreeMap<String, RuntimeSettings> {
    runtime_adapters()
        .into_iter()
        .map(|adapter| (adapter.id().to_string(), adapter.default_settings()))
        .collect()
}

fn apply_settings_layer(
    target: &mut BTreeMap<String, RuntimeSettings>,
    layer: BTreeMap<String, RuntimeSettings>,
    known: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    path: PathBuf,
) {
    for (runtime, settings) in layer {
        if !known.contains(&runtime) {
            diagnostics.push(unknown_runtime_config(&runtime, &path));
            continue;
        }

        if let Some(model) = settings.model {
            target.entry(runtime).or_default().model = Some(model);
        }
    }
}

fn runtime_setting_path(repo_root: &Path, capability: &CapabilityIndexRecord) -> Option<PathBuf> {
    capability
        .files
        .get(SETTINGS_FILE_KEY)
        .map(|path| repo_root.join(&capability.source_path).join(path))
}

fn enabled_runtime_configs(
    profile: &ProfileIndexRecord,
    settings: &BTreeMap<String, RuntimeSettings>,
    known: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    profile_manifest_path: &Path,
) -> Vec<ResolvedRuntimeConfig> {
    let mut configs = Vec::new();

    for (runtime, compatibility) in &profile.runtimes {
        if !compatibility.supported {
            continue;
        }
        if !known.contains(runtime) {
            diagnostics.push(unknown_profile_runtime(runtime, profile_manifest_path));
            continue;
        }

        let mut resolved = settings.get(runtime).cloned().unwrap_or_default();
        if let Some(model) = &compatibility.model {
            resolved.model = Some(model.clone());
        }

        configs.push(ResolvedRuntimeConfig {
            id: runtime.clone(),
            model: resolved.model,
        });
    }

    configs
}

fn select_default_runtime(
    profile: &ProfileIndexRecord,
    user_default: Option<&str>,
    configs: &[ResolvedRuntimeConfig],
    diagnostics: &mut Vec<Diagnostic>,
    profile_manifest_path: &Path,
    user_config_path: &Path,
) -> Option<String> {
    let available = configs
        .iter()
        .map(|config| config.id.as_str())
        .collect::<Vec<_>>();

    if let Some(default) = &profile.default_runtime {
        return if available.contains(&default.as_str()) {
            Some(default.clone())
        } else {
            diagnostics.push(default_runtime_unavailable(
                default,
                "profile",
                profile_manifest_path,
                "runtimes.default",
            ));
            None
        };
    }

    if let Some(default) = user_default
        && available.contains(&default)
    {
        return Some(default.to_string());
    }

    match available.as_slice() {
        [] => {
            diagnostics.push(no_enabled_runtime(profile_manifest_path));
            None
        }
        [runtime] => Some((*runtime).to_string()),
        _ => {
            if let Some(default) = user_default {
                diagnostics.push(default_runtime_unavailable(
                    default,
                    "user",
                    user_config_path,
                    "default_runtime",
                ));
                return None;
            }
            diagnostics.push(ambiguous_runtime(&available, profile_manifest_path));
            None
        }
    }
}

fn known_runtime_ids() -> BTreeSet<String> {
    runtime_adapter_ids()
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn known_runtime_list() -> String {
    runtime_adapter_ids().join(", ")
}

fn config_load_error(label: &str, error: &ConfigError) -> Diagnostic {
    match error {
        ConfigError::Io { path, source } => Diagnostic::new(
            DiagnosticSeverity::Error,
            "profile.runtime-config-read-failed",
            format!("failed to read {label} `{}`: {source}", path.display()),
        )
        .with_location(DiagnosticLocation::manifest_path(path)),
        ConfigError::Parse { path, source } => Diagnostic::new(
            DiagnosticSeverity::Error,
            "profile.runtime-config-parse-failed",
            format!("failed to parse {label} `{}`: {source}", path.display()),
        )
        .with_location(DiagnosticLocation::manifest_path(path)),
    }
}

fn unknown_runtime_config(runtime: &str, path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.runtime-config.unknown",
        format!(
            "runtime config references unknown runtime `{runtime}`; known runtimes: {}",
            known_runtime_list()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        path,
        format!("runtimes.{runtime}"),
    ))
    .with_recovery_hint("use a registered runtime adapter id")
}

fn unknown_profile_runtime(runtime: &str, profile_manifest_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.runtime.unknown",
        format!(
            "profile enables unknown runtime `{runtime}`; known runtimes: {}",
            known_runtime_list()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path,
        format!("runtimes.{runtime}"),
    ))
    .with_recovery_hint("use a registered runtime adapter id")
}

fn default_runtime_unavailable(
    runtime: &str,
    source: &str,
    path: &Path,
    field: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.runtime.default-unavailable",
        format!("{source} default runtime `{runtime}` is not enabled for this profile"),
    )
    .with_location(DiagnosticLocation::manifest_field(path, field))
    .with_recovery_hint("enable the default runtime in the profile or choose an enabled runtime")
}

fn no_enabled_runtime(profile_manifest_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.runtime.none-enabled",
        "profile does not enable any known runtime",
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path,
        "runtimes",
    ))
    .with_recovery_hint(
        "add `[runtimes.codex] enabled = true` or `[runtimes.claude] enabled = true`",
    )
}

fn ambiguous_runtime(available: &[&str], profile_manifest_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.runtime.ambiguous-default",
        format!(
            "profile enables multiple runtimes but no default resolves; available runtimes: {}",
            available.join(", ")
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path,
        "runtimes.default",
    ))
    .with_recovery_hint(
        "set `[runtimes] default` in the profile or `default_runtime` in user config",
    )
}
