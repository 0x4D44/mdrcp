use mdrcp::{exe_filename, run_with_options, CliContext, RunOptions};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn create_and_write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[test]
#[cfg(windows)]
fn test_target_dir_creation_failure_invalid_name() {
    let temp = tempdir().unwrap();
    create_and_write_file(
        &temp.path().join("Cargo.toml"),
        "[package]\nname=\"myapp\"\nversion=\"0.1.0\"",
    )
    .unwrap();

    let rel = temp.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("myapp");
    create_and_write_file(&rel.join(&exe), "content").unwrap();

    // Use an invalid character for Windows paths
    let target_path = temp.path().join("invalid|dir");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut ctx = CliContext::new(&mut stdout, &mut stderr);

    let options = RunOptions {
        target_override: Some(target_path),
        ..Default::default()
    };

    let result = run_with_options(temp.path(), &options, &mut ctx);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to create target directory"));
}

#[test]
fn test_target_dir_creation_failure_with_override() {
    let temp = tempdir().unwrap();
    create_and_write_file(
        &temp.path().join("Cargo.toml"),
        "[package]\nname=\"myapp\"\nversion=\"0.1.0\"",
    )
    .unwrap();

    let rel = temp.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("myapp");
    create_and_write_file(&rel.join(&exe), "content").unwrap();

    // Create a file where we want the target dir to be
    let target_path = temp.path().join("blocked_dir");
    create_and_write_file(&target_path, "blocker").unwrap();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut ctx = CliContext::new(&mut stdout, &mut stderr);

    let options = RunOptions {
        target_override: Some(target_path),
        ..Default::default()
    };

    let result = run_with_options(temp.path(), &options, &mut ctx);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("exists but is not a directory"),
        "Actual error was: '{}'",
        msg
    );
}
