//! Self-update helpers for the `twig self update` command.
//!
//! This module downloads the latest Twig release from GitHub, extracts the
//! platform-appropriate archive, and replaces the currently running binary in a
//! safe and platform-aware manner. Unix platforms perform an atomic rename,
//! while Windows stages an auxiliary PowerShell script that swaps binaries
//! after the current process exits.

use std::ffi::OsStr;
#[cfg(windows)]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::process::Stdio;

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Deserialize;
use tar::Archive;
use twig_core::output::{print_info, print_success, print_warning};
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct SelfUpdateOptions {
  pub force: bool,
}

pub fn run(options: SelfUpdateOptions) -> Result<()> {
  let current_version = env!("CARGO_PKG_VERSION").to_string();
  print_info(&format!("Checking for updates (current version {current_version})…"));

  let client = build_http_client()?;
  let release = fetch_latest_release(&client)?;
  let target = target_config()?;
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
  binary_name: &'static str,
}

impl TargetConfig {
  fn matches(&self, asset: &GithubAsset) -> bool {
    let name = asset.name.to_lowercase();
    if !name.ends_with(self.archive_extension) {
      return false;
    }

    let os_match = self.os_markers.iter().any(|marker| name.contains(marker));
    if !os_match {
      return false;
    }

    let mut arch_match = self.arch_markers.iter().any(|marker| name.contains(marker));

    if !arch_match && self.os_markers.iter().any(|m| *m == "macos" || *m == "darwin") {
      // macOS universal builds should work for both x86_64 and arm64.
      arch_match = name.contains("universal");
    }

    arch_match
  }
}

fn target_config() -> Result<TargetConfig> {
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
      binary_name: "twig",
    }),
    "macos" => Ok(TargetConfig {
      os_markers: vec!["macos", "darwin"],
      arch_markers,
      archive_extension: ".tar.gz",
      binary_name: "twig",
    }),
    "windows" => Ok(TargetConfig {
      os_markers: vec!["windows"],
      arch_markers,
      archive_extension: ".zip",
      binary_name: "twig.exe",
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
    ".tar.gz" => extract_tarball(archive_path, staging_root, target.binary_name),
    ".zip" => extract_zip(archive_path, staging_root, target.binary_name),
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

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&extracted)
      .with_context(|| format!("Failed to read permissions for {}", extracted.display()))?
      .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&extracted, perms)
      .with_context(|| format!("Failed to set permissions on {}", extracted.display()))?;
  }

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

  #[cfg(unix)]
  {
    install_on_unix(binary_path, &current_exe)?;
    Ok(InstallOutcome::Immediate)
  }

  #[cfg(windows)]
  {
    install_on_windows(binary_path, &current_exe)
  }
}

#[cfg(unix)]
fn install_on_unix(new_binary: &Path, current_exe: &Path) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;

  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow!("Executable path has no parent directory"))?;
  let staging_target = parent.join(format!(".twig-update-{}", Uuid::new_v4()));

  if let Err(err) = fs::copy(new_binary, &staging_target) {
    if err.kind() == io::ErrorKind::PermissionDenied {
      run_sudo_install(new_binary, current_exe)?;
      return Ok(());
    }
    return Err(err).with_context(|| format!("Failed to stage update at {}", staging_target.display()));
  }

  let mut perms = fs::metadata(&staging_target)
    .with_context(|| format!("Failed to read metadata for {}", staging_target.display()))?
    .permissions();
  perms.set_mode(0o755);
  fs::set_permissions(&staging_target, perms)
    .with_context(|| format!("Failed to set permissions on {}", staging_target.display()))?;

  match fs::rename(&staging_target, current_exe) {
    Ok(()) => {
      if let Err(err) = fs::remove_file(new_binary) {
        print_warning(&format!("Failed to remove staged binary: {err}"));
      }
      Ok(())
    }
    Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
      if let Err(remove_err) = fs::remove_file(&staging_target) {
        print_warning(&format!(
          "Failed to remove temporary file {}: {remove_err}",
          staging_target.display()
        ));
      }
      run_sudo_install(new_binary, current_exe)?;
      Ok(())
    }
    Err(err) => Err(err).with_context(|| format!("Failed to replace {}", current_exe.display())),
  }
}

#[cfg(unix)]
fn run_sudo_install(new_binary: &Path, current_exe: &Path) -> Result<()> {
  let status = Command::new("sudo")
    .arg("install")
    .arg("-m")
    .arg("755")
    .arg(new_binary)
    .arg(current_exe)
    .status()
    .context("Failed to invoke sudo install")?;

  if status.success() {
    Ok(())
  } else {
    bail!("sudo install exited with status {status}");
  }
}

#[cfg(windows)]
fn install_on_windows(new_binary: &Path, current_exe: &Path) -> Result<InstallOutcome> {
  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow!("Executable path has no parent directory"))?;

  let requires_admin = windows_requires_admin(parent);

  let staging_dir = std::env::temp_dir().join(format!("twig-update-{}", Uuid::new_v4()));
  fs::create_dir(&staging_dir)
    .with_context(|| format!("Failed to create staging directory at {}", staging_dir.display()))?;

  let script_path = staging_dir.join("install.ps1");
  let helper_script = build_powershell_script(new_binary, current_exe)?;
  fs::write(&script_path, helper_script)
    .with_context(|| format!("Failed to write helper script at {}", script_path.display()))?;

  let staged_binary = staging_dir.join(new_binary.file_name().unwrap_or_else(|| OsStr::new("twig.exe")));
  fs::copy(new_binary, &staged_binary)
    .with_context(|| format!("Failed to copy staged binary into {}", staging_dir.display()))?;
  if let Err(err) = fs::remove_file(new_binary) {
    print_warning(&format!("Failed to remove temporary binary: {err}"));
  }

  if let Err(err) = start_powershell_helper(&script_path, &staged_binary, current_exe, requires_admin) {
    if let Err(remove_err) = fs::remove_file(&staged_binary) {
      print_warning(&format!(
        "Failed to remove staged binary {} after helper error: {remove_err}",
        staged_binary.display()
      ));
    }
    if let Err(remove_err) = fs::remove_file(&script_path) {
      print_warning(&format!(
        "Failed to remove helper script {} after helper error: {remove_err}",
        script_path.display()
      ));
    }
    if let Err(remove_err) = fs::remove_dir_all(&staging_dir) {
      print_warning(&format!(
        "Failed to clean staging directory {} after helper error: {remove_err}",
        staging_dir.display()
      ));
    }
    return Err(err);
  }

  Ok(InstallOutcome::Deferred {
    elevated: requires_admin,
  })
}

#[cfg(windows)]
fn windows_requires_admin(parent: &Path) -> bool {
  let probe = parent.join(format!("twig-update-permission-test-{}", Uuid::new_v4()));
  match OpenOptions::new().create_new(true).write(true).open(&probe) {
    Ok(file) => {
      drop(file);
      let _ = fs::remove_file(&probe);
      false
    }
    Err(err) => matches!(err.kind(), io::ErrorKind::PermissionDenied),
  }
}

#[cfg(windows)]
fn build_powershell_script(_source: &Path, _target: &Path) -> Result<String> {
  Ok(format!(
    r#"param(
  [Parameter(Mandatory=$true)][string]$Source,
  [Parameter(Mandatory=$true)][string]$Target,
  [Parameter(Mandatory=$true)][int]$ParentPid
)

function Wait-ForProcess {{
  param([int]$Pid)
  while ($true) {{
    try {{
      $proc = Get-Process -Id $Pid -ErrorAction Stop
      Start-Sleep -Milliseconds 200
    }} catch {{
      break
    }}
  }}
}}

Wait-ForProcess -Pid $ParentPid

$targetDir = Split-Path -Parent $Target
if (-not (Test-Path -LiteralPath $targetDir)) {{
  Write-Error "Target directory does not exist: $targetDir"
  exit 1
}}

if (-not (Test-Path -LiteralPath $Source)) {{
  Write-Error "Staged Twig binary is missing: $Source"
  exit 1
}}

try {{
  Move-Item -LiteralPath $Source -Destination $Target -Force
}} catch {{
  Copy-Item -LiteralPath $Source -Destination $Target -Force
}}

try {{
  Remove-Item -LiteralPath $Source -Force -ErrorAction SilentlyContinue
}} catch {{}}
try {{
  Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force -ErrorAction SilentlyContinue
}} catch {{}}
try {{
  $stagingDir = Split-Path -Parent $Source
  if ($stagingDir -and (Test-Path -LiteralPath $stagingDir)) {{
    Remove-Item -LiteralPath $stagingDir -Force -Recurse -ErrorAction SilentlyContinue
  }}
}} catch {{}}
"#
  ))
}

#[cfg(windows)]
fn start_powershell_helper(
  script_path: &Path,
  staged_binary: &Path,
  current_exe: &Path,
  requires_admin: bool,
) -> Result<()> {
  let pid = std::process::id();

  if requires_admin {
    let argument_list = format!(
      "'-NoProfile','-ExecutionPolicy','Bypass','-File','{}','-Source','{}','-Target','{}','-ParentPid','{}'",
      escape_for_powershell(script_path),
      escape_for_powershell(staged_binary),
      escape_for_powershell(current_exe),
      pid
    );

    let status = Command::new("powershell.exe")
      .arg("-NoProfile")
      .arg("-ExecutionPolicy")
      .arg("Bypass")
      .arg("-Command")
      .arg(format!(
        "Start-Process PowerShell -Verb RunAs -ArgumentList {}",
        argument_list
      ))
      .status()
      .context("Failed to launch elevated PowerShell helper")?;

    if status.success() {
      Ok(())
    } else {
      bail!("Failed to start elevated helper (status {status})");
    }
  } else {
    Command::new("powershell.exe")
      .arg("-NoProfile")
      .arg("-ExecutionPolicy")
      .arg("Bypass")
      .arg("-File")
      .arg(script_path)
      .arg("-Source")
      .arg(staged_binary)
      .arg("-Target")
      .arg(current_exe)
      .arg("-ParentPid")
      .arg(pid.to_string())
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .context("Failed to launch PowerShell helper")?;
    Ok(())
  }
}

#[cfg(windows)]
fn escape_for_powershell(path: &Path) -> String {
  let path = path.to_string_lossy();
  let escaped = path.replace("'", "''");
  format!("'{}'", escaped)
}
