use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use toml::Value;
use anyhow::{Context, Result};

fn main() {
    if let Err(e) = run(Path::new(".")) {
        eprintln!("Error: {}", e);
        eprintln!("\nUsage: deploy-tool");
        eprintln!("Run this tool in a Rust project directory to copy release executables to c:\\apps");
        process::exit(1);
    }
}

/// Find all built executables from workspace members or single package
fn find_built_executables(project_dir: &Path, cargo_data: &Value) -> Result<Vec<String>> {
    let mut package_names = Vec::new();
    
    // Check if this is a workspace
    if let Some(workspace) = cargo_data.get("workspace") {
        if let Some(members) = workspace.get("members").and_then(|m| m.as_array()) {
            // Parse workspace members
            for member in members {
                if let Some(member_path) = member.as_str() {
                    let member_cargo = project_dir.join(member_path).join("Cargo.toml");
                    if let Ok(contents) = fs::read_to_string(&member_cargo) {
                        if let Ok(member_data) = toml::from_str::<Value>(&contents) {
                            if let Some(name) = member_data.get("package")
                                .and_then(|p| p.get("name"))
                                .and_then(|n| n.as_str()) {
                                package_names.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Also check if root has a package (mixed workspace+package)
    if let Some(name) = cargo_data.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str()) {
        package_names.push(name.to_string());
    }
    
    if package_names.is_empty() {
        anyhow::bail!("No packages found in Cargo.toml");
    }
    
    // Filter to only packages with existing release executables
    let release_dir = project_dir.join("target").join("release");
    let mut built_executables = Vec::new();
    
    for package_name in package_names {
        let exe_name = if cfg!(windows) {
            format!("{}.exe", package_name)
        } else {
            package_name.clone()
        };
        
        if release_dir.join(&exe_name).exists() {
            built_executables.push(package_name);
        }
    }
    
    Ok(built_executables)
}

/// Main deployment function that handles both single packages and workspaces
fn run(project_dir: &Path) -> Result<()> {
    let cargo_path = project_dir.join("Cargo.toml");
    if !cargo_path.exists() {
        anyhow::bail!("No Cargo.toml found. Please run this tool in a Rust project directory");
    }

    let cargo_contents = fs::read_to_string(&cargo_path)
        .context("Failed to read Cargo.toml")?;
    
    let cargo_data: Value = toml::from_str(&cargo_contents)
        .context("Failed to parse Cargo.toml")?;

    let built_executables = find_built_executables(project_dir, &cargo_data)?;
    
    if built_executables.is_empty() {
        anyhow::bail!("No built release executables found. Have you run 'cargo build --release'?");
    }

    let target_dir = PathBuf::from(r"c:\apps");
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .context("Failed to create target directory c:\\apps")?;
    }

    let mut copied_count = 0;
    for package_name in built_executables {
        let exe_name = if cfg!(windows) {
            format!("{}.exe", package_name)
        } else {
            package_name.clone()
        };
        
        let source_path = project_dir.join("target").join("release").join(&exe_name);
        let target_path = target_dir.join(&exe_name);
        
        fs::copy(&source_path, &target_path)
            .with_context(|| format!("Failed to copy {} to {}", source_path.display(), target_path.display()))?;
        
        println!("Successfully copied {} to {}", exe_name, target_path.display());
        copied_count += 1;
    }
    
    println!("\nDeployed {} executable(s) to c:\\apps", copied_count);
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
        assert!(err.contains("No built release executables found"), 
            "Expected error message containing 'No built release executables found', got: {}", err);
    }

    #[test]
    fn test_workspace_with_built_members() {
        let temp_dir = tempdir().unwrap();
        
        // Create workspace Cargo.toml
        create_and_write_file(&temp_dir.path().join("Cargo.toml"), 
            "[workspace]\nmembers = [\"project-a\", \"project-b\"]").unwrap();
        
        // Create member directories and Cargo.toml files
        fs::create_dir_all(temp_dir.path().join("project-a")).unwrap();
        create_and_write_file(&temp_dir.path().join("project-a").join("Cargo.toml"),
            "[package]\nname = \"project-a\"\nversion = \"0.1.0\"").unwrap();
            
        fs::create_dir_all(temp_dir.path().join("project-b")).unwrap();
        create_and_write_file(&temp_dir.path().join("project-b").join("Cargo.toml"),
            "[package]\nname = \"project-b\"\nversion = \"0.1.0\"").unwrap();
        
        // Create release directory and executables for both projects
        let release_dir = temp_dir.path().join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        
        let exe_a = if cfg!(windows) { "project-a.exe" } else { "project-a" };
        let exe_b = if cfg!(windows) { "project-b.exe" } else { "project-b" };
        
        create_and_write_file(&release_dir.join(exe_a), "fake executable").unwrap();
        create_and_write_file(&release_dir.join(exe_b), "fake executable").unwrap();
        
        // Create c:\apps directory (use temp dir for testing)
        let apps_dir = temp_dir.path().join("apps");
        fs::create_dir_all(&apps_dir).unwrap();
        
        // Mock the c:\apps path by creating a test version
        // This test verifies the logic works, actual deployment would need integration test
        let result = find_built_executables(temp_dir.path(), &toml::from_str(
            "[workspace]\nmembers = [\"project-a\", \"project-b\"]"
        ).unwrap());
        
        assert!(result.is_ok());
        let executables = result.unwrap();
        assert_eq!(executables.len(), 2);
        assert!(executables.contains(&"project-a".to_string()));
        assert!(executables.contains(&"project-b".to_string()));
    }

    #[test]
    fn test_workspace_with_partial_builds() {
        let temp_dir = tempdir().unwrap();
        
        // Create workspace Cargo.toml
        create_and_write_file(&temp_dir.path().join("Cargo.toml"), 
            "[workspace]\nmembers = [\"project-a\", \"project-b\", \"project-c\"]").unwrap();
        
        // Create member directories and Cargo.toml files
        for project in &["project-a", "project-b", "project-c"] {
            fs::create_dir_all(temp_dir.path().join(project)).unwrap();
            create_and_write_file(&temp_dir.path().join(project).join("Cargo.toml"),
                &format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"", project)).unwrap();
        }
        
        // Create release directory and executables for only 2 of 3 projects
        let release_dir = temp_dir.path().join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        
        let exe_a = if cfg!(windows) { "project-a.exe" } else { "project-a" };
        let exe_c = if cfg!(windows) { "project-c.exe" } else { "project-c" };
        
        create_and_write_file(&release_dir.join(exe_a), "fake executable").unwrap();
        create_and_write_file(&release_dir.join(exe_c), "fake executable").unwrap();
        // Note: project-b executable deliberately not created
        
        let result = find_built_executables(temp_dir.path(), &toml::from_str(
            "[workspace]\nmembers = [\"project-a\", \"project-b\", \"project-c\"]"
        ).unwrap());
        
        assert!(result.is_ok());
        let executables = result.unwrap();
        assert_eq!(executables.len(), 2);
        assert!(executables.contains(&"project-a".to_string()));
        assert!(executables.contains(&"project-c".to_string()));
        assert!(!executables.contains(&"project-b".to_string()));
    }

    #[test]
    fn test_mixed_workspace_and_package() {
        let temp_dir = tempdir().unwrap();
        
        // Create mixed workspace+package Cargo.toml
        create_and_write_file(&temp_dir.path().join("Cargo.toml"), 
            "[package]\nname = \"root-package\"\nversion = \"0.1.0\"\n\n[workspace]\nmembers = [\"sub-project\"]").unwrap();
        
        // Create member directory and Cargo.toml
        fs::create_dir_all(temp_dir.path().join("sub-project")).unwrap();
        create_and_write_file(&temp_dir.path().join("sub-project").join("Cargo.toml"),
            "[package]\nname = \"sub-project\"\nversion = \"0.1.0\"").unwrap();
        
        // Create release directory and executables for both root and member
        let release_dir = temp_dir.path().join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        
        let exe_root = if cfg!(windows) { "root-package.exe" } else { "root-package" };
        let exe_sub = if cfg!(windows) { "sub-project.exe" } else { "sub-project" };
        
        create_and_write_file(&release_dir.join(exe_root), "fake executable").unwrap();
        create_and_write_file(&release_dir.join(exe_sub), "fake executable").unwrap();
        
        let result = find_built_executables(temp_dir.path(), &toml::from_str(
            "[package]\nname = \"root-package\"\nversion = \"0.1.0\"\n\n[workspace]\nmembers = [\"sub-project\"]"
        ).unwrap());
        
        assert!(result.is_ok());
        let executables = result.unwrap();
        assert_eq!(executables.len(), 2);
        assert!(executables.contains(&"root-package".to_string()));
        assert!(executables.contains(&"sub-project".to_string()));
    }

    #[test]
    fn test_workspace_no_built_executables() {
        let temp_dir = tempdir().unwrap();
        
        // Create workspace Cargo.toml
        create_and_write_file(&temp_dir.path().join("Cargo.toml"), 
            "[workspace]\nmembers = [\"project-a\"]").unwrap();
        
        // Create member directory and Cargo.toml
        fs::create_dir_all(temp_dir.path().join("project-a")).unwrap();
        create_and_write_file(&temp_dir.path().join("project-a").join("Cargo.toml"),
            "[package]\nname = \"project-a\"\nversion = \"0.1.0\"").unwrap();
        
        // Create release directory but no executables
        let release_dir = temp_dir.path().join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        
        let result = run(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No built release executables found"));
    }
}
