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

/// Options controlling how the `twig self update` command behaves.
#[derive(Debug, Clone)]
pub struct SelfUpdateOptions {
  /// Install the latest release even if the current version matches.
  pub force: bool,
}

/// Options controlling how `twig self update flow` behaves.
#[derive(Debug, Clone)]
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

/// Constructs an HTTP client configured with a descriptive User-Agent header.
fn build_http_client() -> Result<Client> {
  Client::builder()
    .user_agent(format!(
      "twig/{version} (self-update)",
      version = env!("CARGO_PKG_VERSION")
    ))
    .build()
    .context("Failed to construct HTTP client")
}

/// A GitHub release returned by the Releases API.
#[derive(Debug, Deserialize)]
struct GithubRelease {
  /// The release tag, typically in the form `vX.Y.Z`.
  tag_name: String,
  /// Downloadable assets attached to the release.
  assets: Vec<GithubAsset>,
}

impl GithubRelease {
  /// Returns the version string without a leading `v` prefix.
  fn clean_tag(&self) -> String {
    self.tag_name.trim_start_matches('v').to_string()
  }

  /// Finds the first asset whose name matches the given [`TargetConfig`].
  fn find_matching_asset<'a>(&'a self, target: &TargetConfig) -> Option<&'a GithubAsset> {
    self.assets.iter().find(|asset| target.matches(asset))
  }
}

/// A downloadable asset attached to a GitHub release.
#[derive(Debug, Deserialize)]
struct GithubAsset {
  /// Filename of the asset (e.g., `twig-linux-x86_64-v0.5.0.tar.gz`).
  name: String,
  /// Direct download URL for the asset.
  browser_download_url: String,
}

/// Fetches the latest Twig release metadata from the GitHub Releases API.
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

/// Platform-specific configuration for selecting and extracting release assets.
///
/// This struct encapsulates the conventions used to name release archives so
/// that the correct asset can be selected for the current OS and architecture.
#[derive(Debug)]
struct TargetConfig {
  /// Substrings that identify a matching operating system (e.g., `["linux"]`).
  os_markers: Vec<&'static str>,
  /// Substrings that identify a matching CPU architecture (e.g., `["x86_64",
  /// "amd64"]`).
  arch_markers: Vec<&'static str>,
  /// Expected archive extension (e.g., `.tar.gz` or `.zip`).
  archive_extension: &'static str,
  /// Name of the binary inside the archive (includes `.exe` suffix on Windows).
  binary_name: String,
}

impl TargetConfig {
  /// Returns the product name without any platform-specific suffix.
  fn product_name(&self) -> &str {
    self.binary_name.strip_suffix(".exe").unwrap_or(&self.binary_name)
  }

  /// Returns `true` if the asset filename matches this target configuration.
  ///
  /// Matching is case-insensitive and checks the archive extension, product
  /// name, OS marker, and architecture marker. On macOS, `universal` builds
  /// are accepted for any architecture.
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

/// Builds a [`TargetConfig`] for the given binary name based on the current
/// platform.
///
/// Returns an error if the current operating system is not supported.
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

/// Creates a uniquely named temporary directory for staging downloaded files.
fn create_staging_directory() -> Result<PathBuf> {
  let staging = std::env::temp_dir().join(format!("twig-update-{}", Uuid::new_v4()));
  fs::create_dir_all(&staging).context("Failed to create staging directory")?;
  Ok(staging)
}

/// Downloads a release asset to the staging directory.
///
/// Returns the path to the downloaded archive file.
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

/// Extracts the target binary from an archive, dispatching to the appropriate
/// format handler.
///
/// Returns the path to the extracted binary.
fn extract_archive(archive_path: &Path, staging_root: &Path, target: &TargetConfig) -> Result<PathBuf> {
  match target.archive_extension {
    ".tar.gz" => extract_tarball(archive_path, staging_root, &target.binary_name),
    ".zip" => extract_zip(archive_path, staging_root, &target.binary_name),
    other => Err(anyhow!("Unsupported archive format: {other}")),
  }
}

/// Extracts a binary from a gzipped tarball.
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

/// Extracts a binary from a zip archive (used on Windows).
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

/// Indicates whether the binary installation completed immediately or was
/// deferred.
///
/// On Unix, installation is always immediate via atomic rename. On Windows, if
/// the running executable is locked, installation is deferred to a background
/// process that waits for Twig to exit before completing the replacement.
enum InstallOutcome {
  /// The new binary was installed immediately.
  #[cfg_attr(windows, allow(dead_code))]
  Immediate,
  /// Installation was deferred to a background process (Windows only).
  #[cfg(windows)]
  Deferred {
    /// Whether the deferred process required elevation.
    elevated: bool,
  },
}

/// Installs a new binary to replace the currently running Twig executable.
fn install_new_binary(binary_path: &Path) -> Result<InstallOutcome> {
  let current_exe = std::env::current_exe().context("Failed to locate current executable")?;

  platform::install_new_binary(binary_path, &current_exe)
}

/// Installs a plugin binary to the specified path.
fn install_plugin_binary(binary_path: &Path, install_path: &Path) -> Result<InstallOutcome> {
  platform::install_new_binary(binary_path, install_path)
}

/// Returns the path where the flow plugin should be installed.
///
/// Places the plugin in the same directory as the Twig executable so it can
/// be discovered via PATH.
fn flow_plugin_install_path(target: &TargetConfig) -> Result<PathBuf> {
  let current_exe = std::env::current_exe().context("Failed to locate current executable")?;
  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow!("Executable path has no parent directory"))?;
  Ok(parent.join(&target.binary_name))
}

/// Reads the version of an installed plugin by invoking it with `--version`.
///
/// Returns `Ok(None)` if the plugin does not exist or cannot report its
/// version.
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

/// Parses a version number from command output like `twig-flow 0.2.3`.
///
/// Looks for the first whitespace-separated token that starts with an ASCII
/// digit, then strips any non-version characters from the ends.
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

/// Checks whether a directory is present in the `PATH` environment variable.
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
  use super::{GithubAsset, GithubRelease, TargetConfig, extract_version_from_output, path_contains_dir};

  fn linux_target(binary_name: &str) -> TargetConfig {
    TargetConfig {
      os_markers: vec!["linux"],
      arch_markers: vec!["x86_64", "amd64"],
      archive_extension: ".tar.gz",
      binary_name: binary_name.to_string(),
    }
  }

  fn macos_target(binary_name: &str) -> TargetConfig {
    TargetConfig {
      os_markers: vec!["macos", "darwin"],
      arch_markers: vec!["aarch64", "arm64"],
      archive_extension: ".tar.gz",
      binary_name: binary_name.to_string(),
    }
  }

  fn windows_target(binary_name: &str) -> TargetConfig {
    TargetConfig {
      os_markers: vec!["windows"],
      arch_markers: vec!["x86_64", "amd64"],
      archive_extension: ".zip",
      binary_name: binary_name.to_string(),
    }
  }

  fn asset(name: &str) -> GithubAsset {
    GithubAsset {
      name: name.to_string(),
      browser_download_url: String::new(),
    }
  }

  fn release(tag_name: &str, assets: Vec<GithubAsset>) -> GithubRelease {
    GithubRelease {
      tag_name: tag_name.to_string(),
      assets,
    }
  }

  // GithubRelease::clean_tag tests
  #[test]
  fn clean_tag_strips_v_prefix() {
    let r = release("v1.2.3", vec![]);
    assert_eq!(r.clean_tag(), "1.2.3");
  }

  #[test]
  fn clean_tag_handles_no_prefix() {
    let r = release("1.2.3", vec![]);
    assert_eq!(r.clean_tag(), "1.2.3");
  }

  #[test]
  fn clean_tag_strips_multiple_v_prefixes() {
    let r = release("vvv1.0.0", vec![]);
    assert_eq!(r.clean_tag(), "1.0.0");
  }

  // GithubRelease::find_matching_asset tests
  #[test]
  fn find_matching_asset_returns_match() {
    let assets = vec![
      asset("twig-linux-x86_64-v0.1.0.tar.gz"),
      asset("twig-windows-x86_64-v0.1.0.zip"),
    ];
    let r = release("v0.1.0", assets);
    let target = linux_target("twig");
    let found = r.find_matching_asset(&target);
    assert!(found.is_some());
    assert_eq!(found.map(|a| a.name.as_str()), Some("twig-linux-x86_64-v0.1.0.tar.gz"));
  }

  #[test]
  fn find_matching_asset_returns_none_when_no_match() {
    let assets = vec![asset("twig-windows-x86_64-v0.1.0.zip")];
    let r = release("v0.1.0", assets);
    let target = linux_target("twig");
    assert!(r.find_matching_asset(&target).is_none());
  }

  // TargetConfig::product_name tests
  #[test]
  fn product_name_strips_exe_suffix() {
    let target = windows_target("twig.exe");
    assert_eq!(target.product_name(), "twig");
  }

  #[test]
  fn product_name_returns_name_without_exe() {
    let target = linux_target("twig");
    assert_eq!(target.product_name(), "twig");
  }

  #[test]
  fn product_name_handles_hyphenated_binary() {
    let target = linux_target("twig-flow");
    assert_eq!(target.product_name(), "twig-flow");
  }

  // TargetConfig::matches tests - Linux
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
  fn linux_rejects_wrong_extension() {
    let target = linux_target("twig");
    assert!(!target.matches(&asset("twig-linux-x86_64-v0.1.0.zip")));
  }

  #[test]
  fn linux_rejects_wrong_os() {
    let target = linux_target("twig");
    assert!(!target.matches(&asset("twig-windows-x86_64-v0.1.0.tar.gz")));
  }

  #[test]
  fn linux_rejects_wrong_arch() {
    let target = linux_target("twig");
    assert!(!target.matches(&asset("twig-linux-aarch64-v0.1.0.tar.gz")));
  }

  #[test]
  fn linux_accepts_amd64_alias() {
    let target = linux_target("twig");
    assert!(target.matches(&asset("twig-linux-amd64-v0.1.0.tar.gz")));
  }

  // TargetConfig::matches tests - macOS
  #[test]
  fn macos_accepts_darwin_marker() {
    let target = macos_target("twig");
    assert!(target.matches(&asset("twig-darwin-arm64-v0.1.0.tar.gz")));
  }

  #[test]
  fn macos_accepts_macos_marker() {
    let target = macos_target("twig");
    assert!(target.matches(&asset("twig-macos-arm64-v0.1.0.tar.gz")));
  }

  #[test]
  fn macos_accepts_universal_build() {
    let target = macos_target("twig");
    assert!(target.matches(&asset("twig-macos-universal-v0.1.0.tar.gz")));
  }

  #[test]
  fn macos_accepts_aarch64_alias() {
    let target = macos_target("twig");
    assert!(target.matches(&asset("twig-macos-aarch64-v0.1.0.tar.gz")));
  }

  // TargetConfig::matches tests - Windows
  #[test]
  fn windows_selects_zip_asset() {
    let target = windows_target("twig.exe");
    assert!(target.matches(&asset("twig-windows-x86_64-v0.1.0.zip")));
  }

  #[test]
  fn windows_rejects_tarball() {
    let target = windows_target("twig.exe");
    assert!(!target.matches(&asset("twig-windows-x86_64-v0.1.0.tar.gz")));
  }

  // TargetConfig::matches tests - case insensitivity
  #[test]
  fn matches_is_case_insensitive() {
    let target = linux_target("twig");
    assert!(target.matches(&asset("TWIG-LINUX-X86_64-V0.1.0.TAR.GZ")));
  }

  #[test]
  fn matches_handles_mixed_case() {
    let target = linux_target("twig");
    assert!(target.matches(&asset("Twig-Linux-X86_64-v0.1.0.tar.gz")));
  }

  // TargetConfig::matches tests - edge cases
  #[test]
  fn rejects_asset_with_too_few_parts() {
    let target = linux_target("twig");
    assert!(!target.matches(&asset("twig-linux.tar.gz")));
  }

  #[test]
  fn rejects_empty_asset_name() {
    let target = linux_target("twig");
    assert!(!target.matches(&asset("")));
  }

  #[test]
  fn universal_not_accepted_for_linux() {
    // Universal builds are macOS-specific
    let target = linux_target("twig");
    assert!(!target.matches(&asset("twig-linux-universal-v0.1.0.tar.gz")));
  }

  // extract_version_from_output tests
  #[test]
  fn parses_version_from_output() {
    assert_eq!(
      extract_version_from_output("twig-flow 0.2.3\n").as_deref(),
      Some("0.2.3")
    );
  }

  #[test]
  fn parses_version_with_leading_text() {
    assert_eq!(extract_version_from_output("version 1.0.0").as_deref(), Some("1.0.0"));
  }

  #[test]
  fn parses_version_trims_trailing_chars() {
    assert_eq!(extract_version_from_output("twig 1.2.3-beta").as_deref(), Some("1.2.3"));
  }

  #[test]
  fn version_returns_none_for_empty_output() {
    assert_eq!(extract_version_from_output(""), None);
  }

  #[test]
  fn version_returns_none_for_no_version() {
    assert_eq!(extract_version_from_output("no version here"), None);
  }

  #[test]
  fn version_returns_none_for_only_letters() {
    assert_eq!(extract_version_from_output("twig flow"), None);
  }

  #[test]
  fn parses_version_at_start() {
    assert_eq!(
      extract_version_from_output("1.0.0 is the version").as_deref(),
      Some("1.0.0")
    );
  }

  #[test]
  fn parses_multiline_version_output() {
    assert_eq!(
      extract_version_from_output("twig-flow\nversion 2.0.0\nbuilt on date").as_deref(),
      Some("2.0.0")
    );
  }

  // path_contains_dir tests
  #[test]
  fn path_contains_dir_returns_false_for_none() {
    assert!(!path_contains_dir(None));
  }

  #[test]
  fn path_contains_dir_returns_false_for_missing_dir() {
    use std::path::Path;
    // Use a path that almost certainly isn't in PATH
    let fake_dir = Path::new("/this/path/should/not/exist/in/path/env/var");
    assert!(!path_contains_dir(Some(fake_dir)));
  }

  #[test]
  fn selects_twig_when_flow_comes_first() {
    // Simulates the exact v0.5.2 release asset ordering
    let assets = vec![
      // twig-flow assets come FIRST (alphabetically/by upload order)
      asset("twig-flow-linux-x86_64-v0.5.2.tar.gz"),
      asset("twig-flow-linux-x86_64-v0.5.2.tar.gz.sha256"),
      asset("twig-flow-macos-x86_64-v0.5.2.tar.gz"),
      asset("twig-flow-macos-x86_64-v0.5.2.tar.gz.sha256"),
      asset("twig-flow-windows-x86_64-v0.5.2.zip"),
      asset("twig-flow-windows-x86_64-v0.5.2.zip.sha256"),
      // twig assets come AFTER
      asset("twig-linux-x86_64-v0.5.2.tar.gz"),
      asset("twig-linux-x86_64-v0.5.2.tar.gz.sha256"),
      asset("twig-macos-x86_64-v0.5.2.tar.gz"),
      asset("twig-macos-x86_64-v0.5.2.tar.gz.sha256"),
      asset("twig-windows-x86_64-v0.5.2.zip"),
      asset("twig-windows-x86_64-v0.5.2.zip.sha256"),
    ];
    let r = release("v0.5.2", assets);
    let target = linux_target("twig");
    let found = r.find_matching_asset(&target);
    assert!(found.is_some(), "Should find an asset");
    assert_eq!(
      found.map(|a| a.name.as_str()),
      Some("twig-linux-x86_64-v0.5.2.tar.gz"),
      "Should select twig, not twig-flow"
    );
  }

  #[test]
  fn selects_twig_flow_when_requested() {
    // When we actually want twig-flow, it should be selected correctly
    let assets = vec![
      asset("twig-flow-linux-x86_64-v0.5.2.tar.gz"),
      asset("twig-linux-x86_64-v0.5.2.tar.gz"),
    ];
    let r = release("v0.5.2", assets);
    let target = linux_target("twig-flow");
    let found = r.find_matching_asset(&target);
    assert!(found.is_some(), "Should find twig-flow asset");
    assert_eq!(
      found.map(|a| a.name.as_str()),
      Some("twig-flow-linux-x86_64-v0.5.2.tar.gz"),
      "Should select twig-flow"
    );
  }

  #[test]
  fn twig_flow_does_not_match_twig_target() {
    // Explicit check that twig-flow asset doesn't match twig target
    let target = linux_target("twig");
    assert!(
      !target.matches(&asset("twig-flow-linux-x86_64-v0.5.2.tar.gz")),
      "twig-flow asset should not match twig target"
    );
  }

  #[test]
  fn twig_does_not_match_twig_flow_target() {
    // And vice versa
    let target = linux_target("twig-flow");
    assert!(
      !target.matches(&asset("twig-linux-x86_64-v0.5.2.tar.gz")),
      "twig asset should not match twig-flow target"
    );
  }

  #[test]
  fn rejects_hypothetical_twig_plugin_when_looking_for_twig() {
    // Test with hypothetical plugins to ensure the matching is strict
    let target = linux_target("twig");
    // These should all be rejected
    assert!(!target.matches(&asset("twig-flow-linux-x86_64-v0.5.2.tar.gz")));
    assert!(!target.matches(&asset("twig-extra-linux-x86_64-v0.5.2.tar.gz")));
    assert!(!target.matches(&asset("twig-pro-linux-x86_64-v0.5.2.tar.gz")));
    // But the actual twig asset should match
    assert!(target.matches(&asset("twig-linux-x86_64-v0.5.2.tar.gz")));
  }

  #[test]
  fn handles_multi_hyphen_plugin_names() {
    // Test with a hypothetical multi-hyphen plugin name
    let target = linux_target("twig-flow-extra");
    assert!(target.matches(&asset("twig-flow-extra-linux-x86_64-v0.5.2.tar.gz")));
    // Should not match shorter prefixes
    assert!(!target.matches(&asset("twig-flow-linux-x86_64-v0.5.2.tar.gz")));
    assert!(!target.matches(&asset("twig-linux-x86_64-v0.5.2.tar.gz")));
  }

  #[test]
  fn windows_selects_correct_asset_with_mixed_plugins() {
    let assets = vec![
      asset("twig-flow-windows-x86_64-v0.5.2.zip"),
      asset("twig-windows-x86_64-v0.5.2.zip"),
    ];
    let r = release("v0.5.2", assets);
    let target = windows_target("twig.exe");
    let found = r.find_matching_asset(&target);
    assert_eq!(
      found.map(|a| a.name.as_str()),
      Some("twig-windows-x86_64-v0.5.2.zip"),
      "Should select twig.zip, not twig-flow.zip"
    );
  }

  #[test]
  fn macos_selects_correct_asset_with_mixed_plugins() {
    let assets = vec![
      asset("twig-flow-macos-arm64-v0.5.2.tar.gz"),
      asset("twig-macos-arm64-v0.5.2.tar.gz"),
    ];
    let r = release("v0.5.2", assets);
    let target = macos_target("twig");
    let found = r.find_matching_asset(&target);
    assert_eq!(
      found.map(|a| a.name.as_str()),
      Some("twig-macos-arm64-v0.5.2.tar.gz"),
      "Should select twig, not twig-flow on macOS"
    );
  }
}
