# TODO

## Goals

- [ ] Package `ghostty` and all of its packages
- [ ] Package `fastfetch`, `bat`, `kitty`, `minisign` and `wezterm`
- [ ] Package `zig`

## Bugs

## Features

- [ ] Better `generate-manual` man generation with all subcommands, flags and subflags, and long-description when available.
- [ ] Exposed Lua functions:
      SYSTEM.[OS, ARCH, DISTRO, MANAGER (native package manager)] system info
      PKG.[every field defined in package({}) metadata]
      ZOI.[VERSION (resolved version), PATH.[user (full local path to ~/.zoi), system (full path to the bin location, /usr/local/bin/)]] Zoi info
      UTILS.[PARSE.[json, yaml, toml], FETCH.[url (fetches the URL and give the response or error), GITHUB/GITLAB/GITEA/FORGEJO.[LATEST.[tag, release, commit] ], FILE (download a file)], ] helper functions
      IMPORT import local files from the pkg.lua directory
      INCLUDE import local Lua files from the pkg.lua directory to use their content
