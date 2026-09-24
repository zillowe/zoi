# Contributing

First of all, thank you for considering contributing to Zoi!
We're excited to have you on board and appreciate your help in
making our project better. Every contribution, no matter how small, is valuable to us.

<details>
<summary>Table of Contents</summary>

- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs or Requesting Features](#reporting-bugs-or-requesting-features)
  - [Contributing Code](#contributing-code)
- [Development](#development)
  - [Prerequisites](#prerequisites)
  - [First-Time Setup](#first-time-setup)
  - [Development with Docker](#development-with-docker)
  - [Development Commands](#development-commands)
- [Crash Reports](#crash-reports)
- [Commit Messages](#commit-messages)
- [Code of Conduct](#code-of-conduct)

</details>

## How to Contribute

We welcome contributions in many forms, including bug reports, feature requests,
documentation improvements, and code contributions.

You can only use our main [GitLab mirror](/README.md#repositories-mirrors) for contributions.

- [GitLab](https://gitlab.com/zillowe/zillwen/zusty/zoi): Main
- [GitHub](https://github.com/zillowe/zoi) or
[Codeberg](https://codeberg.org/zillowe/zoi): Issues only are welcomed if
you don't want to use GitLab but GitLab is still the preferred method.

### Reporting Bugs or Requesting Features

If you find a bug or have an idea for a new feature, please check our
[**issue tracker**](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/work_items) to
see if it has already been reported. If not, feel free to open a new issue.

- [Report a Bug](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/work_items/new?issuable_template=Bug%20Report)
- [Request a Feature](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/work_items/new?issuable_template=Feature%20Request)
- [Request an Enhancement](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/work_items/new?issuable_template=Enhancement%20Request)

### Contributing Code

If you'd like to contribute code, please follow these steps:

1. **Fork the repository** on [GitLab](https://gitlab.com/zillowe/zillwen/zusty/zoi).
2. **Clone your fork** to your local machine.
3. **Create a new branch** for your changes.

   ```sh
   git checkout -b my-feature-branch
   ```

4. **Make your changes** and commit them with a clear and descriptive message.
5. **Push your changes** to your fork.
6. **Open a merge request** to the `main` branch of the original repository.

## Development

To get started with developing Zoi, you'll need to set up your local environment.

### Prerequisites

- **Rust:** Make sure you have the latest version of Rust and Cargo installed.
You can find instructions at [rust-lang.org](https://www.rust-lang.org/tools/install).
- **Just:** The `just` command is required for our Justfile-based build process.

See [`PACKAGING.md`](./PACKAGING.md) for more information.

### First-Time Setup

Because Zoi is used to manage its own development, you must first build and
install it manually using `just` (Or you can install
[pre-compiled binaries](./INSTALL.md) instead):

1. **Configure the build:**

   ```sh
   ./configure
   ```

2. **Build and install:**

   ```sh
   just build
   # Or for a debug build:
   # just dev
   sudo just install
   ```

Once you have a working `zoi` command, you can use it for all other development tasks.

### Development with Docker

While a local Rust installation is recommended for active development,
you can use Docker to develop and build Zoi without polluting your local machine.

#### Using the Official Zoi CLI Docker Image

For quick development or testing, you can pull and use the official Zoi CLI
Docker image directly from the GitLab Container Registry.
This image contains the `zoi` binary and its runtime dependencies.

```sh
# Pull the latest Zoi CLI image
docker pull registry.gitlab.com/zillowe/zillwen/zusty/zoi/zoi:latest

# Run a Zoi command using the image
docker run --rm registry.gitlab.com/zillowe/zillwen/zusty/zoi/zoi:latest zoi --version
```

#### Building the Docker Image Locally

If you need to build the Docker image locally (e.g. for custom configurations or
testing changes to the `Dockerfile`), you can do so:

1. **Build the image:**
   The following command builds the final, lightweight Docker image containing the `zoi` binary.

   ```sh
   docker build -t zoi .
   ```

2. **Extract the binary:**
   If you want to get the compiled `zoi` binary from the image to use on your
host system, run these commands:

   ```sh
   docker create --name zoi-container zoi
   docker cp zoi-container:/usr/local/bin/zoi ./zoi
   docker rm zoi-container
   ```

   You will now have a `zoi` executable in your current directory.

### Development Commands

- **`build`**: Builds a release version of Zoi.

  ```sh
  just build
  ```

- **`dev`**: Builds a dev version of Zoi.

  ```sh
  just dev
  ```

## Crash Reports

Zoi has a built-in crash reporter that saves crash reports to disk.
The crash reports are saved to the `$XDG_STATE_HOME/zoi/crash` directory.
If `$XDG_STATE_HOME` is not set, the default is `~/.local/state/zoi/crash`.
Crash reports are not automatically sent anywhere off your machine.

Crash reports are written immediately when a panic occurs.
If Zoi crashes and you start Zoi again with telemetry enabled,
you will be asked whether to upload each pending report.
When telemetry is disabled, reports are only saved locally and you are never prompted.

Note: use the `zoi telemetry crash list` command to get a list of available crash reports.

Crash reports end in the `.zoicrash` extension.
The crash reports are in Sentry envelope format.
You can upload these to your own Sentry account to view their contents with
`zoi telemetry crash send <file>`, but the format is also publicly documented
so any other available tools can also be used.
The `zoi telemetry crash show <file>` command prints a report to stdout.

To send the crash report to the Zoi project, you can use the following CLI command using the Sentry CLI:

```sh
SENTRY_DSN="https://c8d665d2f696aa636baa4b68ff4a44c0@o4511544490459136.ingest.de.sentry.io/4511544501600336" sentry-cli send-envelope --raw <path to crash report>
```

Warning: the crash report can contain sensitive information.
The report doesn't purposely contain sensitive information,
but it does contain a backtrace and runtime context captured at the time of the crash.
This information is used to rebuild the stack trace but can also contain
sensitive data depending on when the crash occurred.

## Commit Messages

Please write clear and descriptive commit messages. A good commit message
should explain the "what" and "why" of your changes.

We mostly use [ZFGM Commits](https://zillowe.qzz.io/docs/methods/zfgm/commits)
when creating our commit messages, to use it with
[GCT](https://gitlab.com/zillowe/zillwen/zusty/gct) follow
[GCT Docs](https://zillowe.qzz.io/docs/zds/gct).

## Code of Conduct

By contributing to Zoi, you agree to abide by our
[Code of Conduct](./CODE_OF_CONDUCT.md).
Please read it to understand our community standards.

Thank you again for your interest in contributing to Zoi! We look forward to your contributions.
