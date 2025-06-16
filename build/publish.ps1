#!/usr/bin/env pwsh
param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$VersionTag,

    [Parameter(Mandatory=$true, Position=1)]
    [string]$VersionName
)

$ErrorActionPreference = "Stop"

$RepoName = "Zusty/Zoi"
$ArchivedDir = ".\build\archived"

if (-not (Get-Command fj -ErrorAction SilentlyContinue)) {
    Write-Host "Error: 'fj' command is not found." -ForegroundColor Red
    Write-Host "Please install the forgejo-cli and ensure it's in your PATH." -ForegroundColor Yellow
    exit 1
}

Write-Host "Starting Zoi Release Preparation for tag: $VersionTag" -ForegroundColor Yellow

Write-Host "`n🗑️  Cleaning up old artifacts..." -ForegroundColor Cyan
if (Test-Path ".\build\compiled") { Remove-Item -Recurse -Force ".\build\compiled" }
if (Test-Path $ArchivedDir) { Remove-Item -Recurse -Force $ArchivedDir }
Write-Host "✓ Cleanup complete." -ForegroundColor Green

Write-Host "`n🏗️  Running the build script..." -ForegroundColor Cyan
try {
    & .\build\build-all.ps1
}
catch {
    Write-Host "`n❌ Build process failed." -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "✓ Build process finished successfully." -ForegroundColor Green

Write-Host "`n📦 Running the archive script..." -ForegroundColor Cyan
try {
    & .\build\archive.ps1
}
catch {
    Write-Host "`n❌ Archival process failed." -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "✓ Archival process finished successfully." -ForegroundColor Green

Write-Host "`n✅ Release preparation complete! Artifacts are in '$ArchivedDir'." -ForegroundColor Green

Write-Host "`nStarting Publishing Process..." -ForegroundColor Yellow

Write-Host "`n🚀 Creating new release on Codeberg for tag '$VersionTag'..." -ForegroundColor Cyan
try {
    fj release create --tag $VersionTag $VersionName
    Write-Host "✓ Release created successfully." -ForegroundColor Green
}
catch {
    Write-Host "`n❌ Failed to create release. Does a release for this tag already exist?" -ForegroundColor Red
    exit 1
}

Write-Host "`n⬆️  Uploading assets to the release..." -ForegroundColor Cyan
$AssetCount = 0
Get-ChildItem -Path $ArchivedDir -File | ForEach-Object {
    $asset = $_
    if ($asset) {
        Write-Host "   - Uploading $($asset.Name)..."
        try {
            fj release asset create $VersionTag $asset.FullName
            $AssetCount++
        }
        catch {
            Write-Host "`n❌ Failed to upload asset '$($asset.Name)'." -ForegroundColor Red
        }
    }
}

Write-Host "`n✓ Uploaded $AssetCount assets successfully." -ForegroundColor Green
Write-Host "`n✅ Publishing for '$VersionName' ($VersionTag) is complete!" -ForegroundColor Green