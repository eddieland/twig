use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use uuid::Uuid;

use super::{InstallOutcome, Result, print_warning};

/// Ensure the extracted binary has executable permissions on Unix platforms.
///
/// This sets mode `0o755` so the staged binary can be executed when replacing
/// the currently running Twig binary.
pub fn finalize_extracted_binary(path: &Path) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;

  let mut perms = fs::metadata(path)
    .with_context(|| format!("Failed to read permissions for {}", path.display()))?
    .permissions();
  perms.set_mode(0o755);
  fs::set_permissions(path, perms).with_context(|| format!("Failed to set permissions on {}", path.display()))
}

/// Replace the current Twig executable with the freshly downloaded binary.
///
/// Performs a best-effort atomic rename after staging, falling back to a sudo
/// installation when permission errors arise. Returns immediately once the
/// binary has been swapped.
pub fn install_new_binary(new_binary: &Path, current_exe: &Path) -> Result<InstallOutcome> {
  use std::io;
  use std::os::unix::fs::PermissionsExt;

  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow::anyhow!("Executable path has no parent directory"))?;
  let staging_target = parent.join(format!(".twig-update-{}", Uuid::new_v4()));

  if let Err(err) = fs::copy(new_binary, &staging_target) {
    if err.kind() == io::ErrorKind::PermissionDenied {
      run_sudo_install(new_binary, current_exe)?;
      return Ok(InstallOutcome::Immediate);
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
      Ok(InstallOutcome::Immediate)
    }
    Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
      if let Err(remove_err) = fs::remove_file(&staging_target) {
        print_warning(&format!(
          "Failed to remove temporary file {}: {remove_err}",
          staging_target.display()
        ));
      }
      run_sudo_install(new_binary, current_exe)?;
      Ok(InstallOutcome::Immediate)
    }
    Err(err) => Err(err).with_context(|| format!("Failed to replace {}", current_exe.display())),
  }
}

fn run_sudo_install(new_binary: &Path, current_exe: &Path) -> Result<()> {
  use std::process::Command;

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
