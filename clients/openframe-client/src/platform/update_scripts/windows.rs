//! Windows PowerShell update script for self-update functionality

pub const UPDATE_SCRIPT_WINDOWS: &str = r#"
param(
    [string]$ArchivePath,
    [string]$ServiceName,
    [string]$TargetExe,
    [string]$UpdateStatePath
)

$ErrorActionPreference = 'Stop'

# Setup logging to ProgramData
$LogDir = Join-Path $env:ProgramData "OpenFrame"
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
}
$LogFile = Join-Path $LogDir "update-script.log"

function Write-Log {
    param([string]$Message, [string]$Level = "INFO")
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"
    $entry = "[$timestamp] [$Level] $Message"
    Add-Content -Path $LogFile -Value $entry -ErrorAction SilentlyContinue
}

# Start new update session
Add-Content -Path $LogFile -Value "" -ErrorAction SilentlyContinue
Add-Content -Path $LogFile -Value "============================================================" -ErrorAction SilentlyContinue
Write-Log "=== OpenFrame Update Script Started ==="
Write-Log "PowerShell Version: $($PSVersionTable.PSVersion)"
Write-Log "OS: $([System.Environment]::OSVersion.VersionString)"
Write-Log "User: $([System.Security.Principal.WindowsIdentity]::GetCurrent().Name)"
Write-Log "Is Admin: $([bool](([System.Security.Principal.WindowsPrincipal][System.Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)))"
Write-Log "Working Directory: $(Get-Location)"
Write-Log "Parameters: ArchivePath=$ArchivePath | ServiceName=$ServiceName | TargetExe=$TargetExe | UpdateStatePath=$UpdateStatePath"

$BackupPath = $null
$TempExtract = $null

try {
    # Validate inputs
    Write-Log "Step 1: Validating inputs"

    if (-not (Test-Path $ArchivePath)) {
        throw "Archive file not found: $ArchivePath"
    }
    $archiveSize = (Get-Item $ArchivePath).Length
    Write-Log "Archive validated: $ArchivePath (size: $archiveSize bytes)"

    if (-not (Test-Path $TargetExe)) {
        throw "Target executable not found: $TargetExe"
    }
    $targetSize = (Get-Item $TargetExe).Length
    Write-Log "Target exe validated: $TargetExe (size: $targetSize bytes)"

    if ($archiveSize -lt 100KB) {
        throw "Archive too small ($archiveSize bytes), likely corrupted"
    }

    # Stop the service
    Write-Log "Step 2: Stopping service '$ServiceName'"
    $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if (-not $service) {
        throw "Service not found: $ServiceName"
    }
    Write-Log "Service current status: $($service.Status)"

    if ($service.Status -ne 'Stopped') {
        Stop-Service -Name $ServiceName -Force -ErrorAction Stop
        Write-Log "Stop-Service command issued"
    }

    # Wait for service to fully stop
    $timeout = 30
    $elapsed = 0
    while ((Get-Service -Name $ServiceName).Status -ne 'Stopped' -and $elapsed -lt $timeout) {
        Start-Sleep -Seconds 1
        $elapsed++
    }

    if ($elapsed -ge $timeout) {
        throw "Service did not stop within $timeout seconds"
    }
    Write-Log "Service stopped after $elapsed seconds"

    Start-Sleep -Seconds 2

    # Create backup
    Write-Log "Step 3: Creating backup"
    $BackupPath = "$TargetExe.backup.$(Get-Date -Format 'yyyyMMddHHmmss')"
    Copy-Item -Path $TargetExe -Destination $BackupPath -Force -ErrorAction Stop
    Write-Log "Backup created: $BackupPath"

    # Extract archive
    Write-Log "Step 4: Extracting archive"
    $TempExtract = Join-Path $env:TEMP "openframe-update-$(New-Guid)"
    Expand-Archive -Path $ArchivePath -DestinationPath $TempExtract -Force -ErrorAction Stop
    Write-Log "Archive extracted to: $TempExtract"

    # Find new executable
    Write-Log "Step 5: Locating new executable"
    $NewExe = Get-ChildItem -Path $TempExtract -Filter "*.exe" -Recurse | Select-Object -First 1

    if (-not $NewExe) {
        throw "No executable found in archive"
    }
    Write-Log "Found executable: $($NewExe.FullName) (size: $($NewExe.Length) bytes)"

    if ($NewExe.Length -lt 100KB) {
        throw "Extracted executable too small ($($NewExe.Length) bytes), likely corrupted"
    }

    # Replace binary
    Write-Log "Step 6: Replacing binary"
    Copy-Item -Path $NewExe.FullName -Destination $TargetExe -Force -ErrorAction Stop
    $newTargetSize = (Get-Item $TargetExe).Length
    Write-Log "Binary replaced successfully (new size: $newTargetSize bytes)"

    # Mark update as completed
    Write-Log "Step 7: Updating state file"
    if ($UpdateStatePath -and (Test-Path $UpdateStatePath)) {
        try {
            $stateContent = Get-Content -Path $UpdateStatePath -Raw | ConvertFrom-Json
            $stateContent.phase = "completed"
            $stateContent | ConvertTo-Json -Depth 10 | Set-Content -Path $UpdateStatePath -Force
            Write-Log "State file updated to 'completed'"
        }
        catch {
            Write-Log "WARNING: Failed to update state file: $_" "WARN"
        }
    } else {
        Write-Log "State file not found or path empty, skipping" "WARN"
    }

    # Start service
    Write-Log "Step 8: Starting service"
    Start-Service -Name $ServiceName -ErrorAction Stop
    Write-Log "Start-Service command issued"

    # Verify service started
    Start-Sleep -Seconds 3
    $service = Get-Service -Name $ServiceName -ErrorAction Stop

    if ($service.Status -ne 'Running') {
        throw "Service failed to start (status: $($service.Status))"
    }
    Write-Log "Service started successfully (status: $($service.Status))"

    # Cleanup
    Write-Log "Step 9: Cleanup"
    Remove-Item -Path $ArchivePath -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue
    Write-Log "Cleanup complete"

    Write-Log "=== Update completed successfully ==="
    exit 0
}
catch {
    Write-Log "FATAL ERROR: $($_.Exception.Message)" "ERROR"
    Write-Log "Error at line: $($_.InvocationInfo.ScriptLineNumber)" "ERROR"
    Write-Log "Stack trace: $($_.ScriptStackTrace)" "ERROR"

    # Attempt rollback if backup exists
    if ($BackupPath -and (Test-Path $BackupPath)) {
        Write-Log "Attempting rollback from: $BackupPath" "ERROR"
        try {
            Copy-Item -Path $BackupPath -Destination $TargetExe -Force -ErrorAction Stop
            Write-Log "Rollback: binary restored" "ERROR"
            Start-Service -Name $ServiceName -ErrorAction SilentlyContinue
            Write-Log "Rollback: service restart attempted" "ERROR"
        }
        catch {
            Write-Log "Rollback FAILED: $($_.Exception.Message)" "ERROR"
        }
    } else {
        Write-Log "No backup available for rollback" "ERROR"
    }

    # Cleanup temp files even on failure
    if ($TempExtract -and (Test-Path $TempExtract)) {
        Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue
    }

    Write-Log "=== Update FAILED ===" "ERROR"
    exit 1
}
"#;
