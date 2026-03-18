# Changelog

You can install any of these versions: `zoi upgrade --tag --force <tag>`

To install Zoi: `curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.sh | bash`, [more installation methods](https://zillowe.qzz.io/docs/zds/zoi).

## [Prod-Release-1.8.5] - 2026-03-18

### ✨ Features

- _(pkg)_ Add automatic build dependency installation

### 🧪 Testing

- _(service)_ Improve service management testability
- _(pkg)_ Add comprehensive test suite for package modules

## [Prod-Release-1.8.4] - 2026-03-08

### ♻️ Refactor

- Better code i think
- Centralize HTTP client and optimize registry sync

### ✨ Features

- _(install)_ Allow specifying exact package versions for install
- _(resolver)_ Implement semantic version range parsing

### 🩹 Bug Fixes

- _(pkg)_ Correct dependency version ranges and install scope

## [Prod-Release-1.8.3] - 2026-03-07

### ♻️ Refactor

- _(http)_ Centralize HTTP client creation and add user agent

### ✨ Features

- _(install)_ Add --build flag to force source compilation
- _(install)_ Skip already installed packages during installation
- _(lua)_ Allow functions to resolve paths relative to BUILD_DIR

### 🩹 Bug Fixes

- _(install)_ Make --repo flag work

## [Prod-Release-1.8.2] - 2026-03-06

### ♻️ Refactor

- _(db)_ Consolidate package updates to local database

## [Prod-Release-1.8.1] - 2026-03-06

### ⚡ Performance

- _(hashing)_ Stream data directly to hashers

## [Prod-Release-1.8.0] - 2026-03-05

### ♻️ Refactor

- _(doctor)_ Integrate external tool checks into doctor command
- _(pkg)_ Refactor package resolution and installation flow

### ✨ Features

- _(pgp)_ Allow non-interactive GPG signing with passphrase
- _(completion)_ Provide package descriptions for shell completion
- _(plugin, config)_ Add shim version hook and configure rollback default
- _(telemetry)_ Enhance data collection and status output
- _(pkg)_ Persist and optimize dependency resolution
- _(system)_ Extend declarative configuration with advanced options
- _(system)_ Add declarative system configuration
- _(pkg)_ Implement package shim system
- _(pkg)_ Add file system build operations to Lua scripts

### 🩹 Bug Fixes

- _(pkg)_ Refine package path handling and dependency resolution
- PGP

## [Prod-Release-1.7.0] - 2026-02-27

### ✨ Features

- _(completions)_ Add dynamic package name listing
- Add zig package manager
- _(cli)_ Add 'provides' command to find packages
- _(service)_ Add package background service management
- _(cmd)_ Add dependency tree visualization command
- _(cli)_ Add project development shell command
- _(shell)_ Add ephemeral shell for temporary environments
- _(cmd)_ Add dry-run flag for install and update
- _(search)_ Add global file search
- _(uninstall)_ Add recursive uninstall for orphaned dependencies
- _(list)_ Add --foreign flag to list packages
- _(resolver)_ Integrate PubGrub for robust dependency resolution
- _(hooks)_ Implement global hook system
- _(pkg)_ Add downgrade command
- _(pkg)_ Add global offline mode and cache commands
- _(pkg)_ Add 'mark' command to modify package installation reason
- _(db)_ Enhance package tracking with sub-packages and scope
- _(pkg)_ Add SQLite database for package metadata
- _(sysroot)_ Add option to define alternative root directory

### 🎯 UX

- _(pkg)_ Enhance multiple package selection with table display
- _(cli)_ Suppress zero size output in install and update commands

### 🔧 Configuration

- Add configurable offline mode and package directories

## [Prod-Release-1.6.0] - 2026-02-26

### ♻️ Refactor

- _(symlink)_ Centralize symlink creation logic

### ✨ Features

- _(pgp)_ Add support for builtin PGP keys
- _(security)_ Implement PGP signature verification for registries
- _(doctor)_ Add orphaned package detection
- _(audit)_ Add package operation audit log and history command
- _(search)_ Add interactive TUI and result sorting
- _(plugin)_ Introduce Lua-based plugin system
- _(pkg)_ Expose package directory paths to Lua
- _(lock)_ Implement advisory file locking
- _(archive)_ Add support for 7z, RAR, and DEB archive extraction
- _(pgp)_ Add key validation and status display

### 🛡️ Dependencies

- Update Cargo dependencies

## [Prod-Release-1.5.0] - 2026-02-22

### ♻️ Refactor

- _(shell)_ Abstract shell command execution

### ⚡ Performance

- _(sync)_ Synchronize multiple registries in parallel

### ✨ Features

- _(cli)_ Add dry-run option to autoremove and clean commands
- _(doctor)_ Add package and PGP health checks
- _(lua/utils)_ Add file system and archive utilities
- _(cli)_ Add --registry filter to list and search
- _(install)_ Enhance file conflict detection
- _(pkg)_ Allow configuring max package resolution depth
- _(uninstall)_ Add 'yes' flag for non-interactive mode
- _(pkg)_ Add Arch User Repository (AUR) support

### 🎯 UX

- _(install)_ Enhance install command with detailed progress and summary

### 🩹 Bug Fixes

- _(sync)_ Dynamically determine remote default branch

## [Prod-Release-1.4.0] - 2025-11-11

### ♻️ Refactor

- _(cli)_ Standardize error handling
- Use `unwrap_or_default` instead of `unwrap_or()`
- _(pkg)_ Decouple package download from installation
- More good code
- Better code i hope
- Merge 'setup' command into 'shell'

### ✨ Features

- _(pkg)_ Enhance package resolution and parsing

## [Prod-Release-1.3.1] - 2025-10-31

### ✨ Features

- _(pkg)_ Add support for typed build dependencies

## [Prod-Release-1.3.0] - 2025-10-29

### ✨ Features

- _(pkg)_ Implement package replacement, provides, and backup
- _(Lua)_ Add support for in package sources
- _(pkg)_ Refine package management UI

### 🔒 Security

- Closes #26 Docker image vulnerability

## [Prod-Release-1.2.2] - 2025-10-27

### ✨ Features

- _(pkg)_ Add 'output_dir' flag to 'cmd'
- Remove 'build_date' from 'manifest.yaml' in 'pkg.tar.zst'

## [Prod-Release-1.2.1] - 2025-10-26

### ✨ Features

- _(db)_ Implement package database write protection
- Remove 'sync' from 'update' command
- _(package)_ Implement test command and build integration

### 🩹 Bug Fixes

- _(upgrade)_ Retain temporary directory for fallback upgrade

## [Prod-Release-1.2.0] - 2025-10-25

### ✨ Features

- _(deps)_ Support sub-package dependencies
- _(pkg)_ Implement uninstall sub-package logic
- _(pkg)_ Implement support for split packages
- _(project)_ Introduce new zoi.lock format and verification
- Add Unkown license custom warning message
- _(doctor)_ Add doctor command for system diagnostics
- Better error messages
- _(pkg-policy)_ Implement package installation policy

### 🧹 Cleanup

- _(build)_ Remove redundant completion and man page binaries

## [Prod-Release-1.1.1] - 2025-10-22

### ✨ Features

- _(install)_ Implement file conflict detection and auto-overwrite

### 🩹 Bug Fixes

- _(uninstall)_ Resolve path placeholders

## [Prod-Release-1.1.0] - 2025-10-21

### ✨ Features

- _(install)_ Add option to specify package build type
- _(registry)_ Enable extensions to manage registries

## [Prod-Release-1.0.0] - 2025-10-21

### ♻️ Refactor

- Remove 'zoi build' command
- Remove patch upgrades and generation
- _(rollback)_ Improve package resolution logic
- _(pkg)_ Centralize package name resolution
- _(project)_ Use anyhow for error management
- _(pkg/build)_ Use anyhow for error handling
- Establish core utilities and package configuration
- _(lib)_ Simplify package management library API
- _(pkg)_ Move update logic and enhance version cleanup
- Remove Library, Config and Service package type
- _(pkg)_ Revamp package definitions and build lifecycle
- _(pkg)_ Streamline package lifecycle operations
- _(pkg)_ Enhance package execution and extension handling
- _(pkg)_ Improve package pinning logic
- _(pkg)_ Enhance dependency resolution and autoremoval
- _(install)_ Implement version-aware package installation
- _(cmd)_ Standardize CLI command definitions and package resolution
- _(core)_ Overhaul package module and type definitions
- _(install)_ Modularize package installation logic
- _(cmd)_ Handle optional repo name for warnings
- _(pkg)_ Revamp repository configuration and sync
- _(pkg)_ Improve package retrieval with repo filters
- _(utils)_ Refactor PATH environment variable check
- Rename Zoi-Pkgs to Zoidberg
- _(pkg)_ Pass resolved version to Lua parser

### ✨ Features

- _(lua)_ Run cmd_util commands in build directory
- _(telemetry)_ Add registry handle to events
- _(pkg)_ Implement transaction system for package operations
- _(pkg)_ Allow explicit version for package build and install
- _(uninstall)_ Add scope options for uninstall command
- _(install)_ Implement parallel package installation
- _(create)_ Revamp app creation with package templates
- _(install)_ Add multi-progress bars for parallel operations
- _(cmd)_ Improve package CLI commands and error handling
- _(cli)_ Add CLI commands for package state and queries
- _(pkg)_ Implement package rollback system
- _(extension)_ Introduce package extension management
- _(pgp)_ Integrate PGP for package verification
- _(pkg)_ Add package lifecycle management operations
- Implement robust package installation and execution flow
- _(pkg)_ Add package recording and robust error handling
- _(install)_ Add --save option for project packages
- _(hooks)_ Add package lifecycle hooks
- _(cli)_ Add 'owner' and 'files' commands
- _(pkg)_ Implement global lock and atomic package installation
- _(build)_ Add PGP signing for packages
- _(install)_ Add project scope and CLI flags
- _(config)_ Implement layered configuration system
- _(ext)_ Implement PGP key management to extensions
- _(about)_ Add packager information to about command
- _(lockfile)_ Implement zoi.lock for package integrity
- _(pkg)_ Introduce project-local package scope
- _(lua/utils)_ Add find and enhance extract utilities
- _(lua)_ Add utility to extract various archive formats
- _(security)_ Add PGP signature verification and MD5 hashing to Lua
- _(pgp)_ Add command to verify detached signatures
- _(lua)_ Add advanced Git API and file import to Lua
- _(lua)_ Introduce Lua scripting utilities
- _(pkg-keys)_ Enhance key management for signature verification
- _(about)_ Show PostHog and Registry configuration
- _(pgp)_ Add command to show stored public key
- _(upgrade)_ Display changelog link after successful upgrade
- _(pkg)_ Enhance repository filtering and display
- _(man)_ Generate man pages for subcommands
- _(pkg)_ Refine build command mapping for OS platforms
- _(meta)_ Add meta command to generate resolved package JSON
- _(resolve)_ Add support for direct git package sources
- _(pkg)_ Enhance package resolution and initial config
- _(registry)_ Display descriptions and refine repo resolution
- _(cli)_ Add helper command
- _(registry)_ Implement support for multiple package registries
- _(pkg)_ Enhance package installation with PGP verification
- _(upgrade)_ Warn when self-upgrading package manager installations
- _(install)_ Implement installer package method and uninstall
- _(install)_ Prevent redundant manual installs after binary installation
- _(meta)_ Add version argument for metadata generation
- _(cli)_ Add hidden command to print man page
- _(packaging)_ Add man page generation to package builds
- _(lua)_ Add fetch utility for making web requests
- _(pkg/package)_ Expand platform resolution for architecture inference

### ➡️ Migrations

- _(lockfile)_ Introduce custom package lockfile

### 🎯 UX

- _(pgp)_ Add 'rm' alias for remove command

### 🏗️ Structure

- _(scripts)_ Rename build directory to scripts

### 🔒 Security

- _(reporting)_ Update vulnerability reporting guidelines

### 🔧 Configuration

- _(registry)_ Use build-time configurable default registry
- _(Cargo)_ Specify minimum Rust version

### 🛠️ Build

- _(build)_ Refactor environment variable loading
- Update zoi.yaml
- _(cargo)_ Gate utility binaries behind 'tools' feature
- Update Cargo dependencies and minimum Rust version to 1.88.0
- Add '--bin zoi' to build scripts
- _(tools)_ Add CLI completion and man page generation
- Add 'build' make command
- Add 'help' make command
- _(setup)_ Consolidate shell configuration
- Remove FreeBSD/OpenBSD support
- Update build scripts

### 🛡️ Dependencies

- Update Cargo dependencies
- Add rayon parallel iteration library
- _(cargo)_ Remove unused cyclonedx-bom and purl crates

### 🩹 Bug Fixes

- _(sync)_ Use compiled-in default registry when unset
- Remove installed_at for zoi.lock
- _(pkg)_ Improve uninstall error handling and messages
- _(pkg)_ Remove symlinks before package directory during uninstall
- _(install)_ Prevent duplicate package installations
- Tests in lib.rs
- _(pkg)_ Ensure symlinks are removed on uninstall
- _(packaging)_ Use GitLab project ID for release fetching
- _(update)_ Correct package resolution for update command
- _(path)_ Correct PATH verification for custom definitions
- _(pkg)_ Prevent resolution of nested packages

## [Prod-Beta-5.0.5] - 2025-09-09

### ➡️ Migrations

- _(pkg)_ Use Lua for package definitions

## [Prod-Beta-5.0.4] - 2025-09-09

### ✨ Features

- _(package)_ Add custom file staging and installation

## [Prod-Beta-5.0.3] - 2025-09-09

### ✨ Features

- _(package)_ Add Docker build support for source packages

## [Prod-Beta-5.0.0] - 2025-09-09

### ♻️ Refactor

- _(pkg)_ Simplify archive filename and URL template
- _(cmd)_ Adapt modules to new package resolution signature
- _(pkg)_ Remove dynamic variable replacements
- _(cli)_ Restructure update command arguments and improve help output

### ✨ Features

- _(meta)_ Allow specifying installation type for meta generation
- _(api)_ Expose core functionality as public library API
- _(package)_ Add source installation support
- _(pgp)_ Add command to search PGP keys
- _(package)_ Add multi-platform build capability
- _(pkg)_ Support direct package names in repo installs
- _(install)_ Add support for installing from git repositories
- _(pkg)_ Add package installation scope
- _(pkg)_ Implement meta-build-install and update package resolution
- _(pkg)_ Implement pre-built package installation from repos
- _(pgp)_ Implement PGP key import from URL and list command
- _(pgp)_ Add PGP key management
- _(package)_ Add package install command
- _(package)_ Add CLI commands for package creation
- _(pkg)_ Support structured package database
- _(extension)_ Allow extensions to manage project configuration file
- _(script-handler)_ Implement script package uninstallation
- _(man)_ Enhance man command with local caching and raw display
- _(dev-setup)_ Implement comprehensive testing and formatting
- _(cli)_ Add new 'man' command for package manuals
- _(script)_ Add support for script package type
- _(show)_ Display package installation status
- _(pkg)_ Implement interactive package selection
- _(install)_ Add --all-optional flag to install command
- _(show)_ Add license verification
- _(cli)_ Enhance package completions and auto-setup
- _(config)_ Allow platform-specific commands and environment variables
- _(sync)_ Add --no-pm flag to skip package manager checks
- _(sync)_ Add fallback mirrors for package database
- _(gemini)_ Add AI flow for GitLab operations

### ➡️ Migrations

- _(pkg-format)_ Switch to Lua for package definitions
- _(parser)_ Transition to Lua package definitions
- _(scripts)_ Migrate install scripts to zillowe.pages.dev

### 🎨 Styling

- _(cli)_ Add custom colors and styling to CLI output

### 🎯 UX

- _(cmd)_ Condense repository names in list and search output
- _(cli)_ Add package name suggestions to CLI arguments

### 🔒 Security

- _(package)_ Implement PGP signature verification
- _(install)_ Implement GPG signature verification

### 🔧 Configuration

- _(about)_ Add contact email to about command
- _(gemini)_ Set up client credentials

### 🛠️ Build

- _(sync)_ Load sync fallbacks from repo.yaml
- _(docker)_ Add Docker build configuration

### 🛡️ Dependencies

- _(cargo)_ Add mlua crate

### 🧹 Cleanup

- _(cli)_ Remove interactive package creation command

### 🩹 Bug Fixes

- _(build)_ Correct checksum mismatch error message formatting
- _(windows)_ Initialize colored crate output

## [Prod-Beta-4.3.7] - 2025-08-20

### ♻️ Refactor

- _(dependencies)_ Remove pre-installation conflict checks
- Enhance package resolution and CLI output

### ✨ Features

- _(pkg)_ Add package update command
- _(exec)_ Execute commands via shell

### 🧹 Cleanup

- _(pkg)_ Remove external command conflict check

## [Prod-Beta-4.3.6] - 2025-08-19

### ♻️ Refactor

- _(pkg-resolve)_ Remove alt source caching and improve download reliability
- _(path)_ Refine PATH check output logic

### ✨ Features

- _(install)_ Add license validation to packages

### ➡️ Migrations

- _(sbom)_ Migrate package recording to CycloneDX SBOM

## [Prod-Beta-4.3.5] - 2025-08-18

### ✨ Features

- _(shell)_ Add Elvish shell path setup
- _(shell)_ Add setup command to configure shell PATH

### 🎯 UX

- _(path)_ Enhance PATH warning for better user guidance

## [Prod-Beta-4.3.4] - 2025-08-18

### ✨ Features

- _(install)_ Add package recording and lockfile installation

## [Prod-Beta-4.3.3] - 2025-08-17

### ✨ Features

- _(pkg)_ Add sharable install manifests

## [Prod-Beta-4.3.2] - 2025-08-17

### ♻️ Refactor

- _(deps)_ Specify optional dependency type in installation output
- _(upgrade)_ Streamline patch upgrade by using current executable

### ✨ Features

- _(service)_ Add Docker Compose support
- _(about)_ Include documentation URL in output

## [Prod-Beta-4.3.1] - 2025-08-16

### ♻️ Refactor

- Address Clippy warnings across codebase

### ✨ Features

- _(pkg)_ Prompt user with important package updates
- _(pkg)_ Add library package type and pkg-config command
- _(pkg)_ Add rollback command and functionality
- _(extension)_ Add extension management commands
- _(config)_ Manage external git repositories

### 🛠️ Build

- Add dedicated lint command

## [Prod-Beta-4.3.0] - 2025-08-15

### ♻️ Refactor

- _(pkg)_ Improve source install binary linking

### ✨ Features

- _(search)_ Paginate search command output
- _(git)_ Add Codeberg support for latest tag resolution
- _(pkg)_ Add {git} placeholder to package install URLs
- _(show)_ Add specific binary types to package info
- _(pkg)_ Allow {git} placeholder in install URLs
- _(install)_ Implement binary package installation
- _(upgrade)_ Allow specifying tag or branch for upgrade
- _(shell)_ Add shell command for completion management

### 🛠️ Build

- _(release)_ Add notes script to CI artifacts

### 🩹 Bug Fixes

- _(ci)_ Fixing CI add bash
- _(pkg)_ Conditionally compile symlink calls for Unix

## [Prod-Beta-4.2.3] - 2025-08-13

### ✨ Features

- _(pkg)_ Resolve package versions from Git release tags

## [Prod-Beta-4.2.2] - 2025-08-13

### ✨ Features

- _(upgrade)_ Add full and force options

## [Prod-Beta-4.2.1] - 2025-08-13

### 🎯 UX

- _(cli)_ Improve auto-completion for source arguments

### 🩹 Bug Fixes

- _(dependencies)_ Fix parsing for package names starting with '@'

## [Prod-Beta-4.2.0] - 2025-08-12

### ♻️ Refactor

- Update Config.toml

### ✨ Features

- _(sync)_ Add registry management for package database
- _(pkg)_ Allow nested paths for git package sources
- _(pkg)_ Improve conflict detection

### 🏗️ Structure

- _(core)_ Rename package and restructure as library

## [Prod-Beta-4.1.3] - 2025-08-12

### 🛠️ Build

- _(pkg)_ Enhance dependency resolution robustness

## [Prod-Beta-4.1.2] - 2025-08-11

### ✨ Features

- _(cmd)_ Pass arguments to custom commands
- _(cmd)_ Add interactive package file creation command
- _(schema)_ Add JSON schema for pkg.yaml validation

### 🔧 Configuration

- _(pkg-config)_ Define Zoi package configuration schema

## [Prod-Beta-4.1.1] - 2025-08-11

### ✨ Features

- _(create)_ Add pre-creation check for existing app directory
- _(cmd)_ Add 'create' command for application packages

## [Prod-Beta-4.1.0] - 2025-08-11

### ✨ Features

- _(pkg)_ Add conflict detection for Zoi packages

### 🛡️ Dependencies

- Update

## [Prod-Beta-4.0.4] - 2025-08-09

### ✨ Features

- _(pkg)_ Add script and Volta package manager support
- _(deps)_ Add support for dependency versioning

## [Prod-Beta-4.0.3] - 2025-08-09

### ♻️ Refactor

- _(cli)_ Enhance input parsing and error handling

## [Prod-Beta-4.0.2] - 2025-08-09

### ✨ Features

- _(pkg)_ Add readme field to package type
- _(telemetry)_ Include package version

## [Prod-Beta-4.0.1] - 2025-08-09

### 🛠️ Build

- _(build)_ Use dotenvy for environment variable loading

## [Prod-Beta-4.0.0] - 2025-08-09

### ✨ Features

- _(telemetry)_ Add opt-in usage analytics
- _(install)_ Add tag and branch options for source installs
- Introduce package tags and improve network resilience

### 📈 Tracking

- _(telemetry)_ Track clone, exec, and uninstall commands

### 🔒 Security

- _(pkg)_ Warn on insecure HTTP downloads

### 🛡️ Dependencies

- _(cargo)_ Update and clean up dependencies

## [Prod-Beta-3.8.2] - 2025-08-08

### ✨ Features

- Add support for windows-arm64 binaries

## [Prod-Beta-3.8.0] - 2025-08-08

### ♻️ Refactor

- _(build)_ Improve binary patch generation and application

### ✨ Features

- _(deps)_ Expand supported package managers and document dependencies
- _(repo)_ Add git subcommands and command aliases
- _(deps)_ Enhance dependency schema with selectable options

### 🎯 UX

- _(dependencies)_ Enhance dependency output format

## [Prod-Beta-3.7.2] - 2025-08-07

### 🛠️ Build

- _(upgrade)_ Adjust patch upgrade strategy for archives

## [Prod-Beta-3.6.0] - 2025-08-07

### ♻️ Refactor

- _(pkg)_ Migrate GPG signature verification

### ✨ Features

- _(security)_ Add GPG key fingerprint support

## [Prod-Beta-3.5.0] - 2025-08-06

### ♻️ Refactor

- Move from 'sh' and 'cmd' to 'bash' and 'pwsh'

### 🔒 Security

- _(pkg)_ Implement GPG signature verification for package artifacts

## [Prod-Beta-3.4.2] - 2025-08-06

### ✨ Features

- _(pkg)_ Add pre-installation conflict detection
- _(pkg)_ Improve dependency handling and uninstallation

## [Prod-Beta-3.4.1] - 2025-08-05

### 🩹 Bug Fixes

- _(upgrade)_ Standardize version parsing for releases

## [Prod-Beta-3.4.0] - 2025-08-05

### ✨ Features

- Enhance package management and CLI command capabilities
- _(install)_ Enable multi-package installation
- _(sync)_ Add external Git repository synchronization

## [Prod-Beta-3.3.2] - 2025-08-04

### 🩹 Bug Fixes

- _(patch)_ Refine binary patch handling

## [Prod-Beta-3.3.1] - 2025-08-03

### ✨ Features

- _(pkg)_ Enhance package installation and resolution

## [Prod-Beta-3.3.0] - 2025-08-03

### ✨ Features

- _(repo)_ Allow adding git repos as package sources
- Add optional dependency resolution and CLI aliases

## [Prod-Beta-3.2.7] - 2025-08-02

### ✨ Features

- _(pkg)_ Add MacPorts and Conda package manager support

## [Prod-Beta-3.2.5] - 2025-07-31

### ♻️ Refactor

- _(upgrade)_ Use 'no\_' methods for HTTP compression

## [Prod-Beta-3.2.3] - 2025-07-31

### ✨ Features

- _(upgrade)_ Display download progress for patches

## [Prod-Beta-3.2.2] - 2025-07-31

### ✨ Features

- _(pkg)_ Add support for more dependency managers

## [Prod-Beta-3.2.0] - 2025-07-30

### ✨ Features

- Introduce service and config package types
