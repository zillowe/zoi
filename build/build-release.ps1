$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Path "build/compiled" -Force | Out-Null

try {
    $commit = (git rev-parse --short=10 HEAD 2>$null)
    if (-not $commit) { throw }
}
catch {
    $commit = "dev"
}

Write-Host "Building Zoi release for Windows..." -ForegroundColor Cyan
go build -o "./build/compiled/zoi-r.exe" `
    -ldflags "-s -w -X main.VerCommit=$commit" `
    ./src

if ($LASTEXITCODE -eq 0) {
    Write-Host "Build successful! Commit: $commit" -ForegroundColor Green
}
else {
    Write-Host "Build failed" -ForegroundColor Red
    exit 1
}