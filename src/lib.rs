use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

pub mod cli;

pub use cli::{
    parse_args, print_help, print_parse_error, print_version_banner, Command, ParseError,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SummaryFormat {
    #[default]
    Text,
    Json,
    JsonPretty,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOptions {
    pub target_override: Option<PathBuf>,
    pub quiet: bool,
    pub summary: SummaryFormat,
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
const HINT_DEFAULT: &str = r"c:\\apps";

#[cfg(not(windows))]
const HINT_DEFAULT: &str = "~/.local/bin";

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
    match run_with_options(cwd, options) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{} {}", "Error:".bold().bright_red(), e);
            eprintln!();
            eprintln!(
                "{} {}",
                "Usage:".bold().yellow(),
                "deploy-tool [OPTIONS]".bold()
            );
            let hint = match default_target_dir() {
                Ok(p) => p.display().to_string(),
                Err(_) => HINT_DEFAULT.to_string(),
            };
            eprintln!(
                "{} {} {}",
                "Hint:".bold().cyan(),
                "Run this tool in a Rust project directory to copy release executables to".dimmed(),
                hint.bold().bright_white()
            );
            eprintln!(
                "{} {}",
                "More info:".bold().cyan(),
                "deploy-tool --help".bold()
            );
            eprintln!(
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
fn find_built_executables(project_dir: &Path, cargo_data: &Value) -> Result<Vec<String>> {
    let release_dir = project_dir.join("target").join("release");
    let mut candidate_names: HashSet<String> = HashSet::new();

    // Root package (if any)
    for name in manifest_bin_names(cargo_data) {
        candidate_names.insert(name);
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
            let member_manifest_path = project_dir.join(member_path).join("Cargo.toml");
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

    // Filter to only candidates with existing release executables
    let mut built_executables = Vec::new();
    for base in candidate_names {
        let exe_name = exe_filename(&base);
        if release_dir.join(&exe_name).exists() {
            built_executables.push(base);
        }
    }
    Ok(built_executables)
}

/// Determine the default deployment target directory per-OS.
#[cfg(windows)]
fn default_target_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(r"c:\\apps"))
}

#[cfg(not(windows))]
fn default_target_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow::anyhow!("HOME is not set; cannot determine ~/.local/bin"))?;
    Ok(Path::new(&home).join(".local").join("bin"))
}

/// Main deployment function that handles both single packages and workspaces
pub fn run_with_options(project_dir: &Path, options: &RunOptions) -> Result<()> {
    let cargo_path = project_dir.join("Cargo.toml");
    if !cargo_path.exists() {
        anyhow::bail!("No Cargo.toml found. Please run this tool in a Rust project directory");
    }

    let cargo_contents = fs::read_to_string(&cargo_path).context("Failed to read Cargo.toml")?;

    let cargo_data: Value =
        toml::from_str(&cargo_contents).context("Failed to parse Cargo.toml")?;

    let built_executables = find_built_executables(project_dir, &cargo_data)?;

    if built_executables.is_empty() {
        anyhow::bail!("No built release executables found. Have you run 'cargo build --release'?");
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
    }

    let mut copied_count = 0;
    let mut copied_binaries: Vec<String> = Vec::new();
    let mut failed_binaries: Vec<FailedCopy> = Vec::new();

    for package_name in built_executables {
        let exe_name = exe_filename(&package_name);

        let source_path = project_dir.join("target").join("release").join(&exe_name);
        let target_path = target_dir.join(&exe_name);

        match fs::copy(&source_path, &target_path) {
            Ok(_) => {
                if emit_text {
                    println!(
                        "{} {} {}",
                        "Copied".bold().green(),
                        exe_name.bold().green(),
                        format!("-> {}", target_path.display()).dimmed()
                    );
                }
                copied_count += 1;
                copied_binaries.push(exe_name);
            }
            Err(e) => {
                let error_msg = format!(
                    "Failed to copy {} to {}: {}",
                    source_path.display(),
                    target_path.display(),
                    e
                );
                if emit_text {
                    eprintln!(
                        "{} {} {}",
                        "Failed".bold().bright_red(),
                        exe_name.bold().yellow(),
                        format!("-> {}: {}", target_path.display(), e).dimmed()
                    );
                }
                failed_binaries.push(FailedCopy {
                    binary: exe_name,
                    error: error_msg,
                });
            }
        }
    }

    if emit_text {
        println!();
        println!(
            "{}",
            format_deployment_summary(copied_count, &target_dir, override_used)
        );

        // Report failures if any
        if !failed_binaries.is_empty() {
            println!();
            eprintln!(
                "{} {}",
                "Failed to copy".bold().bright_red(),
                format!("{} executable(s):", failed_binaries.len())
                    .bold()
                    .bright_red()
            );
            for failed in &failed_binaries {
                eprintln!("  {} {}", "•".bright_red(), failed.error.dimmed());
            }
        }
    }

    let mut override_note: Option<OverrideNote> = None;
    if let Some(raw) = override_raw {
        let note = build_override_note(&raw, &target_dir, default_target.as_deref());
        if emit_text {
            for line in &note.lines {
                println!("{}", line);
            }
        } else {
            for warning in &note.warnings {
                eprintln!("Warning: {}", warning);
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
        println!("{}", summary_json);
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
            anyhow::bail!(
                "Failed to copy {} executable(s)",
                failed_binaries.len()
            );
        }
    }

    Ok(())
}

pub fn run(project_dir: &Path) -> Result<()> {
    run_with_options(project_dir, &RunOptions::default())
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
}
