# Contributing

First of all, thank you for considering contributing to Zoi! We're excited to have you on board and appreciate your help in making our project better. Every contribution, no matter how small, is valuable to us.

<details>
<summary>Table of Contents</summary>

- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs or Requesting Features](#reporting-bugs-or-requesting-features)
  - [Contributing Code](#contributing-code)
- [Development](#development)
  - [Prerequisites](#prerequisites)
  - [First-Time Setup](#first-time-setup)
  - [Development Workflow with Zoi](#development-workflow-with-zoi)
    - [Passing Arguments to Commands](#passing-arguments-to-commands)
    - [Environment Preparation](#environment-preparation)
    - [Development Commands](#development-commands)
- [Commit Messages](#commit-messages)
- [Code of Conduct](#code-of-conduct)

</details>

## How to Contribute

We welcome contributions in many forms, including bug reports, feature requests, documentation improvements, and code contributions.

You can use any of our [mirrors](/README.md#-repositories-mirrors) for contributions.

### Reporting Bugs or Requesting Features

If you find a bug or have an idea for a new feature, please check our [**issue tracker**](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues) to see if it has already been reported. If not, feel free to open a new issue.

- [Report a Bug](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new?issuable_template=Bug%20Report)
- [Request a Feature](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new?issuable_template=Feature%20Request)
- [Request an Enhancement](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new?issuable_template=Enhancement%20Request)

### Contributing Code

If you'd like to contribute code, please follow these steps:

1.  **Fork the repository** on [GitLab](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi) (Or any other [mirror](/README.md#-repositories-mirrors) you like).
2.  **Clone your fork** to your local machine.
3.  **Create a new branch** for your changes.
    ```sh
    git checkout -b my-feature-branch
    ```
4.  **Make your changes** and commit them with a clear and descriptive message.
5.  **Push your changes** to your fork.
6.  **Open a merge request** to the `main` branch of the original repository.

## Development

To get started with developing Zoi, you'll need to set up your local environment.

### Prerequisites

- **Rust:** Make sure you have the latest version of Rust and Cargo installed. You can find instructions at [rust-lang.org](https://www.rust-lang.org/tools/install).
- **Make:** The `make` command is required for our Makefile-based build process.

### First-Time Setup

Because Zoi is used to manage its own development, you must first build and install it manually using `make` (Or you can install [pre-compiled binaries](/README.md#-installation) instead):

1.  **Configure the build:**
    ```sh
    ./configure
    ```
2.  **Build and install:**
    ```sh
    make
    sudo make install
    # Install CLI completions (bash, zsh, fish, elvish, powershell)
    make install-completion
    ```

Once you have a working `zoi` command, you can use it for all other development tasks.

### Development Workflow with Zoi

We use `zoi` itself to manage project tasks, which are defined in the `zoi.yaml` file. You can run tasks using `zoi run <command>` or set up environments with `zoi env <environment>`.

If you run `zoi run` or `zoi env` without arguments, you'll get an interactive list of available commands.

#### Passing Arguments to Commands

To pass arguments to the underlying script, add them after the command alias. Use `--` to separate the arguments from Zoi's own options if needed.

```sh
# This runs 'cargo check --tests'
zoi run check -- --tests
```

#### Environment Preparation

Before you commit changes, run the `pre` environment to ensure your changes meet our quality standards. It will check for unused dependencies, format your code, and run lints and other checks.

```sh
zoi env pre
```

This single command is equivalent to running `zoi run deps`, `zoi run fmt`, `zoi run lint`, and `zoi run check` in sequence.

#### Development Commands

Here are the most common commands defined in `zoi.yaml`:

- **`build`**: Builds a release version of Zoi.

  ```sh
  zoi run build
  ```

- **`check`**: Checks the project for errors without performing a full build.

  ```sh
  zoi run check
  ```

- **`fmt`**: Formats all code in the project according to our style guidelines.

  ```sh
  zoi run fmt
  ```

- **`lint`**: Lints the code using Clippy and applies automatic fixes where possible.

  ```sh
  zoi run lint
  ```

- **`deps`**: Checks for unused dependencies with `cargo-machete`.

  ```sh
  zoi run deps
  ```

- **`install`**: Performs a clean build and installs the latest version of Zoi, including shell completions. This is useful for testing your changes in a live environment.
  ```sh
  zoi run install
  ```

## Commit Messages

Please write clear and descriptive commit messages. A good commit message should explain the "what" and "why" of your changes.

We mostly use [ZFGM Commits](https://zillowe.qzz.io/docs/methods/zfgm/commits) when creating our commit messages, to use it with [GCT](https://gitlab.com/Zillowe/Zillwen/Zusty/GCT) follow [GCT Docs](https://zillowe.qzz.io/docs/zds/gct).

## Code of Conduct

By contributing to Zoi, you agree to abide by our [Code of Conduct](./CODE_OF_CONDUCT.md). Please read it to understand our community standards.

Thank you again for your interest in contributing to Zoi! We look forward to your contributions.
