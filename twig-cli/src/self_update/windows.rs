use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use uuid::Uuid;

use super::{InstallOutcome, Result, print_warning};

/// Apply any platform-specific tweaks after extracting the Windows archive.
///
/// No-op on Windows since the ZIP already carries correct permissions.
pub fn finalize_extracted_binary(_path: &Path) -> Result<()> {
  Ok(())
}

/// Stage the new Twig binary and launch a PowerShell helper to complete the
/// swap.
///
/// When admin rights are needed to replace the current executable, the helper
/// is started elevated; otherwise it runs in the background and finishes the
/// install after this process exits.
pub fn install_new_binary(new_binary: &Path, current_exe: &Path) -> Result<InstallOutcome> {
  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow::anyhow!("Executable path has no parent directory"))?;

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

fn windows_requires_admin(parent: &Path) -> bool {
  use std::fs::OpenOptions;
  use std::io;

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

fn start_powershell_helper(
  script_path: &Path,
  staged_binary: &Path,
  current_exe: &Path,
  requires_admin: bool,
) -> Result<()> {
  use std::process::{Command, Stdio};

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

fn escape_for_powershell(path: &Path) -> String {
  let path = path.to_string_lossy();
  let escaped = path.replace("'", "''");
  format!("'{}'", escaped)
}
