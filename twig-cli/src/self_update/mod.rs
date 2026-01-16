//! Self-update helpers for the `twig self update` command.
//!
//! This module downloads the latest Twig release from GitHub, extracts the
//! platform-appropriate archive, and replaces the currently running binary in a
//! safe and platform-aware manner. Platform-specific installation steps live in
//! dedicated helpers to keep the main workflow cross-platform.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Deserialize;
use tar::Archive;
use twig_core::output::{print_info, print_success, print_warning};
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone)]
/// Options controlling how the `twig self update` command behaves.
pub struct SelfUpdateOptions {
  /// Install the latest release even if the current version matches.
  pub force: bool,
}

#[derive(Debug, Clone)]
/// Options controlling how `twig self flow` behaves.
pub struct PluginInstallOptions {
  /// Reinstall even if the installed plugin already matches the latest release.
  pub force: bool,
}

/// Download and install the latest Twig release for the current platform.
///
/// When `force` is false, the function exits early if the running version
/// already matches the newest GitHub release tag. Otherwise it downloads the
/// platform-appropriate archive, extracts the binary into a temporary staging
/// area, and delegates to platform helpers to atomically replace the current
/// executable.
pub fn run(options: SelfUpdateOptions) -> Result<()> {
  let current_version = env!("CARGO_PKG_VERSION").to_string();
  print_info(&format!("Checking for updates (current version {current_version})…"));

  let client = build_http_client()?;
  let release = fetch_latest_release(&client)?;
  let target = target_config("twig")?;
  let latest_version = release.clean_tag();

  if !options.force && latest_version == current_version {
    print_success("You're already running the latest version of Twig.");
    return Ok(());
  }

  let asset = release
    .find_matching_asset(&target)
    .ok_or_else(|| anyhow!("No release asset available for this platform"))?;

  print_info(&format!("Downloading Twig {latest_version} ({})…", asset.name));

  let staging_root = create_staging_directory()?;
  let archive_path = download_asset(&client, asset, &staging_root)?;
  let binary_path = extract_archive(&archive_path, &staging_root, &target)?;

  print_info("Installing update…");
  let outcome = install_new_binary(&binary_path)?;

  if let Err(err) = fs::remove_dir_all(&staging_root) {
    print_warning(&format!("Failed to clean temporary files: {err}"));
  }

  match outcome {
    InstallOutcome::Immediate => {
      print_success(&format!("Twig has been updated to version {latest_version}."));
    }
    #[cfg(windows)]
    InstallOutcome::Deferred { elevated } => {
      if elevated {
        print_info("An elevated PowerShell helper will finish applying the update once Twig exits.");
      } else {
        print_info("A background PowerShell helper will finish applying the update once Twig exits.");
      }
      print_success(&format!(
        "Twig {latest_version} is staged and will complete installation shortly."
      ));
    }
  }

  Ok(())
}

/// Download and install the latest Twig flow plugin release.
///
/// The plugin binary is placed alongside the running Twig executable so it can
/// be discovered via standard PATH lookups.
pub fn run_flow_plugin_install(options: PluginInstallOptions) -> Result<()> {
  let client = build_http_client()?;
  let release = fetch_latest_release(&client)?;
  let target = target_config("twig-flow")?;
  let latest_version = release.clean_tag();
  let install_path = flow_plugin_install_path(&target)?;

  if !options.force
    && let Some(installed_version) = read_installed_plugin_version(&install_path)?
    && installed_version == latest_version
  {
    print_success("Twig flow plugin is already up to date.");
    return Ok(());
  }

  let asset = release
    .find_matching_asset(&target)
    .ok_or_else(|| anyhow!("No Twig flow plugin asset available for this platform"))?;

  print_info(&format!("Downloading Twig flow {latest_version} ({})…", asset.name));

  let staging_root = create_staging_directory()?;
  let archive_path = download_asset(&client, asset, &staging_root)?;
  let binary_path = extract_archive(&archive_path, &staging_root, &target)?;

  print_info("Installing Twig flow plugin…");
  let outcome = install_plugin_binary(&binary_path, &install_path)?;

  if let Err(err) = fs::remove_dir_all(&staging_root) {
    print_warning(&format!("Failed to clean temporary files: {err}"));
  }

  if !path_contains_dir(install_path.parent()) {
    print_warning(&format!(
      "The plugin was installed to {} which is not on your PATH.",
      install_path.display()
    ));
  }

  match outcome {
    InstallOutcome::Immediate => {
      print_success(&format!(
        "Twig flow {latest_version} is installed at {}.",
        install_path.display()
      ));
    }
    #[cfg(windows)]
    InstallOutcome::Deferred { elevated } => {
      if elevated {
        print_info("An elevated PowerShell helper will finish applying the update once Twig exits.");
      } else {
        print_info("A background PowerShell helper will finish applying the update once Twig exits.");
      }
      print_success(&format!(
        "Twig flow {latest_version} is staged and will complete installation shortly."
      ));
    }
  }

  Ok(())
}

fn build_http_client() -> Result<Client> {
  Client::builder()
    .user_agent(format!(
      "twig/{version} (self-update)",
      version = env!("CARGO_PKG_VERSION")
    ))
    .build()
    .context("Failed to construct HTTP client")
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
  tag_name: String,
  assets: Vec<GithubAsset>,
}

impl GithubRelease {
  fn clean_tag(&self) -> String {
    self.tag_name.trim_start_matches('v').to_string()
  }

  fn find_matching_asset<'a>(&'a self, target: &TargetConfig) -> Option<&'a GithubAsset> {
    self.assets.iter().find(|asset| target.matches(asset))
  }
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
  name: String,
  browser_download_url: String,
}

fn fetch_latest_release(client: &Client) -> Result<GithubRelease> {
  client
    .get("https://api.github.com/repos/eddieland/twig/releases/latest")
    .send()
    .context("Failed to query GitHub Releases")?
    .error_for_status()
    .context("GitHub Releases request was not successful")?
    .json::<GithubRelease>()
    .context("Failed to deserialize GitHub Releases response")
}

#[derive(Debug)]
struct TargetConfig {
  os_markers: Vec<&'static str>,
  arch_markers: Vec<&'static str>,
  archive_extension: &'static str,
  binary_name: String,
}

impl TargetConfig {
  fn product_name(&self) -> &str {
    self.binary_name.strip_suffix(".exe").unwrap_or(&self.binary_name)
  }

  fn matches(&self, asset: &GithubAsset) -> bool {
    let name = asset.name.to_lowercase();
    if !name.ends_with(self.archive_extension) {
      return false;
    }

    let trimmed = name.strip_suffix(self.archive_extension).unwrap_or(&name);
    let parts: Vec<_> = trimmed.split('-').collect();
    let product_parts: Vec<_> = self.product_name().split('-').collect();
    let expected_parts_len = product_parts.len() + 2;
    if parts.len() < expected_parts_len {
      return false;
    }

    let product_segment = product_parts.join("-");
    let asset_product = parts[..product_parts.len()].join("-");
    if asset_product != product_segment {
      return false;
    }

    let os_segment = parts[product_parts.len()];
    let os_match = self.os_markers.iter().any(|marker| os_segment.contains(marker));
    if !os_match {
      return false;
    }

    let arch_segment = parts[product_parts.len() + 1];

    let mut arch_match = self.arch_markers.iter().any(|marker| arch_segment.contains(marker));

    if !arch_match && self.os_markers.iter().any(|m| *m == "macos" || *m == "darwin") {
      // macOS universal builds should work for both x86_64 and arm64.
      arch_match = arch_segment.contains("universal");
    }

    arch_match
  }
}

fn target_config(binary_name: &str) -> Result<TargetConfig> {
  let arch_markers = match std::env::consts::ARCH {
    "x86_64" => vec!["x86_64", "amd64"],
    "aarch64" => vec!["aarch64", "arm64"],
    other => vec![other],
  };

  match std::env::consts::OS {
    "linux" => Ok(TargetConfig {
      os_markers: vec!["linux"],
      arch_markers,
      archive_extension: ".tar.gz",
      binary_name: binary_name.to_string(),
    }),
    "macos" => Ok(TargetConfig {
      os_markers: vec!["macos", "darwin"],
      arch_markers,
      archive_extension: ".tar.gz",
      binary_name: binary_name.to_string(),
    }),
    "windows" => Ok(TargetConfig {
      os_markers: vec!["windows"],
      arch_markers,
      archive_extension: ".zip",
      binary_name: if binary_name.ends_with(".exe") {
        binary_name.to_string()
      } else {
        format!("{binary_name}.exe")
      },
    }),
    other => Err(anyhow!("Unsupported operating system: {other}")),
  }
}

fn create_staging_directory() -> Result<PathBuf> {
  let staging = std::env::temp_dir().join(format!("twig-update-{}", Uuid::new_v4()));
  fs::create_dir_all(&staging).context("Failed to create staging directory")?;
  Ok(staging)
}

fn download_asset(client: &Client, asset: &GithubAsset, staging_root: &Path) -> Result<PathBuf> {
  let archive_path = staging_root.join(&asset.name);
  let mut response = client
    .get(&asset.browser_download_url)
    .send()
    .with_context(|| format!("Failed to download {}", asset.name))?
    .error_for_status()
    .with_context(|| format!("GitHub returned an error downloading {}", asset.name))?;

  let mut file = File::create(&archive_path).with_context(|| format!("Failed to create {}", archive_path.display()))?;
  io::copy(&mut response, &mut file).with_context(|| format!("Failed to write to {}", archive_path.display()))?;
  Ok(archive_path)
}

fn extract_archive(archive_path: &Path, staging_root: &Path, target: &TargetConfig) -> Result<PathBuf> {
  match target.archive_extension {
    ".tar.gz" => extract_tarball(archive_path, staging_root, &target.binary_name),
    ".zip" => extract_zip(archive_path, staging_root, &target.binary_name),
    other => Err(anyhow!("Unsupported archive format: {other}")),
  }
}

fn extract_tarball(archive_path: &Path, staging_root: &Path, binary_name: &str) -> Result<PathBuf> {
  let file = File::open(archive_path).with_context(|| format!("Failed to open {}", archive_path.display()))?;
  let decoder = GzDecoder::new(file);
  let mut archive = Archive::new(decoder);

  let mut extracted = None;
  for entry in archive.entries().context("Invalid tar archive")? {
    let mut entry = entry.context("Failed to read tar entry")?;
    let path = entry.path().context("Invalid path in tar archive")?;
    if path
      .file_name()
      .and_then(OsStr::to_str)
      .map(|name| name == binary_name)
      .unwrap_or(false)
    {
      let output_path = staging_root.join(binary_name);
      entry
        .unpack(&output_path)
        .with_context(|| format!("Failed to unpack {}", binary_name))?;
      extracted = Some(output_path);
      break;
    }
  }

  let extracted = extracted.ok_or_else(|| anyhow!("Binary {binary_name} not found in archive"))?;

  platform::finalize_extracted_binary(&extracted)?;
  Ok(extracted)
}

fn extract_zip(archive_path: &Path, staging_root: &Path, binary_name: &str) -> Result<PathBuf> {
  let file = File::open(archive_path).with_context(|| format!("Failed to open {}", archive_path.display()))?;
  let mut archive = ZipArchive::new(file).context("Invalid zip archive")?;

  for i in 0..archive.len() {
    let mut entry = archive.by_index(i).context("Failed to read zip entry")?;
    if entry.is_dir() {
      continue;
    }

    let entry_name = Path::new(entry.name())
      .file_name()
      .and_then(OsStr::to_str)
      .unwrap_or("");
    if entry_name != binary_name {
      continue;
    }

    let output_path = staging_root.join(binary_name);
    let mut output =
      File::create(&output_path).with_context(|| format!("Failed to create {}", output_path.display()))?;
    io::copy(&mut entry, &mut output).with_context(|| format!("Failed to extract {}", binary_name))?;
    platform::finalize_extracted_binary(&output_path)?;
    return Ok(output_path);
  }

  Err(anyhow!("Binary {binary_name} not found in archive"))
}

enum InstallOutcome {
  #[cfg_attr(windows, allow(dead_code))]
  Immediate,
  #[cfg(windows)]
  Deferred { elevated: bool },
}

fn install_new_binary(binary_path: &Path) -> Result<InstallOutcome> {
  let current_exe = std::env::current_exe().context("Failed to locate current executable")?;

  platform::install_new_binary(binary_path, &current_exe)
}

fn install_plugin_binary(binary_path: &Path, install_path: &Path) -> Result<InstallOutcome> {
  platform::install_new_binary(binary_path, install_path)
}

fn flow_plugin_install_path(target: &TargetConfig) -> Result<PathBuf> {
  let current_exe = std::env::current_exe().context("Failed to locate current executable")?;
  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow!("Executable path has no parent directory"))?;
  Ok(parent.join(&target.binary_name))
}

fn read_installed_plugin_version(path: &Path) -> Result<Option<String>> {
  if !path.exists() {
    return Ok(None);
  }

  let output = std::process::Command::new(path).arg("--version").output();
  let Ok(output) = output else {
    return Ok(None);
  };

  if !output.status.success() {
    return Ok(None);
  }

  let stdout = String::from_utf8_lossy(&output.stdout);
  Ok(extract_version_from_output(&stdout))
}

fn extract_version_from_output(output: &str) -> Option<String> {
  output
    .split_whitespace()
    .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
    .map(|token| {
      token
        .trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .to_string()
    })
}

fn path_contains_dir(directory: Option<&Path>) -> bool {
  let Some(directory) = directory else {
    return false;
  };
  let path_var = std::env::var_os("PATH");
  let Some(path_var) = path_var.as_deref() else {
    return false;
  };

  std::env::split_paths(path_var).any(|path| path == directory)
}

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(not(any(unix, windows)))]
mod platform {
  use std::path::Path;

  use anyhow::bail;

  use super::{InstallOutcome, Result};

  pub fn finalize_extracted_binary(_path: &Path) -> Result<()> {
    Ok(())
  }

  pub fn install_new_binary(_new_binary: &Path, _current_exe: &Path) -> Result<InstallOutcome> {
    bail!("Self-update is unsupported on this platform")
  }
}

#[cfg(test)]
mod tests {
  use super::{GithubAsset, TargetConfig, extract_version_from_output};

  fn linux_target(binary_name: &'static str) -> TargetConfig {
    TargetConfig {
      os_markers: vec!["linux"],
      arch_markers: vec!["x86_64", "amd64"],
      archive_extension: ".tar.gz",
      binary_name: binary_name.to_string(),
    }
  }

  fn asset(name: &str) -> GithubAsset {
    GithubAsset {
      name: name.to_string(),
      browser_download_url: String::new(),
    }
  }

  #[test]
  fn selects_primary_linux_asset() {
    let target = linux_target("twig");
    assert!(target.matches(&asset("twig-linux-x86_64-v0.1.0.tar.gz")));
  }

  #[test]
  fn ignores_twig_flow_asset() {
    let target = linux_target("twig");
    assert!(!target.matches(&asset("twig-flow-linux-x86_64-v0.1.0.tar.gz")));
  }

  #[test]
  fn selects_twig_flow_asset() {
    let target = linux_target("twig-flow");
    assert!(target.matches(&asset("twig-flow-linux-x86_64-v0.1.0.tar.gz")));
  }

  #[test]
  fn parses_version_from_output() {
    assert_eq!(
      extract_version_from_output("twig-flow 0.2.3\n").as_deref(),
      Some("0.2.3")
    );
  }
}
