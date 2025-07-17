if (!(Test-Path "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools")) {
  Write-Host "Installing Visual Studio Build Tools..."
  Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile "vs_buildtools.exe"
  Start-Process -FilePath "vs_buildtools.exe" -ArgumentList "--quiet", "--wait", "--add", "Microsoft.VisualStudio.Workload.VCTools", "--add", "Microsoft.VisualStudio.Component.VC.Tools.ARM64", "--add", "Microsoft.VisualStudio.Component.Windows11SDK.22621" -Wait
  Remove-Item "vs_buildtools.exe"
}

$vsPath = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
  if (Test-Path $vsPath) {
    cmd /c "`"$vsPath`" arm64 && set" | ForEach-Object {
      if ($_ -match "^([^=]+)=(.*)") {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2])
    }
  }
}

if (!(Get-Command rustc -ErrorAction SilentlyContinue)) { 
    Invoke-WebRequest -Uri "https://win.rustup.rs/" -OutFile "rustup-init.exe"
    .\rustup-init.exe -y --default-toolchain stable
    $env:PATH += ";$env:USERPROFILE\.cargo\bin"
    Remove-Item "rustup-init.exe"
}
