# Contributing to Zoi

First of all, thank you for considering contributing to Zoi! We're excited to have you on board and appreciate your help in making our project better. Every contribution, no matter how small, is valuable to us.

## How to Contribute

We welcome contributions in many forms, including bug reports, feature requests, documentation improvements, and code contributions.

You can use any of our [mirrors](/README.md#-contributing) for contributions.

### Reporting Bugs or Requesting Features

If you find a bug or have an idea for a new feature, please check our [**issue tracker**](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues) to see if it has already been reported. If not, feel free to open a new issue.

- [Report a Bug](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new?issuable_template=Bug%20Report)
- [Request a Feature](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new?issuable_template=Feature%20Request)
- [Request an Enhancement](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new?issuable_template=Enhancement%20Request)

### Contributing Code

If you'd like to contribute code, please follow these steps:

1.  **Fork the repository** on [GitLab](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi).
2.  **Clone your fork** to your local machine.
3.  **Create a new branch** for your changes.
    ```sh
    git checkout -b my-feature-branch
    ```
4.  **Make your changes** and commit them with a clear and descriptive message.
5.  **Push your changes** to your fork.
6.  **Open a merge request** to the `main` branch of the original repository.

## Development Setup

To get started with developing Zoi, you'll need to set up your local environment. We use `zoi` itself to manage project tasks, defined in the `zoi.yaml` file.

### Prerequisites

- **Rust:** Make sure you have the latest version of Rust and Cargo installed. You can find instructions at [rust-lang.org](https://www.rust-lang.org/tools/install).
- **Make:** The `make` command is required for our Makefile-based build process.

### Building for the First Time

Since we use Zoi to build Zoi, you'll need to perform an initial build using `make`:

1.  **Configure the build:**
    ```sh
    ./configure
    ```
2.  **Build and install:**
    ```sh
    make
    sudo make install
    ```

Once you have a working `zoi` command, you can use it for subsequent development tasks.

### Development Workflow with Zoi

All common development tasks are defined as commands in `zoi.yaml` and can be executed with `zoi run`. If you run `zoi run` without arguments, you'll get an interactive list of available commands.

- **Prepare your environment:** Before you start coding, run the `pre` environment setup. This will check dependencies, format your code, and run checks.

  ```sh
  zoi env pre
  ```

- **Build the project:** To build a release version of Zoi.

  ```sh
  zoi run build
  ```

- **Run checks:** To run `clippy` and other checks.

  ```sh
  zoi run check
  ```

- **Format code:** To format the code according to project standards.

  ```sh
  zoi run fmt
  ```

- **Install:** To perform a full installation, including shell completions.
  ```sh
  zoi run install
  ```

## Commit Messages

Please write clear and descriptive commit messages. A good commit message should explain the "what" and "why" of your changes.

## Code of Conduct

By contributing to Zoi, you agree to abide by our [Code of Conduct](./CODE_OF_CONDUCT.md). Please read it to understand our community standards.

Thank you again for your interest in contributing to Zoi! We look forward to your contributions.
