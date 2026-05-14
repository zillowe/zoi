# zm.ps1 - Zero-install Zoi Mini script for Windows
# Usage: powershell -c "irm zillowe.pages.dev/zm.ps1 | iex" -args "i <package>"

$GitLabProjectId = "71087662"
$GitLabProjectPath = "Zillowe/Zillwen/Zusty/Zoi"

Write-Host "[INFO] Fetching Zoi Mini for windows(amd64)..." -ForegroundColor Cyan

try {
    $ApiUrl = "https://gitlab.com/api/v4/projects/$GitLabProjectId/releases"
    $Releases = Invoke-RestMethod -Uri $ApiUrl -Method Get
    $LatestTag = $Releases[0].tag_name
} catch {
    Write-Host "[ERROR] Could not fetch the latest release tag." -ForegroundColor Red
    exit 1
}

$BinUrl = "https://gitlab.com/$GitLabProjectPath/-/releases/$LatestTag/downloads/zoi-mini-windows-amd64.exe"
$TempBin = "$env:TEMP\zoi-mini.exe"

Write-Host "[INFO] Downloading from: $BinUrl" -ForegroundColor Cyan
Invoke-WebRequest -Uri $BinUrl -OutFile $TempBin -UseBasicParsing

$cmd = "install"
$cmdArgs = $args
if ($args.Count -gt 0) {
    switch ($args[0]) {
        { $_ -in @("install", "i", "update", "up", "uninstall", "un", "list", "ls") } {
            $cmd = $args[0]
            if ($args.Count -gt 1) {
                $cmdArgs = $args[1..($args.Count - 1)]
            } else {
                $cmdArgs = @()
            }
        }
    }
}

Write-Host "[INFO] Executing Zoi Mini $cmd..." -ForegroundColor Cyan
& $TempBin $cmd $cmdArgs
