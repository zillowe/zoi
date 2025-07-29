param(
    [string]$TargetArch = "aarch64-pc-windows-msvc"
)

Write-Output "Installing Visual C++ Build Tools..."
$VSBuildToolsInstaller = "vs_buildtools.exe"
Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile $VSBuildToolsInstaller
Start-Process -FilePath $VSBuildToolsInstaller -ArgumentList "--quiet", "--wait", "--norestart", "--nocache", "--add", "Microsoft.VisualStudio.Workload.VCTools", "--includeRecommended" -Wait

Write-Output "Building for Windows ARM64..."
rustup target add $TargetArch
cargo build --target $TargetArch --release --verbose

$ReleaseDir = "./build/release"
if (-not (Test-Path -Path $ReleaseDir)) {
    New-Item -ItemType Directory -Path $ReleaseDir
}

$ExeName = "zoi.exe"
$SourcePath = "./target/$TargetArch/release/$ExeName"
$DestinationPath = "$ReleaseDir/zoi-windows-arm64.exe"
Move-Item -Path $SourcePath -Destination $DestinationPath -Force
