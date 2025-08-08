---
title: Supported Archives for Compressed Binaries
description: Archive formats supported by Zoi's `com_binary` installation method.
---

Zoi supports installing compressed binary releases via the `com_binary` method in `pkg.yaml`. The following archive formats are supported across install, exec, and upgrade paths:

- zip
- tar.gz (gz)
- tar.xz (xz)
- tar.zst (zstd)

## Usage in `pkg.yaml`

```yaml
installation:
  - type: com_binary
    url: "https://example.com/app-v{version}-{platform}.{platformComExt}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    platformComExt:
      linux: "tar.zst" # or tar.gz / tar.xz
      macos: "tar.zst" # or tar.gz / tar.xz
      windows: "zip"
```

Zoi will download, verify, and extract the archive, then locate the executable inside by matching the package `name` (with `.exe` on Windows). If exactly one file is present after extraction, Zoi assumes it is the intended binary.

## Where this is implemented

- Installer: handles zip, tar.zst, tar.xz, tar.gz
- Exec (run without install): handles zip, tar.zst, tar.xz, tar.gz
- Self-upgrade (delta and full): uses zip and tar.zst paths for extracting Zoi's own archives

These formats are confirmed in the code paths that read and unpack archives:

- zip via `zip::ZipArchive`
- tar.gz via `flate2::read::GzDecoder` + `tar::Archive`
- tar.xz via `xz2::read::XzDecoder` + `tar::Archive`
- tar.zst via `zstd::stream::read::Decoder` + `tar::Archive`

## Tips

- Choose `zip` for Windows for best compatibility; use `tar.*` on Unix-like systems.
- Prefer `tar.zst` for smaller downloads and fast decompression when supported by your release tooling.
- Ensure the archive contains either a single file (the binary) or the binary named exactly as the package `name`.
