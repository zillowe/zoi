# Roadmap

This document outlines the future direction of Zoi, including planned features, enhancements, and long-term goals. Our roadmap is shaped by community feedback and our vision to create a truly universal and developer-friendly package manager.

> **Note:** This roadmap is a living document. Priorities may change, and features may be added or removed based on development progress and community input.

---

## v5 Beta: The Lua Overhaul

This release represents a foundational rebuild of the packaging system and a major refactor to improve performance, expressiveness, and maintainability. This is the last major beta release planned before a stable 1.0.0.

### Foundational Changes: Lua & The New Build System

- [x] **Transition to Lua-based Packages:** Replace the `pkg.yaml` format with `pkg.lua`. This allows for more dynamic, expressive, and maintainable package definitions, moving from a static data format to a sandboxed scripting language.

- [x] **Archival Packaging System (`zoi package`):** Re-architect the installation flow to use a robust, self-contained package format (`.pkg.tar.zst`), similar to `pacman`. This makes installations faster, more reliable, and enables offline installs from pre-built packages.
      This new system introduces several commands: - `zoi package meta ./path/to/name.pkg.lua`: Executes the package's Lua script in a secure sandbox to generate a static `name.meta.json` file. This file is crucial for package indexers and frontends. - `zoi package build`: Using the generated `name.meta.json`, this command fetches sources or binaries, verifies their integrity, and builds a standard `.pkg.tar.zst` archive for a specific platform. - `zoi package install ./path/to/name-os-arch.pkg.tar.zst`: Installs a package directly from a pre-built archive, allowing for fast, offline installations.

- [x] **Installation Scopes:** Introduce `--scope user` (default, installs to `~/.zoi`) and `--scope system` flags to control package installation locations, enabling better integration for system-level package management.

- [ ] **Enhanced Library & API Experience:** Improve the public API and documentation to make Zoi a powerful and ergonomic library for other Rust applications to leverage.

### Core Features & Enhancements

- [x] **PGP Key Management (`zoi pgp`):** Introduce a `pgp` command to manage public keys for verifying package signatures.

- [x] **Install Packages From Git Repos:** Add the ability to install a package directly from a git repository (GitHub, GitLab, Codeberg) without a full clone, using a field in a `zoi.yaml` file to locate the package definition.
      **Commands:**

```sh
$ zoi install --repo Zillowe/Hello # default is GitHub
$ zoi install --repo gl:Zillowe/Hello # gh: GitHub, gl: GitLab, cb: Codeberg
```

---

## Future & Long-Term Vision

These are features and ideas we are considering for future releases. They are not yet scheduled but represent the direction we want to take Zoi.

- [ ] **Full Platform Parity:** Achieve full build and package support for all targeted platforms, including Android (Termux).
- [ ] **Expanded Platform Support:** Add binary and package support for more platforms, starting with Windows (ARM64) and FreeBSD/OpenBSD.
- [ ] **Managed Components (`zoi component`):** Introduce a new package type for managed, isolated developer tools (e.g. language servers, linters) that are not added to the user's PATH, but are managed by Zoi and can be executed via `zoi component exec` or integrated with other developer tools.
- [ ] **Advanced Platform Selectors:** Enhance the `platforms` field in package definitions to allow for more granular targeting (OS version, kernel, DE, CPU/GPU, etc.).
- [ ] **Project-Local Packages:** Install packages to a project-specific `.zoi/` directory using a `--local` flag, runnable with `zoi exec`.
- [ ] **Bsdiff Self-Update Improvements:** Fix and stabilize the patch-based self-update mechanism for `zoi upgrade`.

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
    - **Code Examples:** For features that change package definitions or introduce new commands, provide a clear example in a code block.

#### Example:

````markdown
- [ ] **Advanced Platform Selectors:** Enhance the `platforms` field to allow for more granular targeting.

      ```lua
      -- Example in name.pkg.lua
      platforms = {
        { os = "linux", arch = "amd64", distro = "ubuntu", version = "^24.04" }
      }
      ```
````

### Roadmap Sections

When adding a new item, please place it under one of the following pre-defined sections. If a suitable section doesn\'t exist, you can propose a new one.

- **Foundational Changes:** Major architectural changes, language transitions, or core system rebuilds.
- **Core Features & Enhancements:** New commands, improvements to existing features, and quality-of-life changes.
- **Security & Integrity:** Features focused on package verification, trust, and protecting users.
- **Ecosystem & Contribution:** Features that make it easier for the community to contribute packages and interact with the Zoi project.
- **Codebase & Performance:** Changes related to code health, refactoring, performance optimization, and internal architecture.
