use owo_colors::OwoColorize;
use std::path::PathBuf;

use super::{BuildProfile, ProjectType, RunOptions, SummaryFormat};

const SUMMARY_ALLOWED: &[&str] = &["text", "json", "json-pretty"];

const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TIMESTAMP: &str = env!("MD_BUILD_TIMESTAMP");

#[derive(Clone, Copy, Debug)]
pub struct VersionMetadata {
    pub name: &'static str,
    pub version: &'static str,
    pub build_timestamp: &'static str,
}

pub fn version_metadata() -> VersionMetadata {
    VersionMetadata {
        name: PACKAGE_NAME,
        version: PACKAGE_VERSION,
        build_timestamp: BUILD_TIMESTAMP,
    }
}

pub fn version_banner() -> String {
    let meta = version_metadata();
    format!(
        "{} {} {}",
        meta.name.bold().bright_white(),
        format!("v{}", meta.version).bright_blue().bold(),
        format!("built {}", meta.build_timestamp).dimmed()
    )
}

pub fn write_version_banner(writer: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(writer, "{}", version_banner())
}

pub fn help_text() -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(version_banner());
    lines.push(String::new());
    lines.push(format!(
        "{} {}",
        "Usage:".bold().yellow(),
        "mdrcp [OPTIONS]".bold()
    ));
    lines.push(String::new());
    lines.push("Options:".bold().bright_white().to_string());
    lines.push(format!(
        "  {} {}",
        "--help, -h".bright_cyan(),
        "Show this help message".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--version, -V".bright_cyan(),
        "Show version information".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--target <path>, -t <path>".bright_cyan(),
        "Copy built binaries into the directory (relative paths resolve from project root)"
            .dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--quiet, -q".bright_cyan(),
        "Suppress version banner and progress output".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--summary <format>".bright_cyan(),
        "Emit deployment summary in the given format (text | json | json-pretty)".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--release".bright_cyan(),
        "Copy from target/release (default behavior)".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--debug".bright_cyan(),
        "Copy from target/debug (use after `cargo build`)".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--tauri".bright_cyan(),
        "Force Tauri project mode (look in src-tauri/)".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "--no-tauri".bright_cyan(),
        "Disable Tauri auto-detection".dimmed()
    ));
    lines.push(format!(
        "  {} {}",
        "(none)".bright_cyan(),
        "Run deployment routine".dimmed()
    ));
    lines.push(String::new());
    lines.push(format!(
        "{} {}",
        "Tip:".bold().cyan(),
        "Use after `cargo build --release` (or `cargo build` with --debug) to copy built binaries."
            .dimmed()
    ));
    lines.push(format!(
        "{} {}",
        "Relative paths:".bold().magenta(),
        "Resolved against the project directory passed to the tool.".dimmed()
    ));
    lines.push(format!(
        "{} {}",
        "Tauri:".bold().magenta(),
        "Auto-detected when src-tauri/Cargo.toml and tauri.conf.json exist.".dimmed()
    ));
    lines.join("\n")
}

pub fn write_help(writer: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(writer, "{}", help_text())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Deploy(RunOptions),
    ShowHelp,
    ShowVersion,
    FinishUpdate { source: PathBuf, dest: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownArgs(Vec<String>),
    MissingValue {
        flag: String,
    },
    InvalidValue {
        flag: String,
        value: String,
        expected: &'static [&'static str],
    },
}

pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    if args.is_empty() {
        return Ok(Command::Deploy(RunOptions::default()));
    }

    if args.len() == 1 {
        match args[0].as_str() {
            "-h" | "--help" => return Ok(Command::ShowHelp),
            "-V" | "--version" => return Ok(Command::ShowVersion),
            _ => {}
        }
    }

    // Internal command for self-update: --finish-update <source> <dest>
    if args.len() == 3 && args[0] == "--finish-update" {
        return Ok(Command::FinishUpdate {
            source: PathBuf::from(&args[1]),
            dest: PathBuf::from(&args[2]),
        });
    }

    let mut options = RunOptions::default();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-t" | "--target" => {
                index += 1;
                if index >= args.len() {
                    return Err(ParseError::MissingValue { flag: arg.clone() });
                }
                let value = args[index].clone();
                options.target_override = Some(PathBuf::from(value));
            }
            "-q" | "--quiet" => {
                options.quiet = true;
            }
            "--summary" => {
                index += 1;
                if index >= args.len() {
                    return Err(ParseError::MissingValue { flag: arg.clone() });
                }
                let value = args[index].clone();
                options.summary =
                    parse_summary_format(&value).ok_or_else(|| ParseError::InvalidValue {
                        flag: arg.clone(),
                        value,
                        expected: SUMMARY_ALLOWED,
                    })?;
            }
            _ if arg.starts_with("--target=") => {
                let value = arg.split_once('=').map(|(_, v)| v).unwrap_or("");
                if value.is_empty() {
                    return Err(ParseError::MissingValue {
                        flag: "--target".to_string(),
                    });
                }
                options.target_override = Some(PathBuf::from(value));
            }
            _ if arg.starts_with("--summary=") => {
                let value = arg.split_once('=').map(|(_, v)| v).unwrap_or("");
                if value.is_empty() {
                    return Err(ParseError::MissingValue {
                        flag: "--summary".to_string(),
                    });
                }
                options.summary =
                    parse_summary_format(value).ok_or_else(|| ParseError::InvalidValue {
                        flag: "--summary".to_string(),
                        value: value.to_string(),
                        expected: SUMMARY_ALLOWED,
                    })?;
            }
            "--release" => {
                options.profile = BuildProfile::Release;
            }
            "--debug" => {
                options.profile = BuildProfile::Debug;
            }
            "--tauri" => {
                options.project_type = Some(ProjectType::Tauri);
            }
            "--no-tauri" => {
                options.project_type = Some(ProjectType::Standard);
            }
            _ => {
                return Err(ParseError::UnknownArgs(args.to_vec()));
            }
        }
        index += 1;
    }

    Ok(Command::Deploy(options))
}

fn parse_summary_format(value: &str) -> Option<SummaryFormat> {
    match value {
        "text" => Some(SummaryFormat::Text),
        "json" => Some(SummaryFormat::Json),
        "json-pretty" => Some(SummaryFormat::JsonPretty),
        _ => None,
    }
}

pub fn write_parse_error(
    writer: &mut impl std::io::Write,
    error: &ParseError,
) -> std::io::Result<()> {
    match error {
        ParseError::UnknownArgs(args) => {
            if args.is_empty() {
                return Ok(());
            }
            let joined = args.join(" ");
            writeln!(
                writer,
                "{} {}",
                "Unknown arguments:".bold().bright_red(),
                joined.bold()
            )?;
            writeln!(
                writer,
                "{} {}",
                "Hint:".bold().cyan(),
                "Run `mdrcp --help` for usage details.".dimmed()
            )?;
        }
        ParseError::MissingValue { flag } => {
            writeln!(
                writer,
                "{} {}",
                "Missing value:".bold().bright_red(),
                flag.bold()
            )?;
            writeln!(
                writer,
                "{} {}",
                "Hint:".bold().cyan(),
                "Pass a directory after the flag, e.g. --target path/to/bin".dimmed()
            )?;
            writeln!(
                writer,
                "{} {}",
                "Relative paths:".bold().magenta(),
                "Resolved against the project directory passed to the tool.".dimmed()
            )?;
        }
        ParseError::InvalidValue {
            flag,
            value,
            expected,
        } => {
            writeln!(
                writer,
                "{} {} {}",
                "Invalid value:".bold().bright_red(),
                flag.bold(),
                value.bold()
            )?;
            writeln!(
                writer,
                "{} {}",
                "Accepted:".bold().cyan(),
                expected.join(", ")
            )?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_metadata_matches_env() {
        let meta = version_metadata();
        assert_eq!(meta.name, env!("CARGO_PKG_NAME"));
        assert_eq!(meta.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(meta.build_timestamp, env!("MD_BUILD_TIMESTAMP"));
    }

    #[test]
    fn test_version_banner_contains_fields() {
        let banner = version_banner();
        assert!(banner.contains(env!("CARGO_PKG_VERSION")));
        assert!(banner.contains(env!("MD_BUILD_TIMESTAMP")));
    }

    #[test]
    fn test_help_text_includes_options() {
        let help = help_text();
        assert!(help.contains("mdrcp [OPTIONS]"));
        assert!(help.contains("--help"));
        assert!(help.contains("--version"));
        assert!(help.contains("--target"));
        assert!(help.contains("Relative paths"));
        assert!(help.contains("--quiet"));
        assert!(help.contains("--summary"));
        assert!(help.contains("--debug"));
        assert!(help.contains("--release"));
    }

    #[test]
    fn test_parse_args_default_deploy() {
        let cmd = parse_args(&[]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert!(opts.target_override.is_none());
                assert!(!opts.quiet);
                assert_eq!(opts.summary, SummaryFormat::Text);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_target_flag() {
        let cmd = parse_args(&["--target".to_string(), "out/bin".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.target_override, Some(PathBuf::from("out/bin")));
                assert!(!opts.quiet);
                assert_eq!(opts.summary, SummaryFormat::Text);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_target_equals_syntax() {
        let cmd = parse_args(&["--target=out/bin".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.target_override, Some(PathBuf::from("out/bin")));
                assert!(!opts.quiet);
                assert_eq!(opts.summary, SummaryFormat::Text);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_quiet_flag() {
        let cmd = parse_args(&["--quiet".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert!(opts.quiet);
                assert!(opts.target_override.is_none());
                assert_eq!(opts.summary, SummaryFormat::Text);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_quiet_short_flag() {
        let cmd = parse_args(&[
            "-q".to_string(),
            "--target".to_string(),
            "out/bin".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert!(opts.quiet);
                assert_eq!(opts.target_override, Some(PathBuf::from("out/bin")));
                assert_eq!(opts.summary, SummaryFormat::Text);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_summary_json() {
        let cmd = parse_args(&["--summary".to_string(), "json".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.summary, SummaryFormat::Json);
                assert!(!opts.quiet);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_summary_equals_syntax() {
        let cmd = parse_args(&["--summary=json".to_string(), "-q".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.summary, SummaryFormat::Json);
                assert!(opts.quiet);
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_summary_invalid_value() {
        let err = parse_args(&["--summary".to_string(), "xml".to_string()]).unwrap_err();
        assert_eq!(
            err,
            ParseError::InvalidValue {
                flag: "--summary".to_string(),
                value: "xml".to_string(),
                expected: SUMMARY_ALLOWED,
            }
        );
    }

    #[test]
    fn test_parse_args_debug_flag() {
        let cmd = parse_args(&["--debug".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.profile, BuildProfile::Debug);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_release_flag() {
        let cmd = parse_args(&["--debug".to_string(), "--release".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.profile, BuildProfile::Release);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_missing_value_errors() {
        let err = parse_args(&["--target".to_string()]).unwrap_err();
        assert_eq!(
            err,
            ParseError::MissingValue {
                flag: "--target".to_string()
            }
        );

        let err_summary = parse_args(&["--summary".to_string()]).unwrap_err();
        assert_eq!(
            err_summary,
            ParseError::MissingValue {
                flag: "--summary".to_string()
            }
        );
    }

    #[test]
    fn test_parse_args_unknown() {
        let err = parse_args(&["--unknown".to_string()]).unwrap_err();
        assert_eq!(err, ParseError::UnknownArgs(vec!["--unknown".to_string()]));
    }

    #[test]
    fn test_parse_args_tauri_flag() {
        let cmd = parse_args(&["--tauri".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.project_type, Some(ProjectType::Tauri));
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_no_tauri_flag() {
        let cmd = parse_args(&["--no-tauri".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.project_type, Some(ProjectType::Standard));
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_args_summary_text_explicit() {
        let cmd = parse_args(&["--summary".to_string(), "text".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.summary, SummaryFormat::Text);
            }
            _ => panic!("Unexpected command"),
        }
    }

    #[test]
    fn test_parse_args_empty_values() {
        // Test --target=
        let err = parse_args(&["--target=".to_string()]).unwrap_err();
        assert!(matches!(err, ParseError::MissingValue { flag } if flag == "--target"));

        // Test --summary=
        let err_summary = parse_args(&["--summary=".to_string()]).unwrap_err();
        assert!(matches!(err_summary, ParseError::MissingValue { flag } if flag == "--summary"));
    }

    #[test]
    fn test_parse_args_finish_update() {
        let args = vec![
            "--finish-update".to_string(),
            "source/path".to_string(),
            "dest/path".to_string(),
        ];
        let cmd = parse_args(&args).unwrap();
        match cmd {
            Command::FinishUpdate { source, dest } => {
                assert_eq!(source, PathBuf::from("source/path"));
                assert_eq!(dest, PathBuf::from("dest/path"));
            }
            _ => panic!("Expected FinishUpdate command"),
        }
    }

    #[test]
    fn test_parse_args_summary_equals_invalid() {
        let err = parse_args(&["--summary=invalid".to_string()]).unwrap_err();
        assert!(matches!(err, ParseError::InvalidValue { flag, .. } if flag == "--summary"));
    }

    #[test]
    fn test_parse_args_tauri_with_debug() {
        let cmd = parse_args(&["--tauri".to_string(), "--debug".to_string()]).unwrap();
        match cmd {
            Command::Deploy(opts) => {
                assert_eq!(opts.project_type, Some(ProjectType::Tauri));
                assert_eq!(opts.profile, BuildProfile::Debug);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_help_text_includes_tauri_options() {
        let help = help_text();
        assert!(help.contains("--tauri"));
        assert!(help.contains("--no-tauri"));
        assert!(help.contains("src-tauri"));
    }

    #[test]
    fn test_print_functions_run_without_panic() {
        // These just ensure the code paths are hit; we capture to a sink.
        let mut sink = Vec::new();
        write_help(&mut sink).unwrap();
        write_version_banner(&mut sink).unwrap();
    }

    #[test]
    fn test_print_parse_errors() {
        let mut sink = Vec::new();
        // Exercise the display logic for different error variants
        write_parse_error(
            &mut sink,
            &ParseError::UnknownArgs(vec!["--bad".to_string()]),
        )
        .unwrap();

        write_parse_error(
            &mut sink,
            &ParseError::MissingValue {
                flag: "--target".to_string(),
            },
        )
        .unwrap();

        write_parse_error(
            &mut sink,
            &ParseError::InvalidValue {
                flag: "--summary".to_string(),
                value: "xml".to_string(),
                expected: &["text", "json"],
            },
        )
        .unwrap();

        // Edge case: UnknownArgs empty (should return early, but good to test)
        write_parse_error(&mut sink, &ParseError::UnknownArgs(vec![])).unwrap();
    }
}
