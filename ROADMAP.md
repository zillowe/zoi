# Roadmap

This document outlines the future direction of Zoi, including planned features, enhancements, and long-term goals. Our roadmap is shaped by community feedback and our vision to create a truly universal and developer-friendly package manager.

> **Note:** This roadmap is a living document. Priorities may change, and features may be added or removed based on development progress and community input.

---

## v1 Stable

This release is the first major stable release.

- [ ] **Repo renaming:** Rename the official Zoi repo to Zoidberg or rename the Zoi Repo term to Zoidberg.
- [ ] **Managed Components (`zoi component`):** Introduce a new package type for managed, isolated developer tools (e.g. language servers, linters) that are not added to the user's PATH, but are managed by Zoi and can be executed via `zoi component exec` or integrated with other developer tools.
- [ ] **Project-Local Packages:** Install packages to a project-specific `.zoi/` directory using a `--local` flag, runnable with `zoi exec`.
- [ ] **Bsdiff Self-Update Improvements:** Fix and stabilize the patch-based self-update mechanism for `zoi upgrade`.

---

## Future & Long-Term Vision

These are features and ideas we are considering for future releases. They are not yet scheduled but represent the direction we want to take Zoi.

- [ ] **Full Platform Parity:** Achieve full build and package support for all targeted platforms, including Android (Termux).
- [ ] **Expanded Platform Support:** Add binary and package support for more platforms, starting with Windows (ARM64) and FreeBSD/OpenBSD.
- [ ] **Advanced Platform Selectors:** Enhance the `platforms` field in package definitions to allow for more granular targeting (OS version, kernel, DE, CPU/GPU, etc.).

### Core Features & Enhancements

- [ ] **Delta Updates for All Packages:** Implement a patch-based update mechanism (e.g. `bsdiff`) for all packages to significantly reduce update download sizes and improve speed.

  This would extend the efficient self-update mechanism used by `zoi upgrade` to the entire package ecosystem. Instead of downloading a full package archive for every version change, `zoi update` would be able to fetch a much smaller binary patch file and apply it to the currently installed version.

  This would involve:
  1.  Generating patch files during the package publishing process in CI.
  2.  Including patch metadata in the repository database.
  3.  Adding logic to `zoi update` to prefer patches, apply them, and verify the resulting file's integrity.

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
