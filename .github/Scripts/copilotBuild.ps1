# Build the Cargo workspace and save the result to Build.log
#
#   copilotBuild.ps1                       debug build of every crate, all targets (lib, bins, examples, tests)
#   copilotBuild.ps1 -Release              release profile
#   copilotBuild.ps1 -Package unipeek   one crate only
#   copilotBuild.ps1 -Fast                 build `preview` against gpui-fast, in target-fast

param(
    [switch]$Release,
    [string]$Package = "",
    [switch]$Fast
)

. $PSScriptRoot\copilotShared.ps1

$workspace = GetWorkspaceDir
$logFile = "$PSScriptRoot\Build.log"

$cargoArgs = @("build", "--all-targets", "--message-format=short")
if ($Fast) {
    if (($Package -ne "") -and ($Package -ne "preview")) {
        throw "-Fast only applies to the preview crate."
    }
    $cargoArgs += @("-p", "preview", "--features", "fast", "--target-dir", "target-fast")
} elseif ($Package -ne "") {
    $cargoArgs += @("-p", $Package)
} else {
    $cargoArgs += "--workspace"
}
if ($Release) {
    $cargoArgs += "--release"
}

$code = InvokeCargoLogged $workspace $cargoArgs $logFile

$log = Get-Content -Path "$logFile.unfinished"
$warnings = @($log | Where-Object { $_ -match ': warning: ' -or $_ -match '^warning: ' } | Where-Object { $_ -notmatch 'generated \d+ warning' }).Count
$errors = @($log | Where-Object { $_ -match ': error(\[E\d+\])?: ' -or $_ -match '^error(\[E\d+\])?: ' } | Where-Object { $_ -notmatch '^error: could not compile' }).Count

if ($code -eq 0) {
    $status = "Build succeeded."
} else {
    $status = "Build failed."
}
FinishLog $logFile @("", $status, "$warnings Warning(s)", "$errors Error(s)")
exit $code
