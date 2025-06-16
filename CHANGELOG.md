# Zoi Changelog

Latest Production Version: Prod. Alpha 1.2.0

Latest Development Version: Dev. Pre-Beta 3.2.0

## `Production`

On the production branch.

### `Alpha 1.2.0`

- **Release**: Refurbished Dev. Alpha 3.2.0 for production.

### `Alpha 1.0.0`

- **Feat**: Added `uninstall` command.

- **Release**: Refurbished for production.

## `Development`

On the development branch.

### `Pre-Beta 1.0.0`

- **[♻️ Refactor](https://codeberg.org/Zusty/Zoi/commit/32d2706782eaf015a1660656e9922eb28c50a7fe)**: Moved to Cobra for command-line tool and Viper for config.

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
