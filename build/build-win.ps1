param(
    [string]$TargetArch = "aarch64-pc-windows-msvc"
)

# Install Visual C++ Build Tools
Write-Output "Installing Visual C++ Build Tools..."
$VSBuildToolsInstaller = "vs_buildtools.exe"
Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile $VSBuildToolsInstaller
Start-Process -FilePath $VSBuildToolsInstaller -ArgumentList "--quiet", "--wait", "--norestart", "--nocache", "--add", "Microsoft.VisualStudio.Workload.VCTools", "--includeRecommended" -Wait

# Install Rust
Write-Output "Installing Rust..."
$RustupInitExe = "rustup-init.exe"
Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile $RustupInitExe
./$RustupInitExe -y
$Env:Path += ";$Env:USERPROFILE\.cargo\bin"

Write-Output "Building for Windows ARM64..."
rustup target add $TargetArch
cargo build --target $TargetArch --release --verbose

# Create the release directory if it doesn't exist
$ReleaseDir = "./build/release"
if (-not (Test-Path -Path $ReleaseDir)) {
    New-Item -ItemType Directory -Path $ReleaseDir
}

# Move the built executable to the release directory
$ExeName = "zoi.exe"
$SourcePath = "./target/$TargetArch/release/$ExeName"
$DestinationPath = "$ReleaseDir/zoi-windows-arm64.exe"
Move-Item -Path $SourcePath -Destination $DestinationPath -Force
