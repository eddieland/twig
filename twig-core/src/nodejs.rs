//! # Node.js Tooling Integration
//!
//! Provides detection and integration with Node.js tooling including package.json
//! parsing, npm/yarn script detection, and Node.js-specific gitignore patterns.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Represents a parsed package.json file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageJson {
  pub name: Option<String>,
  pub version: Option<String>,
  pub description: Option<String>,
  pub scripts: Option<std::collections::HashMap<String, String>>,
  pub dependencies: Option<std::collections::HashMap<String, String>>,
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<std::collections::HashMap<String, String>>,
  pub engines: Option<std::collections::HashMap<String, String>>,
  pub main: Option<String>,
  pub r#type: Option<String>, // "module" or "commonjs"
}

/// Node.js package manager types
#[derive(Debug, Clone, PartialEq)]
pub enum PackageManager {
  Npm,
  Yarn,
  Pnpm,
}

/// Node.js project detection and tooling integration
pub struct NodeJsTooling;

impl NodeJsTooling {
  /// Detect if the given path contains a Node.js project
  pub fn detect_project<P: AsRef<Path>>(path: P) -> bool {
    let package_json_path = path.as_ref().join("package.json");
    package_json_path.exists()
  }

  /// Parse package.json from the given path
  pub fn parse_package_json<P: AsRef<Path>>(path: P) -> Result<PackageJson> {
    let package_json_path = path.as_ref().join("package.json");
    
    if !package_json_path.exists() {
      anyhow::bail!("package.json not found at {:?}", package_json_path);
    }

    let content = fs::read_to_string(&package_json_path)
      .with_context(|| format!("Failed to read package.json from {:?}", package_json_path))?;

    let package_json: PackageJson = serde_json::from_str(&content)
      .with_context(|| format!("Failed to parse package.json from {:?}", package_json_path))?;

    Ok(package_json)
  }

  /// Detect which package manager is used in the project
  pub fn detect_package_manager<P: AsRef<Path>>(path: P) -> Option<PackageManager> {
    let path = path.as_ref();
    
    // Check for lock files and other indicators
    if path.join("pnpm-lock.yaml").exists() {
      return Some(PackageManager::Pnpm);
    }
    
    if path.join("yarn.lock").exists() {
      return Some(PackageManager::Yarn);
    }
    
    if path.join("package-lock.json").exists() {
      return Some(PackageManager::Npm);
    }

    // Check for package manager specific configs
    if path.join(".pnpmfile.cjs").exists() || path.join("pnpm-workspace.yaml").exists() {
      return Some(PackageManager::Pnpm);
    }

    if path.join(".yarnrc").exists() || path.join(".yarnrc.yml").exists() {
      return Some(PackageManager::Yarn);
    }

    // Default to npm if package.json exists but no other indicators
    if path.join("package.json").exists() {
      Some(PackageManager::Npm)
    } else {
      None
    }
  }

  /// Get commonly used npm/yarn scripts from package.json
  pub fn get_available_scripts<P: AsRef<Path>>(path: P) -> Result<Vec<(String, String)>> {
    let package_json = Self::parse_package_json(path)?;
    
    match package_json.scripts {
      Some(scripts) => Ok(scripts.into_iter().collect()),
      None => Ok(Vec::new()),
    }
  }

  /// Get Node.js specific gitignore patterns
  pub fn get_nodejs_gitignore_patterns() -> Vec<&'static str> {
    vec![
      "# Node.js dependencies",
      "node_modules/",
      "",
      "# Build outputs",
      "dist/",
      "build/",
      ".next/",
      ".nuxt/",
      ".output/",
      "out/",
      "",
      "# Package manager files",
      "npm-debug.log*",
      "yarn-debug.log*",
      "yarn-error.log*", 
      "lerna-debug.log*",
      ".pnpm-debug.log*",
      "",
      "# Environment files",
      ".env",
      ".env.local",
      ".env.development.local",
      ".env.test.local",
      ".env.production.local",
      "",
      "# Cache directories",
      ".npm",
      ".yarn/cache",
      ".yarn/unplugged",
      ".yarn/build-state.yml",
      ".yarn/install-state.gz",
      ".pnp.*",
      "",
      "# Coverage directory used by tools like istanbul",
      "coverage/",
      "*.lcov",
      "",
      "# Editor directories and files",
      ".vscode/",
      ".idea/",
      "*.suo",
      "*.ntvs*",
      "*.njsproj",
      "*.sln",
      "*.sw?",
      "",
      "# OS generated files",
      ".DS_Store",
      ".DS_Store?",
      "._*",
      ".Spotlight-V100",
      ".Trashes",
      "ehthumbs.db",
      "Thumbs.db",
    ]
  }

  /// Add Node.js patterns to gitignore if Node.js project is detected
  pub fn enhance_gitignore<P: AsRef<Path>>(repo_path: P) -> Result<bool> {
    let repo_path = repo_path.as_ref();
    
    // Only enhance if this is a Node.js project
    if !Self::detect_project(repo_path) {
      return Ok(false);
    }

    let gitignore_path = repo_path.join(".gitignore");
    let mut gitignore_content = String::new();

    // Read existing gitignore if it exists
    if gitignore_path.exists() {
      gitignore_content = fs::read_to_string(&gitignore_path)
        .context("Failed to read .gitignore file")?;
    }

    // Check if Node.js patterns are already present
    let has_nodejs_patterns = gitignore_content.contains("node_modules/") ||
                             gitignore_content.contains("# Node.js dependencies");

    if !has_nodejs_patterns {
      // Add a separator if file is not empty
      if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
        gitignore_content.push('\n');
      }
      
      if !gitignore_content.is_empty() {
        gitignore_content.push('\n');
      }

      // Add Node.js patterns
      for pattern in Self::get_nodejs_gitignore_patterns() {
        gitignore_content.push_str(pattern);
        gitignore_content.push('\n');
      }

      fs::write(&gitignore_path, gitignore_content)
        .context("Failed to update .gitignore file")?;
        
      return Ok(true);
    }

    Ok(false)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::TempDir;

  #[test]
  fn test_detect_project() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // Should not detect without package.json
    assert!(!NodeJsTooling::detect_project(project_path));

    // Create package.json
    let package_json_path = project_path.join("package.json");
    fs::write(&package_json_path, r#"{"name": "test-project"}"#).unwrap();

    // Should detect with package.json
    assert!(NodeJsTooling::detect_project(project_path));
  }

  #[test]
  fn test_parse_package_json() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();
    let package_json_path = project_path.join("package.json");

    let package_json_content = r#"{
      "name": "test-project",
      "version": "1.0.0",
      "description": "A test project",
      "scripts": {
        "build": "webpack",
        "test": "jest"
      },
      "dependencies": {
        "express": "^4.18.0"
      }
    }"#;

    fs::write(&package_json_path, package_json_content).unwrap();

    let parsed = NodeJsTooling::parse_package_json(project_path).unwrap();
    assert_eq!(parsed.name, Some("test-project".to_string()));
    assert_eq!(parsed.version, Some("1.0.0".to_string()));
    
    let scripts = parsed.scripts.unwrap();
    assert_eq!(scripts.get("build"), Some(&"webpack".to_string()));
    assert_eq!(scripts.get("test"), Some(&"jest".to_string()));
  }

  #[test]
  fn test_detect_package_manager() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // Create package.json first
    fs::write(project_path.join("package.json"), "{}").unwrap();

    // Test pnpm detection
    fs::write(project_path.join("pnpm-lock.yaml"), "").unwrap();
    assert_eq!(NodeJsTooling::detect_package_manager(project_path), Some(PackageManager::Pnpm));
    fs::remove_file(project_path.join("pnpm-lock.yaml")).unwrap();

    // Test yarn detection
    fs::write(project_path.join("yarn.lock"), "").unwrap();
    assert_eq!(NodeJsTooling::detect_package_manager(project_path), Some(PackageManager::Yarn));
    fs::remove_file(project_path.join("yarn.lock")).unwrap();

    // Test npm detection
    fs::write(project_path.join("package-lock.json"), "").unwrap();
    assert_eq!(NodeJsTooling::detect_package_manager(project_path), Some(PackageManager::Npm));
    fs::remove_file(project_path.join("package-lock.json")).unwrap();

    // Test default npm detection with just package.json
    assert_eq!(NodeJsTooling::detect_package_manager(project_path), Some(PackageManager::Npm));
  }

  #[test]
  fn test_get_available_scripts() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();
    let package_json_path = project_path.join("package.json");

    let package_json_content = r#"{
      "scripts": {
        "build": "webpack --mode production",
        "dev": "webpack serve --mode development",
        "test": "jest",
        "lint": "eslint src/"
      }
    }"#;

    fs::write(&package_json_path, package_json_content).unwrap();

    let scripts = NodeJsTooling::get_available_scripts(project_path).unwrap();
    assert_eq!(scripts.len(), 4);
    
    let script_map: std::collections::HashMap<_, _> = scripts.into_iter().collect();
    assert_eq!(script_map.get("build"), Some(&"webpack --mode production".to_string()));
    assert_eq!(script_map.get("dev"), Some(&"webpack serve --mode development".to_string()));
  }

  #[test]
  fn test_enhance_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // Create package.json to make it a Node.js project
    fs::write(project_path.join("package.json"), r#"{"name": "test"}"#).unwrap();

    // Should enhance gitignore
    let enhanced = NodeJsTooling::enhance_gitignore(project_path).unwrap();
    assert!(enhanced);

    // Check that gitignore contains Node.js patterns
    let gitignore_content = fs::read_to_string(project_path.join(".gitignore")).unwrap();
    assert!(gitignore_content.contains("node_modules/"));
    assert!(gitignore_content.contains("# Node.js dependencies"));

    // Should not enhance again
    let enhanced_again = NodeJsTooling::enhance_gitignore(project_path).unwrap();
    assert!(!enhanced_again);
  }

  #[test]
  fn test_enhance_gitignore_existing_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // Create package.json and existing gitignore
    fs::write(project_path.join("package.json"), r#"{"name": "test"}"#).unwrap();
    fs::write(project_path.join(".gitignore"), "# Existing content\n*.log\n").unwrap();

    let enhanced = NodeJsTooling::enhance_gitignore(project_path).unwrap();
    assert!(enhanced);

    let gitignore_content = fs::read_to_string(project_path.join(".gitignore")).unwrap();
    assert!(gitignore_content.contains("# Existing content"));
    assert!(gitignore_content.contains("*.log"));
    assert!(gitignore_content.contains("node_modules/"));
  }

  #[test]
  fn test_no_enhance_without_nodejs_project() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // No package.json, so not a Node.js project
    let enhanced = NodeJsTooling::enhance_gitignore(project_path).unwrap();
    assert!(!enhanced);
    
    // Should not create gitignore file
    assert!(!project_path.join(".gitignore").exists());
  }
}