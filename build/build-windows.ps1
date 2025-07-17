$ErrorActionPreference = "Stop"

$RED   = "`e[0;31m"
$GREEN = "`e[0;32m"
$CYAN  = "`e[0;36m"
$NC    = "`e[0m"

$OUTPUT_DIR = ".\build\release"
$COMMIT = (git rev-parse --short=10 HEAD 2>$null) -or "dev"

$TARGETS = @(
  "x86_64-pc-windows-msvc",
  "aarch64-pc-windows-msvc"
)

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "${RED}❌ 'cargo' is not installed or not in the PATH.${NC}"
    exit 1
}

Write-Host "${CYAN}🏗 Starting native Windows build process...${NC}"
Write-Host "${CYAN}▸ Commit: ${COMMIT}${NC}`n"

if (-not (Test-Path $OUTPUT_DIR)) {
    New-Item -Path $OUTPUT_DIR -ItemType Directory | Out-Null
}

foreach ($target in $TARGETS) {
  switch ($target) {
    "x86_64-pc-windows-msvc"  { $NAME = "zoi-windows-amd64.exe" }
    "aarch64-pc-windows-msvc" { $NAME = "zoi-windows-arm64.exe" }
    default                   { $NAME = "zoi-$target.exe" }
  }
  
  Write-Host "${CYAN}🔧 Natively building for ${target}...${NC}"

  rustup target add $target

  & {
      $env:ZOI_COMMIT_HASH = $COMMIT
      cargo build --target $target --release
  }

  if ($LASTEXITCODE -ne 0) {
      Write-Host "${RED}❌ Build failed for ${target}${NC}"
      exit 1
  }
  
  $SRC_BINARY = ".\target\${target}\release\zoi.exe"
  
  Copy-Item -Path $SRC_BINARY -Destination "$OUTPUT_DIR\$NAME"
  
  Write-Host "${GREEN}✅ Successfully built ${NAME}${NC}`n"
}

Write-Host "`n${GREEN}🎉 All Windows builds completed successfully!${NC}"
Write-Host "${CYAN}Output files in .\build\release directory:${NC}"
Get-ChildItem -Path $OUTPUT_DIR | Select-Object Name, Length
