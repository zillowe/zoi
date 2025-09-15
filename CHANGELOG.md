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

- Introduce service and config package types
