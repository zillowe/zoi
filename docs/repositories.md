---
title: Repositories
description: Official Zoi repositories, mirrors, and repository tiers.
---

This page explains Zoi's official repositories and mirrors, and how package repositories are organized by tier.

## Official project and package database

- Source code (Zoi)
  - Primary: [GitLab](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi)
  - Mirrors: [GitHub](https://github.com/Zillowe/Zoi), [Codeberg](https://codeberg.org/Zillowe/Zoi)

- Packages database (Zoi-Pkgs)
  - Primary: [GitLab](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs)
  - Mirrors: [GitHub](https://github.com/Zillowe/Zoi-Pkgs), [Codeberg](https://codeberg.org/Zillowe/Zoi-Pkgs)

Related links: [Homepage](https://zillowe.rf.gd/zds/zoi)

## Repository tiers

Zoi organizes packages into tiers. Use these to decide where a package belongs and to assess stability.

| Repository  | Purpose                                                                           |
| ----------- | --------------------------------------------------------------------------------- |
| `core`      | Essential packages and libraries; very common and well-maintained.                |
| `main`      | Important packages that don’t fit in `core` but are essential for most users.     |
| `extra`     | New or niche packages; less common and may be less actively maintained.           |
| `community` | User-submitted packages. New entries start here and may graduate to higher tiers. |
| `test`      | Testing ground for new Zoi features and packages prior to release.                |
| `archive`   | Archived packages that are no longer maintained.                                  |

Note: Packages from `community`, `test`, and `archive` may carry higher risk. Zoi prints warnings where appropriate.

## Managing repositories with the CLI

| Command                  | Description                                                                 |
| ------------------------ | --------------------------------------------------------------------------- |
| `zoi repo add`           | Add an official repo by name or a git repo by URL (interactive if no args). |
| `zoi repo remove <name>` | Remove a repository from the active list.                                   |
| `zoi repo list`          | List active repositories. Use `zoi repo list all` to see all available.     |

### Examples

```sh
# Add a repository interactively
zoi repo add

# Add official repositories by name
zoi repo add core
zoi repo add main
zoi repo add community

# Add by git URL (cloned under ~/.zoi/pkgs/git/ and used via @git/<repo>/<pkg>)
zoi repo add https://github.com/YourOrg/my-zoi-repo.git

# Remove and list
zoi repo remove community
zoi repo list
zoi repo list all
```

## Installing from a specific repository

- Top-level repository:

```sh
zoi install @community/htop
```

- Nested repository path (e.g. platform-specific):

```sh
zoi install @core/linux/amd64/nvidia-driver
```

For creating and publishing packages, see [Creating & Publishing Packages](./creating-packages).
