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
    'beastie.exe', 'beastie-ai-worker.exe', 'beastie-tts.exe', 'beastie-stt.exe',
    'assets\manifest.toml',
    'models\manifest.toml', 'models\LICENSE', 'models\README.md',
    'models\Qwen3.5-0.8B-Q4_0.gguf',
    'runtime\llama-server.exe', 'runtime\LICENSE',
    'runtime\espeak-ng.exe', 'runtime\espeak-ng-COPYING',
    'runtime\espeak-ng-1.52.0.tar.gz',
    'models\parakeet-tdt-0.6b-v3-int8\config.json',
    'models\parakeet-tdt-0.6b-v3-int8\decoder_joint-model.int8.onnx',
    'models\parakeet-tdt-0.6b-v3-int8\encoder-model.int8.onnx',
    'models\parakeet-tdt-0.6b-v3-int8\nemo128.onnx',
    'models\parakeet-tdt-0.6b-v3-int8\vocab.txt',
    'models\parakeet-tdt-0.6b-v3-int8\LICENSE',
    'models\parakeet-tdt-0.6b-v3-int8\README.md',
    'THIRD_PARTY_NOTICES', 'LICENSE', 'package-manifest.json'
)
foreach ($relative in $required) {
    $path = Join-Path $PackageRoot $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "verified package is missing $relative"
    }
}
$espeakData = Join-Path $PackageRoot 'runtime\espeak-ng-data'
if (-not (Test-Path -LiteralPath $espeakData -PathType Container)) {
    throw 'verified package is missing runtime\espeak-ng-data'
}
$manifest = Get-Content -Raw -LiteralPath (Join-Path $PackageRoot 'package-manifest.json') | ConvertFrom-Json
if ($manifest.network -ne $false) { throw 'package-manifest.json must declare network=false' }
if ($manifest.platform -ne 'windows') { throw 'package manifest platform is not windows' }
if ($manifest.release_complete -ne $true) { throw 'package is not release-complete' }
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
