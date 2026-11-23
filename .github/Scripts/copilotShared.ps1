# Shared helpers for the copilot*.ps1 scripts.

function GetWorkspaceDir {
    $dir = Get-Item -LiteralPath (Get-Location).Path
    while ($null -ne $dir) {
        $manifest = Join-Path $dir.FullName "Cargo.toml"
        if ((Test-Path $manifest) -and (Select-String -Path $manifest -Pattern '^\[workspace\]' -Quiet)) {
            return $dir.FullName
        }
        $dir = $dir.Parent
    }
    throw "Could not find a Cargo workspace (Cargo.toml with [workspace]) in the current directory or any parent directory."
}

# Runs cargo with the given arguments in the workspace, streaming output to the console and to $logFile.
# "$logFile.unfinished" exists while cargo is running and is renamed to $logFile when it exits.
function InvokeCargoLogged($workspace, $cargoArgs, $logFile, [hashtable]$environment = @{}) {
    $unfinished = "$logFile.unfinished"
    Remove-Item -Path $logFile, $unfinished -Force -ErrorAction SilentlyContinue

    $saved = @{}
    foreach ($key in $environment.Keys) {
        $saved[$key] = [Environment]::GetEnvironmentVariable($key)
        [Environment]::SetEnvironmentVariable($key, $environment[$key])
    }

    Push-Location $workspace
    try {
        "> cargo $($cargoArgs -join ' ')" | Out-File -FilePath $unfinished -Encoding utf8
        # cargo writes progress and diagnostics to stderr; merge them so the log has everything in order.
        $ErrorActionPreference = "Continue"
        & cargo @cargoArgs 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $unfinished -Append | Out-Host
        $code = $LASTEXITCODE
    } finally {
        Pop-Location
        foreach ($key in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($key, $saved[$key])
        }
    }
    return $code
}

function FinishLog($logFile, $lines) {
    $unfinished = "$logFile.unfinished"
    $lines | Out-File -FilePath $unfinished -Append -Encoding utf8
    $lines | ForEach-Object { Write-Host $_ }
    Move-Item -Path $unfinished -Destination $logFile -Force
}
