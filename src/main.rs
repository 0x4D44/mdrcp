use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use toml::Value;
use anyhow::{Context, Result};

fn main() {
    if let Err(e) = run(Path::new(".")) {
        eprintln!("Error: {}", e);
        eprintln!("\nUsage: deploy-tool");
        eprintln!("Run this tool in a Rust project directory to copy the release executable to c:\\apps");
        process::exit(1);
    }
}

/// Refactor `run` to accept a directory path instead of using the current dir.
fn run(project_dir: &Path) -> Result<()> {
    let cargo_path = project_dir.join("Cargo.toml");
    if !cargo_path.exists() {
        anyhow::bail!("No Cargo.toml found. Please run this tool in a Rust project directory");
    }

    let cargo_contents = fs::read_to_string(&cargo_path)
        .context("Failed to read Cargo.toml")?;
    
    let cargo_data: Value = toml::from_str(&cargo_contents)
        .context("Failed to parse Cargo.toml")?;

    let package_name = cargo_data
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .context("Failed to find package name in Cargo.toml")?;

    let exe_name = if cfg!(windows) {
        format!("{}.exe", package_name)
    } else {
        package_name.to_string()
    };

    let source_path = project_dir.join("target").join("release").join(&exe_name);
    if !source_path.exists() {
        anyhow::bail!(
            "Release executable not found at {}. Have you run 'cargo build --release'?",
            source_path.display()
        );
    }

    let target_dir = PathBuf::from(r"c:\apps");
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .context("Failed to create target directory c:\\apps")?;
    }

    let target_path = target_dir.join(&exe_name);
    fs::copy(&source_path, &target_path)
        .with_context(|| format!("Failed to copy {} to {}", source_path.display(), target_path.display()))?;

    println!("Successfully copied {} to {}", exe_name, target_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_and_write_file(path: &Path, contents: &str) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(contents.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    #[test]
    fn test_missing_cargo_toml() {
        let temp_dir = tempdir().unwrap();
        // No Cargo.toml created here, so it should fail with "No Cargo.toml found."
        let result = run(temp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No Cargo.toml found"));
    }

    #[test]
    fn test_invalid_cargo_toml() {
        let temp_dir = tempdir().unwrap();
        
        // Create an invalid Cargo.toml
        create_and_write_file(&temp_dir.path().join("Cargo.toml"), "invalid = toml [ content").unwrap();
        
        let result = run(temp_dir.path());
        assert!(result.is_err(), "Expected an error due to invalid TOML");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse"), 
            "Expected error message containing 'Failed to parse', got: {}", err);
    }

    #[test]
    fn test_missing_release_binary() {
        let temp_dir = tempdir().unwrap();
        
        // Create a valid Cargo.toml
        create_and_write_file(&temp_dir.path().join("Cargo.toml"), 
            "[package]\nname = \"test-project\"\nversion = \"0.1.0\"").unwrap();
        
        // Create the target/release directory structure, but don't create the binary
        let release_dir = temp_dir.path().join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        
        let result = run(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Release executable not found"), 
            "Expected error message containing 'Release executable not found', got: {}", err);
    }
}
