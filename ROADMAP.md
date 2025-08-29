# Roadmap

This document outlines the planned features and improvements for the upcoming release of Zoi.

Upcoming release: Beta 5.0.0

## New Features

- [ ] Package command that package packages before installing them.
      When installing or building a package from a pkg.yaml it will do like pacman PKGBUILD by getting all the files and info and packaging it into a name.pkg.tar.zst archive then installing it.
      It will work like pacman, aur/yay
      So when we have mirrors, it will package them and Zoi will install the pkg.tar.zst file and install it.
      You can build a package archive using this command:
      `$ zoi package file.pkg.yaml`
      This will produce a file.pkg.tar.zst
      This command will run on every new package (or update) on the mirrors.

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

- [ ] Project specific installation for packages
      Install packages to a specific project by adding `--local` flag, to run said package we do `zoi exec` command.
      `zoi exec` first check for installed packages locally, then installed package globally, then for cached packages.
      `zoi pkg-config` same as above but for libraries.
      The installed packages with the SBOM are in the local `.zoi/` directory.
      To uninstall a local package we also add `--local` flag to it.
- [ ] MCP package type and `mcp` command
      You can add and remove mcps, an mcp package is just a normal package with some extra fields such as `mcp type`, so it could be http server, a binary or something else.
      To add or remove mcp servers we use these commands `add/rm mcp <package> <tool>`.
      `<tool>` is like Codex/Claude Code/Gemini/Cursor, etc.
      If the mcp is a binary it will not be added to path, it will be only executable via this command `mpc exec <package>`.
      First party support for tools are: OpenCode (SST), Gemini CLI, Codex CLI, Claude Code, VSCode, Cursor and Windsurf.
- [ ] Publish command that creates an issue for adding new packages
      `publish ./path/to/name.pkg.yaml` this command will create an issue or GitHub, GitLab or Codeberg requesting to add a new package from a local pkg.yaml file.
      When publishing a new package you can choose a mirror `publish ... mirror-name`, if not specified it will choose the default mirror from your config file.
      The publish command will check for the package name and repo/nested repo for existing packages.
      If you want to update an existing package it will check the version to see if there's an update or no, if there's an update it will create a different type of issue.
- [ ] Add package type script that runs platform specific commands with dependencies.
- [ ] Man command for viewing manual
      `man <package>` command for viewing a text or markdown manual.
      Add this to the pkg.yaml: `man: url-to-plain-text-or-markdown`
- [ ] Tab completion for packages in active repos
- [ ] PGP command
      `pgp` command for managing public pgp keys to verify installed packages (`sig` field)
      We have a collection of trusted pgp keys, you can import pgp keys to be trusted, also work with zoi type extension to import pgp keys from urls or custom/default registries.
      PGP keys are stored at ~/.zoi/pgp/

## Enhancements & Improvements

- [ ] Ability to add a pgp key from a custom registry in maintainer and author fields
- [ ] Better platform choices
      something like this, you can add bulk if the others match:

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

- [ ] Better `make` command.
- [ ] Better UI/UX experience
- [x] Improve `run` and `env` commands with platform specific commands and envs.
- [ ] Improve the library side of Zoi with better docs.
- [ ] More platforms support
      Adding more platforms support in the release CI and build scripts and in upgrade command and maybe packages.
      Isn't a high priority so it may not be shipped in the next release.
  - [ ] Windows arm64
  - [ ] FreeBSD amd64, arm64
  - [ ] OpenBSD amd64, arm64
  - [ ] Android Termux?

---

> **Note:** This roadmap is subject to change based on community feedback and development progress. Features/Enhancements may be added, removed, or re-prioritized as needed.
