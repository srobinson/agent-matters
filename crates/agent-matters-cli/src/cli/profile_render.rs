//! Human rendering helpers for `agent-matters profiles`.

use std::collections::BTreeMap;

use agent_matters_capabilities::profiles::ShowProfileResult;
use agent_matters_core::catalog::{ProfileIndexRecord, RuntimeCompatibilitySummary};

pub(crate) fn render_profile_list(profiles: Vec<ProfileIndexRecord>) {
    for profile in profiles {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            profile.id,
            profile.kind,
            render_enabled_runtime_names(&profile.runtimes),
            profile.scope.enforcement,
            profile.summary
        );
    }
}

pub(crate) fn render_profile_show(result: &ShowProfileResult) {
    let Some(record) = &result.record else {
        return;
    };

    println!("Profile: {}", record.id);
    println!("kind: {}", record.kind);
    println!("summary: {}", record.summary);
    println!("source path: {}", record.source_path);

    println!();
    render_scope(record);
    render_declared_runtimes(record);
    render_runtime_configs(result);
    render_capabilities(result);
    render_instruction_fragments(result);
}

fn render_scope(record: &ProfileIndexRecord) {
    println!("scope:");
    println!("enforcement: {}", record.scope.enforcement);
    render_string_list("paths", &record.scope.paths);
    render_string_list("github repos", &record.scope.github_repos);
}

fn render_declared_runtimes(record: &ProfileIndexRecord) {
    println!();
    println!("declared runtimes:");
    if record.runtimes.is_empty() {
        println!("none");
        return;
    }

    for (runtime, support) in &record.runtimes {
        let state = if support.supported {
            "enabled"
        } else {
            "disabled"
        };
        let default = if record.default_runtime.as_deref() == Some(runtime.as_str()) {
            " default"
        } else {
            ""
        };
        if let Some(model) = &support.model {
            println!("{runtime}\t{state}{default}\tmodel={model}");
        } else {
            println!("{runtime}\t{state}{default}");
        }
    }
}

fn render_runtime_configs(result: &ShowProfileResult) {
    println!();
    println!("resolved runtime config:");
    if result.runtime_configs.is_empty() {
        println!("none");
        return;
    }

    for config in &result.runtime_configs {
        let selected = if result.selected_runtime.as_deref() == Some(config.id.as_str()) {
            " selected"
        } else {
            ""
        };
        if let Some(model) = &config.model {
            println!("{}\tmodel={}{}", config.id, model, selected);
        } else {
            println!("{}{}", config.id, selected);
        }
    }
}

fn render_capabilities(result: &ShowProfileResult) {
    println!();
    println!("resolved capabilities:");
    if result.effective_capabilities.is_empty() {
        println!("none");
        return;
    }

    for capability in &result.effective_capabilities {
        println!(
            "{}\t{}\t{}\t{}",
            capability.id,
            capability.kind,
            render_enabled_runtime_names(&capability.runtimes),
            capability.source_path
        );
    }
}

fn render_instruction_fragments(result: &ShowProfileResult) {
    println!();
    println!("ordered instructions:");
    if result.instruction_fragments.is_empty() {
        println!("none");
        return;
    }

    for fragment in &result.instruction_fragments {
        println!(
            "{}\t{}\t{}\t{}",
            fragment.id,
            fragment.kind,
            fragment.source_path,
            render_file_map(&fragment.files)
        );
    }
}

fn render_string_list(label: &str, values: &[String]) {
    if values.is_empty() {
        println!("{label}: none");
    } else {
        println!("{label}: {}", values.join(","));
    }
}

fn render_file_map(files: &BTreeMap<String, String>) -> String {
    if files.is_empty() {
        return "files=none".to_string();
    }

    let rendered = files
        .iter()
        .map(|(name, path)| format!("{name}:{path}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("files={rendered}")
}

fn render_enabled_runtime_names(
    runtimes: &BTreeMap<String, RuntimeCompatibilitySummary>,
) -> String {
    let enabled = runtimes
        .iter()
        .filter_map(|(runtime, support)| support.supported.then_some(runtime.as_str()))
        .collect::<Vec<_>>();

    if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join(",")
    }
}
