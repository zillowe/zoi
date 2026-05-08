$ErrorActionPreference = "Stop"

$outputDir = ".\scripts\release"
$binaryName = "zoi.exe"
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

Write-Host "Building Zoi release binaries for Windows..." -ForegroundColor Cyan
Write-Host "Commit: $commit" -ForegroundColor Cyan

$env:ZOI_COMMIT_HASH = $commit
cargo build --bins --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Cargo build failed" -ForegroundColor Red
    Remove-Item Env:\ZOI_COMMIT_HASH -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "Cargo build successful." -ForegroundColor Green

Write-Host "Attempting to strip release binaries for size optimization..." -ForegroundColor Cyan
try {
    Get-Command strip -ErrorAction Stop | Out-Null
    
    strip $srcBinaryPath
    strip ".\target\release\zoi-mini.exe"
    
    Write-Host "Binaries stripped successfully." -ForegroundColor Green
}
catch {
    Write-Host "Strip command not found. Skipping size optimization." -ForegroundColor Yellow
}

Write-Host "Copying final binaries to $outputDir..." -ForegroundColor Cyan
Copy-Item -Path $srcBinaryPath -Destination $finalBinaryPath -Force
Copy-Item -Path ".\target\release\zoi-mini.exe" -Destination (Join-Path -Path $outputDir -ChildPath "zoi-mini.exe") -Force

Write-Host "Release build complete!" -ForegroundColor Green

Remove-Item Env:\ZOI_COMMIT_HASH -ErrorAction SilentlyContinue
