# Run something in the Cargo workspace and save the result to Execute.log
#
#   copilotExecute.ps1 -Mode UnitTest -Executable lab-render [-Filter name] [-NoCapture]
#   copilotExecute.ps1 -Mode CLI      -Executable unipeek -Arguments "info","pkg.unitypackage","--json"
#   copilotExecute.ps1 -Mode Example  -Executable thumb -Package lab-render -Arguments ...
#   copilotExecute.ps1 -Mode Preview  -Executable readback -Arguments model.fbx,textures,1100x800 [-Fast] [-Seconds 10]
#
# -Executable is a crate name for UnitTest and CLI, an example name for Example,
# and a mode of the preview application (overlay, fast, readback) for Preview.
# Use -Executable all with -Mode UnitTest to test the whole workspace.

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("UnitTest", "CLI", "Example", "Preview")]
    [string]$Mode,
    [Parameter(Mandatory = $true)]
    [string]$Executable,
    [string]$Package = "",
    [string[]]$Arguments = @(),
    [string]$Filter = "",
    [switch]$NoCapture,
    [switch]$Release,
    [switch]$Fast,
    [int]$Seconds = 10
)

. $PSScriptRoot\copilotShared.ps1

$workspace = GetWorkspaceDir
$logFile = "$PSScriptRoot\Execute.log"
$environment = @{ "RUST_BACKTRACE" = "1" }

switch ($Mode) {
    "UnitTest" {
        $cargoArgs = @("test")
        if ($Executable -eq "all") { $cargoArgs += "--workspace" } else { $cargoArgs += @("-p", $Executable) }
        if ($Release) { $cargoArgs += "--release" }
        $testArgs = @()
        if ($Filter -ne "") { $testArgs += $Filter }
        if ($NoCapture) { $testArgs += "--nocapture" }
        if ($testArgs.Count -gt 0) { $cargoArgs += @("--") + $testArgs }
    }
    "CLI" {
        $cargoArgs = @("run", "-p", $Executable)
        if ($Release) { $cargoArgs += "--release" }
        $cargoArgs += @("--") + $Arguments
    }
    "Example" {
        if ($Package -eq "") { throw "-Package is required with -Mode Example." }
        $cargoArgs = @("run", "-p", $Package, "--example", $Executable)
        if ($Release) { $cargoArgs += "--release" }
        $cargoArgs += @("--") + $Arguments
    }
    "Preview" {
        $cargoArgs = @("run", "-p", "preview")
        if ($Fast -or ($Executable -eq "fast")) { $cargoArgs += @("--features", "fast", "--target-dir", "target-fast") }
        if ($Release) { $cargoArgs += "--release" }
        $cargoArgs += @("--", $Executable) + $Arguments
        # The preview window quits by itself so a script never waits on a window forever.
        $environment["LAB_SECS"] = "$Seconds"
    }
}

$code = InvokeCargoLogged $workspace $cargoArgs $logFile $environment

$summary = @("", "Exit code: $code")
if ($Mode -eq "UnitTest") {
    $log = Get-Content -Path "$logFile.unfinished"
    $results = @($log | Where-Object { $_ -match '^test result: ' })
    $binaries = $results.Count
    $passedBinaries = @($results | Where-Object { $_ -match '^test result: ok\.' }).Count
    $passed = 0
    $total = 0
    foreach ($line in $results) {
        if ($line -match '(\d+) passed; (\d+) failed; (\d+) ignored') {
            $passed += [int]$Matches[1]
            $total += [int]$Matches[1] + [int]$Matches[2]
        }
    }
    $summary += "Passed test binaries: $passedBinaries/$binaries"
    $summary += "Passed test cases: $passed/$total"
    if (($code -ne 0) -and ($binaries -eq 0)) {
        $summary += "No test summary was printed: the build failed or a test binary crashed. Read the lines above."
    }
}
FinishLog $logFile $summary
exit $code
