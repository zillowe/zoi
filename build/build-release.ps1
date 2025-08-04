$ErrorActionPreference = "Stop"

$outputDir = ".\build\release"
$binaryName = "zoi-cli.exe"
$finalBinaryName = "zoi.exe"
$finalBinaryPath = Join-Path -Path $outputDir -ChildPath $finalBinaryName
$srcBinaryPath = ".\target\release\$binaryName"

New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

try {
    $commit = git rev-parse --short=10 HEAD 2>$null
    if (-not $commit) { throw }
}
catch {
    $commit = "dev"
}

Write-Host "Building Zoi release binary for Windows..." -ForegroundColor Cyan
Write-Host "Commit: $commit" -ForegroundColor Cyan

$env:ZOI_COMMIT_HASH = $commit
cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Cargo build failed" -ForegroundColor Red
    Remove-Item Env:\ZOI_COMMIT_HASH -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "Cargo build successful." -ForegroundColor Green

Write-Host "Attempting to strip release binary for size optimization..." -ForegroundColor Cyan
try {
    Get-Command strip -ErrorAction Stop | Out-Null
    
    strip $srcBinaryPath
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Binary stripped successfully." -ForegroundColor Green
    } else {
        Write-Host "Strip command failed." -ForegroundColor Yellow
    }
}
catch {
    Write-Host "Strip command not found. Skipping size optimization." -ForegroundColor Yellow
    Write-Host "To enable stripping, install a GCC toolchain and add it to your PATH." -ForegroundColor Yellow
}

Write-Host "Copying final binary to $finalBinaryPath..." -ForegroundColor Cyan
Copy-Item -Path $srcBinaryPath -Destination $finalBinaryPath -Force

Write-Host "Release build complete! Zoi is ready at $finalBinaryPath" -ForegroundColor Green

Remove-Item Env:\ZOI_COMMIT_HASH -ErrorAction SilentlyContinue
