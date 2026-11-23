# Prepare Copilot workspace files
#
#   copilotPrepare.ps1            back up and reset Copilot_Task.md, Copilot_Planning.md, Copilot_Execution.md
#   copilotPrepare.ps1 -Backup    back up and delete them
#   copilotPrepare.ps1 -Earliest  print the earliest backup folder in ..\Learning

param(
    [switch]$Backup,
    [switch]$Earliest
)

if (@($Backup, $Earliest).Where({ $_ }).Count -gt 1) {
    throw "At most one switch can be true: -Backup, -Earliest."
}

$taskLogs = "$PSScriptRoot\..\TaskLogs"
$learningRoot = "$PSScriptRoot\..\Learning"

if ($Earliest) {
    $earliestFolder = Get-ChildItem -LiteralPath $learningRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object -Property Name |
        Select-Object -First 1

    if ($null -eq $earliestFolder) {
        throw "No folder is found in '$learningRoot'."
    }

    Write-Output $earliestFolder.FullName
    exit 0
}

# Reset on every new task
$filesToOverride = [ordered]@{
    "Copilot_Task.md" = "# !!!TASK!!!"
    "Copilot_Planning.md" = "# !!!PLANNING!!!"
    "Copilot_Execution.md" = "# !!!EXECUTION!!!"
}

# Created only when missing
$filesToCreate = [ordered]@{
    "Copilot_Scrum.md" = "# !!!SCRUM!!!"
    "Copilot_KB.md" = "# !!!KNOWLEDGE BASE!!!"
    "Copilot_Investigate.md" = "# !!!INVESTIGATE!!!"
}

$findingFile = "$taskLogs\Copilot_Execution_Finding.md"

# Back up task documents for learning, skipping files that only contain their title
$filesToBackup = @()
foreach ($file in $filesToOverride.GetEnumerator()) {
    $filePath = "$taskLogs\$($file.Key)"
    if ((Test-Path $filePath) -and ((Get-Content -Raw $filePath).Trim() -ne $file.Value)) {
        $filesToBackup += $filePath
    }
}
if (Test-Path $findingFile) {
    $filesToBackup += $findingFile
}

if ($filesToBackup.Count -gt 0) {
    $timestamp = Get-Date -Format "yyyy-MM-dd-HH-mm-ss"
    $backupFolder = "$learningRoot\$timestamp"
    New-Item -ItemType Directory -Path $backupFolder -Force | Out-Null
    Write-Output (Resolve-Path $backupFolder).Path

    foreach ($filePath in $filesToBackup) {
        $fileName = Split-Path $filePath -Leaf
        Write-Host "Backing up $fileName to Learning folder..."
        Copy-Item -Path $filePath -Destination (Join-Path $backupFolder $fileName) -Force
    }
}

if (Test-Path $findingFile) {
    Write-Host "Deleting Copilot_Execution_Finding.md..."
    Remove-Item -Path $findingFile -Force
}

if ($Backup) {
    foreach ($file in $filesToOverride.GetEnumerator()) {
        $filePath = "$taskLogs\$($file.Key)"
        if (Test-Path $filePath) {
            Write-Host "Deleting $($file.Key)..."
            Remove-Item -Path $filePath -Force
        }
    }
} else {
    foreach ($file in $filesToOverride.GetEnumerator()) {
        Write-Host "Creating/overriding $($file.Key)..."
        Set-Content -Path "$taskLogs\$($file.Key)" -Value $file.Value -Encoding utf8
    }

    foreach ($file in $filesToCreate.GetEnumerator()) {
        $filePath = "$taskLogs\$($file.Key)"
        if (-not (Test-Path $filePath)) {
            Write-Host "Creating $($file.Key)..."
            Set-Content -Path $filePath -Value $file.Value -Encoding utf8
        } else {
            Write-Host "Skipping $($file.Key) (already exists)..."
        }
    }
}

Write-Host "Copilot preparation completed."
