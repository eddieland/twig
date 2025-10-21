use std::ffi::OsStr;
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use anyhow::anyhow;
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use tar::Archive;
use twig_core::output::{print_info, print_success, print_warning};
use uuid::Uuid;
use zip::read::ZipArchive;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
  tag_name: String,
  assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize, Clone)]
struct GitHubAsset {
  name: String,
  browser_download_url: String,
}

struct PlatformInfo {
  os: &'static str,
  arch: &'static str,
  extension: &'static str,
  binary_name: &'static str,
}

pub fn run_self_update() -> Result<()> {
  let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
  rt.block_on(async { perform_update().await })
}

async fn perform_update() -> Result<()> {
  let current_version = env!("CARGO_PKG_VERSION");
  print_info(&format!("Current Twig version: {current_version}"));
  print_info("Checking for the latest release on GitHub...");

  let client = Client::builder()
    .user_agent(format!("twig-self-update/{current_version}"))
    .build()
    .context("failed to build HTTP client")?;

  let release = fetch_latest_release(&client).await?;
  let platform = detect_platform()?;
  let asset = select_asset(&release, &platform)
    .with_context(|| format!("no release asset found for {}-{}", platform.os, platform.arch))?;

  let latest_version = release.tag_name.trim_start_matches('v');
  if latest_version == current_version {
    print_warning("You are already running the latest version of Twig.");
    return Ok(());
  }

  print_info(&format!("Latest available version: {}", release.tag_name));
  print_info(&format!("Downloading {}...", asset.name));

  let asset_bytes = download_asset(&client, &asset.browser_download_url).await?;
  let binary = extract_binary(&asset.name, &asset_bytes, platform.binary_name)?;

  install_binary(&binary, &release.tag_name)?;

  print_success(&format!("Twig has been updated to {}!", release.tag_name));
  Ok(())
}

async fn fetch_latest_release(client: &Client) -> Result<GitHubRelease> {
  let response = client
    .get("https://api.github.com/repos/eddieland/twig/releases/latest")
    .send()
    .await
    .context("failed to contact GitHub")?;

  let response = response.error_for_status().context("GitHub API returned an error")?;
  let release = response
    .json::<GitHubRelease>()
    .await
    .context("failed to parse release metadata")?;
  Ok(release)
}

fn detect_platform() -> Result<PlatformInfo> {
  let os = std::env::consts::OS;
  let arch = std::env::consts::ARCH;

  let (extension, binary_name) = match os {
    "linux" | "macos" => ("tar.gz", "twig"),
    "windows" => ("zip", "twig.exe"),
    other => bail!("unsupported operating system: {other}"),
  };

  Ok(PlatformInfo {
    os,
    arch,
    extension,
    binary_name,
  })
}

fn select_asset<'a>(release: &'a GitHubRelease, platform: &PlatformInfo) -> Option<&'a GitHubAsset> {
  let prefix = format!("twig-{}-{}", platform.os, platform.arch);
  release
    .assets
    .iter()
    .find(|asset| asset.name.starts_with(&prefix) && asset.name.ends_with(platform.extension))
}

async fn download_asset(client: &Client, url: &str) -> Result<Vec<u8>> {
  let response = client
    .get(url)
    .send()
    .await
    .with_context(|| format!("failed to download asset from {url}"))?;
  let response = response
    .error_for_status()
    .context("download returned an error status")?;
  let bytes = response.bytes().await.context("failed to read download")?;
  Ok(bytes.to_vec())
}

fn extract_binary(asset_name: &str, bytes: &[u8], expected_binary: &str) -> Result<Vec<u8>> {
  if asset_name.ends_with(".tar.gz") {
    extract_from_tar(bytes, expected_binary)
  } else if asset_name.ends_with(".zip") {
    extract_from_zip(bytes, expected_binary)
  } else {
    bail!("unsupported asset type: {asset_name}");
  }
}

fn extract_from_tar(bytes: &[u8], expected_binary: &str) -> Result<Vec<u8>> {
  let gz = GzDecoder::new(Cursor::new(bytes));
  let mut archive = Archive::new(gz);
  for file in archive.entries().context("failed to read tar archive")? {
    let mut entry = file.context("failed to read archive entry")?;
    if !entry.header().entry_type().is_file() {
      continue;
    }
    let path = entry.path().context("failed to read entry path")?;
    if path.file_name() == Some(OsStr::new(expected_binary)) {
      let mut data = Vec::new();
      entry.read_to_end(&mut data).context("failed to extract binary")?;
      return Ok(data);
    }
  }

  bail!("binary {expected_binary} not found in archive")
}

fn extract_from_zip(bytes: &[u8], expected_binary: &str) -> Result<Vec<u8>> {
  let reader = Cursor::new(bytes);
  let mut archive = ZipArchive::new(reader).context("failed to open zip archive")?;
  for i in 0..archive.len() {
    let mut file = archive.by_index(i).context("failed to read zip entry")?;
    if file.is_dir() {
      continue;
    }
    let name = file
      .name()
      .rsplit_once('/')
      .map(|(_, file)| file)
      .unwrap_or(file.name());
    if name == expected_binary {
      let mut data = Vec::new();
      file.read_to_end(&mut data).context("failed to extract binary")?;
      return Ok(data);
    }
  }

  bail!("binary {expected_binary} not found in archive")
}

fn install_binary(data: &[u8], release_tag: &str) -> Result<()> {
  let exe_path = std::env::current_exe().context("unable to locate current executable")?;
  let target_path = exe_path.canonicalize().unwrap_or(exe_path.clone());

  #[cfg(unix)]
  {
    install_on_unix(data, &target_path, release_tag)
  }

  #[cfg(windows)]
  {
    install_on_windows(data, &target_path, release_tag)
  }
}

#[cfg(unix)]
fn install_on_unix(data: &[u8], target_path: &Path, release_tag: &str) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;

  let target_dir = target_path.parent().context("executable has no parent directory")?;
  let file_name = target_path.file_name().and_then(|name| name.to_str()).unwrap_or("twig");

  let stage_path = match write_stage_file(Some(target_dir), file_name, data) {
    Ok(path) => path,
    Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
      print_warning("Current location is not writable, staging update in temporary directory.");
      write_stage_file(None, file_name, data)?
    }
    Err(err) => return Err(err).context("failed to stage updated binary")?,
  };

  let permissions = fs::Permissions::from_mode(0o755);
  fs::set_permissions(&stage_path, permissions.clone()).context("failed to set permissions on staged binary")?;

  if stage_path.parent() == Some(target_dir) {
    match fs::rename(&stage_path, target_path) {
      Ok(_) => return Ok(()),
      Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
        print_warning("Permission denied when replacing binary, attempting sudo install...");
      }
      Err(err) => {
        print_warning(&format!(
          "Failed to atomically replace twig: {}. Falling back to copy.",
          err
        ));
      }
    }
  }

  match fs::copy(&stage_path, target_path) {
    Ok(_) => {
      fs::set_permissions(target_path, permissions).context("failed to set permissions on updated binary")?;
      fs::remove_file(&stage_path).ok();
      return Ok(());
    }
    Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
      print_warning("Permission denied when copying binary, attempting sudo install...");
    }
    Err(err) => return Err(err).context("failed to copy updated binary into place")?,
  }

  install_with_sudo(&stage_path, target_path, release_tag)?;
  fs::remove_file(&stage_path).ok();
  Ok(())
}

#[cfg(unix)]
fn install_with_sudo(stage_path: &Path, target_path: &Path, release_tag: &str) -> Result<()> {
  let status = Command::new("sudo")
    .arg("install")
    .arg("-m")
    .arg("755")
    .arg(stage_path)
    .arg(target_path)
    .status();

  match status {
    Ok(result) if result.success() => {
      print_info(&format!("Installed Twig {} with elevated permissions.", release_tag));
      Ok(())
    }
    Ok(_) => bail!("sudo install did not complete successfully"),
    Err(err) if err.kind() == io::ErrorKind::NotFound => {
      bail!(
        "sudo is not available. Rerun 'twig self update' with elevated permissions to install {}.",
        release_tag
      )
    }
    Err(err) => Err(err).context("failed to invoke sudo"),
  }
}

#[cfg(windows)]
fn install_on_windows(data: &[u8], target_path: &Path, release_tag: &str) -> Result<()> {
  let stage_path = write_stage_file(None, "twig.exe", data).context("failed to stage updated binary")?;

  let stage_str = stage_path
    .to_str()
    .ok_or_else(|| anyhow!("staged binary path is not valid UTF-16"))?;
  let target_str = target_path
    .to_str()
    .ok_or_else(|| anyhow!("target path is not valid UTF-16"))?;

  let escaped_stage = stage_str.replace('"', "\"");
  let escaped_target = target_str.replace('"', "\"");
  let pid = std::process::id();

  let script = format!(
    "$ErrorActionPreference = 'Stop'; \
     $source = \"{escaped_stage}\"; \
     $destination = \"{escaped_target}\"; \
     $pid = {pid}; \
     while (Get-Process -Id $pid -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 200 }}; \
     Move-Item -Force -Path $source -Destination $destination;"
  );
  let escaped_script = script
    .replace('\'', "''")
    .replace('"', "\"");

  let command =
    format!("Start-Process -FilePath powershell -ArgumentList '-NoProfile','-Command','{escaped_script}' -Verb RunAs");

  let status = Command::new("powershell")
    .args(["-NoProfile", "-Command", &command])
    .status()
    .context("failed to launch elevated PowerShell process")?;

  if status.success() {
    print_info(
      "An elevated PowerShell window will complete the update shortly. Please keep this terminal open until it finishes.",
    );
    Ok(())
  } else {
    bail!("failed to launch elevated update helper")
  }
}

fn write_stage_file(preferred_dir: Option<&Path>, file_name: &str, data: &[u8]) -> io::Result<PathBuf> {
  if let Some(dir) = preferred_dir {
    match write_stage_file_internal(dir, file_name, data) {
      Ok(path) => return Ok(path),
      Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {}
      Err(err) => return Err(err),
    }
  }

  let temp_dir = std::env::temp_dir();
  write_stage_file_internal(&temp_dir, file_name, data)
}

fn write_stage_file_internal(dir: &Path, file_name: &str, data: &[u8]) -> io::Result<PathBuf> {
  let stage_name = format!("{file_name}.update-{}", Uuid::new_v4());
  let stage_path = dir.join(stage_name);
  let mut file = fs::File::create(&stage_path)?;
  file.write_all(data)?;
  file.sync_all()?;
  Ok(stage_path)
}
