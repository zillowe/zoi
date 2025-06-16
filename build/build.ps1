$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Path "build/compiled" -Force | Out-Null

try {
    $commit = (git rev-parse --short=10 HEAD 2>$null)
    if (-not $commit) { throw }
}
catch {
    $commit = "dev"
}

Write-Host "Building Zoi for Windows..." -ForegroundColor Cyan
go build -o "./build/compiled/zoi.exe" `
    -ldflags "-X main.VerCommit=$commit" `
    .

if ($LASTEXITCODE -eq 0) {
    Write-Host "Build successful! Commit: $commit" -ForegroundColor Green
}
else {
    Write-Host "Build failed" -ForegroundColor Red
    exit 1
}