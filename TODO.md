# TODO

## Goals

- [ ] Package `ghostty` and all of its packages
- [ ] Package `fastfetch`, `bat`, `kitty`, `minisign` and `wezterm`
- [ ] Package `zig`

## Bugs

- [ ] `app/install.sh/ps1` and `src/pkg/upgrade.rs` gitlab api
- [ ] `update` command to work again

## Features

- [ ] `meta` command that generates a static version of the `pkg.lua`, `pkg.json`, this is not the same as `package meta` (used only for `pkg.tar.zst`), this will be used for searching and viewing packages.
- [ ] Add to `repo.yaml` a pgp key URL or fingerprint with pgp-name so when installing pre-built `pkg.tar.zst` we sign them with the key, using `pgp` command.
- [ ] Make the default package registry an environment variable where you can set at build time which uses official Zoi registry by default.
- [ ] Make the repos inside the registry set in `repo.yaml`, for [types: [unoffical (not trusted), offical (trusted), community (community repo), archive (archive repo), test (testing repo)], active: [true or false (either an active repo or not)]]
      Like this

```yaml
repos:
  - name: core # name, the top-level repo (the top-level folders in the repo)
    type: offical # unoffical, official, community, archive, test
    active: true # true, false
```

- [ ] Exposed Lua functions:
      SYSTEM.[OS, ARCH, DISTRO, MANAGER (native package manager)] system info
      PKG.[every field defined in package({}) metadata]
      ZOI.[VERSION (resolved version), PATH.[user (full local path to ~/.zoi), system (full path to the bin location, /usr/local/bin/)]] Zoi info
      UTILS.[PARSE.[json, yaml, toml], FETCH.[url (fetches the URL and give the response or error), GITHUB/GITLAB/GITEA/FORGEJO.[LATEST.[tag, release, commit] ], FILE (download a file)], ] helper functions
      IMPORT import local files from the pkg.lua directory
      INCLUDE import local Lua files from the pkg.lua directory to use their content
