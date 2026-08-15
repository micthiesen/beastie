param(
    [Parameter(Mandatory = $true)][string]$PackageRoot,
    [switch]$ConfigOnly,
    [switch]$LaunchSmoke
)

$ErrorActionPreference = 'Stop'
if ($ConfigOnly) {
    if (-not (Test-Path -LiteralPath $PackageRoot -PathType Leaf)) {
        throw "installer configuration is missing: $PackageRoot"
    }
    Write-Output "Windows installer configuration present: $PackageRoot"
    exit 0
}
$required = @(
    'beastie.exe', 'beastie-ai-worker.exe', 'assets\manifest.toml',
    'models\manifest.toml', 'models\LICENSE', 'models\README.md',
    'runtime\llama-server.exe', 'runtime\LICENSE', 'package-manifest.json'
)
foreach ($relative in $required) {
    $path = Join-Path $PackageRoot $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "verified package is missing $relative"
    }
}
$manifest = Get-Content -Raw -LiteralPath (Join-Path $PackageRoot 'package-manifest.json') | ConvertFrom-Json
if ($manifest.network -ne $false) { throw 'package-manifest.json must declare network=false' }
if ($manifest.platform -ne 'windows') { throw 'package manifest platform is not windows' }
if ($LaunchSmoke) {
    # A real package launch is performed only on a Windows runner with the display available.
    $env:BEASTIE_DISABLE_NETWORK = '1'
    $process = Start-Process -FilePath (Join-Path $PackageRoot 'beastie.exe') -ArgumentList '--smoke' -WorkingDirectory $PackageRoot -PassThru
    if (-not $process.WaitForExit(30000)) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
        throw 'launch smoke did not exit within 30 seconds'
    }
    if ($process.ExitCode -ne 0) { throw "launch smoke exited $($process.ExitCode)" }
}
Write-Output "Windows package layout verified: $PackageRoot"
