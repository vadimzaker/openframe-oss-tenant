use anyhow::{Context, Result, anyhow};
use std::process::Command;
use std::os::windows::process::CommandExt;
use tracing::{info, error, warn};
use uuid::Uuid;

use super::UpdaterParams;
use crate::platform::get_powershell_path;
use crate::platform::update_scripts::UPDATE_SCRIPT_WINDOWS;

/// Launch PowerShell updater script on Windows
/// Uses CREATE_NO_WINDOW flag to run detached from console
pub async fn launch_updater(params: UpdaterParams) -> Result<()> {
    info!("Launching Windows PowerShell updater");
    info!(
        archive_path = %params.binary_path,
        service_name = %params.service_name,
        target_exe = %params.target_exe,
        update_state_path = %params.update_state_path,
        "Update parameters"
    );

    // Validate archive exists before launching script
    let archive_path = std::path::Path::new(&params.binary_path);
    if !archive_path.exists() {
        error!(path = %params.binary_path, "Archive file does not exist before script launch");
        return Err(anyhow!("Archive file does not exist: {}", params.binary_path));
    }
    let archive_meta = std::fs::metadata(archive_path)
        .context("Failed to read archive metadata")?;
    info!(archive_size_bytes = archive_meta.len(), "Archive file validated");

    // Validate target exe exists
    let target_path = std::path::Path::new(&params.target_exe);
    if !target_path.exists() {
        error!(path = %params.target_exe, "Target executable does not exist");
        return Err(anyhow!("Target executable does not exist: {}", params.target_exe));
    }
    info!(target_exe = %params.target_exe, "Target executable validated");

    // Resolve PowerShell path
    let ps_path = get_powershell_path().map_err(|e| {
        error!(error = %e, "Failed to resolve PowerShell path");
        anyhow!(e)
    })?;
    info!(powershell_path = %ps_path, "PowerShell resolved");

    // Verify PowerShell binary exists
    if !std::path::Path::new(&ps_path).exists() {
        error!(path = %ps_path, "PowerShell binary not found at resolved path");
        return Err(anyhow!("PowerShell binary not found: {}", ps_path));
    }

    // Save PowerShell script to temp file
    let temp_dir = std::env::temp_dir();
    info!(temp_dir = %temp_dir.display(), "Using temp directory");

    let script_path = temp_dir.join(format!(
        "openframe-updater-{}.ps1",
        Uuid::new_v4()
    ));

    tokio::fs::write(&script_path, UPDATE_SCRIPT_WINDOWS).await
        .context("Failed to write PowerShell script to temp directory")?;

    // Verify script was written successfully
    let script_meta = std::fs::metadata(&script_path)
        .context("Failed to verify written PowerShell script")?;
    if script_meta.len() == 0 {
        error!(path = %script_path.display(), "PowerShell script file is empty after write");
        return Err(anyhow!("PowerShell script file is empty after write"));
    }
    info!(
        script_path = %script_path.display(),
        script_size_bytes = script_meta.len(),
        "PowerShell script saved and verified"
    );

    // Log the full command for diagnostics
    info!(
        command = %format!(
            "{} -ExecutionPolicy Bypass -NoProfile -File {} -ArchivePath {} -ServiceName {} -TargetExe {} -UpdateStatePath {}",
            ps_path, script_path.display(), params.binary_path, params.service_name, params.target_exe, params.update_state_path
        ),
        "Spawning PowerShell process"
    );

    let child = Command::new(&ps_path)
        .arg("-ExecutionPolicy").arg("Bypass")
        .arg("-NoProfile")
        .arg("-File").arg(&script_path)
        .arg("-ArchivePath").arg(&params.binary_path)
        .arg("-ServiceName").arg(&params.service_name)
        .arg("-TargetExe").arg(&params.target_exe)
        .arg("-UpdateStatePath").arg(&params.update_state_path)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| {
            error!(
                error = %e,
                error_kind = ?e.kind(),
                powershell_path = %ps_path,
                script_path = %script_path.display(),
                "Failed to spawn PowerShell updater process"
            );
            anyhow!("Failed to spawn PowerShell updater: {} (kind: {:?})", e, e.kind())
        })?;

    info!(pid = child.id(), "PowerShell updater launched successfully");

    Ok(())
}
