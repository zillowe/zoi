$ErrorActionPreference = "Stop"

$COMMIT = git rev-parse --short=10 HEAD

New-Item -ItemType Directory -Path "build/compiled" -Force | Out-Null

$targets = @(
  @{OS = "linux"; Arch = "amd64"; Ext = "" },
  @{OS = "linux"; Arch = "arm64"; Ext = "" },
  @{OS = "windows"; Arch = "amd64"; Ext = ".exe" },
  @{OS = "windows"; Arch = "arm64"; Ext = ".exe" },
  @{OS = "darwin"; Arch = "amd64"; Ext = "" },
  @{OS = "darwin"; Arch = "arm64"; Ext = "" }
)

foreach ($target in $targets) {
  $output = "zoi-$($target.OS)-$($target.Arch)$($target.Ext)"
  $env:GOOS = $target.OS
  $env:GOARCH = $target.Arch
    
  Write-Host "Building $output..." -ForegroundColor Cyan
  go build -o "./build/compiled/$output" `
    -ldflags "-s -w -X main.VerCommit=$COMMIT" `
    .

  if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed for $output" -ForegroundColor Red
    exit 1
  }
}

Write-Host "All builds completed!" -ForegroundColor Green