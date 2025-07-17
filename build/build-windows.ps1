$ErrorActionPreference = "Stop"

$OUTPUT_DIR = ".\build\release"
$COMMIT = (git rev-parse --short=10 HEAD 2>$null) -or "dev"

$TARGETS = @(
  "x86_64-pc-windows-msvc",
  "aarch64-pc-windows-msvc"
)

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host ("'cargo' is not installed or not in the PATH.")
    exit 1
}

Write-Host ("Starting native Windows build process...")
Write-Host ("Commit: " + $COMMIT + "`n")

if (-not (Test-Path $OUTPUT_DIR)) {
    New-Item -Path $OUTPUT_DIR -ItemType Directory | Out-Null
}

foreach ($target in $TARGETS) {
  switch ($target) {
    "x86_64-pc-windows-msvc"  { $NAME = "zoi-windows-amd64.exe" }
    "aarch64-pc-windows-msvc" { $NAME = "zoi-windows-arm64.exe" }
    default                   { $NAME = "zoi-$target.exe" }
  }
  
  Write-Host ("Natively building for " + $target + "...")

  rustup target add $target

  & {
      $env:ZOI_COMMIT_HASH = $COMMIT
      cargo build --target $target --release
  }

  if ($LASTEXITCODE -ne 0) {
      Write-Host ("Build failed for " + $target)
      exit 1
  }
  
  $SRC_BINARY = ".\target\${target}\release\zoi.exe"
  
  Copy-Item -Path $SRC_BINARY -Destination "$OUTPUT_DIR\$NAME"
  
  Write-Host ("Successfully built " + $NAME + "`n")
}

Write-Host ("`n" + "All Windows builds completed successfully!")
Write-Host ('Output files in .\build\release directory:')
Get-ChildItem -Path $OUTPUT_DIR | Select-Object Name, Length
