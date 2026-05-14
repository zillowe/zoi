# zm.ps1 - Zero-install Zoi Mini script for Windows
# Usage: powershell -c "irm zillowe.pages.dev/zm.ps1 | iex" -args "i <package>"

$ErrorActionPreference = "Stop"

$GitLabProjectId = "71087662"
$GitLabProjectPath = "Zillowe/Zillwen/Zusty/Zoi"
$PublicKeyUrl = "https://zillowe.pages.dev/keys/zillowe-main.asc"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Cyan }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Warning { param($Message) Write-Host "[WARN] $Message" -ForegroundColor Yellow }
function Write-Error-Exit {
    param($Message, $Exception = $null)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
    if ($Exception) {
        Write-Host "  $($Exception.Message)" -ForegroundColor Red
    }
    exit 1
}

Write-Info "Fetching the latest release tag from GitLab API..."
try {
    $ApiUrl = "https://gitlab.com/api/v4/projects/$GitLabProjectId/releases"
    $Releases = Invoke-RestMethod -Uri $ApiUrl -Method Get
    $LatestTag = $Releases[0].tag_name
    if ([string]::IsNullOrEmpty($LatestTag)) { throw "API response did not contain a valid tag name." }
    Write-Info "Latest tag found: $LatestTag"
} catch {
    Write-Error-Exit "Could not fetch the latest release tag." $_.Exception
}

$Os = "windows"
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'amd64' }

$BaseUrl = "https://gitlab.com/$GitLabProjectPath/-/releases/$LatestTag/downloads"
$TargetArchive = "zoi-mini-${Os}-${Arch}.zip"
$DownloadUrl = "$BaseUrl/$TargetArchive"
$SignatureUrl = "$DownloadUrl.asc"
$ChecksumUrl = "$BaseUrl/checksums.txt"

$TempDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$TempZipPath = Join-Path $TempDir $TargetArchive
$TempSignaturePath = Join-Path $TempDir "$($TargetArchive).asc"
$TempChecksumPath = Join-Path $TempDir "checksums.txt"
$TempPubKeyPath = Join-Path $TempDir "pubkey.asc"

try {
    Write-Info "Downloading Zoi Mini for $Os ($Arch)..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZipPath -UseBasicParsing

    Write-Info "Verifying checksum..."
    Invoke-WebRequest -Uri $ChecksumUrl -OutFile $TempChecksumPath -UseBasicParsing
    $ExpectedHash = (Get-Content $TempChecksumPath | Select-String -Pattern $TargetArchive).Line.Split(" ")[0]
    $ActualHash = (Get-FileHash -Path $TempZipPath -Algorithm SHA512).Hash.ToLower()

    if ($ActualHash -ne $ExpectedHash) { throw "Checksum mismatch!" }
    Write-Success "Checksum verified successfully."

    $GpgPath = Get-Command gpg -ErrorAction SilentlyContinue
    if ($GpgPath) {
        Write-Info "Verifying GPG signature..."
        Invoke-WebRequest -Uri $SignatureUrl -OutFile $TempSignaturePath -UseBasicParsing
        Invoke-WebRequest -Uri $PublicKeyUrl -OutFile $TempPubKeyPath -UseBasicParsing
        & gpg --import $TempPubKeyPath 2>&1 | Out-Null
        $GpgResult = & gpg --verify $TempSignaturePath $TempZipPath 2>&1
        if ($LASTEXITCODE -ne 0) { throw "GPG signature verification failed. $GpgResult" }
        Write-Success "GPG signature verified successfully."
    } else {
        Write-Warning "GPG not found. Skipping signature verification."
    }

    Write-Info "Extracting binary..."
    Expand-Archive -Path $TempZipPath -DestinationPath $TempDir -Force
    $TempBin = Join-Path $TempDir "zoi-mini.exe"
    if (-not (Test-Path $TempBin)) { throw "Could not find 'zoi-mini.exe' in archive." }

    $cmd = "install"
    $cmdArgs = $args
    if ($args.Count -gt 0) {
        switch ($args[0]) {
            { $_ -in @("install", "i", "update", "up", "uninstall", "un", "list", "ls") } {
                $cmd = $args[0]
                $cmdArgs = if ($args.Count -gt 1) { $args[1..($args.Count - 1)] } else { @() }
            }
        }
    }

    Write-Info "Executing Zoi Mini $cmd..."
    & $TempBin $cmd $cmdArgs
} finally {
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
}
