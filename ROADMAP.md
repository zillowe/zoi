# Roadmap

This document outlines the future direction of Zoi, including planned features, enhancements, and long-term goals. Our roadmap is shaped by community feedback and our vision to create a truly universal and developer-friendly package manager.

> **Note:** This roadmap is a living document. Priorities may change, and features may be added or removed based on development progress and community input.

---

## Beta 5.0.0

This release focuses on improving project-local workflows, enhancing security, and adding quality-of-life enhancements.

### Core Package Management

- [ ] **Project-Local Packages:** Install packages to a project-specific `.zoi/` directory using a `--local` flag, runnable with `zoi exec`.

- [ ] **MCP (AI Completion Proxy) Server (`mcp` command):** Introduce a new `mcp` package type and `zoi mcp` command to manage local AI completion tool servers.
      This feature allows Zoi to manage different AI coding assistants (like Codex, Gemini, Claude) as local servers or binaries. An `mcp` package can be an HTTP server or a binary that is not added to the user's PATH.
      **Supported Tools:**
      First-party support is planned for tools like `OpenCode (SST)`, `Gemini CLI`, `Codex CLI`, `Claude Code`, `VSCode`, `Cursor`, and `Windsurf`.
      **Commands:**

```sh
# Add or remove an MCP server for a specific tool
zoi mcp add <package> <tool>
zoi mcp rm <package> <tool>

# Execute an MCP binary directly
zoi mcp exec <package>
```

### Security & Integrity

- [ ] **PGP Key Management (`zoi pgp`):** Introduce a `pgp` command to manage public keys for verifying package signatures.

### Ecosystem & Contribution

- [ ] **Package Publishing Workflow (`zoi publish`):** Streamline submitting new packages via the `publish` command, which will auto-generate an issue/PR to the `Zoi-Pkgs` repo.
- [ ] **Install Packages From Git Repos:** Adding the ability to install a package from a git repo (like how we install it from a URL), support are for these git providers: GitHub, GitLab and Codeberg.
      This new `--repo` command will look into the repo for a field in `zoi.yaml` that defines the package location, either a URL, local package in the repo or a package in the registry that Zoi installs, e.g. `@community/editors/my-app`, all that without cloning the repo.
      **Commands:**

```sh
$ zoi install --repo Zillowe/Hello # default is GitHub
$ zoi install --repo gl:Zillowe/Hello # gh: GitHub, gl: GitLab, cb: Codeberg, you can use full names instead, e.g. codeberg:Zillowe/Hello
```

- [ ] **Cloud-Native Registries (S3/R2 Support):** Add support for S3-compatible object storage as a package registry backend.
      **Commands:**

```sh
$ zoi sync set this-is-a-url --s3 # or --r2
$ Choose if its S3 AWS or S3 compatible # s3 only
$ Enter credentials # saved at the global config
```

### Enhancements & Improvements

- [ ] **Advanced Platform Selectors:** Enhance the `platforms` field in `pkg.yaml` to allow for more granular targeting (OS version, kernel, DE, CPU/GPU, etc.).
      **Code:**

```yaml
platforms:
  - os: [linux]
    arch: [amd64, arm64]
    distro: [ubuntu, debian] # optional
    server: [wayland, xorg] # optional
    version: ^24.04 # optional, os/distro version, semver
    kernel: ^6.16.2 # optional, linux kernel version, semver
    de: [gnome, plasma] # optional
    wm: [kwinn, hyprland] # optional
    cpu: [intel, amd] # optional
    gpu: [nvidia@^340, amd] # optional, @ for driver version, semver
```

- [ ] **Improved `zoi make` Command:** Improve the TUI and validation for the interactive package creation tool.
- [ ] **Bsdiff Self-Update Improvements:** Fix and stabilize the patch-based self-update mechanism for `zoi upgrade`.
- [ ] **Expanded Platform Support:** Add binary and package support for more platforms, starting with Windows (ARM64) and FreeBSD/OpenBSD.

---

## Beta 6.0.0

This release will focus on a foundational rebuild of the packaging system and a major refactor to improve performance and maintainability.

This should be the last release before `Release 1.0.0`

### Core Package Management

- [ ] **Archival Packaging System (`zoi package`):** Re-architect the installation flow to use a robust, self-contained package format (`.pkg.tar.zst`), similar to `pacman`. This will make installations faster and more reliable.

```sh
$ zoi install fastfetch
# Here we ask for confirmation if there's conflicts and checking if it's work on the user platform
$ ... installing dependencies
# Here we ask for confirmation about options and optional dependencies
$ ... downloading the package
$ ... preparing the package
# Here's the fastfetch.pkg.tar.zst package archive begins installing, we need just that file for installing packages
$ ... installing the package
$ fastfetch from main installed!
```

### Codebase & Performance

- [ ] **Major Refactor:** Undertake a significant refactoring of the codebase to improve modularity, performance, and prepare for a stable 1.0 release.

### Ecosystem & Contribution

- [ ] **Enhanced Library & API Experience:** Improve the public API and documentation to make Zoi a powerful library for other Rust applications.

---

## Future & Long-Term Vision

These are features and ideas we are considering for future releases. They are not yet scheduled but represent the direction we want to take Zoi.

- [ ] **Full Platform Parity:** Achieve full build and package support for all targeted platforms, including Android (Termux).
- [ ] **Managed Components (`zoi component`):** Introduce a new package type for managed, isolated developer tools (e.g. language servers, linters) that are not added to the user's PATH, but are managed by Zoi and can be executed via `zoi component exec` or integrated with other developer tools.

---

## Contributing to the Roadmap

To add or suggest an item for the roadmap, please open an issue or a pull request. Follow the style guide below to ensure consistency.

### Roadmap Item Style

Each item in the roadmap should be a checklist item (`- [ ]`) and follow this structure:

1.  **Title:** A short, descriptive title in bold. If the feature introduces a new command, include it in backticks.
    - `- [ ] **Project-Local Packages:**`
    - `- [ ] **PGP Key Management ('zoi pgp'):**`

2.  **Description:** A concise, one-sentence description of the feature's purpose and benefit.
    - `- [ ] **Project-Local Packages:** Install packages to a project-specific ".zoi/" directory using a '--local' flag, runnable with 'zoi exec'`.

3.  **More Info (Optional):** For complex features, you can add more details, user stories, or examples in a block below the main item. Use indentation to keep it visually associated with the checklist item.
    - **Code Examples:** For features that change `pkg.yaml` or introduce new commands, provide a clear example in a YAML or shell code block.

#### Example:

````markdown
- [ ] **Advanced Platform Selectors:** Enhance the `platforms` field in `pkg.yaml` to allow for more granular targeting (OS version, kernel, DE, CPU/GPU, etc.).

      ```yaml
      platforms:
        - os: [linux]
          arch: [amd64]
          distro: [ubuntu]
          version: "^24.04"
          gpu: [nvidia@^550]
      ```
````

### Roadmap Sections

When adding a new item, please place it under one of the following pre-defined sections. If a suitable section doesn't exist, you can propose a new one.

- **Core Package Management:** Features related to the fundamental processes of installing, updating, and managing packages.
- **Enhancements & Improvements:** General improvements to existing features, user experience, and quality-of-life changes.
- **Codebase & Performance:** Changes related to code health, refactoring, performance optimization, and internal architecture.
- **Security & Integrity:** Features focused on package verification, trust, and protecting users.
- **Ecosystem & Contribution:** Features that make it easier for the community to contribute packages and interact with the Zoi project.
