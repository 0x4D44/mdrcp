use mdrcp::{run_with_options, BuildProfile, CliContext, RunOptions};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

#[cfg(windows)]
fn exe_filename(base: &str) -> String {
    format!("{}.exe", base)
}

#[cfg(not(windows))]
fn exe_filename(base: &str) -> String {
    base.to_string()
}

fn create_and_write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[test]
fn test_self_update_trigger() {
    let temp_dir = tempdir().unwrap();

    // Setup a fake project
    create_and_write_file(
        &temp_dir.path().join("Cargo.toml"),
        "[package]\nname=\"myapp\"\nversion=\"0.1.0\"",
    )
    .unwrap();

    let rel = temp_dir.path().join("target").join("release");
    fs::create_dir_all(&rel).unwrap();
    let exe = exe_filename("myapp");
    create_and_write_file(&rel.join(&exe), "new content").unwrap();

    // Pretend we are installing to a location where 'myapp' is already running from
    let install_dir = temp_dir.path().join("install");
    fs::create_dir_all(&install_dir).unwrap();
    let installed_exe = install_dir.join(&exe);
    create_and_write_file(&installed_exe, "old content").unwrap();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut ctx = CliContext::new(&mut stdout, &mut stderr);

    // Mock the current executable as the target file
    ctx.current_exe = Some(installed_exe.clone());

    let options = RunOptions {
        target_override: Some(install_dir.clone()),
        profile: BuildProfile::Release,
        ..Default::default()
    };

    let result = run_with_options(temp_dir.path(), &options, &mut ctx);

    // It is expected to fail because we can't actually spawn the dummy text file as an executable
    assert!(result.is_err());

    let output_out = String::from_utf8(stdout).unwrap();
    let output_err = String::from_utf8(stderr).unwrap();

    assert!(output_out.contains("Deferred"));
    assert!(output_err.contains("Failed to self-update"));
}
