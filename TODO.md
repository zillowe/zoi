# Todo

This list tracks planned features, known bugs, and areas for improvement in Zoi.

## 🔴 High Priority (Bugs & Security)

- **Rollback Integrity:** Implement backup of non-Zoi-owned files before overwriting during installation. Currently, if Zoi overwrites a system file during an install that later fails, the original file is lost and cannot be restored by the transaction rollback.
- **Inconsistent Platform Support:** Update `src/pkg/upgrade.rs` to support FreeBSD and OpenBSD, aligning it with the platform detection logic in `src/utils.rs`.
- **Windows Binary "Linking":** Investigate using symbolic links (`mklink`) or hard links for binaries on Windows instead of `fs::copy` to save disk space and maintain behavioral consistency with Unix platforms.
- **GPG Validation:** Enhance `zoi pgp add` to check for key expiration and revocation status during import.
- **Recursive Resolution Safety:** Hard-limit or improve the prompt for `max_resolution_depth` to prevent edge-case infinite loops in complex `alt` package definitions.

## 🟡 Medium Priority (Features & Logic)

- **Zoi Doctor Auto-Fix:** Add a `--fix` flag to `zoi doctor` to automatically resolve common issues like broken symlinks, missing PATH entries, or registry inconsistencies.
- **Parallel External Managers:** Enable parallel execution of dependencies belonging to different external managers (e.g. running `npm` and `pip` installs concurrently) to speed up environment setup.
- **Enhanced Search Experience:** Improve `zoi search` with richer formatting (using `ratatui` or advanced tables), result sorting, and better metadata display (e.g. license status, popularity).
- **Expanded Archive Support:** Add support for `.7z`, `.rar`, and `.deb` (extracted as an archive) in the Lua `UTILS.EXTRACT` helper.
- **Version Pinning Parity:** Implement version pinning logic for managers that currently have "Partial" or "No" support in `docs/dependencies.mdx` (e.g. `brew`, `apk`, `dnf`).
- **Registry Metadata Sync:** Support metadata-only synchronization via compressed JSON/YAML over HTTP to provide a faster alternative to `git clone` for large registries.

## 🟢 Low Priority (Improvements & Expansion)

- **Shell Integration:** Provide a native `zoi shell-hook` (similar to `direnv`) to automatically load project environments (`zoi env`) when entering a directory.
- **Publishing Workflow:** Add a `zoi package publish` or `zoi repo push` command to streamline the process of submitting packages to registries.
- **Plugin Architecture:** Explore allowing WASM or Lua-based plugins to extend Zoi's core CLI functionality beyond configuration extensions.
- **TUI Package Manager:** Implement a Terminal User Interface (TUI) mode for interactive package browsing, installation, and management.
- **Sudo Wrapper Configuration:** Allow users to configure the privilege escalation tool (e.g. `sudo`, `doas`, `pexec`) used for system-wide operations.

## 📖 Documentation

- **Troubleshooting Guide:** Create a dedicated guide for common installation and environment errors.
- **Architecture Deep-Dive:** Add documentation explaining the Zoi store structure and transaction log format for library contributors.
- **Lua API Cookbook:** Expand the Lua API documentation with a "Cookbook" of common patterns (e.g. complex platform mapping, multi-stage builds).
- **Security Policy Manual:** Detailed guide on configuring enterprise-grade `policy` objects in `config.yaml`.
