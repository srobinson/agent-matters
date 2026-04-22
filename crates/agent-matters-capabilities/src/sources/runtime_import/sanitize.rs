use std::path::Path;

use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

pub(super) fn id_segment(raw: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;
    for ch in raw.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            output.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            output.push('-');
            last_was_dash = true;
        }
    }

    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "imported".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn should_skip_file(path: &Path) -> Option<&'static str> {
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    match file_name.as_str() {
        "auth.json" | ".credentials.json" | "credentials.json" | "oauth.json" => {
            Some("credential file is not copied into catalog content")
        }
        _ => None,
    }
}

pub(super) fn secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "credential",
        "api_key",
        "apikey",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

pub(super) fn sanitize_toml_value(value: &mut TomlValue, env_keys: &mut Vec<String>) -> bool {
    match value {
        TomlValue::Table(table) => {
            let mut changed = false;
            for (key, value) in table.iter_mut() {
                if key == "env" {
                    if let TomlValue::Table(env) = value {
                        for (name, value) in env.iter_mut() {
                            env_keys.push(name.clone());
                            *value = TomlValue::String("required".to_string());
                        }
                        changed = !env.is_empty();
                    }
                } else if secret_key(key) {
                    *value = TomlValue::String("<redacted>".to_string());
                    changed = true;
                } else {
                    changed |= sanitize_toml_value(value, env_keys);
                }
            }
            changed
        }
        TomlValue::Array(values) => {
            let mut changed = false;
            for value in values {
                changed |= sanitize_toml_value(value, env_keys);
            }
            changed
        }
        _ => false,
    }
}

pub(super) fn sanitize_json_value(value: &mut JsonValue, env_keys: &mut Vec<String>) -> bool {
    match value {
        JsonValue::Object(map) => {
            let mut changed = false;
            for (key, value) in map.iter_mut() {
                if key == "env" {
                    if let JsonValue::Object(env) = value {
                        for (name, value) in env.iter_mut() {
                            env_keys.push(name.clone());
                            *value = JsonValue::String("required".to_string());
                        }
                        changed = !env.is_empty();
                    }
                } else if secret_key(key) {
                    *value = JsonValue::String("<redacted>".to_string());
                    changed = true;
                } else {
                    changed |= sanitize_json_value(value, env_keys);
                }
            }
            changed
        }
        JsonValue::Array(values) => {
            let mut changed = false;
            for value in values {
                changed |= sanitize_json_value(value, env_keys);
            }
            changed
        }
        _ => false,
    }
}
