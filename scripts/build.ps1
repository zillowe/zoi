$ErrorActionPreference = "Stop"

$outputDir = ".\scripts\compiled"
$binaryName = "zoi.exe"
$finalBinaryName = "zoi.exe"
$finalBinaryPath = Join-Path -Path $outputDir -ChildPath $finalBinaryName
$srcBinaryPath = ".\target\debug\$binaryName"

New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

try {
    $commit = git rev-parse --short=10 HEAD 2>$null
    if (-not $commit) {
        throw
    }
}
catch {
    $commit = "dev"
}

Write-Host "Building Zoi for Windows..." -ForegroundColor Cyan
Write-Host "Commit: $commit" -ForegroundColor Cyan

$env:ZOI_COMMIT_HASH = $commit
cargo build --bin zoi --release

if ($LASTEXITCODE -eq 0) {
    Write-Host "Cargo build successful." -ForegroundColor Green

    Write-Host "Copying binary to $finalBinaryPath..." -ForegroundColor Cyan
    Copy-Item -Path $srcBinaryPath -Destination $finalBinaryPath -Force

    Write-Host "Build complete! Zoi is ready at $finalBinaryPath" -ForegroundColor Green
}
else {
    Write-Host "Build failed" -ForegroundColor Red
    exit 1
}

Remove-Item Env:\ZOI_COMMIT_HASH
