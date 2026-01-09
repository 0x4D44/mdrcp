use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use toml::Value;

const UPDATER_TEMP_NAME: &str = "mdrcp_updater.exe";

pub mod cli;

pub use cli::{
    parse_args, write_help, write_parse_error, write_version_banner, Command, ParseError,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SummaryFormat {
    #[default]
    Text,
    Json,
    JsonPretty,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildProfile {
    #[default]
    Release,
    Debug,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProjectType {
    #[default]
    Standard,
    Tauri,
}

impl ProjectType {
    fn label(self) -> &'static str {
        match self {
            ProjectType::Standard => "standard",
            ProjectType::Tauri => "Tauri",
        }
    }
}

impl BuildProfile {
    fn artifact_dir(self) -> &'static str {
        match self {
            BuildProfile::Release => "release",
            BuildProfile::Debug => "debug",
        }
    }

    fn label(self) -> &'static str {
        match self {
            BuildProfile::Release => "release",
            BuildProfile::Debug => "debug",
        }
    }

    fn cargo_hint(self, project_type: ProjectType) -> &'static str {
        match (self, project_type) {
            (BuildProfile::Release, ProjectType::Tauri) => "cargo tauri build",
            (BuildProfile::Debug, ProjectType::Tauri) => "cargo tauri build --debug",
            (BuildProfile::Release, ProjectType::Standard) => "cargo build --release",
            (BuildProfile::Debug, ProjectType::Standard) => "cargo build",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOptions {
    pub target_override: Option<PathBuf>,
    pub quiet: bool,
    pub summary: SummaryFormat,
    pub profile: BuildProfile,
    pub project_type: Option<ProjectType>, // None = auto-detect
}

#[cfg(windows)]
pub fn exe_filename(base: &str) -> String {
    format!("{}.exe", base)
}

#[cfg(not(windows))]
pub fn exe_filename(base: &str) -> String {
    base.to_string()
}

#[cfg(windows)]
const HINT_DEFAULT: &str = r"c:\apps";

#[cfg(not(windows))]
const HINT_DEFAULT: &str = "~/.local/bin";

const TARGET_OVERRIDE_ENV: &str = "MD_TARGET_DIR";

fn format_deployment_summary(count: usize, target_dir: &Path, override_used: bool) -> String {
    let base = format!(
        "{} {} {} {}",
        "Deployed".bold().green(),
        count.to_string().bold().green(),
        "executable(s) to".dimmed(),
        target_dir.display().to_string().bold().bright_white()
    );
    if override_used {
        format!("{} {}", base, "[--target]".bright_cyan())
    } else {
        base
    }
}

#[derive(Default)]
struct OverrideNote {
    lines: Vec<String>,
    warnings: Vec<String>,
}

fn build_override_note(raw: &Path, resolved: &Path, default_target: Option<&Path>) -> OverrideNote {
    let mut note = OverrideNote::default();
    note.lines.push(format!(
        "{} {}",
        "Note:".bold().cyan(),
        "Destination provided via --target.".dimmed()
    ));
    note.lines.push(format!(
        "{} {}",
        "  Passed:".bold().magenta(),
        raw.display()
    ));
    note.lines.push(format!(
        "{} {}",
        "  Resolved:".bold().magenta(),
        resolved.display()
    ));
    note.lines.push(format!(
        "{} {}",
        "  Relative paths:".bold().magenta(),
        "Resolved against the project directory.".dimmed()
    ));
    if let Some(default) = default_target {
        if default == resolved {
            let message = "Resolved target matches default destination; override may be redundant.";
            note.lines.push(format!(
                "{} {}",
                "Warning:".bold().yellow(),
                message.dimmed()
            ));
            note.warnings.push(message.to_string());
        }
    }
    note
}

#[derive(Serialize)]
struct DeploymentSummary {
    status: &'static str,
    copied_count: usize,
    target_dir: String,
    override_used: bool,
    copied_binaries: Vec<String>,
    failed_binaries: Vec<FailedCopy>,
    warnings: Vec<String>,
}

#[derive(Clone, Serialize)]
struct FailedCopy {
    binary: String,
    error: String,
}

pub fn do_main_with_options(cwd: &Path, options: &RunOptions) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut ctx = CliContext::new(&mut stdout, &mut stderr);
    match run_with_options(cwd, options, &mut ctx) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(ctx.stderr, "{} {}", "Error:".bold().bright_red(), e);
            let _ = writeln!(ctx.stderr);
            let _ = writeln!(
                ctx.stderr,
                "{} {}",
                "Usage:".bold().yellow(),
                "mdrcp [OPTIONS]".bold()
            );
            let hint = match default_target_dir() {
                Ok(p) => p.display().to_string(),
                Err(_) => HINT_DEFAULT.to_string(),
            };
            let _ = writeln!(
                ctx.stderr,
                "{} {} {}",
                "Hint:".bold().cyan(),
                format!(
                    "Run this tool in a Rust project directory to copy {} executables to",
                    options.profile.label()
                )
                .dimmed(),
                hint.bold().bright_white()
            );
            let _ = writeln!(
                ctx.stderr,
                "{} {}",
                "More info:".bold().cyan(),
                "mdrcp --help".bold()
            );
            let _ = writeln!(
                ctx.stderr,
                "{} {}",
                "Docs:".bold().cyan(),
                "See README.md troubleshooting section".dimmed()
            );
            1
        }
    }
}

pub fn do_main(cwd: &Path) -> i32 {
    do_main_with_options(cwd, &RunOptions::default())
}
/// Extract candidate binary names from a manifest `Value`.
/// Prefers `[[bin]].name`; falls back to `package.name` if no explicit bins.
fn manifest_bin_names(manifest: &Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Some(bins) = manifest.get("bin").and_then(|v| v.as_array()) {
        for b in bins {
            if let Some(name) = b.get("name").and_then(|n| n.as_str()) {
                names.push(name.to_string());
                continue;
            }
            if let Some(path) = b.get("path").and_then(|p| p.as_str()) {
                if let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    if let Some(name) = manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
    {
        if !names.iter().any(|existing| existing == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// Find all built executables from workspace members or single package.
/// Returns the base names of executables (without `.exe`).
/// `rust_base_dir` is the directory containing Cargo.toml and target/.
fn find_built_executables(
    rust_base_dir: &Path,
    cargo_data: &Value,
    profile: BuildProfile,
    extra_names: &[String],
) -> Result<Vec<String>> {
    let profile_dir = rust_base_dir.join("target").join(profile.artifact_dir());
    let mut candidate_names: HashSet<String> = HashSet::new();

    // Root package (if any)
    for name in manifest_bin_names(cargo_data) {
        candidate_names.insert(name);
    }

    // Add extra names (e.g., from tauri.conf.json productName)
    for name in extra_names {
        candidate_names.insert(name.clone());
    }

    // Workspace members (if any)
    if let Some(members) = cargo_data
        .get("workspace")
        .and_then(|ws| ws.get("members"))
        .and_then(|m| m.as_array())
    {
        for member in members {
            let Some(member_path) = member.as_str() else {
                continue;
            };
            let member_manifest_path = rust_base_dir.join(member_path).join("Cargo.toml");
            let Ok(contents) = fs::read_to_string(&member_manifest_path) else {
                continue;
            };
            let Ok(member_data) = toml::from_str::<Value>(&contents) else {
                continue;
            };
            for name in manifest_bin_names(&member_data) {
                candidate_names.insert(name);
            }
        }
    }

    if candidate_names.is_empty() {
        anyhow::bail!("No packages or bins found in Cargo.toml");
    }

    // Filter to only candidates with existing executables for the selected profile
    let mut built_executables = Vec::new();
    for base in candidate_names {
        let exe_name = exe_filename(&base);
        if profile_dir.join(&exe_name).exists() {
            built_executables.push(base);
        }
    }
    Ok(built_executables)
}

/// Determine the default deployment target directory per-OS.
fn target_dir_override_from_env() -> Result<Option<PathBuf>> {
    if let Some(raw) = std::env::var_os(TARGET_OVERRIDE_ENV) {
        if raw.is_empty() {
            anyhow::bail!(
                "{} is set but empty; provide an absolute path",
                TARGET_OVERRIDE_ENV
            );
        }
        return Ok(Some(PathBuf::from(raw)));
    }
    Ok(None)
}

#[cfg(windows)]
fn default_target_dir() -> Result<PathBuf> {
    if let Some(custom) = target_dir_override_from_env()? {
        return Ok(custom);
    }
    Ok(PathBuf::from(r"c:\apps"))
}

#[cfg(not(windows))]
fn default_target_dir() -> Result<PathBuf> {
    if let Some(custom) = target_dir_override_from_env()? {
        return Ok(custom);
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow::anyhow!("HOME is not set; cannot determine ~/.local/bin"))?;
    Ok(Path::new(&home).join(".local").join("bin"))
}

/// Check if the target path is our own running executable
fn is_self_update_target(target_path: &Path, current_exe_override: Option<&Path>) -> bool {
    let current_exe = if let Some(p) = current_exe_override {
        p.to_path_buf()
    } else {
        match std::env::current_exe() {
            Ok(exe) => exe,
            Err(_) => return false,
        }
    };
    let current_exe_canonical = current_exe.canonicalize().unwrap_or(current_exe);
    let target_canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.to_path_buf());
    current_exe_canonical == target_canonical
}

/// Result of attempting a self-update
enum SelfUpdateResult {
    /// Not a self-update scenario
    NotApplicable,
    /// Self-update spawned successfully - caller should exit
    Spawned,
    /// Self-update failed to spawn
    Failed(String),
}

/// Check if we're trying to overwrite our own executable, and if so,
/// spawn a temp copy to perform the update.
fn try_self_update(
    source_path: &Path,
    target_path: &Path,
    current_exe_override: Option<&Path>,
) -> SelfUpdateResult {
    // Get canonical path of current executable
    let current_exe = if let Some(p) = current_exe_override {
        p.to_path_buf()
    } else {
        match std::env::current_exe() {
            Ok(exe) => exe,
            Err(_) => return SelfUpdateResult::NotApplicable,
        }
    };

    // Canonicalize paths for comparison
    let current_exe_canonical = current_exe.canonicalize().unwrap_or(current_exe);
    let target_canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.to_path_buf());

    // Check if we're trying to overwrite ourselves
    if current_exe_canonical != target_canonical {
        return SelfUpdateResult::NotApplicable;
    }

    // We're updating ourselves - copy current exe to temp and spawn it
    let temp_dir = std::env::temp_dir();
    let updater_path = temp_dir.join(UPDATER_TEMP_NAME);

    // Copy current executable to temp location
    if let Err(e) = fs::copy(&current_exe_canonical, &updater_path) {
        return SelfUpdateResult::Failed(format!("Failed to copy self to temp: {}", e));
    }

    // Spawn the temp copy with --finish-update
    match ProcessCommand::new(&updater_path)
        .arg("--finish-update")
        .arg(source_path)
        .arg(target_path)
        .spawn()
    {
        Ok(_) => SelfUpdateResult::Spawned,
        Err(e) => SelfUpdateResult::Failed(format!("Failed to spawn updater: {}", e)),
    }
}

/// Detect whether this is a Tauri project by checking for src-tauri/Cargo.toml
/// and tauri.conf.json (or .json5) in the src-tauri directory.
fn detect_project_type(project_dir: &Path) -> ProjectType {
    let tauri_cargo = project_dir.join("src-tauri").join("Cargo.toml");
    let tauri_conf = project_dir.join("src-tauri").join("tauri.conf.json");
    let tauri_conf5 = project_dir.join("src-tauri").join("tauri.conf.json5");

    if tauri_cargo.exists() && (tauri_conf.exists() || tauri_conf5.exists()) {
        ProjectType::Tauri
    } else {
        ProjectType::Standard
    }
}

/// Read the productName from tauri.conf.json or tauri.conf.json5.
/// Returns None if file doesn't exist or productName is not set.
fn read_tauri_product_name(project_dir: &Path) -> Option<String> {
    let conf_paths = [
        project_dir.join("src-tauri").join("tauri.conf.json"),
        project_dir.join("src-tauri").join("tauri.conf.json5"),
    ];

    for conf_path in &conf_paths {
        if let Ok(contents) = fs::read_to_string(conf_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                // productName can be at root level or under package
                if let Some(name) = json.get("productName").and_then(|v| v.as_str()) {
                    return Some(name.to_string());
                }
                if let Some(name) = json
                    .get("package")
                    .and_then(|p| p.get("productName"))
                    .and_then(|v| v.as_str())
                {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

/// Execution context for IO and environment mocking
pub struct CliContext<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    /// Mock for std::env::current_exe()
    pub current_exe: Option<PathBuf>,
}

impl<'a> CliContext<'a> {
    pub fn new(stdout: &'a mut dyn Write, stderr: &'a mut dyn Write) -> Self {
        Self {
            stdout,
            stderr,
            current_exe: None,
        }
    }
}

/// Main deployment function that handles both single packages and workspaces
pub fn run_with_options(
    project_dir: &Path,
    options: &RunOptions,
    ctx: &mut CliContext,
) -> Result<()> {
    // Determine project type: use explicit option or auto-detect
    let (project_type, auto_detected) = match options.project_type {
        Some(pt) => (pt, false),
        None => (detect_project_type(project_dir), true),
    };

    // For Tauri projects, the Rust project is in src-tauri/
    let rust_base_dir = if project_type == ProjectType::Tauri {
        project_dir.join("src-tauri")
    } else {
        project_dir.to_path_buf()
    };

    let cargo_path = rust_base_dir.join("Cargo.toml");
    if !cargo_path.exists() {
        if project_type == ProjectType::Tauri {
            anyhow::bail!(
                "No Cargo.toml found at {}. Is this a valid Tauri project?",
                cargo_path.display()
            );
        } else {
            anyhow::bail!("No Cargo.toml found. Please run this tool in a Rust project directory");
        }
    }

    let cargo_contents = fs::read_to_string(&cargo_path).context("Failed to read Cargo.toml")?;

    let cargo_data: Value =
        toml::from_str(&cargo_contents).context("Failed to parse Cargo.toml")?;

    // For Tauri projects, also check productName in tauri.conf.json
    let mut extra_names: Vec<String> = Vec::new();
    if project_type == ProjectType::Tauri {
        if let Some(product_name) = read_tauri_product_name(project_dir) {
            extra_names.push(product_name);
        }
    }

    let profile = options.profile;
    let built_executables =
        find_built_executables(&rust_base_dir, &cargo_data, profile, &extra_names)?;

    if built_executables.is_empty() {
        anyhow::bail!(
            "No built {} executables found. Have you run '{}'?",
            profile.label(),
            profile.cargo_hint(project_type)
        );
    }

    let override_raw = options.target_override.clone();
    let override_used = override_raw.is_some();
    let summary_format = options.summary;
    let emit_text = summary_format == SummaryFormat::Text && !options.quiet;
    let produce_json = matches!(
        summary_format,
        SummaryFormat::Json | SummaryFormat::JsonPretty
    );
    let mut default_target: Option<PathBuf> = None;
    let target_dir = match override_raw.as_ref() {
        Some(override_dir) => {
            if let Ok(default_dir) = default_target_dir() {
                default_target = Some(default_dir);
            }
            if override_dir.is_absolute() {
                override_dir.clone()
            } else {
                project_dir.join(override_dir)
            }
        }
        None => {
            let default_dir = default_target_dir()?;
            default_target = Some(default_dir.clone());
            default_dir
        }
    };
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).with_context(|| {
            format!("Failed to create target directory {}", target_dir.display())
        })?;
    } else if !target_dir.is_dir() {
        anyhow::bail!(
            "Target path {} exists but is not a directory",
            target_dir.display()
        );
    }

    // Print project type if not quiet
    if emit_text && project_type == ProjectType::Tauri {
        let mode_indicator = if auto_detected { "[auto]" } else { "[--tauri]" };
        writeln!(
            ctx.stdout,
            "{} {} {}",
            "Detected".bold().cyan(),
            project_type.label().bold().bright_white(),
            format!("project {}", mode_indicator).dimmed()
        )?;
    }

    let mut copied_count = 0;
    let mut copied_binaries: Vec<String> = Vec::new();
    let mut failed_binaries: Vec<FailedCopy> = Vec::new();
    // Track if we need to self-update (deferred until after all copies attempted)
    let mut pending_self_update: Option<(PathBuf, PathBuf)> = None;

    let source_dir = rust_base_dir.join("target").join(profile.artifact_dir());

    for package_name in built_executables {
        let exe_name = exe_filename(&package_name);

        let source_path = source_dir.join(&exe_name);
        let target_path = target_dir.join(&exe_name);

        // Check if this is a self-update scenario
        if is_self_update_target(&target_path, ctx.current_exe.as_deref()) {
            // Defer self-update until after all other copies
            if emit_text {
                writeln!(
                    ctx.stdout,
                    "{} {} {}",
                    "Deferred".bold().cyan(),
                    exe_name.bold().cyan(),
                    "(self-update will be attempted after other copies)".dimmed()
                )?;
            }
            pending_self_update = Some((source_path, target_path));
            continue;
        }

        match fs::copy(&source_path, &target_path) {
            Ok(_) => {
                if emit_text {
                    writeln!(
                        ctx.stdout,
                        "{} {} {}",
                        "Copied".bold().green(),
                        exe_name.bold().green(),
                        format!("-> {}", target_path.display()).dimmed()
                    )?;
                }
                copied_count += 1;
                copied_binaries.push(exe_name);
            }
            Err(e) => {
                // Normal copy failure
                let error_msg = format!(
                    "Failed to copy {} to {}: {}",
                    source_path.display(),
                    target_path.display(),
                    e
                );
                if emit_text {
                    writeln!(
                        ctx.stderr,
                        "{} {} {}",
                        "Failed".bold().bright_red(),
                        exe_name.bold().yellow(),
                        format!("-> {}: {}", target_path.display(), e).dimmed()
                    )?;
                }
                failed_binaries.push(FailedCopy {
                    binary: exe_name,
                    error: error_msg,
                });
            }
        }
    }

    // Handle pending self-update after all other copies
    if let Some((source_path, target_path)) = pending_self_update {
        let exe_name = target_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Only attempt self-update if there were no other failures
        if failed_binaries.is_empty() {
            match try_self_update(&source_path, &target_path, ctx.current_exe.as_deref()) {
                SelfUpdateResult::Spawned => {
                    if emit_text {
                        writeln!(
                            ctx.stdout,
                            "{} {}",
                            "Self-update:".bold().cyan(),
                            "Spawned updater process. Update will complete momentarily.".dimmed()
                        )?;
                    }
                    // Return Ok so the process exits cleanly
                    return Ok(());
                }
                SelfUpdateResult::Failed(msg) => {
                    let error_msg = format!(
                        "Failed to self-update {} to {}: {}",
                        source_path.display(),
                        target_path.display(),
                        msg
                    );
                    if emit_text {
                        writeln!(
                            ctx.stderr,
                            "{} {} {}",
                            "Failed".bold().bright_red(),
                            exe_name.bold().yellow(),
                            format!("-> {}: {}", target_path.display(), msg).dimmed()
                        )?;
                    }
                    failed_binaries.push(FailedCopy {
                        binary: exe_name,
                        error: error_msg,
                    });
                }
                SelfUpdateResult::NotApplicable => {
                    // Shouldn't happen since we already checked, but handle it
                    let error_msg = format!(
                        "Failed to copy {} to {}: file in use",
                        source_path.display(),
                        target_path.display()
                    );
                    failed_binaries.push(FailedCopy {
                        binary: exe_name,
                        error: error_msg,
                    });
                }
            }
        } else {
            // There were other failures - don't attempt self-update, just report it
            if emit_text {
                writeln!(
                    ctx.stderr,
                    "{} {}",
                    "Skipped self-update:".bold().yellow(),
                    "Fix other copy failures first, then re-run.".dimmed()
                )?;
            }
            failed_binaries.push(FailedCopy {
                binary: exe_name,
                error: "Self-update skipped due to other failures".to_string(),
            });
        }
    }

    if emit_text {
        writeln!(ctx.stdout)?;
        writeln!(
            ctx.stdout,
            "{}",
            format_deployment_summary(copied_count, &target_dir, override_used)
        )?;

        // Report failures if any
        if !failed_binaries.is_empty() {
            writeln!(ctx.stdout)?;
            writeln!(
                ctx.stderr,
                "{} {}",
                "Failed to copy".bold().bright_red(),
                format!("{} executable(s):", failed_binaries.len())
                    .bold()
                    .bright_red()
            )?;
            for failed in &failed_binaries {
                writeln!(
                    ctx.stderr,
                    "  {} {}",
                    "•".bright_red(),
                    failed.error.dimmed()
                )?;
            }
        }
    }

    let mut override_note: Option<OverrideNote> = None;
    if let Some(raw) = override_raw {
        let note = build_override_note(&raw, &target_dir, default_target.as_deref());
        if emit_text {
            for line in &note.lines {
                writeln!(ctx.stdout, "{}", line)?;
            }
        } else {
            for warning in &note.warnings {
                writeln!(ctx.stderr, "Warning: {}", warning)?;
            }
        }
        override_note = Some(note);
    }

    if produce_json {
        let warnings = override_note
            .as_ref()
            .map(|n| n.warnings.clone())
            .unwrap_or_default();
        let status = if failed_binaries.is_empty() {
            "ok"
        } else if copied_count > 0 {
            "partial"
        } else {
            "failed"
        };
        let summary = DeploymentSummary {
            status,
            copied_count,
            target_dir: target_dir.display().to_string(),
            override_used,
            copied_binaries,
            failed_binaries: failed_binaries.clone(),
            warnings,
        };
        let summary_json = match summary_format {
            SummaryFormat::Json => {
                serde_json::to_string(&summary).context("Failed to serialize deployment summary")?
            }
            SummaryFormat::JsonPretty => serde_json::to_string_pretty(&summary)
                .context("Failed to serialize deployment summary")?,
            SummaryFormat::Text => unreachable!(),
        };
        writeln!(ctx.stdout, "{}", summary_json)?;
    }

    // Return error if any copies failed
    if !failed_binaries.is_empty() {
        if copied_count > 0 {
            anyhow::bail!(
                "Failed to copy {} of {} executables (copied {} successfully)",
                failed_binaries.len(),
                copied_count + failed_binaries.len(),
                copied_count
            );
        } else {
            anyhow::bail!("Failed to copy {} executable(s)", failed_binaries.len());
        }
    }

    Ok(())
}

pub fn run(project_dir: &Path) -> Result<()> {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut ctx = CliContext::new(&mut stdout, &mut stderr);
    run_with_options(project_dir, &RunOptions::default(), &mut ctx)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_format_deployment_summary_override_flag() {
        let summary = format_deployment_summary(1, Path::new("/tmp/bin"), true);
        assert!(summary.contains("[--target]"));
    }

    #[test]
    fn test_format_deployment_summary_default_no_flag() {
        let summary = format_deployment_summary(1, Path::new("/tmp/bin"), false);
        assert!(!summary.contains("[--target]"));
    }

    #[test]
    fn test_override_note_warn_when_redundant() {
        let note = build_override_note(
            Path::new("/tmp/bin"),
            Path::new("/tmp/bin"),
            Some(Path::new("/tmp/bin")),
        );
        assert!(note.lines.iter().any(|l| l.contains("Warning:")));
        assert_eq!(
            note.warnings,
            vec![
                "Resolved target matches default destination; override may be redundant."
                    .to_string()
            ]
        );
    }

    #[test]
    fn test_override_note_no_warn_when_unique() {
        let note = build_override_note(
            Path::new("/tmp/bin"),
            Path::new("/tmp/out"),
            Some(Path::new("/tmp/default")),
        );
        assert!(!note.lines.iter().any(|l| l.contains("Warning:")));
        assert!(note.warnings.is_empty());
    }

    #[test]
    fn test_labels() {
        assert_eq!(ProjectType::Standard.label(), "standard");
        assert_eq!(ProjectType::Tauri.label(), "Tauri");
        assert_eq!(BuildProfile::Release.label(), "release");
        assert_eq!(BuildProfile::Debug.label(), "debug");
    }

    #[test]
    fn test_manifest_bin_names_explicit_bin() {
        let toml_str = r#"
            [package]
            name = "my-pkg"
            
            [[bin]]
            name = "custom-bin"
            path = "src/main.rs"
        "#;
        let val: Value = toml::from_str(toml_str).unwrap();
        let names = manifest_bin_names(&val);
        assert!(names.contains(&"custom-bin".to_string()));
        // The current implementation adds the package name if it's not in the list.
        // Even if explicit [[bin]] exists, it optimistically adds package name.
        assert!(names.contains(&"my-pkg".to_string()));
    }

    #[test]
    fn test_manifest_bin_names_explicit_bin_same_as_package() {
        let toml_str = r#"
            [package]
            name = "my-pkg"
            
            [[bin]]
            name = "my-pkg"
        "#;
        let val: Value = toml::from_str(toml_str).unwrap();
        let names = manifest_bin_names(&val);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "my-pkg");
    }

    #[test]
    fn test_manifest_bin_names_multiple() {
        let toml_str = r#"
            [package]
            name = "pkg"
            
            [[bin]]
            name = "bin1"
            
            [[bin]]
            name = "bin2"
        "#;
        let val: Value = toml::from_str(toml_str).unwrap();
        let names = manifest_bin_names(&val);
        assert!(names.contains(&"bin1".to_string()));
        assert!(names.contains(&"bin2".to_string()));
        assert!(names.contains(&"pkg".to_string()));
    }

    #[test]
    fn test_manifest_bin_names_fallback_path() {
        let toml_str = r#"
            [package]
            name = "pkg"
            
            [[bin]]
            path = "src/bin/other.rs"

            [[bin]]
            # empty entry
        "#;
        let val: Value = toml::from_str(toml_str).unwrap();
        let names = manifest_bin_names(&val);
        assert!(names.contains(&"other".to_string()));
        assert!(names.contains(&"pkg".to_string()));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_find_built_executables_empty() {
        let root = Path::new(".");
        let data = Value::Table(toml::map::Map::new());
        let res = find_built_executables(root, &data, BuildProfile::Release, &[]);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("No packages or bins found"));
    }

    #[test]
    fn test_write_error_paths() {
        struct FailingWriter;
        impl std::io::Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fail"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("fail"))
            }
        }
        let mut fw1 = FailingWriter;
        let mut fw2 = FailingWriter;
        // Test various functions that write
        let _ = write_help(&mut fw1);
        let _ = write_version_banner(&mut fw1);
        let _ = write_parse_error(&mut fw1, &ParseError::UnknownArgs(vec!["x".into()]));

        let mut ctx = CliContext::new(&mut fw1, &mut fw2);
        let opts = RunOptions {
            project_type: Some(ProjectType::Tauri),
            ..Default::default()
        };
        // This will likely fail early on first write
        let _ = run_with_options(Path::new("."), &opts, &mut ctx);
    }

    #[test]
    fn test_read_tauri_product_name_in_package() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let src_tauri = root.join("src-tauri");
        std::fs::create_dir_all(&src_tauri).unwrap();

        std::fs::write(
            src_tauri.join("tauri.conf.json"),
            r#"{ "package": { "productName": "InsidePkg" } }"#,
        )
        .unwrap();
        assert_eq!(read_tauri_product_name(root), Some("InsidePkg".to_string()));
    }

    #[test]
    fn test_read_tauri_product_name_failures() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let src_tauri = root.join("src-tauri");
        std::fs::create_dir_all(&src_tauri).unwrap();

        // 1. Invalid JSON
        std::fs::write(src_tauri.join("tauri.conf.json"), "{ invalid }").unwrap();
        assert!(read_tauri_product_name(root).is_none());

        // 2. Valid JSON, missing productName
        std::fs::write(src_tauri.join("tauri.conf.json"), r#"{ "foo": "bar" }"#).unwrap();
        assert!(read_tauri_product_name(root).is_none());

        // 3. Valid JSON, missing productName in package
        std::fs::write(
            src_tauri.join("tauri.conf.json"),
            r#"{ "package": { "version": "1.0" } }"#,
        )
        .unwrap();
        assert!(read_tauri_product_name(root).is_none());
    }
}
