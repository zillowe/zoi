#!/usr/bin/env pwsh
param(
    [Switch]$NoPathUpdate = $false 
)

$ErrorActionPreference = "Stop"

$RepoOwner = "Zusty"
$RepoName = "Zoi"
$BaseUrl = "https://codeberg.org/$RepoOwner/$RepoName/releases/download/latest"
$BinName = "zoi.exe"                             

$Os = "windows"
$Arch = ""
try {
    $SystemType = (Get-CimInstance Win32_ComputerSystem).SystemType
    if ($SystemType -match "x64-based") {
        $Arch = "amd64"
    }
    elseif ($SystemType -match "ARM64") {
        $Arch = "arm64" 
    }
    else {
        throw "Unsupported architecture: $SystemType"
    }
}
catch {
    Write-Error "Install Failed: Zoi currently requires a 64-bit (x64 or ARM64) Windows system."
    Write-Error $_.Exception.Message
    exit 1
}


$TargetArchive = "zoi-${Os}-${Arch}.zip"
$DownloadUrl = "$BaseUrl/$TargetArchive"
$ChecksumUrl = "$BaseUrl/checksums.txt"
$OutputPath = Join-Path $InstallDir $BinName

$TempDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$TempZipPath = Join-Path $TempDir $TargetArchive
$TempChecksumPath = Join-Path $TempDir "checksums.txt"


Write-Host "Installing/Updating Zoi for $Os ($Arch)..."

if (-not (Test-Path $InstallDir)) {
    Write-Host "Creating installation directory: $InstallDir"
    New-Item -Path $InstallDir -ItemType Directory -Force | Out-Null
}

Write-Host "Downloading Zoi from: $DownloadUrl"
try {
    if (Get-Command Start-BitsTransfer -ErrorAction SilentlyContinue) {
        Start-BitsTransfer -Source $DownloadUrl -Destination $TempZipPath
    } else {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZipPath -UseBasicParsing
    }
    Write-Host "Downloaded successfully to: $TempZipPath"
}
catch {
    Write-Error "Install Failed: Could not download Zoi from $DownloadUrl"
    Write-Error $_.Exception.Message
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
    exit 1
}

Write-Host "Verifying checksum..."
try {
    Invoke-WebRequest -Uri $ChecksumUrl -OutFile $TempChecksumPath -UseBasicParsing
    
    $ExpectedHash = (Get-Content $TempChecksumPath | Select-String -Pattern $TargetArchive).Line.Split(" ")[0]
    if (-not $ExpectedHash) {
        throw "Could not find checksum for '$TargetArchive' in the checksums file."
    }

    $ActualHash = (Get-FileHash -Path $TempZipPath -Algorithm SHA256).Hash.ToLower()

    if ($ActualHash -ne $ExpectedHash) {
        throw "Checksum mismatch! The downloaded file may be corrupt or tampered with."
    }

    Write-Host "Checksum verified successfully."
}
catch {
    Write-Error "Security Verification Failed: $($_.Exception.Message)"
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
    exit 1
}

if (Test-Path $OutputPath) {
    Write-Host "Removing existing binary at $OutputPath..."
    Remove-Item $OutputPath -Force -ErrorAction SilentlyContinue | Out-Null
}

Write-Host "Extracting archive..."
try {
    Expand-Archive -Path $TempZipPath -DestinationPath $InstallDir -Force
    
    $ExtractedExe = Join-Path $InstallDir "zoi.exe"
    if (-not (Test-Path $ExtractedExe)) {
        throw "Could not find 'zoi.exe' in the extracted archive."
    }
    
    
    Write-Host "Extraction successful to $InstallDir."
}
catch {
    Write-Error "Install Failed: Could not extract archive $TempZipPath"
    Write-Error $_.Exception.Message
    exit 1
}
finally {
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
}

if (-not $NoPathUpdate) {
    Write-Host "Checking user PATH environment variable..."
    try {
        $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($UserPath -notlike "*$InstallDir*") {
            Write-Host "Adding '$InstallDir' to user PATH..."
            $Separator = ""
            if (-not ([string]::IsNullOrEmpty($UserPath)) -and (-not $UserPath.EndsWith(";"))) {
                $Separator = ";"
            }
            $NewPath = $UserPath + $Separator + $InstallDir
            [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
            Write-Host "PATH updated. You need to restart your terminal for the change to take effect." -ForegroundColor Green
        }
        else {
            Write-Host "'$InstallDir' is already in the user PATH."
        }
    }
    catch {
        Write-Warning "Could not automatically update user PATH. Error: $($_.Exception.Message)"
        Write-Warning "Please add '$InstallDir' to your PATH manually."
    }
}
else {
    Write-Host "Skipping PATH update as requested. Add '$InstallDir' to your PATH manually."
}

Write-Host ""
Write-Host "Zoi ($TargetArchive) installed/updated successfully to: $InstallDir" -ForegroundColor Green
Write-Host "Run 'zoi --version' in a *new* terminal window to verify."