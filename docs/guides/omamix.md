---
title: Replacing Omarchy/Omakub with Zoi
description: How to create a personalized development environment using Zoi, similar to Omarchy or Omakub.
---

Omarchy and Omakub are tools that provide an opinionated setup for Arch Linux. While powerful, they are tailored to a specific operating system and a specific set of tools. Zoi allows you to achieve a similar result—a fully configured development environment—but with the flexibility to choose your own tools and run your setup on any platform Zoi supports (Linux, macOS, and Windows).

This guide will walk you through creating your own personalized "Omarchy-like" setup using Zoi's `collection` and `config` packages.

## The Core Idea: Collections & Configs

The key to replicating this with Zoi is to separate your tools from your configuration:

1.  **Collection Package:** A `collection` is a meta-package that doesn't install any software itself. Instead, it defines a list of other packages as dependencies. This will be the core of your setup, defining all the applications you want (editors, browsers, CLIs, etc.).

2.  **Config Package:** A `config` package is designed to manage configuration files. You can use this to deploy your personal dotfiles, set up shell aliases, and configure your tools exactly how you like them.

By combining these two, you can install and configure your entire development environment with a single Zoi command.

## Step 1: Create a Git Repository for Your Setup

It's best practice to store your custom Zoi packages in your own Git repository. This makes them easy to manage, version, and share across your machines.

1.  Create a new repository on GitHub, GitLab, or another provider.
2.  Clone it to your local machine.

You will add your custom `pkg.yaml` files to this repository.

## Step 2: Define Your Core Tools with a Collection

First, let's define the list of software you want to install.

Create a file in your new repository named `dev-environment.pkg.yaml`:

```yaml
# dev-environment.pkg.yaml
# yaml-language-server: $schema=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/pkg.schema.json
name: dev-environment
repo: user/personal
type: collection
version: "1.0"
description: "My personal development environment, including all my essential tools."
tags: [collection, dev-environment]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

dependencies:
  runtime:
    required:
      # Core Utilities
      - native:git
      - zoi:neovim
      - zoi:starship
      - zoi:wezterm

      # Web Browsers (choose one)
      options:
        - name: "Web Browser"
          desc: "Select a web browser to install."
          all: false
          depends:
            - brew-cask:google-chrome:Google Chrome for macOS
            - scoop:googlechrome:Google Chrome for Windows
            - yay:google-chrome:Google Chrome for Arch Linux

    optional:
      # Programming Languages
      - zoi:node:For JavaScript/TypeScript development
      - zoi:go:For Go development
      - zoi:rust:For Rust development

      # Other Tools
      - zoi:docker:For containerization
```

### Key Concepts in this Collection:

- **`type: collection`**: This tells Zoi that this package only installs dependencies.
- **`dependencies.runtime.required`**: These packages will always be installed.
  - We use `native:git` to let Zoi pick the best way to install `git` on any OS.
  - We pull other tools like `neovim` and `starship` from the Zoi repositories.
- **`options`**: This block gives you a choice during installation. Here, you can choose which browser to install, and Zoi will use the correct package manager for your OS.
- **`optional`**: These packages are for tools you don't always need. Zoi will prompt you to ask if you want to install them.

## Step 3: Manage Your Dotfiles with a Config Package

Next, let's create a package to manage your configuration files (dotfiles). This example assumes you store your dotfiles in a separate Git repository.

Create a file named `my-dotfiles.pkg.yaml` in your setup repository:

```yaml
# my-dotfiles.pkg.yaml
# yaml-language-server: $schema=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/pkg.schema.json
name: my-dotfiles
repo: user/personal
type: config
version: "1.0"
description: "My personal dotfiles and shell configuration."
tags: [config, dotfiles]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

# This config package depends on the tools it configures.
dependencies:
  runtime:
    required:
      - zoi:starship
      - zoi:neovim

config:
  - platforms: ["linux", "macos"]
    install:
      - "git clone https://github.com/YourUsername/dotfiles.git ~/.dotfiles"
      - "ln -s ~/.dotfiles/.config/starship.toml ~/.config/starship.toml"
      - "ln -s ~/.dotfiles/.config/nvim ~/.config/nvim"
    uninstall:
      - "rm ~/.config/starship.toml"
      - "rm -rf ~/.config/nvim"
      - "rm -rf ~/.dotfiles"
```

### Key Concepts in this Config:

- **`type: config`**: Declares this as a configuration package.
- **`dependencies`**: It's good practice to make your config depend on the tools it configures.
- **`config.install`**: A list of shell commands to run when the package is installed. Here, we clone a dotfiles repository and create symlinks.
- **`config.uninstall`**: Commands to clean up when the package is uninstalled.

## Step 4: Link Your Config to Your Collection

Now, update your `dev-environment.pkg.yaml` to include your new dotfiles config as a dependency:

```yaml
# dev-environment.pkg.yaml
# ... (keep the rest of the file the same)

dependencies:
  runtime:
    required:
      # Your dotfiles config
      - zoi:@user/personal/my-dotfiles

      # Core Utilities
      - native:git
      # ... (rest of your dependencies)
```

By adding `zoi:@user/personal/my-dotfiles` (adjust the repo path as needed), you ensure that your dotfiles are installed automatically whenever you install your `dev-environment` collection.

## Step 5: Install Your Custom Environment

Now you're ready to deploy your setup.

1.  **Add Your Repository to Zoi:**
    First, tell Zoi about your custom package repository.

    ```sh
    # Replace with your repository's URL
    zoi repo add https://github.com/YourUsername/my-zoi-setup.git
    ```

2.  **Install Your Collection:**
    Install your main collection package. Use the `@git/` prefix to specify your custom repository.

    ```sh
    # The repo name is derived from the URL (e.g. 'my-zoi-setup')
    zoi install @git/my-zoi-setup/dev-environment
    ```

Zoi will now:

1.  Install `my-dotfiles`, which in turn installs `starship` and `neovim`.
2.  Run the `install` commands from `my-dotfiles` to clone your dotfiles and create symlinks.
3.  Install the other `required` dependencies from your collection (`git`, `wezterm`).
4.  Prompt you to choose a web browser.
5.  Prompt you to select which `optional` tools you want to install.

Your fully configured, personalized development environment is now ready. You can run this same command on any new machine to replicate your setup instantly.
