$ErrorActionPreference = "Stop"

Write-Host "--- Checking for and installing Visual Studio Build Tools... ---"

$vsInstallPath = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"

if (-not (Test-Path $vsInstallPath)) {
  Write-Host "Visual Studio Build Tools not found. Installing now..."
  
  Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile "vs_buildtools.exe"
  
  $vsArgs = "--quiet", "--wait", "--norestart", "--add", "Microsoft.VisualStudio.Workload.VCTools", "--add", "Microsoft.VisualStudio.Component.VC.Tools.ARM64", "--add", "Microsoft.VisualStudio.Component.Windows11SDK.22621"
  
  Start-Process -FilePath ".\vs_buildtools.exe" -ArgumentList $vsArgs -Wait
  
  Remove-Item "vs_buildtools.exe"
  
  Write-Host "Visual Studio Build Tools installed."
} else {
  Write-Host "Visual Studio Build Tools already installed."
}

Write-Host "--- Importing MSVC environment variables for cross-compilation... ---"

$vcvarsPath = "$vsInstallPath\VC\Auxiliary\Build\vcvarsall.bat"

if (Test-Path $vcvarsPath) {
  cmd /c "`"$vcvarsPath`" amd64_arm64 && set" | ForEach-Object {
    if ($_ -match "^([^=]+)=(.*)") {
      Set-Item -Path "env:$($matches[1])" -Value $matches[2]
    }
  }
  Write-Host "MSVC environment loaded."
} else {
  Write-Host "Could not find vcvarsall.bat!"
  exit 1
}

Write-Host "--- Installing Rust toolchain... ---"

if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) { 
    Invoke-WebRequest -Uri "https://win.rustup.rs/" -OutFile "rustup-init.exe"
    
    .\rustup-init.exe -y --default-toolchain stable
    
    $env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
    Remove-Item "rustup-init.exe"
    
    Write-Host "Rust installed."
} else {
    Write-Host "Rust is already installed."
}

Write-Host "--- Final Verification ---"
$linkerCheck = Get-Command link.exe -ErrorAction SilentlyContinue
if ($linkerCheck) {
  Write-Host "linker 'link.exe' found at: $($linkerCheck.Path)"
} else {
  Write-Host "CRITICAL: linker 'link.exe' still not found in PATH!"
  exit 1
}
Write-Host "Windows runner setup complete."
