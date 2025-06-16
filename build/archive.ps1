$ErrorActionPreference = "Stop"

$CompiledDir = "build/compiled"
$ArchiveDir = "build/archived"
$ChecksumFile = Join-Path $ArchiveDir "checksums.txt"
$7zipPath = ""

$7zipInPath = Get-Command 7z -ErrorAction SilentlyContinue
if ($7zipInPath) {
    $7zipPath = $7zipInPath.Source
}
elseif (Test-Path "C:\Program Files\7-Zip\7z.exe") {
    $7zipPath = "C:\Program Files\7-Zip\7z.exe"
}
elseif (Test-Path "C:\Program Files (x86)\7-Zip\7z.exe") {
    $7zipPath = "C:\Program Files (x86)\7-Zip\7z.exe"
}
else {
    Write-Host "Error: 7-Zip command-line tool (7z.exe) not found." -ForegroundColor Red
    Write-Host "Please install 7-Zip and ensure it's in your PATH or default location." -ForegroundColor Yellow
    exit 1
}

Write-Host "Using 7-Zip from: $7zipPath" -ForegroundColor Cyan

if (-not (Test-Path $CompiledDir)) {
    Write-Host "Error: Compiled directory '$CompiledDir' not found." -ForegroundColor Red
    Write-Host "Hint: Run ./build/build-all.ps1 first." -ForegroundColor Cyan
    exit 1
}

if (Test-Path $ArchiveDir) {
    Remove-Item -Recurse -Force $ArchiveDir
}
New-Item -ItemType Directory -Path $ArchiveDir | Out-Null
Write-Host "📦 Starting archival process..." -ForegroundColor Cyan

Get-ChildItem -Path $CompiledDir -File | ForEach-Object {
    $binaryFile = $_
    $filename = $binaryFile.Name

    $finalBinaryName = "zoi"
    if ($binaryFile.Extension -eq ".exe") {
        $finalBinaryName = "zoi.exe"
    }

    $archiveBasename = $binaryFile.BaseName
    
    Write-Host "  -> Archiving $filename..." -ForegroundColor Cyan

    $tmpDir = New-Item -ItemType Directory -Path (Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString()))
    Copy-Item -Path $binaryFile.FullName -Destination (Join-Path $tmpDir.FullName $finalBinaryName)

    if ($filename -like "*windows*") {
        $archivePath = Join-Path $ArchiveDir "$archiveBasename.zip"
        & $7zipPath a -tzip "$archivePath" (Join-Path $tmpDir.FullName "*") -mx=9 | Out-Null
    }
    else {
        $tarPath = Join-Path $ArchiveDir "$archiveBasename.tar"
        $xzPath = Join-Path $ArchiveDir "$archiveBasename.tar.xz"
        
        & $7zipPath a -ttar "$tarPath" (Join-Path $tmpDir.FullName "*") | Out-Null
        & $7zipPath a -txz "$xzPath" "$tarPath" | Out-Null
        Remove-Item $tarPath
    }

    Remove-Item -Recurse -Force $tmpDir.FullName
}

Write-Host "🔐 Generating checksums..." -ForegroundColor Cyan
$checksums = @()
Get-ChildItem -Path $ArchiveDir -File | ForEach-Object {
    if ($_.Name -ne "checksums.txt") {
        $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
        $checksums += "$hash  $($_.Name)"
    }
}
$checksums | Out-File -FilePath $ChecksumFile -Encoding utf8

Write-Host "`n✅ Archiving and checksum generation complete!" -ForegroundColor Green
Write-Host "Output files are in the '$ArchiveDir' directory." -ForegroundColor Cyan
Get-ChildItem -Path $ArchiveDir | Select-Object Name, @{Name="Size (KB)"; Expression={"{0:N0}" -f ($_.Length / 1KB)}}