# Changelog

You can install any of these versions: `zoi upgrade --tag --force <tag>`

To install Zoi: `curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.sh | bash`, [more installation methods](https://zillowe.qzz.io/docs/zds/zoi).

## [Prod-Release-1.11.0] - 2026-04-25

### ✨ Features

- *(pkg)* Add reproducible installs, transaction inspection, and mirror support
- *(pkg)* Add interactive selection for installed packages

### 🧹 Cleanup

- *(clippy)* Address collapsible_match warnings

### 🩹 Bug Fixes

- *(pkg)* Harden extension lifecycle and runtime state

## [Prod-Release-1.10.0] - 2026-04-02

### ✨ Features

- *(ux)* Standardize install/uninstall/update preflight, explain, and bump to 1.10.0
- *(migrate)* Convert Scoop manifests to full pkg.lua scaffolds
- *(install)* Enforce frozen lockfile and audit chain verification
- *(lib)* Enhance public API with typed options and resolution
- *(config)* Add parallel jobs unoverridable policy

### 🎯 UX

- *(install)* Add --verbose flag for package origins and preflight info

### 🔒 Security

- *(policy)* Enforce allow/deny and license rules

### 🩹 Bug Fixes

- *(resolver)* Improve local package and channel resolution

## [Prod-Release-1.9.4] - 2026-03-28

### 🎯 UX

- Thingy thing

### 🩹 Bug Fixes

- *(pkg)* Improve file download and symlink handling

## [Prod-Release-1.9.3] - 2026-03-23

### ⚡ Performance

- *(pkg-resolver)* Optimize dependency resolution with caching

### ✨ Features

- *(pkg)* Enhance system info and project handlers
- *(plugin)* Add extended Lua plugin APIs and project install hook
- *(list)* Add option to show outdated packages
- *(pkg)* Add platform filtering for package builds

## [Prod-Release-1.9.2] - 2026-03-21

### ✨ Features

- *(docker)* Enable GPG signing for Docker builds

## [Prod-Release-1.9.1] - 2026-03-21

### ✨ Features

- *(security)* Implement sub-package advisories and enforcement policy

## [Prod-Release-1.9.0] - 2026-03-21

### 🔒 Security

- *(audit)* Add security auditing command and vulnerability checks

## [Prod-Release-1.8.8] - 2026-03-21

### ✨ Features

- *(pm)* Add dynamic sudo handling for package managers

## [Prod-Release-1.8.7] - 2026-03-20

### ✨ Features

- *(build)* Make build type optional

## [Prod-Release-1.8.6] - 2026-03-19

### ✨ Features

- *(package)* Add Docker build method

### 🩹 Bug Fixes

- Bug from tests

## [Prod-Release-1.8.5] - 2026-03-18

### ✨ Features

- *(pkg)* Add automatic build dependency installation

### 🧪 Testing

- *(service)* Improve service management testability
- *(pkg)* Add comprehensive test suite for package modules

## [Prod-Release-1.8.4] - 2026-03-08

### ♻️ Refactor

- Better code i think
- Centralize HTTP client and optimize registry sync

### ✨ Features

- *(install)* Allow specifying exact package versions for install
- *(resolver)* Implement semantic version range parsing

### 🩹 Bug Fixes

- *(pkg)* Correct dependency version ranges and install scope

## [Prod-Release-1.8.3] - 2026-03-07

### ♻️ Refactor

- *(http)* Centralize HTTP client creation and add user agent

### ✨ Features

- *(install)* Add --build flag to force source compilation
- *(install)* Skip already installed packages during installation
- *(lua)* Allow functions to resolve paths relative to BUILD_DIR

### 🩹 Bug Fixes

- *(install)* Make --repo flag work

## [Prod-Release-1.8.2] - 2026-03-06

### ♻️ Refactor

- *(db)* Consolidate package updates to local database

## [Prod-Release-1.8.1] - 2026-03-06

### ⚡ Performance

- *(hashing)* Stream data directly to hashers

## [Prod-Release-1.8.0] - 2026-03-05

### ♻️ Refactor

- *(doctor)* Integrate external tool checks into doctor command
- *(pkg)* Refactor package resolution and installation flow

### ✨ Features

- *(pgp)* Allow non-interactive GPG signing with passphrase
- *(completion)* Provide package descriptions for shell completion
- *(plugin, config)* Add shim version hook and configure rollback default
- *(telemetry)* Enhance data collection and status output
- *(pkg)* Persist and optimize dependency resolution
- *(system)* Extend declarative configuration with advanced options
- *(system)* Add declarative system configuration
- *(pkg)* Implement package shim system
- *(pkg)* Add file system build operations to Lua scripts

### 🩹 Bug Fixes

- *(pkg)* Refine package path handling and dependency resolution
- PGP

## [Prod-Release-1.7.0] - 2026-02-27

### ✨ Features

- *(completions)* Add dynamic package name listing
- Add zig package manager
- *(cli)* Add 'provides' command to find packages
- *(service)* Add package background service management
- *(cmd)* Add dependency tree visualization command
- *(cli)* Add project development shell command
- *(shell)* Add ephemeral shell for temporary environments
- *(cmd)* Add dry-run flag for install and update
- *(search)* Add global file search
- *(uninstall)* Add recursive uninstall for orphaned dependencies
- *(list)* Add --foreign flag to list packages
- *(resolver)* Integrate PubGrub for robust dependency resolution
- *(hooks)* Implement global hook system
- *(pkg)* Add downgrade command
- *(pkg)* Add global offline mode and cache commands
- *(pkg)* Add 'mark' command to modify package installation reason
- *(db)* Enhance package tracking with sub-packages and scope
- *(pkg)* Add SQLite database for package metadata
- *(sysroot)* Add option to define alternative root directory

### 🎯 UX

- *(pkg)* Enhance multiple package selection with table display
- *(cli)* Suppress zero size output in install and update commands

### 🔧 Configuration

- Add configurable offline mode and package directories

## [Prod-Release-1.6.0] - 2026-02-26

### ♻️ Refactor

- *(symlink)* Centralize symlink creation logic

### ✨ Features

- *(pgp)* Add support for builtin PGP keys
- *(security)* Implement PGP signature verification for registries
- *(doctor)* Add orphaned package detection
- *(audit)* Add package operation audit log and history command
- *(search)* Add interactive TUI and result sorting
- *(plugin)* Introduce Lua-based plugin system
- *(pkg)* Expose package directory paths to Lua
- *(lock)* Implement advisory file locking
- *(archive)* Add support for 7z, RAR, and DEB archive extraction
- *(pgp)* Add key validation and status display

### 🛡️ Dependencies

- Update Cargo dependencies

## [Prod-Release-1.5.0] - 2026-02-22

### ♻️ Refactor

- *(shell)* Abstract shell command execution

### ⚡ Performance

- *(sync)* Synchronize multiple registries in parallel

### ✨ Features

- *(cli)* Add dry-run option to autoremove and clean commands
- *(doctor)* Add package and PGP health checks
- *(lua/utils)* Add file system and archive utilities
- *(cli)* Add --registry filter to list and search
- *(install)* Enhance file conflict detection
- *(pkg)* Allow configuring max package resolution depth
- *(uninstall)* Add 'yes' flag for non-interactive mode
- *(pkg)* Add Arch User Repository (AUR) support

### 🎯 UX

- *(install)* Enhance install command with detailed progress and summary

### 🩹 Bug Fixes

- *(sync)* Dynamically determine remote default branch

## [Prod-Release-1.4.0] - 2025-11-11

### ♻️ Refactor

- *(cli)* Standardize error handling
- Use `unwrap_or_default` instead of `unwrap_or()`
- *(pkg)* Decouple package download from installation
- More good code
- Better code i hope
- Merge 'setup' command into 'shell'

### ✨ Features

- *(pkg)* Enhance package resolution and parsing

## [Prod-Release-1.3.1] - 2025-10-31

### ✨ Features

- *(pkg)* Add support for typed build dependencies

## [Prod-Release-1.3.0] - 2025-10-29

### ✨ Features

- *(pkg)* Implement package replacement, provides, and backup
- *(Lua)* Add support for in package sources
- *(pkg)* Refine package management UI

### 🔒 Security

- Closes #26 Docker image vulnerability

## [Prod-Release-1.2.2] - 2025-10-27

### ✨ Features

- *(pkg)* Add 'output_dir' flag to 'cmd'
- Remove 'build_date' from 'manifest.yaml' in 'pkg.tar.zst'

## [Prod-Release-1.2.1] - 2025-10-26

### ✨ Features

- *(db)* Implement package database write protection
- Remove 'sync' from 'update' command
- *(package)* Implement test command and build integration

### 🩹 Bug Fixes

- *(upgrade)* Retain temporary directory for fallback upgrade

## [Prod-Release-1.2.0] - 2025-10-25

### ✨ Features

- *(deps)* Support sub-package dependencies
- *(pkg)* Implement uninstall sub-package logic
- *(pkg)* Implement support for split packages
- *(project)* Introduce new zoi.lock format and verification
- Add Unkown license custom warning message
- *(doctor)* Add doctor command for system diagnostics
- Better error messages
- *(pkg-policy)* Implement package installation policy

### 🧹 Cleanup

- *(build)* Remove redundant completion and man page binaries

## [Prod-Release-1.1.1] - 2025-10-22

### ✨ Features

- *(install)* Implement file conflict detection and auto-overwrite

### 🩹 Bug Fixes

- *(uninstall)* Resolve path placeholders

## [Prod-Release-1.1.0] - 2025-10-21

### ✨ Features

- *(install)* Add option to specify package build type
- *(registry)* Enable extensions to manage registries

## [Prod-Release-1.0.0] - 2025-10-21

### ♻️ Refactor

- Remove 'zoi build' command
- Remove patch upgrades and generation
- *(rollback)* Improve package resolution logic
- *(pkg)* Centralize package name resolution
- *(project)* Use anyhow for error management
- *(pkg/build)* Use anyhow for error handling
- Establish core utilities and package configuration
- *(lib)* Simplify package management library API
- *(pkg)* Move update logic and enhance version cleanup
- Remove Library, Config and Service package type
- *(pkg)* Revamp package definitions and build lifecycle
- *(pkg)* Streamline package lifecycle operations
- *(pkg)* Enhance package execution and extension handling
- *(pkg)* Improve package pinning logic
- *(pkg)* Enhance dependency resolution and autoremoval
- *(install)* Implement version-aware package installation
- *(cmd)* Standardize CLI command definitions and package resolution
- *(core)* Overhaul package module and type definitions
- *(install)* Modularize package installation logic
- *(cmd)* Handle optional repo name for warnings
- *(pkg)* Revamp repository configuration and sync
- *(pkg)* Improve package retrieval with repo filters
- *(utils)* Refactor PATH environment variable check
- Rename Zoi-Pkgs to Zoidberg
- *(pkg)* Pass resolved version to Lua parser

### ✨ Features

- *(lua)* Run cmd_util commands in build directory
- *(telemetry)* Add registry handle to events
- *(pkg)* Implement transaction system for package operations
- *(pkg)* Allow explicit version for package build and install
- *(uninstall)* Add scope options for uninstall command
- *(install)* Implement parallel package installation
- *(create)* Revamp app creation with package templates
- *(install)* Add multi-progress bars for parallel operations
- *(cmd)* Improve package CLI commands and error handling
- *(cli)* Add CLI commands for package state and queries
- *(pkg)* Implement package rollback system
- *(extension)* Introduce package extension management
- *(pgp)* Integrate PGP for package verification
- *(pkg)* Add package lifecycle management operations
- Implement robust package installation and execution flow
- *(pkg)* Add package recording and robust error handling
- *(install)* Add --save option for project packages
- *(hooks)* Add package lifecycle hooks
- *(cli)* Add 'owner' and 'files' commands
- *(pkg)* Implement global lock and atomic package installation
- *(build)* Add PGP signing for packages
- *(install)* Add project scope and CLI flags
- *(config)* Implement layered configuration system
- *(ext)* Implement PGP key management to extensions
- *(about)* Add packager information to about command
- *(lockfile)* Implement zoi.lock for package integrity
- *(pkg)* Introduce project-local package scope
- *(lua/utils)* Add find and enhance extract utilities
- *(lua)* Add utility to extract various archive formats
- *(security)* Add PGP signature verification and MD5 hashing to Lua
- *(pgp)* Add command to verify detached signatures
- *(lua)* Add advanced Git API and file import to Lua
- *(lua)* Introduce Lua scripting utilities
- *(pkg-keys)* Enhance key management for signature verification
- *(about)* Show PostHog and Registry configuration
- *(pgp)* Add command to show stored public key
- *(upgrade)* Display changelog link after successful upgrade
- *(pkg)* Enhance repository filtering and display
- *(man)* Generate man pages for subcommands
- *(pkg)* Refine build command mapping for OS platforms
- *(meta)* Add meta command to generate resolved package JSON
- *(resolve)* Add support for direct git package sources
- *(pkg)* Enhance package resolution and initial config
- *(registry)* Display descriptions and refine repo resolution
- *(cli)* Add helper command
- *(registry)* Implement support for multiple package registries
- *(pkg)* Enhance package installation with PGP verification
- *(upgrade)* Warn when self-upgrading package manager installations
- *(install)* Implement installer package method and uninstall
- *(install)* Prevent redundant manual installs after binary installation
- *(meta)* Add version argument for metadata generation
- *(cli)* Add hidden command to print man page
- *(packaging)* Add man page generation to package builds
- *(lua)* Add fetch utility for making web requests
- *(pkg/package)* Expand platform resolution for architecture inference

### ➡️ Migrations

- *(lockfile)* Introduce custom package lockfile

### 🎯 UX

- *(pgp)* Add 'rm' alias for remove command

### 🏗️ Structure

- *(scripts)* Rename build directory to scripts

### 🔒 Security

- *(reporting)* Update vulnerability reporting guidelines

### 🔧 Configuration

- *(registry)* Use build-time configurable default registry
- *(Cargo)* Specify minimum Rust version

### 🛠️ Build

- *(build)* Refactor environment variable loading
- Update zoi.yaml
- *(cargo)* Gate utility binaries behind 'tools' feature
- Update Cargo dependencies and minimum Rust version to 1.88.0
- Add '--bin zoi' to build scripts
- *(tools)* Add CLI completion and man page generation
- Add 'build' make command
- Add 'help' make command
- *(setup)* Consolidate shell configuration
- Remove FreeBSD/OpenBSD support
- Update build scripts

### 🛡️ Dependencies

- Update Cargo dependencies
- Add rayon parallel iteration library
- *(cargo)* Remove unused cyclonedx-bom and purl crates

### 🩹 Bug Fixes

- *(sync)* Use compiled-in default registry when unset
- Remove installed_at for zoi.lock
- *(pkg)* Improve uninstall error handling and messages
- *(pkg)* Remove symlinks before package directory during uninstall
- *(install)* Prevent duplicate package installations
- Tests in lib.rs
- *(pkg)* Ensure symlinks are removed on uninstall
- *(packaging)* Use GitLab project ID for release fetching
- *(update)* Correct package resolution for update command
- *(path)* Correct PATH verification for custom definitions
- *(pkg)* Prevent resolution of nested packages

## [Prod-Beta-5.0.5] - 2025-09-09

### ➡️ Migrations

- *(pkg)* Use Lua for package definitions

## [Prod-Beta-5.0.4] - 2025-09-09

### ✨ Features

- *(package)* Add custom file staging and installation

## [Prod-Beta-5.0.3] - 2025-09-09

### ✨ Features

- *(package)* Add Docker build support for source packages

## [Prod-Beta-5.0.0] - 2025-09-09

### ♻️ Refactor

- *(pkg)* Simplify archive filename and URL template
- *(cmd)* Adapt modules to new package resolution signature
- *(pkg)* Remove dynamic variable replacements
- *(cli)* Restructure update command arguments and improve help output

### ✨ Features

- *(meta)* Allow specifying installation type for meta generation
- *(api)* Expose core functionality as public library API
- *(package)* Add source installation support
- *(pgp)* Add command to search PGP keys
- *(package)* Add multi-platform build capability
- *(pkg)* Support direct package names in repo installs
- *(install)* Add support for installing from git repositories
- *(pkg)* Add package installation scope
- *(pkg)* Implement meta-build-install and update package resolution
- *(pkg)* Implement pre-built package installation from repos
- *(pgp)* Implement PGP key import from URL and list command
- *(pgp)* Add PGP key management
- *(package)* Add package install command
- *(package)* Add CLI commands for package creation
- *(pkg)* Support structured package database
- *(extension)* Allow extensions to manage project configuration file
- *(script-handler)* Implement script package uninstallation
- *(man)* Enhance man command with local caching and raw display
- *(dev-setup)* Implement comprehensive testing and formatting
- *(cli)* Add new 'man' command for package manuals
- *(script)* Add support for script package type
- *(show)* Display package installation status
- *(pkg)* Implement interactive package selection
- *(install)* Add --all-optional flag to install command
- *(show)* Add license verification
- *(cli)* Enhance package completions and auto-setup
- *(config)* Allow platform-specific commands and environment variables
- *(sync)* Add --no-pm flag to skip package manager checks
- *(sync)* Add fallback mirrors for package database
- *(gemini)* Add AI flow for GitLab operations

### ➡️ Migrations

- *(pkg-format)* Switch to Lua for package definitions
- *(parser)* Transition to Lua package definitions
- *(scripts)* Migrate install scripts to zillowe.pages.dev

### 🎨 Styling

- *(cli)* Add custom colors and styling to CLI output

### 🎯 UX

- *(cmd)* Condense repository names in list and search output
- *(cli)* Add package name suggestions to CLI arguments

### 🔒 Security

- *(package)* Implement PGP signature verification
- *(install)* Implement GPG signature verification

### 🔧 Configuration

- *(about)* Add contact email to about command
- *(gemini)* Set up client credentials

### 🛠️ Build

- *(sync)* Load sync fallbacks from repo.yaml
- *(docker)* Add Docker build configuration

### 🛡️ Dependencies

- *(cargo)* Add mlua crate

### 🧹 Cleanup

- *(cli)* Remove interactive package creation command

### 🩹 Bug Fixes

- *(build)* Correct checksum mismatch error message formatting
- *(windows)* Initialize colored crate output

## [Prod-Beta-4.3.7] - 2025-08-20

### ♻️ Refactor

- *(dependencies)* Remove pre-installation conflict checks
- Enhance package resolution and CLI output

### ✨ Features

- *(pkg)* Add package update command
- *(exec)* Execute commands via shell

### 🧹 Cleanup

- *(pkg)* Remove external command conflict check

## [Prod-Beta-4.3.6] - 2025-08-19

### ♻️ Refactor

- *(pkg-resolve)* Remove alt source caching and improve download reliability
- *(path)* Refine PATH check output logic

### ✨ Features

- *(install)* Add license validation to packages

### ➡️ Migrations

- *(sbom)* Migrate package recording to CycloneDX SBOM

## [Prod-Beta-4.3.5] - 2025-08-18

### ✨ Features

- *(shell)* Add Elvish shell path setup
- *(shell)* Add setup command to configure shell PATH

### 🎯 UX

- *(path)* Enhance PATH warning for better user guidance

## [Prod-Beta-4.3.4] - 2025-08-18

### ✨ Features

- *(install)* Add package recording and lockfile installation

## [Prod-Beta-4.3.3] - 2025-08-17

### ✨ Features

- *(pkg)* Add sharable install manifests

## [Prod-Beta-4.3.2] - 2025-08-17

### ♻️ Refactor

- *(deps)* Specify optional dependency type in installation output
- *(upgrade)* Streamline patch upgrade by using current executable

### ✨ Features

- *(service)* Add Docker Compose support
- *(about)* Include documentation URL in output

## [Prod-Beta-4.3.1] - 2025-08-16

### ♻️ Refactor

- Address Clippy warnings across codebase

### ✨ Features

- *(pkg)* Prompt user with important package updates
- *(pkg)* Add library package type and pkg-config command
- *(pkg)* Add rollback command and functionality
- *(extension)* Add extension management commands
- *(config)* Manage external git repositories

### 🛠️ Build

- Add dedicated lint command

## [Prod-Beta-4.3.0] - 2025-08-15

### ♻️ Refactor

- *(pkg)* Improve source install binary linking

### ✨ Features

- *(search)* Paginate search command output
- *(git)* Add Codeberg support for latest tag resolution
- *(pkg)* Add {git} placeholder to package install URLs
- *(show)* Add specific binary types to package info
- *(pkg)* Allow {git} placeholder in install URLs
- *(install)* Implement binary package installation
- *(upgrade)* Allow specifying tag or branch for upgrade
- *(shell)* Add shell command for completion management

### 🛠️ Build

- *(release)* Add notes script to CI artifacts

### 🩹 Bug Fixes

- *(ci)* Fixing CI add bash
- *(pkg)* Conditionally compile symlink calls for Unix

## [Prod-Beta-4.2.3] - 2025-08-13

### ✨ Features

- *(pkg)* Resolve package versions from Git release tags

## [Prod-Beta-4.2.2] - 2025-08-13

### ✨ Features

- *(upgrade)* Add full and force options

## [Prod-Beta-4.2.1] - 2025-08-13

### 🎯 UX

- *(cli)* Improve auto-completion for source arguments

### 🩹 Bug Fixes

- *(dependencies)* Fix parsing for package names starting with '@'

## [Prod-Beta-4.2.0] - 2025-08-12

### ♻️ Refactor

- Update Config.toml

### ✨ Features

- *(sync)* Add registry management for package database
- *(pkg)* Allow nested paths for git package sources
- *(pkg)* Improve conflict detection

### 🏗️ Structure

- *(core)* Rename package and restructure as library

## [Prod-Beta-4.1.3] - 2025-08-12

### 🛠️ Build

- *(pkg)* Enhance dependency resolution robustness

## [Prod-Beta-4.1.2] - 2025-08-11

### ✨ Features

- *(cmd)* Pass arguments to custom commands
- *(cmd)* Add interactive package file creation command
- *(schema)* Add JSON schema for pkg.yaml validation

### 🔧 Configuration

- *(pkg-config)* Define Zoi package configuration schema

## [Prod-Beta-4.1.1] - 2025-08-11

### ✨ Features

- *(create)* Add pre-creation check for existing app directory
- *(cmd)* Add 'create' command for application packages

## [Prod-Beta-4.1.0] - 2025-08-11

### ✨ Features

- *(pkg)* Add conflict detection for Zoi packages

### 🛡️ Dependencies

- Update

## [Prod-Beta-4.0.4] - 2025-08-09

### ✨ Features

- *(pkg)* Add script and Volta package manager support
- *(deps)* Add support for dependency versioning

## [Prod-Beta-4.0.3] - 2025-08-09

### ♻️ Refactor

- *(cli)* Enhance input parsing and error handling

## [Prod-Beta-4.0.2] - 2025-08-09

### ✨ Features

- *(pkg)* Add readme field to package type
- *(telemetry)* Include package version

## [Prod-Beta-4.0.1] - 2025-08-09

### 🛠️ Build

- *(build)* Use dotenvy for environment variable loading

## [Prod-Beta-4.0.0] - 2025-08-09

### ✨ Features

- *(telemetry)* Add opt-in usage analytics
- *(install)* Add tag and branch options for source installs
- Introduce package tags and improve network resilience

### 📈 Tracking

- *(telemetry)* Track clone, exec, and uninstall commands

### 🔒 Security

- *(pkg)* Warn on insecure HTTP downloads

### 🛡️ Dependencies

- *(cargo)* Update and clean up dependencies

## [Prod-Beta-3.8.2] - 2025-08-08

### ✨ Features

- Add support for windows-arm64 binaries

## [Prod-Beta-3.8.0] - 2025-08-08

### ♻️ Refactor

- *(build)* Improve binary patch generation and application

### ✨ Features

- *(deps)* Expand supported package managers and document dependencies
- *(repo)* Add git subcommands and command aliases
- *(deps)* Enhance dependency schema with selectable options

### 🎯 UX

- *(dependencies)* Enhance dependency output format

## [Prod-Beta-3.7.2] - 2025-08-07

### 🛠️ Build

- *(upgrade)* Adjust patch upgrade strategy for archives

## [Prod-Beta-3.6.0] - 2025-08-07

### ♻️ Refactor

- *(pkg)* Migrate GPG signature verification

### ✨ Features

- *(security)* Add GPG key fingerprint support

## [Prod-Beta-3.5.0] - 2025-08-06

### ♻️ Refactor

- Move from 'sh' and 'cmd' to 'bash' and 'pwsh'

### 🔒 Security

- *(pkg)* Implement GPG signature verification for package artifacts

## [Prod-Beta-3.4.2] - 2025-08-06

### ✨ Features

- *(pkg)* Add pre-installation conflict detection
- *(pkg)* Improve dependency handling and uninstallation

## [Prod-Beta-3.4.1] - 2025-08-05

### 🩹 Bug Fixes

- *(upgrade)* Standardize version parsing for releases

## [Prod-Beta-3.4.0] - 2025-08-05

### ✨ Features

- Enhance package management and CLI command capabilities
- *(install)* Enable multi-package installation
- *(sync)* Add external Git repository synchronization

## [Prod-Beta-3.3.2] - 2025-08-04

### 🩹 Bug Fixes

- *(patch)* Refine binary patch handling

## [Prod-Beta-3.3.1] - 2025-08-03

### ✨ Features

- *(pkg)* Enhance package installation and resolution

## [Prod-Beta-3.3.0] - 2025-08-03

### ✨ Features

- *(repo)* Allow adding git repos as package sources
- Add optional dependency resolution and CLI aliases

## [Prod-Beta-3.2.7] - 2025-08-02

### ✨ Features

- *(pkg)* Add MacPorts and Conda package manager support

## [Prod-Beta-3.2.5] - 2025-07-31

### ♻️ Refactor

- *(upgrade)* Use 'no\_' methods for HTTP compression

## [Prod-Beta-3.2.3] - 2025-07-31

### ✨ Features

- *(upgrade)* Display download progress for patches

## [Prod-Beta-3.2.2] - 2025-07-31

### ✨ Features

- *(pkg)* Add support for more dependency managers

## [Prod-Beta-3.2.0] - 2025-07-30

### ✨ Features

- Introduce service and config package types
