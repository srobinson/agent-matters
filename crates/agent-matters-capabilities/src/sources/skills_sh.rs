//! Concrete `skills.sh` source adapter backed by `npx skills`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_matters_core::domain::{CapabilityId, CapabilityKind, Provenance, RuntimeId};
use agent_matters_core::manifest::{
    CapabilityFilesManifest, CapabilityManifest, CapabilityRuntimeManifest,
    CapabilityRuntimesManifest,
};
use serde_json::json;

use super::skills_sh_parse::{
    SkillsShLocator, parse_find_output, parse_locator, skill_metadata, strip_ansi,
};
use super::{
    SourceAdapter, SourceAdapterError, SourceImportFile, SourceImportRequest, SourceImportResult,
    SourceSearchRequest, SourceSearchResult,
};

const SOURCE_ID: &str = "skills.sh";
const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.code == 0
    }

    fn message(&self) -> String {
        let stdout = strip_ansi(&self.stdout);
        let stderr = strip_ansi(&self.stderr);
        [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|part| !part.is_empty())
            .unwrap_or("command failed without output")
            .to_string()
    }
}

pub trait SkillsShCommand {
    fn find(&self, query: &str) -> io::Result<CommandOutput>;
    fn add(&self, package: &str, skill: &str, workdir: &Path) -> io::Result<CommandOutput>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NpxSkillsCommand;

impl SkillsShCommand for NpxSkillsCommand {
    fn find(&self, query: &str) -> io::Result<CommandOutput> {
        let mut command = skills_command();
        command.arg("find").arg(query);
        run_command(command)
    }

    fn add(&self, package: &str, skill: &str, workdir: &Path) -> io::Result<CommandOutput> {
        let mut command = skills_command();
        command.current_dir(workdir).args([
            "add", package, "--skill", skill, "--agent", "codex", "--copy", "-y",
        ]);
        run_command(command)
    }
}

#[derive(Debug, Clone)]
pub struct SkillsShAdapter<C = NpxSkillsCommand> {
    command: C,
}

impl Default for SkillsShAdapter<NpxSkillsCommand> {
    fn default() -> Self {
        Self {
            command: NpxSkillsCommand,
        }
    }
}

impl<C: SkillsShCommand> SkillsShAdapter<C> {
    pub fn with_command(command: C) -> Self {
        Self { command }
    }
}

impl<C: SkillsShCommand> SourceAdapter for SkillsShAdapter<C> {
    fn source_id(&self) -> &str {
        SOURCE_ID
    }

    fn search(
        &self,
        request: SourceSearchRequest,
    ) -> Result<SourceSearchResult, SourceAdapterError> {
        let output = self.command.find(&request.query).map_err(|source| {
            SourceAdapterError::search_failed(
                SOURCE_ID,
                format!("failed to run `npx skills find`: {source}"),
            )
        })?;
        if !output.success() {
            return Err(SourceAdapterError::search_failed(
                SOURCE_ID,
                format!(
                    "`npx skills find` exited {}: {}",
                    output.code,
                    output.message()
                ),
            ));
        }

        Ok(SourceSearchResult {
            source: SOURCE_ID.to_string(),
            query: request.query.clone(),
            entries: parse_find_output(SOURCE_ID, &request.query, &output.stdout)?,
            diagnostics: Vec::new(),
        })
    }

    fn import_capability(
        &self,
        request: SourceImportRequest,
    ) -> Result<SourceImportResult, SourceAdapterError> {
        let locator = parse_locator(&request.locator).map_err(|message| {
            SourceAdapterError::invalid_record(SOURCE_ID, request.locator, message)
        })?;
        let workspace = TempWorkspace::new().map_err(|source| {
            SourceAdapterError::import_failed(
                SOURCE_ID,
                locator.display.clone(),
                format!("failed to create temporary import workspace: {source}"),
            )
        })?;
        let output = self
            .command
            .add(&locator.package, &locator.skill, workspace.path())
            .map_err(|source| {
                SourceAdapterError::import_failed(
                    SOURCE_ID,
                    locator.display.clone(),
                    format!("failed to run `npx skills add`: {source}"),
                )
            })?;
        if !output.success() {
            return Err(SourceAdapterError::import_failed(
                SOURCE_ID,
                locator.display,
                format!(
                    "`npx skills add` exited {}: {}",
                    output.code,
                    output.message()
                ),
            ));
        }

        import_from_workspace(locator, workspace.path(), output)
    }
}

fn import_from_workspace(
    locator: SkillsShLocator,
    workspace: &Path,
    output: CommandOutput,
) -> Result<SourceImportResult, SourceAdapterError> {
    let skill_dir = workspace.join(".agents/skills").join(&locator.skill);
    let files = read_skill_files(&skill_dir).map_err(|source| {
        invalid_import(&locator, format!("failed to read skill files: {source}"))
    })?;
    let Some(skill_file) = files
        .iter()
        .find(|file| file.relative_path == Path::new(SKILL_FILE))
    else {
        return Err(invalid_import(
            &locator,
            "imported skill did not include SKILL.md",
        ));
    };

    let metadata = skill_metadata(&skill_file.contents, &locator.skill);
    let manifest = capability_manifest(&locator, &metadata)?;
    let record = raw_record(&locator, &metadata, &files, &output)?;
    let mut vendor_files = vec![
        SourceImportFile {
            relative_path: PathBuf::from("record.json"),
            contents: record,
        },
        SourceImportFile {
            relative_path: PathBuf::from("install-output.txt"),
            contents: strip_ansi(&output.stdout),
        },
    ];
    vendor_files.extend(files.iter().map(|file| SourceImportFile {
        relative_path: PathBuf::from("files").join(&file.relative_path),
        contents: file.contents.clone(),
    }));

    Ok(SourceImportResult {
        source: SOURCE_ID.to_string(),
        locator: locator.display,
        manifest,
        catalog_files: files,
        vendor_files,
        diagnostics: Vec::new(),
    })
}

fn capability_manifest(
    locator: &SkillsShLocator,
    metadata: &super::skills_sh_parse::SkillMetadata,
) -> Result<CapabilityManifest, SourceAdapterError> {
    let mut files = BTreeMap::new();
    files.insert("source".to_string(), SKILL_FILE.to_string());

    let mut runtimes = BTreeMap::new();
    for runtime in ["codex", "claude"] {
        runtimes.insert(
            RuntimeId::new(runtime).expect("static runtime id is valid"),
            CapabilityRuntimeManifest { supported: true },
        );
    }

    Ok(CapabilityManifest {
        id: CapabilityId::new(CapabilityKind::Skill, locator.skill.clone()).map_err(|source| {
            invalid_import(
                locator,
                format!("skill name cannot become a capability id: {source}"),
            )
        })?,
        kind: CapabilityKind::Skill,
        summary: metadata.summary.clone(),
        files: CapabilityFilesManifest { entries: files },
        runtimes: CapabilityRuntimesManifest { entries: runtimes },
        requires: None,
        origin: Some(Provenance::external(
            SOURCE_ID,
            &locator.display,
            metadata.version.clone(),
        )),
    })
}

fn raw_record(
    locator: &SkillsShLocator,
    metadata: &super::skills_sh_parse::SkillMetadata,
    files: &[SourceImportFile],
    output: &CommandOutput,
) -> Result<String, SourceAdapterError> {
    serde_json::to_string_pretty(&json!({
        "source": SOURCE_ID,
        "locator": locator.display,
        "package": locator.package,
        "skill": locator.skill,
        "summary": metadata.summary,
        "version": metadata.version,
        "command": {
            "program": "npx",
            "args": ["--yes", "skills", "add", locator.package, "--skill", locator.skill, "--agent", "codex", "--copy", "-y"],
            "exit_code": output.code
        },
        "stdout": strip_ansi(&output.stdout),
        "stderr": strip_ansi(&output.stderr),
        "files": files
            .iter()
            .map(|file| file.relative_path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
    }))
    .map_err(|source| invalid_import(locator, format!("failed to encode raw record: {source}")))
}

fn read_skill_files(skill_dir: &Path) -> io::Result<Vec<SourceImportFile>> {
    let mut files = Vec::new();
    collect_files(skill_dir, skill_dir, &mut files)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn collect_files(base: &Path, current: &Path, files: &mut Vec<SourceImportFile>) -> io::Result<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(base, &path, files)?;
        } else if file_type.is_file() {
            files.push(SourceImportFile {
                relative_path: path.strip_prefix(base).unwrap_or(&path).to_path_buf(),
                contents: fs::read_to_string(&path)?,
            });
        }
    }
    Ok(())
}

fn invalid_import(locator: &SkillsShLocator, message: impl Into<String>) -> SourceAdapterError {
    SourceAdapterError::invalid_record(SOURCE_ID, locator.display.clone(), message)
}

fn skills_command() -> Command {
    match std::env::var_os("AGENT_MATTERS_SKILLS_BIN") {
        Some(path) => Command::new(path),
        None => {
            let mut command = Command::new("npx");
            command.args(["--yes", "skills"]);
            command
        }
    }
}

fn run_command(mut command: Command) -> io::Result<CommandOutput> {
    let output = command.output()?;
    Ok(CommandOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new() -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for attempt in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "agent-matters-skills-sh-{}-{stamp}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => return Err(source),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate unique temporary import workspace",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
