# Roadmap

Roadmap for the next release.

## v1.4

This release mainly focuses on improving the UX and UI of Zoi CLI, improving existing commands and fixing critical bugs.

- [ ] Improve the UI/UX of these commands: [`install`, `update`, `uninstall`, `sync`]
- [ ] Improve the logic of handling pre-built `pkg.tar.zst` archives (downloading them, verifying the hashes, the signatures and the sizes) and building the archives if not available as pre-built.
- [ ] Improve dependency handling.
- [ ] Add a new field in `repo.yaml` (`pkg.size`) which locate to a plain text file that contains the size of the pre-built archive in bytes (e.g. `24129328` which is roughly 24mb).
- [ ] Package a set of packages and make them available to the Zoidberg registry (and as a pre-built archive). [`ghostty`, `kitty`, `fastfetch`]
