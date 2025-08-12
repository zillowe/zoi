# Gemini Assistant Guidelines for Zoi Project

This document provides guidelines for the Gemini assistant when working on the Zoi project. Please adhere to these instructions to ensure consistency and quality.

## General Principles

- **Confidence Threshold:** Do not write or modify any code unless you are at least 95% confident that you understand the user's request and the required changes. If there is any ambiguity, ask for clarification before proceeding.
- **Proactive Communication:** Keep the user informed about your plan and progress. Explain what you are about to do, especially for complex tasks.

## Development Workflow

1.  **Analyze and Plan:** Before making any changes, thoroughly analyze the user's request and the relevant codebase. Formulate a clear plan of action.
2.  **Implement:** Write or modify the code according to the plan.
3.  **Verify with Build:** After every single code change, run `cargo build` to check for compilation errors and ensure the project remains in a buildable state. Do not proceed if the build is broken.
4.  **Update Documentation:** After implementing and verifying a feature or change, update the relevant documentation to reflect the new state of the codebase.

## Documentation Guidelines

Maintaining up-to-date and consistent documentation is crucial for the Zoi project.

### Documentation Structure

- `/docs/`: Contains general user-facing documentation.
- `/docs/lib/`: Contains documentation specifically for developers using Zoi as a library.

### When to Update Docs

- **User-Facing Changes:** Any change that affects how a user interacts with Zoi (e.g. new commands, changed command behavior, new features) requires an update to the documentation in the `/docs/` directory.
- **Library API Changes:** Any change to the public library API requires an update to the documentation in the `/docs/lib/` directory. This includes updating any relevant doc comments in `src/lib.rs` for `docs.rs`.
- **Read First:** Before updating any documentation, read the entire docs directory (`/docs/`) to understand the existing structure, style, and conventions.
- **Create New Files:** If the new documentation doesn't fit into an existing file, create a new `.md` file in the appropriate directory.

### Formatting and Style

- **Framework:** The documentation uses [FumaDocs](https://fumadocs.dev). Please follow its conventions.
- **Internal Linking:** When linking to other pages within the Zoi documentation, use the following absolute path format: `/docs/zds/zoi/{path-to-page-without-md-extension}/`.
  - Example: A link to `docs/repositories.md` should be written as `[Repositories](/docs/zds/zoi/repositories/)`.
  - Example: A link to `docs/lib/index.md` should be written as `[Library API](/docs/zds/zoi/lib/)`.
