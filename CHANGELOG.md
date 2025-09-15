## [Prod-Beta-5.0.5] - 2025-09-09

### ➡️ Migrations

- _(pkg)_ Use Lua for package definitions\* Use Lua for package definitions

## [Prod-Beta-5.0.4] - 2025-09-09

### ✨ Features

- _(package)_ Add custom file staging and installation\* Add custom file staging and installation

## [Prod-Beta-5.0.3] - 2025-09-09

### ✨ Features

- _(package)_ Add Docker build support for source packages\* Add Docker build support for source packages

## [Prod-Beta-5.0.0] - 2025-09-09

### ♻️ Refactor

- _(cli)_ Restructure update command arguments and improve help output\* Restructure update command arguments and improve help output

- _(pkg)_ Remove dynamic variable replacements\* Remove dynamic variable replacements

- _(cmd)_ Adapt modules to new package resolution signature\* Adapt modules to new package resolution signature

- _(pkg)_ Simplify archive filename and URL template\* Simplify archive filename and URL template

### ✨ Features

- _(gemini)_ Add AI flow for GitLab operations\* Add AI flow for GitLab operations

- _(sync)_ Add fallback mirrors for package database\* Add fallback mirrors for package database

- _(sync)_ Add --no-pm flag to skip package manager checks\* Add --no-pm flag to skip package manager checks

- _(config)_ Allow platform-specific commands and environment variables\* Allow platform-specific commands and environment variables

- _(cli)_ Enhance package completions and auto-setup\* Enhance package completions and auto-setup

- _(show)_ Add license verification\* Add license verification

- _(install)_ Add --all-optional flag to install command\* Add --all-optional flag to install command

- _(pkg)_ Implement interactive package selection\* Implement interactive package selection

- _(show)_ Display package installation status\* Display package installation status

- _(script)_ Add support for script package type\* Add support for script package type

- _(cli)_ Add new 'man' command for package manuals\* Add new 'man' command for package manuals

- _(dev-setup)_ Implement comprehensive testing and formatting\* Implement comprehensive testing and formatting

- _(man)_ Enhance man command with local caching and raw display\* Enhance man command with local caching and raw display

- _(script-handler)_ Implement script package uninstallation\* Implement script package uninstallation

- _(extension)_ Allow extensions to manage project configuration file\* Allow extensions to manage project configuration file

- _(pkg)_ Support structured package database\* Support structured package database

- _(package)_ Add CLI commands for package creation\* Add CLI commands for package creation

- _(package)_ Add package install command\* Add package install command

- _(pgp)_ Add PGP key management\* Add PGP key management

- _(pgp)_ Implement PGP key import from URL and list command\* Implement PGP key import from URL and list command

- _(pkg)_ Implement pre-built package installation from repos\* Implement pre-built package installation from repos

- _(pkg)_ Implement meta-build-install and update package resolution\* Implement meta-build-install and update package resolution

- _(pkg)_ Add package installation scope\* Add package installation scope

- _(install)_ Add support for installing from git repositories\* Add support for installing from git repositories

- _(pkg)_ Support direct package names in repo installs\* Support direct package names in repo installs

- _(package)_ Add multi-platform build capability\* Add multi-platform build capability

- _(pgp)_ Add command to search PGP keys\* Add command to search PGP keys

- _(package)_ Add source installation support\* Add source installation support

- _(api)_ Expose core functionality as public library API\* Expose core functionality as public library API

- _(meta)_ Allow specifying installation type for meta generation\* Allow specifying installation type for meta generation

### ➡️ Migrations

- _(scripts)_ Migrate install scripts to zillowe.pages.dev\* Migrate install scripts to zillowe.pages.dev

- _(parser)_ Transition to Lua package definitions\* Transition to Lua package definitions

- _(pkg-format)_ Switch to Lua for package definitions\* Switch to Lua for package definitions

### 🎨 Styling

- _(cli)_ Add custom colors and styling to CLI output\* Add custom colors and styling to CLI output

### 🎯 UX

- _(cli)_ Add package name suggestions to CLI arguments\* Add package name suggestions to CLI arguments

- _(cmd)_ Condense repository names in list and search output\* Condense repository names in list and search output

### 🔒 Security

- _(install)_ Implement GPG signature verification\* Implement GPG signature verification

- _(package)_ Implement PGP signature verification\* Implement PGP signature verification

### 🔧 Configuration

- _(gemini)_ Set up client credentials\* Set up client credentials

- _(about)_ Add contact email to about command\* Add contact email to about command

### 🛠️ Build

- _(docker)_ Add Docker build configuration\* Add Docker build configuration

- _(sync)_ Load sync fallbacks from repo.yaml\* Load sync fallbacks from repo.yaml

### 🛡️ Dependencies

- _(cargo)_ Add mlua crate\* Add mlua crate

### 🧹 Cleanup

- _(cli)_ Remove interactive package creation command\* Remove interactive package creation command

### 🩹 Bug Fixes

- _(windows)_ Initialize colored crate output\* Initialize colored crate output

- _(build)_ Correct checksum mismatch error message formatting\* Correct checksum mismatch error message formatting

## [Prod-Beta-4.3.7] - 2025-08-20

### ♻️ Refactor

- Enhance package resolution and CLI output\* Enhance package resolution and CLI output

- _(dependencies)_ Remove pre-installation conflict checks\* Remove pre-installation conflict checks

### ✨ Features

- _(exec)_ Execute commands via shell\* Execute commands via shell

- _(pkg)_ Add package update command\* Add package update command

### 🧹 Cleanup

- _(pkg)_ Remove external command conflict check\* Remove external command conflict check

## [Prod-Beta-4.3.6] - 2025-08-19

### ♻️ Refactor

- _(path)_ Refine PATH check output logic\* Refine PATH check output logic

- _(pkg-resolve)_ Remove alt source caching and improve download reliability\* Remove alt source caching and improve download reliability

### ✨ Features

- _(install)_ Add license validation to packages\* Add license validation to packages

### ➡️ Migrations

- _(sbom)_ Migrate package recording to CycloneDX SBOM\* Migrate package recording to CycloneDX SBOM

## [Prod-Beta-4.3.5] - 2025-08-18

### ✨ Features

- _(shell)_ Add setup command to configure shell PATH\* Add setup command to configure shell PATH

- _(shell)_ Add Elvish shell path setup\* Add Elvish shell path setup

### 🎯 UX

- _(path)_ Enhance PATH warning for better user guidance\* Enhance PATH warning for better user guidance

## [Prod-Beta-4.3.4] - 2025-08-18

### ✨ Features

- _(install)_ Add package recording and lockfile installation\* Add package recording and lockfile installation

## [Prod-Beta-4.3.3] - 2025-08-17

### ✨ Features

- _(pkg)_ Add sharable install manifests\* Add sharable install manifests

## [Prod-Beta-4.3.2] - 2025-08-17

### ♻️ Refactor

- _(upgrade)_ Streamline patch upgrade by using current executable\* Streamline patch upgrade by using current executable

- _(deps)_ Specify optional dependency type in installation output\* Specify optional dependency type in installation output

### ✨ Features

- _(about)_ Include documentation URL in output\* Include documentation URL in output

- _(service)_ Add Docker Compose support\* Add Docker Compose support

## [Prod-Beta-4.3.1] - 2025-08-16

### ♻️ Refactor

- Address Clippy warnings across codebase\* Address Clippy warnings across codebase

### ✨ Features

- _(config)_ Manage external git repositories\* Manage external git repositories

- _(extension)_ Add extension management commands\* Add extension management commands

- _(pkg)_ Add rollback command and functionality\* Add rollback command and functionality

- _(pkg)_ Add library package type and pkg-config command\* Add library package type and pkg-config command

- _(pkg)_ Prompt user with important package updates\* Prompt user with important package updates

### 🛠️ Build

- Add dedicated lint command\* Add dedicated lint command

## [Prod-Beta-4.3.0] - 2025-08-15

### ♻️ Refactor

- _(pkg)_ Improve source install binary linking\* Improve source install binary linking

### ✨ Features

- _(shell)_ Add shell command for completion management\* Add shell command for completion management

- _(upgrade)_ Allow specifying tag or branch for upgrade\* Allow specifying tag or branch for upgrade

- _(install)_ Implement binary package installation\* Implement binary package installation

- _(pkg)_ Allow {git} placeholder in install URLs\* Allow {git} placeholder in install URLs

- _(show)_ Add specific binary types to package info\* Add specific binary types to package info

- _(pkg)_ Add {git} placeholder to package install URLs\* Add {git} placeholder to package install URLs

- _(git)_ Add Codeberg support for latest tag resolution\* Add Codeberg support for latest tag resolution

- _(search)_ Paginate search command output\* Paginate search command output

### 🛠️ Build

- _(release)_ Add notes script to CI artifacts\* Add notes script to CI artifacts

### 🩹 Bug Fixes

- _(pkg)_ Conditionally compile symlink calls for Unix\* Conditionally compile symlink calls for Unix

- _(ci)_ Fixing CI add bash\* Fixing CI add bash

## [Prod-Beta-4.2.3] - 2025-08-13

### ✨ Features

- _(pkg)_ Resolve package versions from Git release tags\* Resolve package versions from Git release tags

## [Prod-Beta-4.2.2] - 2025-08-13

### ✨ Features

- _(upgrade)_ Add full and force options\* Add full and force options

## [Prod-Beta-4.2.1] - 2025-08-13

### 🎯 UX

- _(cli)_ Improve auto-completion for source arguments\* Improve auto-completion for source arguments

### 🩹 Bug Fixes

- _(dependencies)_ Fix parsing for package names starting with '@'\* Fix parsing for package names starting with '@'

## [Prod-Beta-4.2.0] - 2025-08-12

### ♻️ Refactor

- Update Config.toml\* Update Config.toml

### ✨ Features

- _(pkg)_ Improve conflict detection\* Improve conflict detection

- _(pkg)_ Allow nested paths for git package sources\* Allow nested paths for git package sources

- _(sync)_ Add registry management for package database\* Add registry management for package database

### 🏗️ Structure

- _(core)_ Rename package and restructure as library\* Rename package and restructure as library

## [Prod-Beta-4.1.3] - 2025-08-12

### 🛠️ Build

- _(pkg)_ Enhance dependency resolution robustness\* Enhance dependency resolution robustness

## [Prod-Beta-4.1.2] - 2025-08-11

### ✨ Features

- _(schema)_ Add JSON schema for pkg.yaml validation\* Add JSON schema for pkg.yaml validation

- _(cmd)_ Add interactive package file creation command\* Add interactive package file creation command

- _(cmd)_ Pass arguments to custom commands\* Pass arguments to custom commands

### 🔧 Configuration

- _(pkg-config)_ Define Zoi package configuration schema\* Define Zoi package configuration schema

## [Prod-Beta-4.1.1] - 2025-08-11

### ✨ Features

- _(cmd)_ Add 'create' command for application packages\* Add 'create' command for application packages

- _(create)_ Add pre-creation check for existing app directory\* Add pre-creation check for existing app directory

## [Prod-Beta-4.1.0] - 2025-08-11

### ✨ Features

- _(pkg)_ Add conflict detection for Zoi packages\* Add conflict detection for Zoi packages

### 🛡️ Dependencies

- Update\* Update

## [Prod-Beta-4.0.4] - 2025-08-09

### ✨ Features

- _(deps)_ Add support for dependency versioning\* Add support for dependency versioning

- _(pkg)_ Add script and Volta package manager support\* Add script and Volta package manager support

## [Prod-Beta-4.0.3] - 2025-08-09

### ♻️ Refactor

- _(cli)_ Enhance input parsing and error handling\* Enhance input parsing and error handling

## [Prod-Beta-4.0.2] - 2025-08-09

### ✨ Features

- _(telemetry)_ Include package version\* Include package version

- _(pkg)_ Add readme field to package type\* Add readme field to package type

## [Prod-Beta-4.0.1] - 2025-08-09

### 🛠️ Build

- _(build)_ Use dotenvy for environment variable loading\* Use dotenvy for environment variable loading

## [Prod-Beta-4.0.0] - 2025-08-09

### ✨ Features

- Introduce package tags and improve network resilience\* Introduce package tags and improve network resilience

- _(install)_ Add tag and branch options for source installs\* Add tag and branch options for source installs

- _(telemetry)_ Add opt-in usage analytics\* Add opt-in usage analytics

### 📈 Tracking

- _(telemetry)_ Track clone, exec, and uninstall commands\* Track clone, exec, and uninstall commands

### 🔒 Security

- _(pkg)_ Warn on insecure HTTP downloads\* Warn on insecure HTTP downloads

### 🛡️ Dependencies

- _(cargo)_ Update and clean up dependencies\* Update and clean up dependencies

## [Prod-Beta-3.8.2] - 2025-08-08

### ✨ Features

- Add support for windows-arm64 binaries\* Add support for windows-arm64 binaries

## [Prod-Beta-3.8.0] - 2025-08-08

### ♻️ Refactor

- _(build)_ Improve binary patch generation and application\* Improve binary patch generation and application

### ✨ Features

- _(deps)_ Enhance dependency schema with selectable options\* Enhance dependency schema with selectable options

- _(repo)_ Add git subcommands and command aliases\* Add git subcommands and command aliases

- _(deps)_ Expand supported package managers and document dependencies\* Expand supported package managers and document dependencies

### 🎯 UX

- _(dependencies)_ Enhance dependency output format\* Enhance dependency output format

## [Prod-Beta-3.7.2] - 2025-08-07

### 🛠️ Build

- _(upgrade)_ Adjust patch upgrade strategy for archives\* Adjust patch upgrade strategy for archives

## [Prod-Beta-3.6.0] - 2025-08-07

### ♻️ Refactor

- _(pkg)_ Migrate GPG signature verification\* Migrate GPG signature verification

### ✨ Features

- _(security)_ Add GPG key fingerprint support\* Add GPG key fingerprint support

## [Prod-Beta-3.5.0] - 2025-08-06

### ♻️ Refactor

- Move from 'sh' and 'cmd' to 'bash' and 'pwsh'\* Move from 'sh' and 'cmd' to 'bash' and 'pwsh'

### 🔒 Security

- _(pkg)_ Implement GPG signature verification for package artifacts\* Implement GPG signature verification for package artifacts

## [Prod-Beta-3.4.2] - 2025-08-06

### ✨ Features

- _(pkg)_ Improve dependency handling and uninstallation\* Improve dependency handling and uninstallation

- _(pkg)_ Add pre-installation conflict detection\* Add pre-installation conflict detection

## [Prod-Beta-3.4.1] - 2025-08-05

### 🩹 Bug Fixes

- _(upgrade)_ Standardize version parsing for releases\* Standardize version parsing for releases

## [Prod-Beta-3.4.0] - 2025-08-05

### ✨ Features

- _(sync)_ Add external Git repository synchronization\* Add external Git repository synchronization

- _(install)_ Enable multi-package installation\* Enable multi-package installation

- Enhance package management and CLI command capabilities\* Enhance package management and CLI command capabilities

## [Prod-Beta-3.3.2] - 2025-08-04

### 🩹 Bug Fixes

- _(patch)_ Refine binary patch handling\* Refine binary patch handling

## [Prod-Beta-3.3.1] - 2025-08-03

### ✨ Features

- _(pkg)_ Enhance package installation and resolution\* Enhance package installation and resolution

## [Prod-Beta-3.3.0] - 2025-08-03

### ✨ Features

- Add optional dependency resolution and CLI aliases\* Add optional dependency resolution and CLI aliases

- _(repo)_ Allow adding git repos as package sources\* Allow adding git repos as package sources

## [Prod-Beta-3.2.7] - 2025-08-02

### ✨ Features

- _(pkg)_ Add MacPorts and Conda package manager support\* Add MacPorts and Conda package manager support

## [Prod-Beta-3.2.5] - 2025-07-31

### ♻️ Refactor

- _(upgrade)_ Use 'no*' methods for HTTP compression\* Use 'no*' methods for HTTP compression

## [Prod-Beta-3.2.3] - 2025-07-31

### ✨ Features

- _(upgrade)_ Display download progress for patches\* Display download progress for patches

## [Prod-Beta-3.2.2] - 2025-07-31

### ✨ Features

- _(pkg)_ Add support for more dependency managers\* Add support for more dependency managers

## [Prod-Beta-3.2.0] - 2025-07-30

### ✨ Features

- Introduce service and config package types\* Introduce service and config package types

- Introduce service and config package types\* Introduce service and config package types
