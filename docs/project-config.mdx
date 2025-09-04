---
title: Project Configuration
description: Define project commands and environments with zoi.yaml.
---

Zoi can manage per-project commands and environments using a `zoi.yaml` file placed in your project root. This page describes the schema and provides examples.

## File location

- Create `zoi.yaml` in the root of your project repository.
- Commands refer to paths relative to the project root unless you use absolute paths.

## Schema

The `zoi.yaml` file allows for simple and complex configurations, including platform-specific commands and environment variables.

```yaml
# Required project name (used in output)
name: my-project

# Optional: Verify important tools are available
# Each item runs a check command; non-zero exit indicates missing or incompatible
packages:
  - name: git
    check: git --version
  - name: node
    check: node --version

# Optional: Short, named commands runnable via `zoi run <cmd>`
commands:
  # Simple command
  - cmd: test
    run: npm test

  # Platform-specific command with environment variables
  - cmd: dev
    run:
      # Platform is in format <os>-<arch>, e.g. linux-amd64, macos-arm64, windows-amd64
      linux-amd64: npm run dev:linux
      macos-amd64: npm run dev:mac
      windows-amd64: npm run dev:win
      default: npm run dev # Fallback for other platforms
    env:
      # Simple env vars for all platforms
      API_KEY: "12345"
      # Platform-specific env vars
      platform:
        linux-amd64:
          ENDPOINT: "https://api.linux.dev"
        macos-amd64:
          ENDPOINT: "https://api.mac.dev"
        default:
          ENDPOINT: "https://api.dev"

# Optional: Environment setups runnable via `zoi env <alias>`
environments:
  - name: Web development environment
    cmd: web
    run:
      default:
        - npm ci
        - npm run build
      windows-amd64:
        - npm ci
        - echo "Building for Windows..."
        - npm run build:win
    env:
      default:
        NODE_ENV: "production"
      windows-amd64:
        NODE_ENV: "production_win"

  - name: Rust toolchain setup
    cmd: rust
    run:
      - rustup toolchain install stable
      - rustup component add clippy rustfmt
```

### Field Reference

- `name`: `string` (required)
- `packages`: `list` of objects (optional)
  - `name`: `string` (label only)
  - `check`: `string` (command to validate presence/version)
- `commands`: `list` of objects (optional)
  - `cmd`: `string` (alias)
  - `run`: `string` or `map` (command)
    - If a `string`, it's the command for all platforms.
    - If a `map`, keys are platforms (`<os>-<arch>`) and values are command strings. A `default` key can be used as a fallback.
  - `env`: `map` (optional)
    - Can be a simple `map` of `string: string` for environment variables.
    - Can be a `map` where the key `platform` contains platform-specific environment variables, and other keys are global.
- `environments`: `list` of objects (optional)
  - `name`: `string` (label)
  - `cmd`: `string` (alias)
  - `run`: `list of strings` or `map` (commands)
    - If a `list of strings`, it's the command list for all platforms.
    - If a `map`, keys are platforms (`<os>-<arch>`) and values are lists of command strings. A `default` key can be used as a fallback.
  - `env`: `map` (optional) - Same structure as in `commands`.

Zoi determines the platform from the OS and architecture (e.g. `linux-amd64`, `macos-arm64`, `windows-amd64`).

## CLI usage

- Run a command by alias:

```sh
zoi run dev
```

- To pass arguments to the underlying script, add them after the command alias. Use `--` to separate the arguments from Zoi's own options if needed.

```sh
# If 'test' is 'npm test', this runs 'npm test -- --watch'
zoi run test -- --watch

# If 'fmt' is 'cargo fmt', this runs 'cargo fmt -- --all'
zoi run fmt --all
```

- Interactively choose a command (no alias provided):

```sh
zoi run
```

- Set up an environment by alias:

```sh
zoi env web
```

- Interactively choose an environment:

```sh
zoi env
```

If `zoi.yaml` is missing, Zoi prints an error. If no commands or environments are defined, the respective subcommands will also error.

## Best practices

- Keep `check` commands fast and side-effect free.
- Prefer explicit toolchain versions in environment steps to ensure reproducibility.
- Use short, memorable `cmd` aliases.
- Split long setups into multiple environments (e.g. `deps`, `build`, `lint`).
- Use the `default` key in platform-specific maps to provide a good fallback experience.
