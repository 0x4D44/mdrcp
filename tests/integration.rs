use mdrcp::{do_main, exe_filename, run, run_with_options, BuildProfile, ProjectType, RunOptions};
use serde_json::Value;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

const TARGET_OVERRIDE_ENV: &str = "MD_TARGET_DIR";

fn create_and_write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    Ok(())
}

struct EnvVarGuard {
    key: &'static str,
    prev: Option<OsString>,
}

impl EnvVarGuard {
    fn set_path_if_windows(key: &'static str, value: &Path) -> Option<Self> {
        if cfg!(windows) {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value);
            Some(Self { key, prev })
        } else {
            None
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if cfg!(windows) {
            if let Some(prev) = self.prev.take() {
                std::env::set_var(self.key, prev);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}

#[test]
fn test_missing_cargo_toml() {
    let temp_dir = tempdir().unwrap();
    let result = run(temp_dir.path());
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No Cargo.toml found"));
}

#[test]
fn test_invalid_cargo_toml() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "invalid = toml [ content",
    )
    .unwrap();
    let result = run(temp_dir.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to parse"));
}

#[test]
fn test_missing_release_binary() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[package]\nname=\"test\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.path().join("target").join("release")).unwrap();
    let result = run(temp_dir.path());
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No built release executables found"));
}

#[test]
fn test_missing_debug_binary() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[package]\nname=\"test\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.path().join("target").join("debug")).unwrap();

    let mut options = RunOptions::default();
    options.profile = BuildProfile::Debug;
    let result = run_with_options(temp_dir.path(), &options);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No built debug executables found"));
}

#[test]
fn test_no_packages_or_bins() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(&temp_dir.path().join("Cargo.toml"), "").unwrap();
    let result = run(temp_dir.path());
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No packages or bins found"));
}

#[test]
fn test_workspace_with_built_members() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"a\",\"b\"]",
    )
    .unwrap();
    for m in ["a", "b"] {
        fs::create_dir_all(temp_dir.path().join(m)).unwrap();
        create_and_write_file(
            &temp_dir.path().join(m).join("Cargo.toml"),
            &format!("[package]\nname=\"{}\"\nversion=\"0.1.0\"", m),
        )
        .unwrap();
    }
    let rel = temp_dir.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    for exe in [exe_filename("a"), exe_filename("b")] {
        create_and_write_file(&rel.join(exe), "x").unwrap();
    }
    // Run will copy both; we just ensure it succeeds
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    let target_dir = tmp_home.path().join("bin_out");
    let _target_guard = EnvVarGuard::set_path_if_windows(TARGET_OVERRIDE_ENV, &target_dir);
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_dir.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
}

#[test]
fn test_workspace_with_partial_builds() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"a\",\"b\",\"c\"]",
    )
    .unwrap();
    for m in ["a", "b", "c"] {
        fs::create_dir_all(temp_dir.path().join(m)).unwrap();
        create_and_write_file(
            &temp_dir.path().join(m).join("Cargo.toml"),
            &format!("[package]\nname=\"{}\"\nversion=\"0.1.0\"", m),
        )
        .unwrap();
    }
    let rel = temp_dir.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    for exe in [exe_filename("a"), exe_filename("c")] {
        create_and_write_file(&rel.join(exe), "x").unwrap();
    }
    // Should succeed copying two
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    let target_dir = tmp_home.path().join("bin_out");
    let _target_guard = EnvVarGuard::set_path_if_windows(TARGET_OVERRIDE_ENV, &target_dir);
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_dir.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
}

#[test]
fn test_mixed_workspace_and_package() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[package]\nname=\"root\"\nversion=\"0.1.0\"\n[workspace]\nmembers=[\"sub\"]",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.path().join("sub")).unwrap();
    create_and_write_file(
        &temp_dir.path().join("sub").join("Cargo.toml"),
        "[package]\nname=\"sub\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_dir.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    for exe in [exe_filename("root"), exe_filename("sub")] {
        create_and_write_file(&rel.join(exe), "x").unwrap();
    }
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    let target_dir = tmp_home.path().join("bin_out");
    let _target_guard = EnvVarGuard::set_path_if_windows(TARGET_OVERRIDE_ENV, &target_dir);
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_dir.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
}

#[test]
fn test_workspace_members_with_invalid_entries_are_ignored() {
    let temp_dir = tempdir().unwrap();
    // Include: valid 'a', non-string 42, missing path 'missing', invalid toml 'bad', valid 'c'
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"a\", 42, \"missing\", \"bad\", \"c\"]",
    )
    .unwrap();

    // Create valid members 'a' and 'c'
    for m in ["a", "c"] {
        fs::create_dir_all(temp_dir.path().join(m)).unwrap();
        create_and_write_file(
            &temp_dir.path().join(m).join("Cargo.toml"),
            &format!("[package]\nname=\"{}\"\nversion=\"0.1.0\"", m),
        )
        .unwrap();
    }
    // Create member 'bad' with invalid Cargo.toml
    fs::create_dir_all(temp_dir.path().join("bad")).unwrap();
    create_and_write_file(
        &temp_dir.path().join("bad").join("Cargo.toml"),
        "this = is not [ valid",
    )
    .unwrap();

    // Build executables for a and c only
    let rel = temp_dir.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    for exe in [exe_filename("a"), exe_filename("c")] {
        create_and_write_file(&rel.join(exe), "x").unwrap();
    }

    // Run
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    let target_dir = tmp_home.path().join("bin_out");
    let _target_guard = EnvVarGuard::set_path_if_windows(TARGET_OVERRIDE_ENV, &target_dir);
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_dir.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
}

#[test]
fn test_workspace_no_built_executables() {
    let temp_dir = tempdir().unwrap();
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"a\"]",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.path().join("a")).unwrap();
    create_and_write_file(
        &temp_dir.path().join("a").join("Cargo.toml"),
        "[package]\nname=\"a\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.path().join("target").join("release")).unwrap();
    let res = run(temp_dir.path());
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("No built release executables found"));
}

#[cfg(target_family = "unix")]
#[test]
fn test_deploy_to_linux_home_local_bin() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_project.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
    let target = tmp_home.path().join(".local").join("bin").join(exe);
    assert!(target.exists());
}

#[cfg(target_family = "unix")]
#[test]
fn test_path_stem_fallback_copies_tool() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    // [[bin]] with only path; no explicit name
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[[bin]]\npath=\"src/tools/tool.rs\"\n[package]\nname=\"pkg\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("tool");
    create_and_write_file(&rel.join(&exe), "x").unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_project.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
    assert!(tmp_home.path().join(".local/bin").join(exe).exists());
}

#[cfg(target_family = "unix")]
#[test]
fn test_single_package_named_bin_copies() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"pkg\"\nversion=\"0.1.0\"\n\n[[bin]]\nname=\"toolx\"\npath=\"src/main.rs\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("toolx");
    create_and_write_file(&rel.join(&exe), "x").unwrap();

    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_project.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_ok());
    assert!(tmp_home.path().join(".local/bin").join(exe).exists());
}

#[test]
fn test_run_with_target_override_relative_path() {
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();

    let override_dir = PathBuf::from("custom/bin");
    let mut options = RunOptions::default();
    options.target_override = Some(override_dir.clone());

    run_with_options(temp_project.path(), &options).unwrap();

    let expected_target = temp_project.path().join(override_dir).join(exe);
    assert!(expected_target.exists());
}

#[test]
fn test_run_with_debug_profile_and_override() {
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let dbg = temp_project.path().join("target").join("debug");
    fs::create_dir_all(&dbg).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&dbg.join(&exe), "x").unwrap();

    let override_dir = PathBuf::from("debug/bin");
    let mut options = RunOptions::default();
    options.target_override = Some(override_dir.clone());
    options.profile = BuildProfile::Debug;

    run_with_options(temp_project.path(), &options).unwrap();

    let expected_target = temp_project.path().join(override_dir).join(exe);
    assert!(expected_target.exists());
}

#[test]
fn test_run_with_summary_json_quiet() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();

    let bin = env!("CARGO_BIN_EXE_mdrcp");
    let tmp_home = tempdir().unwrap();
    let old_home = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp_home.path());
    let output = std::process::Command::new(bin)
        .current_dir(temp_project.path())
        .args(["--quiet", "--summary", "json", "--target", "dist/bin"])
        .output()
        .unwrap();
    match old_home {
        Some(val) => std::env::set_var("HOME", val),
        None => std::env::remove_var("HOME"),
    }

    assert!(output.status.success());

    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["status"], "ok");
    assert_eq!(summary["copied_count"], 1);
    assert_eq!(summary["override_used"], true);
    assert!(summary["warnings"].as_array().unwrap().is_empty());
    assert!(summary["target_dir"]
        .as_str()
        .unwrap()
        .ends_with("dist/bin"));
    assert!(summary["copied_binaries"]
        .as_array()
        .unwrap()
        .contains(&serde_json::Value::String(exe.clone())));
}

#[test]
fn test_run_with_summary_json_pretty() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();

    let bin = env!("CARGO_BIN_EXE_mdrcp");
    let tmp_home = tempdir().unwrap();
    let old_home = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp_home.path());
    let output = std::process::Command::new(bin)
        .current_dir(temp_project.path())
        .args([
            "--quiet",
            "--summary",
            "json-pretty",
            "--target",
            "dist/bin",
        ])
        .output()
        .unwrap();
    match old_home {
        Some(val) => std::env::set_var("HOME", val),
        None => std::env::remove_var("HOME"),
    }

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\n  \"copied_binaries\""));
    let summary: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(summary["override_used"], true);
    assert!(summary["target_dir"]
        .as_str()
        .unwrap()
        .ends_with("dist/bin"));
}

#[test]
fn test_do_main_error_hint_home_missing_integration() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let tmp = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    std::env::remove_var("HOME");
    assert_eq!(do_main(tmp.path()), 1);
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
}
#[cfg(target_family = "unix")]
#[test]
fn test_copy_failure_existing_dir() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp_home.path());
    let bin_dir = tmp_home.path().join(".local").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(bin_dir.join(&exe)).unwrap();
    let res = run(temp_project.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Failed to copy"));
}

#[cfg(target_family = "unix")]
#[test]
fn test_target_dir_creation_failure() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    // create ~/.local as a file to force create_dir_all to fail
    create_and_write_file(&tmp_home.path().join(".local"), "not a dir").unwrap();
    std::env::set_var("HOME", tmp_home.path());
    let res = run(temp_project.path());
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Failed to create target directory"));
}

#[test]
fn test_do_main_error_and_success() {
    // error path
    let temp_dir = tempdir().unwrap();
    assert_eq!(do_main(temp_dir.path()), 1);

    // success path
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("demo");
    create_and_write_file(&rel.join(&exe), "x").unwrap();
    let tmp_home = tempdir().unwrap();
    let old = std::env::var_os("HOME");
    let target_dir = tmp_home.path().join("bin_out");
    let _target_guard = EnvVarGuard::set_path_if_windows(TARGET_OVERRIDE_ENV, &target_dir);
    std::env::set_var("HOME", tmp_home.path());
    assert_eq!(do_main(temp_project.path()), 0);
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
}

// ============== Tauri Support Tests ==============

#[test]
fn test_tauri_auto_detect_and_deploy() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();

    // Create Tauri project structure
    let src_tauri = temp_project.path().join("src-tauri");
    fs::create_dir_all(&src_tauri).unwrap();
    create_and_write_file(
        &src_tauri.join("Cargo.toml"),
        "[package]\nname=\"my-tauri-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    create_and_write_file(
        &temp_project.path().join("tauri.conf.json"),
        r#"{"productName": "My Tauri App"}"#,
    )
    .unwrap();

    // Create the release executable
    let rel = src_tauri.join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("my-tauri-app");
    create_and_write_file(&rel.join(&exe), "binary content").unwrap();

    // Set up target directory
    let target_dir = tempdir().unwrap();
    let options = RunOptions {
        target_override: Some(target_dir.path().to_path_buf()),
        quiet: true,
        ..Default::default()
    };

    // Deploy
    let result = run_with_options(temp_project.path(), &options);
    assert!(result.is_ok(), "Tauri deploy failed: {:?}", result);

    // Verify executable was copied
    assert!(target_dir.path().join(&exe).exists());
}

#[test]
fn test_tauri_with_product_name_from_config() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();

    // Create Tauri project with different productName
    let src_tauri = temp_project.path().join("src-tauri");
    fs::create_dir_all(&src_tauri).unwrap();
    create_and_write_file(
        &src_tauri.join("Cargo.toml"),
        "[package]\nname=\"tauri-backend\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    // productName differs from Cargo.toml package name
    create_and_write_file(
        &temp_project.path().join("tauri.conf.json"),
        r#"{"productName": "MyApp"}"#,
    )
    .unwrap();

    // Create executables for both names
    let rel = src_tauri.join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let product_exe = exe_filename("MyApp");
    create_and_write_file(&rel.join(&product_exe), "product binary").unwrap();

    // Set up target directory
    let target_dir = tempdir().unwrap();
    let options = RunOptions {
        target_override: Some(target_dir.path().to_path_buf()),
        quiet: true,
        ..Default::default()
    };

    // Deploy
    let result = run_with_options(temp_project.path(), &options);
    assert!(result.is_ok(), "Tauri deploy failed: {:?}", result);

    // Verify productName executable was copied
    assert!(target_dir.path().join(&product_exe).exists());
}

#[test]
fn test_tauri_debug_profile() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();

    // Create Tauri project structure
    let src_tauri = temp_project.path().join("src-tauri");
    fs::create_dir_all(&src_tauri).unwrap();
    create_and_write_file(
        &src_tauri.join("Cargo.toml"),
        "[package]\nname=\"debug-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    create_and_write_file(
        &temp_project.path().join("tauri.conf.json"),
        "{}",
    )
    .unwrap();

    // Create the debug executable (not release)
    let dbg = src_tauri.join("target").join("debug");
    fs::create_dir_all(&dbg).unwrap();
    let exe = exe_filename("debug-app");
    create_and_write_file(&dbg.join(&exe), "debug binary").unwrap();

    // Set up target directory with debug profile
    let target_dir = tempdir().unwrap();
    let options = RunOptions {
        target_override: Some(target_dir.path().to_path_buf()),
        profile: BuildProfile::Debug,
        quiet: true,
        ..Default::default()
    };

    // Deploy
    let result = run_with_options(temp_project.path(), &options);
    assert!(result.is_ok(), "Tauri debug deploy failed: {:?}", result);

    // Verify executable was copied
    assert!(target_dir.path().join(&exe).exists());
}

#[test]
fn test_force_tauri_on_standard_project_fails() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();

    // Create a standard Rust project (no src-tauri)
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"standard-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();

    let target_dir = tempdir().unwrap();
    let options = RunOptions {
        target_override: Some(target_dir.path().to_path_buf()),
        project_type: Some(ProjectType::Tauri), // Force Tauri mode
        quiet: true,
        ..Default::default()
    };

    // Should fail because src-tauri/Cargo.toml doesn't exist
    let result = run_with_options(temp_project.path(), &options);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No Cargo.toml found"));
}

#[test]
fn test_no_tauri_flag_uses_root_cargo() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();

    // Create a Tauri project structure but also have a root Cargo.toml
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"root-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();

    let src_tauri = temp_project.path().join("src-tauri");
    fs::create_dir_all(&src_tauri).unwrap();
    create_and_write_file(
        &src_tauri.join("Cargo.toml"),
        "[package]\nname=\"tauri-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    create_and_write_file(
        &temp_project.path().join("tauri.conf.json"),
        "{}",
    )
    .unwrap();

    // Create release executable in root target (not src-tauri/target)
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("root-app");
    create_and_write_file(&rel.join(&exe), "root binary").unwrap();

    // Force standard mode with --no-tauri
    let target_dir = tempdir().unwrap();
    let options = RunOptions {
        target_override: Some(target_dir.path().to_path_buf()),
        project_type: Some(ProjectType::Standard), // --no-tauri
        quiet: true,
        ..Default::default()
    };

    // Deploy should use root Cargo.toml
    let result = run_with_options(temp_project.path(), &options);
    assert!(result.is_ok(), "Standard deploy failed: {:?}", result);

    // Verify root-app was copied, not tauri-app
    assert!(target_dir.path().join(&exe).exists());
}

#[test]
fn test_tauri_not_detected_without_tauri_conf() {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp_project = tempdir().unwrap();

    // Create src-tauri/Cargo.toml but NO tauri.conf.json
    // This should NOT be detected as Tauri
    create_and_write_file(
        &temp_project.path().join("Cargo.toml"),
        "[package]\nname=\"hybrid-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();

    let src_tauri = temp_project.path().join("src-tauri");
    fs::create_dir_all(&src_tauri).unwrap();
    create_and_write_file(
        &src_tauri.join("Cargo.toml"),
        "[package]\nname=\"tauri-part\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    // Note: NO tauri.conf.json

    // Create release executable in root target
    let rel = temp_project.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("hybrid-app");
    create_and_write_file(&rel.join(&exe), "hybrid binary").unwrap();

    // Auto-detect should pick standard mode
    let target_dir = tempdir().unwrap();
    let options = RunOptions {
        target_override: Some(target_dir.path().to_path_buf()),
        quiet: true,
        ..Default::default()
    };

    let result = run_with_options(temp_project.path(), &options);
    assert!(result.is_ok(), "Standard deploy failed: {:?}", result);
    assert!(target_dir.path().join(&exe).exists());
}
