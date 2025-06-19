# Zoi Changelog

Latest Production Version: Prod. Beta 1.2.0

Latest Development Version: Dev. Beta 1.2.0

## `Production`

Refurbished for production

## `Development`

On the development branch.

### `Beta 1.2.1`

- **[♻️ Refactor(info)](https://codeberg.org/Zusty/Zoi/commit/db4f8544994e7e5c74f3e97f33280b957fd1add9)**: Source system details from configuration.

- **[🩹 Fix(cmd)](https://codeberg.org/Zusty/Zoi/commit/07546f3e6769b20c0cdcf7ebee191f4e59acc0a6)**: Removed redundant error print and unused import.

### `Beta 1.2.0`

- **[✨ Feat(env)](https://codeberg.org/Zusty/Zoi/commit/1d1178f93f9492301351ba014fdf22d3d9b453cb)**: Made command interactive.
- **[✨ Feat(vm)](https://codeberg.org/Zusty/Zoi/commit/e0289860409d116f83662b7c1620f953e77e4d1)**: Implemented Go version manager.

### `Beta 1.1.0`

- **[✨ Feat(update)](https://codeberg.org/Zusty/Zoi/commit/7ac47821df5e5d2e9b7cec11265b688e2ffd1267)**: Added --force flag for reinstallation.

- **[✨ Feat(set)](https://codeberg.org/Zusty/Zoi/commit/bd6265c53aad0a696c72f4185220aa0614a4e894)**: Added interactive mode for config values.

- **[✨ Feat(run)](https://codeberg.org/Zusty/Zoi/commit/efd9b7a3118626b64d63afe4b2e14b9c9e4a5b3e)**: Allowed interactive command selection.

### `Beta 1.0.0`

- **[♻️ Refactor](https://codeberg.org/Zusty/Zoi/commit/7972a3ab92978d44e38e8cff49651f5eb1d59dc7)**: Moved to Cobra for command-line tool and Viper for config.

### `Alpha 3.2.0`

- **[Feat(install)](https://codeberg.org/Zusty/Zoi/commit/32d2706782eaf015a1660656e9922eb28c50a7fe)**: Added update packages before installing.

### `Alpha 3.1.0`

- **Feat**: Added a bunch of features.

### `Alpha 3.0.0`

- **Feat**: Added `uninstall` subcommand to `zoi vm`.
  - Allows users to remove specific installed language versions (e.g. `zoi vm uninstall go@1.20.0`).
- **Refactor**: Improved main `zoi` help message clarity.
  - Refined command descriptions in the global `zoi help` output for better readability and understanding.

### `Alpha 2.0.0`

- **Feat**: Added version managing command.
  - Added Go and Python version managing.

### `Alpha 1.0.0`

- **Refactor**: Major code rewrite and reformat.
  - Moved the commands to a `commands` folder.
  - Better code structure and better code overall.

### `Pre-Alpha 2.4.0`

- **Build**: Added update command that update Zoi.
- **Build**: Added build all script that build arm64/amd64 versions of linux/macos/windows.

### `Pre-Alpha 2.3.0`

- **Feat**: Added install command that install system packages.

### `Pre-Alpha 2.2.0`

- **Feat**: Added set command that set the apps url in a config file.

### `Pre-Alpha 2.1.0`

- **Feat**: Added check command that checks network and golang + git versions.

### `Pre-Alpha 2.0.0`

- **Build**: Added build scripts.
  - Build scripts for Linux/MacOS and Windows.
- **Migration**: Moved from NodeJS to Golang.
  - Command usage for `make` changed from json file to yaml.

### `Pre-Alpha 1.4.0`

- **Update**: Making the structure better.

### `Pre-Alpha 1.3.0`

- **Update**: Adding the ability to create apps from a local json file.

### `Pre-Alpha 1.2.0`

- **Update**: Making the app fetch the frameworks and apps from the website.

### `Pre-Alpha 1.1.0`

- **Update**: Another rewriting some of the files.
- **Update**: Rewriting some of the files.

### `Pre-Alpha 1.0.0`

- **Update**: Adding the ability to create new apps and frameworks, adding Ruby on Rails support.
- **Update**: The main foundation of the project.
- **Update**: Init.
