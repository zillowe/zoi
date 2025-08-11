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

Zoi will download, verify, and extract the archive, then locate the executable:

- By default, it matches the package `name` (or `name.exe` on Windows).
- You can override this by specifying `binary_path` in the `com_binary` installation method. This can be a relative path (e.g. `bin/tool`) or just a filename (e.g. `tool.exe`). On Windows targets, if your `binary_path` does not end with `.exe`, Zoi will also look for the same path with `.exe` appended.
- If exactly one file is present after extraction, Zoi assumes it is the intended binary.

When installing, Zoi renames the found executable to the package name. If the located file ends with `.exe`, the installed filename will be `<package>.exe`; otherwise it will be `<package>`.

## Where this is implemented

- Installer: handles zip, tar.zst, tar.xz, tar.gz
- Exec (run without install): handles zip, tar.zst, tar.xz, tar.gz
- Self-upgrade (delta and full): uses zip and tar.zst paths for extracting Zoi's own archives

These formats are confirmed in the code paths that read and unpack archives:

- zip via `zip::ZipArchive`
- tar.gz via `flate2::read::GzDecoder` + `tar::Archive`
- tar.xz via `xz2::read::XzDecoder` + `tar::Archive`
- tar.zst via `zstd::stream::read::Decoder` + `tar::Archive`

Note: For source-based installations (`type: source`), you can specify optional `tag` or `branch` in the method to select a specific ref to build from. Only one may be provided. Both support `{version}` placeholders like `v{version}`.

## Tips

- Choose `zip` for Windows for best compatibility; use `tar.*` on Unix-like systems.
- Prefer `tar.zst` for smaller downloads and fast decompression when supported by your release tooling.
- Ensure the archive contains either a single file (the binary) or the binary named exactly as the package `name`.
