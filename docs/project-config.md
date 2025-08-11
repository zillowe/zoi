---
title: Project Configuration
description: Define project commands and environments with zoi.yaml.
---

Zoi can manage per-project commands and environments using a `zoi.yaml` file placed in your project root. This page describes the schema and provides examples.

## File location

- Create `zoi.yaml` in the root of your project repository.
- Commands refer to paths relative to the project root unless you use absolute paths.

## Schema

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
# Each command has:
# - cmd: alias name you type
# - run: the shell command to execute
commands:
  - cmd: dev
    run: npm run dev
  - cmd: test
    run: npm test

# Optional: Environment setups runnable via `zoi env <alias>`
# Each environment has:
# - name: human-friendly label (for interactive selection)
# - cmd: alias name you type
# - run: ordered list of shell commands executed sequentially
environments:
  - name: Web development environment
    cmd: web
    run:
      - npm ci
      - npm run build
  - name: Rust toolchain setup
    cmd: rust
    run:
      - rustup toolchain install stable
      - rustup component add clippy rustfmt
```

Field reference:

- name: string (required)
- packages: list of objects (optional)
  - name: string (label only)
  - check: string (command to validate presence/version)
- commands: list of objects (optional)
  - cmd: string (alias)
  - run: string (command)
- environments: list of objects (optional)
  - name: string (label)
  - cmd: string (alias)
  - run: list of strings (commands)

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
