param(
    [string]$TargetArch = "aarch64-pc-windows-msvc"
)

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
