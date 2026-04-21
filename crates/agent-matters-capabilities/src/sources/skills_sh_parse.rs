//! Parsing helpers for the `skills.sh` command line output.

use serde_json::json;

use super::{SourceAdapterError, SourceSearchEntry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SkillsShLocator {
    pub package: String,
    pub skill: String,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SkillMetadata {
    pub summary: String,
    pub version: Option<String>,
}

pub(super) fn parse_locator(input: &str) -> Result<SkillsShLocator, String> {
    let mut value = input.trim();
    if let Some(rest) = value.strip_prefix("skills.sh:") {
        value = rest;
    }
    if let Some(rest) = value.strip_prefix("//") {
        value = rest;
    }

    if let Some(path) = value
        .strip_prefix("https://skills.sh/")
        .or_else(|| value.strip_prefix("http://skills.sh/"))
    {
        let parts = path
            .trim_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() >= 3 {
            return locator_from_parts(parts[0], parts[1], parts[2]);
        }
    }

    let Some((package, skill)) = value.rsplit_once('@') else {
        return Err("expected locator format `owner/repo@skill-name`".to_string());
    };
    let mut package_parts = package.split('/');
    let Some(owner) = package_parts.next() else {
        return Err("expected locator package owner".to_string());
    };
    let Some(repo) = package_parts.next() else {
        return Err("expected locator package repository".to_string());
    };
    if package_parts.next().is_some() {
        return Err("expected locator package to be `owner/repo`".to_string());
    }
    locator_from_parts(owner, repo, skill)
}

pub(super) fn parse_find_output(
    source_id: &str,
    query: &str,
    output: &str,
) -> Result<Vec<SourceSearchEntry>, SourceAdapterError> {
    let clean = strip_ansi(output);
    let lines = clean
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let mut entries = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let Some(locator) = candidate_locator(line) else {
            continue;
        };
        let url = lines.get(index + 1).and_then(|line| url_from_line(line));
        let installs = installs_from_line(line, &locator.display);
        entries.push(SourceSearchEntry {
            locator: locator.display.clone(),
            summary: installs
                .as_ref()
                .map(|value| format!("{value} installs"))
                .or_else(|| url.clone()),
            version: None,
            raw: json!({
                "locator": locator.display,
                "package": locator.package,
                "skill": locator.skill,
                "url": url,
                "installs": installs,
                "line": line,
            }),
        });
    }

    if entries.is_empty() && !clean.contains("No skills found") {
        return Err(SourceAdapterError::invalid_record(
            source_id,
            query,
            "expected `owner/repo@skill-name` result lines from `npx skills find`",
        ));
    }

    Ok(entries)
}

pub(super) fn skill_metadata(markdown: &str, fallback_skill: &str) -> SkillMetadata {
    let (description, version) = frontmatter_values(markdown);
    let summary = description
        .or_else(|| first_heading(markdown))
        .unwrap_or_else(|| format!("Imported skills.sh skill `{fallback_skill}`."));

    SkillMetadata { summary, version }
}

pub(super) fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            consume_escape(&mut chars);
        } else if ch == '\r' {
            output.push('\n');
        } else {
            output.push(ch);
        }
    }
    output
}

fn locator_from_parts(owner: &str, repo: &str, skill: &str) -> Result<SkillsShLocator, String> {
    for (label, value) in [("owner", owner), ("repository", repo), ("skill", skill)] {
        if value.is_empty() || value.chars().any(|ch| ch.is_whitespace() || ch == '/') {
            return Err(format!("locator {label} `{value}` is invalid"));
        }
    }

    Ok(SkillsShLocator {
        package: format!("{owner}/{repo}"),
        skill: skill.to_string(),
        display: format!("{owner}/{repo}@{skill}"),
    })
}

fn candidate_locator(line: &str) -> Option<SkillsShLocator> {
    parse_locator(line.split_whitespace().next()?).ok()
}

fn url_from_line(line: &str) -> Option<String> {
    let start = line.find("https://skills.sh/")?;
    Some(line[start..].trim().to_string())
}

fn installs_from_line(line: &str, locator: &str) -> Option<String> {
    line.strip_prefix(locator)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.strip_suffix(" installs").unwrap_or(value).to_string())
}

fn frontmatter_values(markdown: &str) -> (Option<String>, Option<String>) {
    let Some(rest) = markdown.strip_prefix("---") else {
        return (None, None);
    };
    let Some((frontmatter, _body)) = rest.split_once("\n---") else {
        return (None, None);
    };

    let mut description = None;
    let mut version = None;
    for line in frontmatter.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("description:") {
            description = clean_scalar(value);
        } else if let Some(value) = line.strip_prefix("version:") {
            version = clean_scalar(value);
        }
    }
    (description, version)
}

fn first_heading(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn clean_scalar(value: &str) -> Option<String> {
    let clean = value.trim().trim_matches('"').trim_matches('\'').trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

fn consume_escape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.next() {
        Some('[') => {
            for ch in chars.by_ref() {
                if ch.is_ascii_alphabetic() || ch == '~' {
                    break;
                }
            }
        }
        Some(']') => {
            for ch in chars.by_ref() {
                if ch == '\u{7}' {
                    break;
                }
            }
        }
        Some(_) | None => {}
    }
}
