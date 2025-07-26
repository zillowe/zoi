#!/usr/bin/env pwsh

param(
    [Switch]$NoPathUpdate = $false
)

$ErrorActionPreference = "Stop"

$GitLabProjectPath = "Zillowe/Zillwen/Zusty/Zoi"
$InstallDir = Join-Path $env:USERPROFILE ".zoi\bin"
$BinName = "zoi.exe"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Cyan }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
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
    $EncodedProjectPath = [System.Web.HttpUtility]::UrlEncode($GitLabProjectPath)
    $ApiUrl = "https://gitlab.com/api/v4/projects/$EncodedProjectPath/releases"
    
    $Releases = Invoke-RestMethod -Uri $ApiUrl -Method Get
    
    $LatestTag = $Releases[0].tag_name


    if ([string]::IsNullOrEmpty($LatestTag)) {
        throw "API response did not contain a valid tag name."
    }
    Write-Info "Latest tag found: $LatestTag"
}
catch {
    Write-Error-Exit "Could not fetch the latest release tag. Please check the repository path and network." $_.Exception
}

$Os = "windows"
$Arch = ""
try {
    if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
        $Arch = 'arm64'
    } elseif ($env:PROCESSOR_ARCHITECTURE -eq 'AMD64') {
        $Arch = 'amd64'
    } else {
        throw "Zoi currently requires a 64-bit (x64 or ARM64) Windows system."
    }
} catch {
    Write-Error-Exit "Architecture detection failed." $_.Exception
}

$BaseUrl = "https://gitlab.com/$GitLabProjectPath/-/releases/$LatestTag/downloads"
$TargetArchive = "zoi-${Os}-${Arch}.zip"
$DownloadUrl = "$BaseUrl/$TargetArchive"
$ChecksumUrl = "$BaseUrl/checksums.txt"
$OutputPath = Join-Path $InstallDir $BinName

$TempDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$TempZipPath = Join-Path $TempDir $TargetArchive
$TempChecksumPath = Join-Path $TempDir "checksums.txt"

Write-Info "Installing/Updating Zoi for $Os ($Arch)..."
Write-Info "Target: $InstallDir"

if (-not (Test-Path $InstallDir)) {
    Write-Info "Creating installation directory: $InstallDir"
    New-Item -Path $InstallDir -ItemType Directory -Force | Out-Null
}

Write-Info "Downloading Zoi from: $DownloadUrl"
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZipPath -UseBasicParsing
    Write-Info "Downloaded successfully to: $TempZipPath"
}
catch {
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
    Write-Error-Exit "Could not download Zoi from $DownloadUrl" $_.Exception
}

Write-Info "Verifying checksum..."
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

    Write-Success "Checksum verified successfully."
}
catch {
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
    Write-Error-Exit "Security Verification Failed:" $_.Exception
}

if (Test-Path $OutputPath) {
    Write-Info "Removing existing binary at $OutputPath..."
    Remove-Item $OutputPath -Force -ErrorAction SilentlyContinue | Out-Null
}

Write-Info "Extracting archive to $InstallDir..."
try {
    Expand-Archive -Path $TempZipPath -DestinationPath $InstallDir -Force
    
    if (-not (Test-Path $OutputPath)) {
        throw "Could not find '$BinName' in the extracted archive."
    }
    
    Write-Success "Extraction successful."
}
catch {
    Write-Error-Exit "Could not extract archive $TempZipPath" $_.Exception
}
finally {
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
}

if (-not $NoPathUpdate) {
    Write-Info "Checking user PATH environment variable..."
    try {
        $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($UserPath -notlike "*$InstallDir*") {
            Write-Info "Adding '$InstallDir' to user PATH..."
            if (-not ([string]::IsNullOrEmpty($UserPath)) -and (-not $UserPath.EndsWith(";"))) {
                $UserPath += ";"
            }
            $NewPath = $UserPath + $InstallDir
            [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
            Write-Success "PATH updated. You must restart your terminal for the change to take effect."
        }
        else {
            Write-Info "'$InstallDir' is already in the user PATH."
        }
    }
    catch {
        Write-Warning "Could not automatically update user PATH. Error: $($_.Exception.Message)"
        Write-Warning "Please add '$InstallDir' to your PATH manually."
    }
}
else {
    Write-Info "Skipping PATH update as requested. Add '$InstallDir' to your PATH manually."
}

Write-Info "Installing PowerShell completions..."
try {
    $ProfilePath = $PROFILE
    if (-not (Test-Path (Split-Path $ProfilePath -Parent -ErrorAction SilentlyContinue))) {
        New-Item -ItemType Directory -Path (Split-Path $ProfilePath -Parent) -Force | Out-Null
    }
    
    $CompletionScript = & "$OutputPath" generate-completions powershell
    $Comment = "# Zoi PowerShell completion"
    
    if (Test-Path $ProfilePath) {
        $ProfileContent = Get-Content $ProfilePath -Raw -ErrorAction SilentlyContinue
        if ($ProfileContent -notlike "*$Comment*") {
            Add-Content -Path $ProfilePath -Value ([System.Environment]::NewLine + $Comment + [System.Environment]::NewLine + $CompletionScript)
            Write-Success "Completion script added to your PowerShell profile."
        } else {
            Write-Info "Completion script already present in your PowerShell profile."
        }
    } else {
        Set-Content -Path $ProfilePath -Value ($Comment + [System.Environment]::NewLine + $CompletionScript)
        Write-Success "Created PowerShell profile and added completion script."
    }
    Write-Info "Please restart your shell or run '. `$PROFILE' to activate it."
}
catch {
    Write-Warning "Could not install PowerShell completions. Error: $($_.Exception.Message)"
    Write-Warning "You can install them manually by adding the output of 'zoi generate-completions powershell' to your profile."
}

Write-Host ""
Write-Success "Zoi ($TargetArchive) installed/updated successfully to: $InstallDir"
Write-Info "Run 'zoi --version' in a *new* terminal window to verify."
